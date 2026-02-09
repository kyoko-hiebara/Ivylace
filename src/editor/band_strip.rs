/// Per-band control strip layout.
/// Composes: GR Meter, Threshold knob, Ratio/Attack/Release segmented,
/// Makeup knob, SC HPF knob, Saturation section, LINK/IN/Solo toggles.
use std::sync::Arc;
use std::sync::atomic::Ordering;

use nih_plug_vizia::vizia::prelude::*;

use super::knob::{GlassKnob, KnobSize, SlotParamKind, SlotValueSource};
use super::meter::{GrMeterWidget, AnalogGrMeter};
use super::segmented::{SegmentedParam, SlotEnumKind, SlotEnumSource};
use super::toggle::{ToggleButton, ToggleVariant};
use super::theme::{self, ThemeMode};
use super::Data;
use crate::{GlobalOverrides, IvylaceParams, SlotStorage};

fn mode_lens() -> impl Lens<Target = ThemeMode> {
    Data::params.map(|p| {
        if p.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark }
    })
}

pub struct BandStrip;

impl BandStrip {
    pub fn new(
        cx: &mut Context,
        band_idx: usize,
        all_params: Arc<IvylaceParams>,
        slot_a: Arc<SlotStorage>,
        slot_b: Arc<SlotStorage>,
    ) -> Handle<'_, Self> {
        let glass_mode = all_params.glass_mode.clone();
        let ab_is_b = all_params.ab_active_is_b.clone();
        let color = theme::band_color(band_idx, ThemeMode::Dark);

