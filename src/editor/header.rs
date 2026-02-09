/// Global controls header bar.
/// Contains: Input Gain, Mix, Sat Type, OS Realtime, OS Render, Output Gain,
///           AUTO + Delta (stacked), A/B + INIT (stacked), TRIM + BYPASS (stacked).
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use crate::{GlobalOverrides, TrimState};

use super::ab_toggle::{AbToggle, InitButton};
use super::auto_button::{AutoButton, SharedAutoPhase};
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
    pub fn new(
        cx: &mut Context,
        glass_mode: Arc<AtomicBool>,
        auto_phase: Arc<SharedAutoPhase>,
        trim_state: Arc<TrimState>,
    ) -> Handle<'_, Self> {
        Self.build(cx, move |cx| {
            let gm = glass_mode.clone();
            let ovr_header = Data::params.get(cx).global_overrides.clone();
            HStack::new(cx, move |cx| {
                // ── Left group: In Gain + Mix ──
                let gm_left = gm.clone();
                let ovr_left = ovr_header.clone();
                HStack::new(cx, move |cx| {
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.input_gain,
                        KnobSize::Sm,
                        theme::accent(),
                        "In Gain",
                        gm_left.clone(),
                        Some((ovr_left.clone(), super::knob::GlobalOverrideKind::InputGainDb)),
                    );
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.dry_wet,
                        KnobSize::Sm,
                        theme::accent(),
                        "Mix",
                        gm_left.clone(),
                        Some((ovr_left.clone(), super::knob::GlobalOverrideKind::DryWet)),
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

                // ── Right group: Out Gain + button grid ──
                let gm_right = gm.clone();
                let p_right = Data::params.get(cx);
                let delta_monitor = p_right.delta_monitor.clone();
                let trim_enabled = p_right.trim_enabled.clone();
                let auto_phase_right = auto_phase.clone();
                let trim_state_right = trim_state.clone();
                let ovr_right = ovr_header.clone();
                HStack::new(cx, move |cx| {
                    GlassKnob::new(
                        cx,
                        Data::params,
                        |p| &p.output_gain,
                        KnobSize::Sm,
                        theme::accent(),
                        "Out Gain",
                        gm_right.clone(),
                        Some((ovr_right.clone(), super::knob::GlobalOverrideKind::OutputGainDb)),
                    );

                    // Button grid: [AUTO, DELTA] + [A/B, INIT] + [TRIM, BYPASS]
                    let gm_stack = gm_right.clone();
                    let auto_st = Data::auto_state.get(cx);
                    let auto_ph = auto_phase_right.clone();
                    let p_ab = Data::params.get(cx);
                    let sa_ab = Data::slot_a.get(cx);
                    let sb_ab = Data::slot_b.get(cx);
                    HStack::new(cx, move |cx| {
                        // Column 1: AUTO + DELTA
                        let gm_col1 = gm_stack.clone();
                        VStack::new(cx, move |cx| {
                            AutoButton::new(
                                cx,
                                auto_st.clone(),
                                auto_ph.clone(),
                                gm_col1.clone(),
                            );
                            DeltaMonitorToggle::new(cx, delta_monitor.clone(), gm_col1.clone());
                        })
                        .row_between(Pixels(6.0))
                        .width(Auto)
                        .height(Auto);

                        // Column 2: A/B toggle + INIT
                        let gm_col2 = gm_stack.clone();
                        VStack::new(cx, move |cx| {
                            AbToggle::new(cx, p_ab.clone(), sa_ab.clone(), sb_ab.clone(), gm_col2.clone());
                            InitButton::new(cx, p_ab.clone(), sa_ab.clone(), sb_ab.clone(), gm_col2.clone());
                        })
                        .row_between(Pixels(6.0))
                        .width(Auto)
                        .height(Auto)
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));

                        // Column 3: TRIM + BYPASS
                        let gm_col3 = gm_stack.clone();
                        let p_bypass = Data::params.get(cx);
                        VStack::new(cx, move |cx| {
                            TrimButton::new(cx, trim_state_right.clone(), trim_enabled.clone(), gm_col3.clone());
                            BypassButton::new(cx, p_bypass.clone(), gm_col3.clone());
                        })
                        .row_between(Pixels(6.0))
                        .width(Auto)
                        .height(Auto)
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));
                    })
                    .col_between(Pixels(4.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0))
                    .width(Auto)
                    .height(Auto);
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

// ── Delta Monitor Toggle ────────────────────────────────────

struct DeltaMonitorToggle {
    delta_monitor: Arc<AtomicBool>,
    glass_mode: Arc<AtomicBool>,
    font_id: Cell<Option<vg::FontId>>,
}

impl DeltaMonitorToggle {
    fn new(cx: &mut Context, delta_monitor: Arc<AtomicBool>, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self> {
        Self { delta_monitor, glass_mode, font_id: Cell::new(None) }
            .build(cx, |_| {})
            .width(Pixels(44.0))
            .height(Pixels(22.0))
            .cursor(CursorIcon::Hand)
    }

    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        match self.font_id.get() {
            Some(id) => Some(id),
            None => {
                let id = canvas.add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR).ok();
                if let Some(id) = id {
                    self.font_id.set(Some(id));
                }
                id
            }
        }
    }
}

impl View for DeltaMonitorToggle {
    fn element(&self) -> Option<&'static str> {
        Some("delta-monitor-toggle")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 1.0 || bounds.h < 1.0 {
            return;
        }

        let active = self.delta_monitor.load(Ordering::Relaxed);
        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();

        // Button background
        let mut path = vg::Path::new();
        path.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpi);

        if active {
            // Active: delta accent color (cyan/teal)
            let active_color = theme::delta_monitor_active(mode);
            let paint = vg::Paint::linear_gradient(
                bounds.x, bounds.y,
                bounds.x, bounds.y + bounds.h,
                vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.93),
                vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.80),
            );
            canvas.fill_path(&path, &paint);

            let mut border_paint = vg::Paint::color(theme::with_alpha(active_color, 0.53));
            border_paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border_paint);

            // Glow
            let glow = vg::Paint::box_gradient(
                bounds.x - 2.0, bounds.y - 2.0,
                bounds.w + 4.0, bounds.h + 4.0,
                6.0, 8.0,
                theme::with_alpha(active_color, 0.25),
                vg::Color::rgba(0, 0, 0, 0),
            );
            let mut glow_path = vg::Path::new();
            glow_path.rect(bounds.x - 8.0, bounds.y - 8.0, bounds.w + 16.0, bounds.h + 16.0);
            canvas.fill_path(&glow_path, &glow);
        } else {
            canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
            let mut border_paint = vg::Paint::color(theme::toggle_off_border(mode));
            border_paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border_paint);
        }

        // "DELTA" label
        if let Some(font) = self.ensure_font(canvas) {
            let text_color = if active {
                theme::text_on_active(mode)
            } else {
                theme::toggle_off_text(mode)
            };
            let mut paint = vg::Paint::color(text_color);
            paint.set_font_size(9.0 * dpi);
            paint.set_text_align(vg::Align::Center);
            paint.set_text_baseline(vg::Baseline::Middle);
            paint.set_font(&[font]);
            let _ = canvas.fill_text(
                bounds.x + bounds.w * 0.5,
                bounds.y + bounds.h * 0.5,
                "DELTA",
                &paint,
            );
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let current = self.delta_monitor.load(Ordering::Relaxed);
                self.delta_monitor.store(!current, Ordering::Relaxed);
                cx.needs_redraw();
                meta.consume();
            }
        });
    }
}

