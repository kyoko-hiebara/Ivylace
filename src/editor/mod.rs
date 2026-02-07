use std::sync::Arc;

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
        VStack::new(cx, |cx| {
            build_title_bar(cx);
            header::Header::new(cx);
            crossover::CrossoverDisplay::new(cx);

            HStack::new(cx, |cx| {
                let p = Data::params.get(cx);
                for i in 0..4 {
                    band_strip::BandStrip::new(cx, i, p.clone());
                }
            })
            .height(Stretch(1.0))
            .width(Stretch(1.0));
        })
        .background_color(Color::rgb(0x0A, 0x0A, 0x1A))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        ResizeHandle::new(cx);
    })
}

// ── Title Bar ────────────────────────────────────────────────

fn build_title_bar(cx: &mut Context) {
    HStack::new(cx, |cx| {
        Label::new(cx, &format!("Ivylace v{}", env!("CARGO_PKG_VERSION")))
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_size(10.0)
            .font_weight(FontWeightKeyword::Medium)
            .color(Color::rgba(255, 255, 255, 100))
            .text_align(TextAlign::Center)
            .width(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
    })
    .height(Pixels(28.0))
    .border_color(Color::rgba(255, 255, 255, 13))
    .border_width(Pixels(1.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}
