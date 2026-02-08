/// Global controls header bar.
/// Contains: Input Gain, Mix, Sat Type, OS Realtime, OS Render, Output Gain, Theme Toggle.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::knob::{GlassKnob, KnobSize};
use super::segmented::SegmentedParam;
use super::theme::{self, ThemeMode};
use super::Data;

fn mode_lens() -> impl Lens<Target = ThemeMode> {
    Data::params.map(|p| {
        if p.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark }
    })
}

pub struct Header;

impl Header {
    pub fn new(cx: &mut Context, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self> {
        Self.build(cx, move |cx| {
            let gm = glass_mode.clone();
            HStack::new(cx, move |cx| {
                // ── Left group: In Gain + Mix ──
                let gm_left = gm.clone();
                HStack::new(cx, move |cx| {
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.input_gain,
                        KnobSize::Sm,
                        theme::accent(),
                        "In Gain",
                        gm_left.clone(),
                    );
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.dry_wet,
                        KnobSize::Sm,
                        theme::accent(),
                        "Mix",
                        gm_left.clone(),
                    );
                })
                .col_between(Pixels(16.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Auto);

                // ── Center group: Sat Type / OS controls ──
                let gm_center = gm.clone();
                HStack::new(cx, move |cx| {
                    // Sat Type segmented control
                    let gm_c1 = gm_center.clone();
                    SegmentedParam::new(cx, Data::params, |p| &p.sat_type, Some("SAT TYPE"), gm_c1.clone())
                        .height(Auto);

                    // Divider
                    let div1_color = mode_lens().map(|m| theme::to_vizia(theme::divider(*m)));
                    Element::new(cx)
                        .width(Pixels(1.0))
                        .height(Pixels(32.0))
                        .background_color(div1_color)
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));

                    // OS Realtime segmented control
                    let gm_c2 = gm_center.clone();
                    SegmentedParam::new(cx, Data::params, |p| &p.os_realtime, Some("OS REALTIME"), gm_c2.clone())
                        .height(Auto);

                    // Divider
                    let div2_color = mode_lens().map(|m| theme::to_vizia(theme::divider(*m)));
                    Element::new(cx)
                        .width(Pixels(1.0))
                        .height(Pixels(32.0))
                        .background_color(div2_color)
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));

                    // OS Render segmented control
                    let gm_c3 = gm_center.clone();
                    SegmentedParam::new(cx, Data::params, |p| &p.os_render, Some("OS RENDER"), gm_c3.clone())
                        .height(Auto);
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
                .background_color(mode_lens().map(|m| theme::to_vizia(theme::glass_bg(*m))))
                .border_color(mode_lens().map(|m| theme::to_vizia(theme::glass_border(*m))))
                .border_width(Pixels(1.0))
                .border_radius(Pixels(8.0));

                // ── Right group: Out Gain + Theme Toggle ──
                let gm_right = gm.clone();
                HStack::new(cx, move |cx| {
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.output_gain,
                        KnobSize::Sm,
                        theme::accent(),
                        "Out Gain",
                        gm_right.clone(),
                    );
                    ThemeToggle::new(cx, gm_right.clone());
                })
                .col_between(Pixels(12.0))
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
        .background_color(mode_lens().map(|m| theme::to_vizia(theme::header_bg(*m))))
        .border_color(mode_lens().map(|m| theme::to_vizia(theme::header_border(*m))))
        .border_width(Pixels(1.0))
    }
}

impl View for Header {
    fn element(&self) -> Option<&'static str> {
        Some("header")
    }
}

// ── Theme Toggle Button ─────────────────────────────────────

struct ThemeToggle {
    glass_mode: Arc<AtomicBool>,
}

impl ThemeToggle {
    fn new(cx: &mut Context, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self> {
        Self { glass_mode }
            .build(cx, |_| {})
            .width(Pixels(28.0))
            .height(Pixels(28.0))
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
        let mode = if is_glass { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let cx_x = bounds.x + bounds.w * 0.5;
        let cy = bounds.y + bounds.h * 0.5;
        let r = 10.0 * dpi;

        // Background circle
        let mut path = vg::Path::new();
        path.circle(cx_x, cy, r);

        if is_glass {
            // Glass mode active: show moon icon (dark circle with cutout feel)
            canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
            let mut border = vg::Paint::color(theme::toggle_off_border(mode));
            border.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border);

            // Moon shape: crescent via overlapping circles
            let moon_r = 6.0 * dpi;
            let mut moon = vg::Path::new();
            moon.circle(cx_x - 1.0 * dpi, cy, moon_r);
            canvas.fill_path(&moon, &vg::Paint::color(theme::theme_toggle_moon(mode)));
        } else {
            // Dark mode: show sun icon (bright circle with rays)
            canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
            let mut border = vg::Paint::color(theme::toggle_off_border(mode));
            border.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border);

            // Sun circle
            let sun_r = 4.0 * dpi;
            let mut sun = vg::Path::new();
            sun.circle(cx_x, cy, sun_r);
            canvas.fill_path(&sun, &vg::Paint::color(theme::theme_toggle_sun(mode)));

            // Sun rays
            let ray_len = 2.5 * dpi;
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