// ── TRIM Button ─────────────────────────────────────────────

struct TrimButton {
    trim_state: Arc<TrimState>,
    trim_enabled: Arc<AtomicBool>,
    glass_mode: Arc<AtomicBool>,
    font_id: Cell<Option<vg::FontId>>,
}

impl TrimButton {
    fn new(
        cx: &mut Context,
        trim_state: Arc<TrimState>,
        trim_enabled: Arc<AtomicBool>,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        Self { trim_state, trim_enabled, glass_mode, font_id: Cell::new(None) }
            .build(cx, |_| {})
            .width(Pixels(44.0))
            .height(Pixels(22.0))
            .cursor(CursorIcon::Hand)
    }

    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        match self.font_id.get() {
            Some(id) => Some(id),
            None => {
                let id = canvas.add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR).ok();
                if let Some(id) = id {
                    self.font_id.set(Some(id));
                }
                id
            }
        }
    }
}

impl View for TrimButton {
    fn element(&self) -> Option<&'static str> {
        Some("trim-button")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 1.0 || bounds.h < 1.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let measuring = self.trim_state.active.load(Ordering::Relaxed);
        let trim_on = self.trim_enabled.load(Ordering::Relaxed);

        let mut path = vg::Path::new();
        path.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpi);

        if measuring {
            // Measuring: blink green (use progress as time proxy)
            let progress = self.trim_state.progress.load().clamp(0.0, 1.0);
            let blink = ((progress * 8.0 * std::f32::consts::PI).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            let active_color = theme::trim_active(mode);
            let off_color = theme::toggle_off_bg(mode);
            // Interpolate between off and active
            let r = off_color.r + (active_color.r - off_color.r) * blink;
            let g = off_color.g + (active_color.g - off_color.g) * blink;
            let b = off_color.b + (active_color.b - off_color.b) * blink;
            let a = off_color.a + (active_color.a - off_color.a) * blink;
            canvas.fill_path(&path, &vg::Paint::color(vg::Color::rgbaf(r, g, b, a)));
            let mut bp = vg::Paint::color(theme::with_alpha(active_color, 0.53 * blink));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);

            // Note: continuous redraw is driven by GR meters polling
        } else if trim_on {
            // Enabled: steady green
            let active_color = theme::trim_active(mode);
            let paint = vg::Paint::linear_gradient(
                bounds.x, bounds.y,
                bounds.x, bounds.y + bounds.h,
                vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.93),
                vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.80),
            );
            canvas.fill_path(&path, &paint);

            let mut bp = vg::Paint::color(theme::with_alpha(active_color, 0.53));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);

            // Glow
            let glow = vg::Paint::box_gradient(
                bounds.x - 2.0, bounds.y - 2.0,
                bounds.w + 4.0, bounds.h + 4.0,
                6.0, 8.0,
                theme::with_alpha(active_color, 0.25),
                vg::Color::rgba(0, 0, 0, 0),
            );
            let mut glow_path = vg::Path::new();
            glow_path.rect(bounds.x - 8.0, bounds.y - 8.0, bounds.w + 16.0, bounds.h + 16.0);
            canvas.fill_path(&glow_path, &glow);
        } else {
            canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
            let mut bp = vg::Paint::color(theme::toggle_off_border(mode));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);
        }

        // Label
        if let Some(font) = self.ensure_font(canvas) {
            let text = "TRIM";

            let tc = if measuring || trim_on {
                theme::text_on_active(mode)
            } else {
                theme::toggle_off_text(mode)
            };
            let mut paint = vg::Paint::color(tc);
            paint.set_font_size(9.0 * dpi);
            paint.set_text_align(vg::Align::Center);
            paint.set_text_baseline(vg::Baseline::Middle);
            paint.set_font(&[font]);
            let _ = canvas.fill_text(
                bounds.x + bounds.w * 0.5,
                bounds.y + bounds.h * 0.5,
                text,
                &paint,
            );
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let measuring = self.trim_state.active.load(Ordering::Relaxed);
                let trim_on = self.trim_enabled.load(Ordering::Relaxed);

                if measuring {
                    // Cancel measurement
                    self.trim_state.active.store(false, Ordering::Release);
                    self.trim_enabled.store(false, Ordering::Relaxed);
                } else if trim_on {
                    // Disable trim
                    self.trim_enabled.store(false, Ordering::Relaxed);
                } else {
                    // Start new measurement
                    self.trim_state.done.store(false, Ordering::Relaxed);
                    self.trim_state.progress.store(0.0);
                    self.trim_state.active.store(true, Ordering::Release);
                    // trim_enabled will be set when measurement completes
                }

                cx.needs_redraw();
                meta.consume();
            }
        });
    }
}

