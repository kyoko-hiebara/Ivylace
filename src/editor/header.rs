/// Global controls header bar.
/// Contains: Input Gain, Mix, Sat Type, OS Realtime, OS Render, Output Gain.
use nih_plug_vizia::vizia::prelude::*;

use super::knob::{GlassKnob, KnobSize};
use super::segmented::SegmentedParam;
use super::theme;
use super::Data;

pub struct Header;

impl Header {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |cx| {
            HStack::new(cx, |cx| {
                // ── Left group: In Gain + Mix ──
                HStack::new(cx, |cx| {
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.input_gain,
                        KnobSize::Sm,
                        theme::accent(),
                        "In Gain",
                    );
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.dry_wet,
                        KnobSize::Sm,
                        theme::accent(),
                        "Mix",
                    );
                })
                .col_between(Pixels(16.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Auto);

                // ── Center group: Sat Type / OS controls ──
                HStack::new(cx, |cx| {
                    // Sat Type segmented control
                    VStack::new(cx, |cx| {
                        SegmentedParam::new(cx, Data::params, |p| &p.sat_type, Some("SAT TYPE"));
                    })
                    .width(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));

                    // Divider
                    Element::new(cx)
                        .width(Pixels(1.0))
                        .height(Pixels(32.0))
                        .background_color(Color::rgba(255, 255, 255, 20))
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));

                    // OS Realtime segmented control
                    VStack::new(cx, |cx| {
                        SegmentedParam::new(cx, Data::params, |p| &p.os_realtime, Some("OS REALTIME"));
                    })
                    .width(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));

                    // Divider
                    Element::new(cx)
                        .width(Pixels(1.0))
                        .height(Pixels(32.0))
                        .background_color(Color::rgba(255, 255, 255, 20))
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));

                    // OS Render segmented control
                    VStack::new(cx, |cx| {
                        SegmentedParam::new(cx, Data::params, |p| &p.os_render, Some("OS RENDER"));
                    })
                    .width(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));
                })
                .col_between(Pixels(8.0))
                .left(Pixels(8.0))
                .right(Pixels(8.0))
                .child_left(Pixels(8.0))
                .child_right(Pixels(8.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Stretch(1.0))
                .top(Pixels(6.0))
                .bottom(Pixels(6.0))
                .background_color(Color::rgba(255, 255, 255, 10))
                .border_color(Color::rgba(255, 255, 255, 20))
                .border_width(Pixels(1.0))
                .border_radius(Pixels(8.0));

                // ── Right group: Out Gain ──
                HStack::new(cx, |cx| {
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.output_gain,
                        KnobSize::Sm,
                        theme::accent(),
                        "Out Gain",
                    );
                })
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Auto);
            })
            .col_between(Pixels(8.0))
            .left(Pixels(20.0))
            .right(Pixels(20.0))
            .height(Stretch(1.0))
            .width(Stretch(1.0));
        })
        .height(Pixels(70.0))
        .width(Stretch(1.0))
        .background_color(Color::rgba(255, 255, 255, 18))
        .border_color(Color::rgba(255, 255, 255, 15))
        .border_width(Pixels(1.0))
    }
}

impl View for Header {
    fn element(&self) -> Option<&'static str> {
        Some("header")
    }
}
