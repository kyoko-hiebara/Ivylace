use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::ResizeHandle;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};

use crate::dsp::gr_meter::GrMeterOutputs;
use crate::dsp::spectrum::SpectrumBuffer;
use crate::IvylaceParams;

pub mod theme;
mod knob;
mod meter;
mod crossover;
mod segmented;
mod toggle;
mod band_strip;
mod header;
mod spectrum;
mod about;

// ── Data Model (shared between audio and GUI via lenses) ─────

#[derive(Lens)]
pub(crate) struct Data {
    pub params: Arc<IvylaceParams>,
    pub gr_outputs: Arc<GrMeterOutputs>,
}

impl Model for Data {}

// ── Editor entry points ──────────────────────────────────────

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (960, 740))
}

pub(crate) fn create(
    params: Arc<IvylaceParams>,
    gr_outputs: Arc<GrMeterOutputs>,
    _spectrum_buf: Arc<SpectrumBuffer>,
    _sample_rate: f32,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _gui_context| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_thin(cx);
        assets::register_noto_sans_regular(cx);
        assets::register_noto_sans_bold(cx);

        Data {
            params: params.clone(),
            gr_outputs: gr_outputs.clone(),
        }
        .build(cx);

        // ── Full GUI: Header + Crossover + 4 Band Strips ──
        let bg_color_lens = Data::params.map(|p| {
            let mode = if p.glass_mode.load(Ordering::Relaxed) {
                theme::ThemeMode::Glass
            } else {
                theme::ThemeMode::Dark
            };
            theme::to_vizia(theme::page_bg(mode))
        });

        let about_visible = Arc::new(AtomicBool::new(false));

        VStack::new(cx, |cx| {
            let p = Data::params.get(cx);
            let glass_mode = p.glass_mode.clone();

            build_title_bar(cx, about_visible.clone());
            header::Header::new(cx, glass_mode.clone());
            crossover::CrossoverDisplay::new(cx, glass_mode.clone());

            HStack::new(cx, |cx| {
                let p = Data::params.get(cx);
                for i in 0..4 {
                    band_strip::BandStrip::new(cx, i, p.clone());
                }
            })
            .height(Stretch(1.0))
            .width(Stretch(1.0));

            // About dialog overlay (rendered last → on top of everything)
            about::AboutDialog::new(cx, about_visible.clone(), glass_mode.clone());
        })
        .background_color(bg_color_lens)
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        ResizeHandle::new(cx);
    })
}

// ── Title Bar ────────────────────────────────────────────────

fn build_title_bar(cx: &mut Context, about_visible: Arc<AtomicBool>) {
    let border_lens = Data::params.map(|p| {
        let mode = if p.glass_mode.load(Ordering::Relaxed) { theme::ThemeMode::Glass } else { theme::ThemeMode::Dark };
        theme::to_vizia(theme::title_bar_border(mode))
    });

    HStack::new(cx, move |cx| {
        TitleBarLabel::new(cx, about_visible.clone())
            .width(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
    })
    .height(Pixels(28.0))
    .border_color(border_lens)
    .border_width(Pixels(1.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

// ── Clickable Title Bar Label ───────────────────────────────

struct TitleBarLabel {
    about_visible: Arc<AtomicBool>,
}

impl TitleBarLabel {
    fn new(cx: &mut Context, about_visible: Arc<AtomicBool>) -> Handle<'_, Self> {
        let text_lens = Data::params.map(|p| {
            let mode = if p.glass_mode.load(Ordering::Relaxed) { theme::ThemeMode::Glass } else { theme::ThemeMode::Dark };
            theme::to_vizia(theme::title_bar_text(mode))
        });

        Self { about_visible }
            .build(cx, |cx| {
                Label::new(cx, &format!("Ivylace v{}", env!("CARGO_PKG_VERSION")))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(10.0)
                    .font_weight(FontWeightKeyword::Medium)
                    .color(text_lens)
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .hoverable(false);
            })
            .cursor(CursorIcon::Hand)
            .height(Stretch(1.0))
    }
}

impl View for TitleBarLabel {
    fn element(&self) -> Option<&'static str> {
        Some("title-bar-label")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                // Toggle about dialog visibility
                let current = self.about_visible.load(Ordering::Relaxed);
                self.about_visible.store(!current, Ordering::Relaxed);
                cx.needs_redraw();
                meta.consume();
            }
        });
    }
}
