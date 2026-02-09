use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};

use crate::dsp::gr_meter::GrMeterOutputs;
use crate::dsp::spectrum::SpectrumBuffer;
use crate::{AutoAnalysisState, IvylaceParams, SlotStorage, TrimState};

use auto_button::{AutoOverlay, SharedAutoPhase};

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
mod pets;
pub(crate) mod auto_button;
pub(crate) mod ab_toggle;

// ── Data Model (shared between audio and GUI via lenses) ─────

#[derive(Lens)]
pub(crate) struct Data {
    pub params: Arc<IvylaceParams>,
    pub gr_outputs: Arc<GrMeterOutputs>,
    pub auto_state: Arc<AutoAnalysisState>,
    pub slot_a: Arc<SlotStorage>,
    pub slot_b: Arc<SlotStorage>,
    pub trim_state: Arc<TrimState>,
}

impl Model for Data {}

// ── Editor entry points ──────────────────────────────────────

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (960, 860))
}

pub(crate) fn create(
    params: Arc<IvylaceParams>,
    gr_outputs: Arc<GrMeterOutputs>,
    spectrum_pre: Arc<SpectrumBuffer>,
    spectrum_post: Arc<SpectrumBuffer>,
    sample_rate: f32,
    auto_state: Arc<AutoAnalysisState>,
    editor_state: Arc<ViziaState>,
    slot_a: Arc<SlotStorage>,
    slot_b: Arc<SlotStorage>,
    trim_state: Arc<TrimState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, gui_context| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_thin(cx);
        assets::register_noto_sans_regular(cx);
        assets::register_noto_sans_bold(cx);

        Data {
            params: params.clone(),
            gr_outputs: gr_outputs.clone(),
            auto_state: auto_state.clone(),
            slot_a: slot_a.clone(),
            slot_b: slot_b.clone(),
            trim_state: trim_state.clone(),
        }
        .build(cx);

        // ── Full GUI: Header + Crossover + 4 Band Strips ──
        let about_visible = Arc::new(AtomicBool::new(false));
        let auto_phase = Arc::new(SharedAutoPhase::new());

        let p_for_glass = Data::params.get(cx);
        let glass_mode_for_bg = p_for_glass.glass_mode.clone();

        VStack::new(cx, |cx| {
            // Gradient background (SelfDirected, drawn behind everything)
            BackgroundGradient::new(cx, glass_mode_for_bg.clone());

            let p = Data::params.get(cx);
            let glass_mode = p.glass_mode.clone();

            build_title_bar(cx, about_visible.clone(), glass_mode.clone());
            header::Header::new(cx, glass_mode.clone(), auto_phase.clone(), trim_state.clone());
            crossover::CrossoverDisplay::new(cx, glass_mode.clone());
            spectrum::SpectrumAnalyzer::new(
                cx,
                spectrum_pre.clone(),
                spectrum_post.clone(),
                sample_rate,
                glass_mode.clone(),
            );

            HStack::new(cx, |cx| {
                let p = Data::params.get(cx);
                let sa = Data::slot_a.get(cx);
                let sb = Data::slot_b.get(cx);
                for i in 0..4 {
                    band_strip::BandStrip::new(cx, i, p.clone(), sa.clone(), sb.clone());
                }
            })
            .height(Stretch(1.0))
            .width(Stretch(1.0));

            // Walking pets (cosmetic easter egg)
            pets::WalkingPets::new(cx, glass_mode.clone());

            // Auto overlay (full-screen status display during analysis)
            let auto_st_overlay = Data::auto_state.get(cx);
            let p_overlay = Data::params.get(cx);
            let slot_a_overlay = Data::slot_a.get(cx);
            let slot_b_overlay = Data::slot_b.get(cx);
            AutoOverlay::new(cx, auto_st_overlay, p_overlay, auto_phase.clone(), glass_mode.clone(), gui_context.clone(), slot_a_overlay, slot_b_overlay);

            // About dialog overlay (rendered last → on top of everything)
            about::AboutDialog::new(cx, about_visible.clone(), glass_mode.clone());
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}

// ── Title Bar ────────────────────────────────────────────────

fn build_title_bar(cx: &mut Context, about_visible: Arc<AtomicBool>, glass_mode: Arc<AtomicBool>) {
    let border_lens = Data::params.map(|p| {
        let mode = if p.glass_mode.load(Ordering::Relaxed) { theme::ThemeMode::Glass } else { theme::ThemeMode::Dark };
        theme::to_vizia(theme::title_bar_border(mode))
    });

    HStack::new(cx, move |cx| {
        TitleBarLabel::new(cx, about_visible.clone())
            .width(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));

        ThemeToggle::new(cx, glass_mode.clone())
            .top(Stretch(1.0))
            .bottom(Stretch(1.0))
            .right(Pixels(8.0));
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

// ── Theme Toggle Button (sun/moon icon) ─────────────────────

struct ThemeToggle {
    glass_mode: Arc<AtomicBool>,
}

impl ThemeToggle {
    fn new(cx: &mut Context, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self> {
        Self { glass_mode }
            .build(cx, |_| {})
            .width(Pixels(24.0))
            .height(Pixels(24.0))
            .cursor(CursorIcon::Hand)
    }
}

impl View for ThemeToggle {
    fn element(&self) -> Option<&'static str> {
        Some("theme-toggle")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 1.0 || bounds.h < 1.0 {
            return;
        }

        let is_glass = self.glass_mode.load(Ordering::Relaxed);
        let mode = if is_glass { theme::ThemeMode::Glass } else { theme::ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let cx_x = bounds.x + bounds.w * 0.5;
        let cy = bounds.y + bounds.h * 0.5;
        let r = 9.0 * dpi;

        // Background circle
        let mut path = vg::Path::new();
        path.circle(cx_x, cy, r);

        if is_glass {
            // Glass mode active: show crescent moon icon (switch to Dark)
            // Use opaque page background color so the bite fully erases
            let bg_opaque = theme::theme_toggle_bite_bg(mode);
            canvas.fill_path(&path, &vg::Paint::color(bg_opaque));
            let mut border = vg::Paint::color(theme::toggle_off_border(mode));
            border.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border);

            // Crescent moon: draw full moon circle, then erase with offset circle
            let moon_r = 6.0 * dpi;
            let mut moon = vg::Path::new();
            moon.circle(cx_x - 1.0 * dpi, cy, moon_r);
            canvas.fill_path(&moon, &vg::Paint::color(theme::theme_toggle_moon(mode)));

            // "Bite" circle erases part of the moon to form crescent shape
            let bite_r = 5.5 * dpi;
            let bite_offset = 4.0 * dpi;
            let mut bite = vg::Path::new();
            bite.circle(cx_x - 1.0 * dpi + bite_offset, cy, bite_r);
            canvas.fill_path(&bite, &vg::Paint::color(bg_opaque));
        } else {
            // Dark mode: show sun icon (bright circle with rays)
            let bg_opaque = theme::theme_toggle_bite_bg(mode);
            canvas.fill_path(&path, &vg::Paint::color(bg_opaque));
            let mut border = vg::Paint::color(theme::toggle_off_border(mode));
            border.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border);

            // Sun circle
            let sun_r = 3.5 * dpi;
            let mut sun = vg::Path::new();
            sun.circle(cx_x, cy, sun_r);
            canvas.fill_path(&sun, &vg::Paint::color(theme::theme_toggle_sun(mode)));

            // Sun rays
            let ray_len = 2.0 * dpi;
            let ray_start = sun_r + 1.5 * dpi;
            let mut ray_paint = vg::Paint::color(theme::theme_toggle_sun_ray(mode));
            ray_paint.set_line_width(1.5 * dpi);
            for i in 0..8 {
                let angle = (i as f32) * std::f32::consts::PI / 4.0;
                let x0 = cx_x + angle.cos() * ray_start;
                let y0 = cy + angle.sin() * ray_start;
                let x1 = cx_x + angle.cos() * (ray_start + ray_len);
                let y1 = cy + angle.sin() * (ray_start + ray_len);
                let mut ray = vg::Path::new();
                ray.move_to(x0, y0);
                ray.line_to(x1, y1);
                canvas.stroke_path(&ray, &ray_paint);
            }
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let current = self.glass_mode.load(Ordering::Relaxed);
                self.glass_mode.store(!current, Ordering::Relaxed);
                cx.needs_redraw();
                meta.consume();
            }
        });
    }
}

// ── Background Gradient View ────────────────────────────────

struct BackgroundGradient {
    glass_mode: Arc<AtomicBool>,
}

impl BackgroundGradient {
    fn new(cx: &mut Context, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self> {
        Self { glass_mode }
            .build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .hoverable(false)
    }
}

impl View for BackgroundGradient {
    fn element(&self) -> Option<&'static str> {
        Some("background-gradient")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) {
            theme::ThemeMode::Glass
        } else {
            theme::ThemeMode::Dark
        };

        let top_color = theme::page_bg_top(mode);
        let bottom_color = theme::page_bg_bottom(mode);

        let paint = vg::Paint::linear_gradient(
            bounds.x,
            bounds.y,
            bounds.x,
            bounds.y + bounds.h,
            top_color,
            bottom_color,
        );

        let mut path = vg::Path::new();
        path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&path, &paint);
    }
}