        Self.build(cx, move |cx| {
            let gm = glass_mode.clone();
            VStack::new(cx, move |cx| {
                // ── Top accent line ──
                let accent_lens = mode_lens().map(move |m| theme::to_vizia(theme::band_color(band_idx, *m)));
                Element::new(cx)
                    .height(Pixels(2.0))
                    .width(Stretch(1.0))
                    .background_color(accent_lens);

                // ── Band name ──
                let name_lens = mode_lens().map(move |m| theme::to_vizia(theme::band_color(band_idx, *m)));
                Label::new(cx, &theme::BAND_NAMES[band_idx].to_uppercase())
                    .font_size(11.0)
                    .font_weight(FontWeightKeyword::Bold)
                    .color(name_lens)
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .height(Pixels(20.0));

                // ── GR Meters (digital bar + analog needle) ──
                {
                    let gm_meter = gm.clone();
                    HStack::new(cx, move |cx| {
                        let level_lens = Data::gr_outputs.map(move |o| o.band_gr[band_idx].load());
                        let peak_lens = Data::gr_outputs.map(move |o| o.band_peak_gr[band_idx].load());
                        GrMeterWidget::new(cx, level_lens, peak_lens, gm_meter.clone());

                        let analog_lens = Data::gr_outputs.map(move |o| o.band_gr[band_idx].load());
                        AnalogGrMeter::new(cx, analog_lens, gm_meter.clone());
                    })
                    .col_between(Pixels(10.0))
                    .child_left(Stretch(1.0))
                    .child_right(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .height(Auto)
                    .width(Stretch(1.0));
                }

                // ── Threshold knob (with link + slot value) ──
                GlassKnob::new_full(
                    cx,
                    Data::params,
                    move |p| &p.bands[band_idx].threshold,
                    KnobSize::Md,
                    color,
                    "Threshold",
                    gm.clone(),
                    Some(all_params.clone()),
                    band_idx,
                    Some(SlotValueSource {
                        slot_a: slot_a.clone(),
                        slot_b: slot_b.clone(),
                        is_b: ab_is_b.clone(),
                        kind: SlotParamKind::ThresholdDb,
                        band_idx,
                    }),
                    None, // no global override (uses slot atomics)
                )
                .left(Stretch(1.0))
                .right(Stretch(1.0));

                // ── Ratio / Attack / Release (grouped with slot sources) ──
                let gm2 = gm.clone();
                let sa2 = slot_a.clone();
                let sb2 = slot_b.clone();
                let ab2 = ab_is_b.clone();
                VStack::new(cx, move |cx| {
                    SegmentedParam::new_with_slot(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].ratio,
                        Some("RATIO"),
                        gm2.clone(),
                        Some(SlotEnumSource {
                            slot_a: sa2.clone(), slot_b: sb2.clone(),
                            is_b: ab2.clone(), kind: SlotEnumKind::Ratio, band_idx,
                        }),
                    );
                    SegmentedParam::new_with_slot(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].attack,
                        Some("ATTACK"),
                        gm2.clone(),
                        Some(SlotEnumSource {
                            slot_a: sa2.clone(), slot_b: sb2.clone(),
                            is_b: ab2.clone(), kind: SlotEnumKind::Attack, band_idx,
                        }),
                    );
                    SegmentedParam::new_with_slot(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].release,
                        Some("RELEASE"),
                        gm2.clone(),
                        Some(SlotEnumSource {
                            slot_a: sa2.clone(), slot_b: sb2.clone(),
                            is_b: ab2.clone(), kind: SlotEnumKind::Release, band_idx,
                        }),
                    );
                })
                .row_between(Pixels(15.0))
                .left(Pixels(3.0))
                .right(Pixels(3.0))
                .width(Stretch(1.0));

                // ── Makeup + SC HPF (with slot values) ──
                let gm3 = gm.clone();
                let sa3 = slot_a.clone();
                let sb3 = slot_b.clone();
                let ab3 = ab_is_b.clone();
                HStack::new(cx, move |cx| {
                    GlassKnob::new_full(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].makeup,
                        KnobSize::Sm,
                        color,
                        "Makeup",
                        gm3.clone(),
                        None,
                        band_idx,
                        Some(SlotValueSource {
                            slot_a: sa3.clone(), slot_b: sb3.clone(),
                            is_b: ab3.clone(), kind: SlotParamKind::MakeupDb, band_idx,
                        }),
                        None,
                    );
                    GlassKnob::new_full(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].sc_hpf,
                        KnobSize::Sm,
                        color,
                        "SC HPF",
                        gm3.clone(),
                        None,
                        band_idx,
                        Some(SlotValueSource {
                            slot_a: sa3.clone(), slot_b: sb3.clone(),
                            is_b: ab3.clone(), kind: SlotParamKind::ScHpfHz, band_idx,
                        }),
                        None,
                    );
                })
                .col_between(Pixels(2.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .top(Pixels(24.0))
                .width(Stretch(1.0));

                // ── Saturation section ──
                let gm4 = gm.clone();
                VStack::new(cx, move |cx| {
                    Label::new(cx, "SATURATION")
                        .font_size(8.0)
                        .font_weight(FontWeightKeyword::SemiBold)
                        .color(mode_lens().map(|m| theme::to_vizia(theme::sat_label(*m))))
                        .text_align(TextAlign::Center)
                        .width(Stretch(1.0))
                        .height(Pixels(12.0));

                    let gm4b = gm4.clone();
                    let ovr_drive = Data::params.get(cx).global_overrides.clone();
                    HStack::new(cx, move |cx| {
                        GlassKnob::new(
                            cx,
                            Data::params,
                            move |p| &p.sat_bands[band_idx].drive,
                            KnobSize::Sm,
                            theme::accent(),
                            "Drive",
                            gm4b.clone(),
                            Some((ovr_drive.clone(), super::knob::GlobalOverrideKind::DriveDb(band_idx))),
                        );
                        ToggleButton::new(
                            cx,
                            Data::params,
                            move |p| &p.sat_bands[band_idx].enabled,
                            "SAT",
                            ToggleVariant::Sat,
                            theme::accent(),
                            gm4b.clone(),
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
                .background_color(mode_lens().map(|m| theme::to_vizia(theme::sat_section_bg(*m))))
                .border_color(mode_lens().map(|m| theme::to_vizia(theme::sat_section_border(*m))))
                .border_width(Pixels(1.0))
                .border_radius(Pixels(6.0))
                .width(Stretch(1.0));

                // ── Footer: LINK / Power / Solo ──
                let gm5 = gm.clone();
                HStack::new(cx, move |cx| {
                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].link,
                        "LINK",
                        ToggleVariant::Normal,
                        theme::accent(),
                        gm5.clone(),
                    )
                    .width(Stretch(1.0));

                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].enabled,
                        "",
                        ToggleVariant::Power,
                        color,
                        gm5.clone(),
                    )
                    .width(Pixels(30.0));

                    ToggleButton::new(
                        cx,
                        Data::params,
                        move |p| &p.bands[band_idx].solo,
                        "SOLO",
                        ToggleVariant::Solo,
                        theme::solo_color(),
                        gm5.clone(),
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
        .background_color(mode_lens().map(|m| theme::to_vizia(theme::panel_bg(*m))))
        .border_color(mode_lens().map(|m| theme::to_vizia(theme::panel_border(*m))))
        .border_width(Pixels(1.0))
        .overflow(Overflow::Hidden)
    }
}

impl View for BandStrip {
    fn element(&self) -> Option<&'static str> {
        Some("band-strip")
    }
}
