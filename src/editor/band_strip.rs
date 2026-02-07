/// Per-band control strip layout.
/// Composes: GR Meter, Threshold knob, Ratio/Attack/Release segmented,
/// Makeup knob, SC HPF knob, Saturation section, LINK/IN/Solo toggles.
use std::sync::Arc;

use nih_plug_vizia::vizia::prelude::*;

use super::knob::{GlassKnob, KnobSize};
use super::meter::GrMeterWidget;
use super::segmented::SegmentedParam;
use super::toggle::{ToggleButton, ToggleVariant};
use super::theme;
use super::Data;
use crate::IvylaceParams;

pub struct BandStrip;

impl BandStrip {
    pub fn new(cx: &mut Context, band_idx: usize, all_params: Arc<IvylaceParams>) -> Handle<'_, Self> {
        let color = theme::band_color(band_idx);
        let vizia_color: Color = Color::rgba(
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            255,
        );

        Self.build(cx, |cx| {
            VStack::new(cx, |cx| {
                // ── Top accent line ──
                Element::new(cx)
                    .height(Pixels(2.0))
                    .width(Stretch(1.0))
                    .background_color(vizia_color);

                // ── Band name ──
                Label::new(cx, &theme::BAND_NAMES[band_idx].to_uppercase())
                    .font_size(11.0)
                    .font_weight(FontWeightKeyword::Bold)
                    .color(vizia_color)
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .height(Pixels(20.0));

                // ── GR Meter ──
                {
                    let level_lens = Data::gr_outputs.map(move |o| o.band_gr[band_idx].load());
                    let peak_lens = Data::gr_outputs.map(move |o| o.band_peak_gr[band_idx].load());
                    GrMeterWidget::new(cx, level_lens, peak_lens)
                        .left(Stretch(1.0))
                        .right(Stretch(1.0));
                }

                // ── Threshold knob ──
                GlassKnob::new_with_link(
                    cx,
                    Data::params,
                    move |p| &p.bands[band_idx].threshold,
                    KnobSize::Md,
                    color,
                    "Threshold",
                    all_params.clone(),
                    band_idx,
                )
                .left(Stretch(1.0))
                .right(Stretch(1.0));

                // ── Ratio / Attack / Release (grouped) ──
                VStack::new(cx, |cx| {
                    SegmentedParam::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].ratio,
                        Some("RATIO"),
                    );
                    SegmentedParam::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].attack,
                        Some("ATTACK"),
                    );
                    SegmentedParam::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].release,
                        Some("RELEASE"),
                    );
                })
                .row_between(Pixels(15.0))
                .left(Pixels(3.0))
                .right(Pixels(3.0))
                .width(Stretch(1.0));

                // ── Makeup + SC HPF ──
                HStack::new(cx, |cx| {
                    GlassKnob::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].makeup,
                        KnobSize::Sm,
                        color,
                        "Makeup",
                    );
                    GlassKnob::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].sc_hpf,
                        KnobSize::Sm,
                        color,
                        "SC HPF",
                    );
                })
                .col_between(Pixels(2.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .top(Pixels(24.0))
                .width(Stretch(1.0));

                // ── Saturation section ──
                VStack::new(cx, |cx| {
                    Label::new(cx, "SATURATION")
                        .font_size(8.0)
                        .font_weight(FontWeightKeyword::SemiBold)
                        .color(Color::rgba(255, 255, 255, 77))
                        .text_align(TextAlign::Center)
                        .width(Stretch(1.0))
                        .height(Pixels(12.0));

                    HStack::new(cx, |cx| {
                        GlassKnob::new(
                            cx,
                            Data::params,
                            move |p| &p.sat_bands[band_idx].drive,
                            KnobSize::Sm,
                            theme::accent(),
                            "Drive",
                        );
                        ToggleButton::new(
                            cx,
                            Data::params,
                            move |p| &p.sat_bands[band_idx].enabled,
                            "SAT",
                            ToggleVariant::Sat,
                            theme::accent(),
                        )
                        .width(Pixels(34.0))
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));
                    })
                    .col_between(Pixels(4.0))
                    .child_left(Stretch(1.0))
                    .child_right(Stretch(1.0))
                    .height(Auto)
                    .width(Stretch(1.0));
                })
                .row_between(Pixels(4.0))
                .left(Pixels(4.0))
                .right(Pixels(4.0))
                .top(Pixels(4.0))
                .bottom(Pixels(8.0))
                .height(Auto)
                .background_color(Color::rgba(255, 255, 255, 8))
                .border_color(Color::rgba(255, 255, 255, 15))
                .border_width(Pixels(1.0))
                .border_radius(Pixels(6.0))
                .width(Stretch(1.0));

                // ── Footer: LINK / Power / Solo ──
                HStack::new(cx, |cx| {
                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].link,
                        "LINK",
                        ToggleVariant::Normal,
                        theme::accent(),
                    )
                    .width(Stretch(1.0));

                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].enabled,
                        "",
                        ToggleVariant::Power,
                        color,
                    )
                    .width(Pixels(30.0));

                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].solo,
                        "SOLO",
                        ToggleVariant::Solo,
                        theme::solo_color(),
                    )
                    .width(Stretch(1.0));
                })
                .col_between(Pixels(3.0))
                .left(Pixels(4.0))
                .right(Pixels(4.0))
                .top(Pixels(6.0))
                .bottom(Pixels(10.0))
                .width(Stretch(1.0));
            })
            .row_between(Pixels(10.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .background_color(Color::rgba(255, 255, 255, 10))
        .border_color(Color::rgba(255, 255, 255, 20))
        .border_width(Pixels(1.0))
        .overflow(Overflow::Hidden)
    }
}

impl View for BandStrip {
    fn element(&self) -> Option<&'static str> {
        Some("band-strip")
    }
}
