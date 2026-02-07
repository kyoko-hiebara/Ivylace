/// Per-band control strip layout.
/// Composes: GR Meter, Threshold knob, Ratio/Attack/Release segmented,
/// Makeup knob, SC HPF knob, Saturation section, IN/Solo toggles.
use nih_plug_vizia::vizia::prelude::*;

use super::knob::{GlassKnob, KnobSize};
use super::meter::GrMeterWidget;
use super::segmented::SegmentedParam;
use super::toggle::{ToggleButton, ToggleVariant};
use super::theme;
use super::Data;

pub struct BandStrip;

impl BandStrip {
    pub fn new(cx: &mut Context, band_idx: usize) -> Handle<'_, Self> {
        let color = theme::band_color(band_idx);
        let vizia_color: Color = Color::rgba(
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            255,
        );

        Self.build(cx, |cx| {
            // Scrollable VStack to handle overflow gracefully
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
                    .height(Pixels(18.0))
                    .top(Pixels(2.0));

                // ── GR Meter (compact) ──
                {
                    let level_lens = Data::gr_outputs.map(move |o| o.band_gr[band_idx].load());
                    let peak_lens = Data::gr_outputs.map(move |o| o.band_peak_gr[band_idx].load());
                    GrMeterWidget::new(cx, level_lens, peak_lens)
                        .left(Stretch(1.0))
                        .right(Stretch(1.0));
                }

                // ── Threshold knob (medium size to save space) ──
                GlassKnob::new(
                    cx,
                    Data::params,
                    move |p| &p.bands[band_idx].threshold,
                    KnobSize::Md,
                    color,
                    "Threshold",
                )
                .left(Stretch(1.0))
                .right(Stretch(1.0));

                // ── Ratio / Attack / Release segmented controls ──
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
                        Some("ATTACK (ms)"),
                    );
                    SegmentedParam::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].release,
                        Some("RELEASE (ms)"),
                    );
                })
                .row_between(Pixels(2.0))
                .left(Pixels(3.0))
                .right(Pixels(3.0))
                .width(Stretch(1.0));

                // ── Makeup + SC HPF (both small) ──
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
                    .width(Stretch(1.0));
                })
                .row_between(Pixels(2.0))
                .left(Pixels(4.0))
                .right(Pixels(4.0))
                .top(Pixels(4.0))
                .bottom(Pixels(4.0))
                .background_color(Color::rgba(255, 255, 255, 8))
                .border_color(Color::rgba(255, 255, 255, 15))
                .border_width(Pixels(1.0))
                .border_radius(Pixels(6.0))
                .width(Stretch(1.0));

                // ── Footer: IN / Solo toggles (with top divider) ──
                HStack::new(cx, |cx| {
                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].enabled,
                        "IN",
                        ToggleVariant::Normal,
                        color,
                    )
                    .width(Stretch(1.0));

                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].solo,
                        "S",
                        ToggleVariant::Solo,
                        theme::solo_color(),
                    )
                    .width(Stretch(1.0));
                })
                .col_between(Pixels(4.0))
                .left(Pixels(4.0))
                .right(Pixels(4.0))
                .top(Pixels(4.0))
                .bottom(Pixels(4.0))
                .border_color(Color::rgba(255, 255, 255, 13))
                .border_width(Pixels(1.0))
                .width(Stretch(1.0));
            })
            .row_between(Pixels(1.0))
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