// ── BYPASS Button ───────────────────────────────────────────

struct BypassButton {
    params: Arc<crate::IvylaceParams>,
    glass_mode: Arc<AtomicBool>,
    font_id: Cell<Option<vg::FontId>>,
    gesture_active: bool,
}

impl BypassButton {
    fn new(
        cx: &mut Context,
        params: Arc<crate::IvylaceParams>,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        Self { params, glass_mode, font_id: Cell::new(None), gesture_active: false }
            .build(cx, |_| {})
            .width(Pixels(44.0))
            .height(Pixels(22.0))
            .cursor(CursorIcon::Hand)
    }

    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        match self.font_id.get() {
            Some(id) => Some(id),
            None => {
                let id = canvas.add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR).ok();
                if let Some(id) = id {
                    self.font_id.set(Some(id));
                }
                id
            }
        }
    }
}

impl View for BypassButton {
    fn element(&self) -> Option<&'static str> {
        Some("bypass-button")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 1.0 || bounds.h < 1.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let is_bypassed = self.params.bypass.value();

        let mut path = vg::Path::new();
        path.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpi);

        if is_bypassed {
            // Bypassed: red/orange warning
            let active_color = theme::bypass_active(mode);
            let paint = vg::Paint::linear_gradient(
                bounds.x, bounds.y,
                bounds.x, bounds.y + bounds.h,
                vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.93),
                vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.80),
            );
            canvas.fill_path(&path, &paint);

            let mut bp = vg::Paint::color(theme::with_alpha(active_color, 0.53));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);

            // Glow
            let glow = vg::Paint::box_gradient(
                bounds.x - 2.0, bounds.y - 2.0,
                bounds.w + 4.0, bounds.h + 4.0,
                6.0, 8.0,
                theme::with_alpha(active_color, 0.25),
                vg::Color::rgba(0, 0, 0, 0),
            );
            let mut glow_path = vg::Path::new();
            glow_path.rect(bounds.x - 8.0, bounds.y - 8.0, bounds.w + 16.0, bounds.h + 16.0);
            canvas.fill_path(&glow_path, &glow);
        } else {
            canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
            let mut bp = vg::Paint::color(theme::toggle_off_border(mode));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);
        }

        // "BYPASS" label
        if let Some(font) = self.ensure_font(canvas) {
            let tc = if is_bypassed {
                theme::text_on_active(mode)
            } else {
                theme::toggle_off_text(mode)
            };
            let mut paint = vg::Paint::color(tc);
            paint.set_font_size(8.0 * dpi);
            paint.set_text_align(vg::Align::Center);
            paint.set_text_baseline(vg::Baseline::Middle);
            paint.set_font(&[font]);
            let _ = canvas.fill_text(
                bounds.x + bounds.w * 0.5,
                bounds.y + bounds.h * 0.5,
                "BYPASS",
                &paint,
            );
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            match window_event {
                WindowEvent::MouseDown(MouseButton::Left) => {
                    // Gesture split: begin on MouseDown (Cubase compatibility)
                    let param = &self.params.bypass;
                    let current = param.unmodulated_normalized_value();
                    let new_val = if current > 0.5 { 0.0 } else { 1.0 };

                    cx.emit(RawParamEvent::BeginSetParameter(param.as_ptr()));
                    cx.emit(RawParamEvent::SetParameterNormalized(param.as_ptr(), new_val));
                    self.gesture_active = true;
                    cx.capture();
                    cx.needs_redraw();
                    meta.consume();
                }
                WindowEvent::MouseUp(MouseButton::Left) => {
                    if self.gesture_active {
                        let param = &self.params.bypass;
                        cx.emit(RawParamEvent::EndSetParameter(param.as_ptr()));
                        self.gesture_active = false;
                        cx.release();
                        cx.needs_redraw();
                        meta.consume();
                    }
                }
                _ => {}
            }
        });
    }
}
