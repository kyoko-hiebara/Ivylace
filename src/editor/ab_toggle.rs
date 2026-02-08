/// A/B toggle button for switching between manual (A) and AUTO (B) parameter slots.
/// Displays "A" or "B" and toggles `ab_active_is_b` on click.
/// When switching slots, persists the outgoing slot and syncs the incoming slot.

use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use super::theme::{self, ThemeMode};
use crate::{IvylaceParams, SlotPersist, SlotStorage};

pub struct AbToggle {
    params: Arc<IvylaceParams>,
    slot_a: Arc<SlotStorage>,
    slot_b: Arc<SlotStorage>,
    glass_mode: Arc<AtomicBool>,
    font_id: Cell<Option<vg::FontId>>,
}

impl AbToggle {
    pub fn new(
        cx: &mut Context,
        params: Arc<IvylaceParams>,
        slot_a: Arc<SlotStorage>,
        slot_b: Arc<SlotStorage>,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        Self {
            params,
            slot_a,
            slot_b,
            glass_mode,
            font_id: Cell::new(None),
        }
        .build(cx, |_| {})
        .width(Pixels(44.0))
        .height(Pixels(22.0))
        .cursor(CursorIcon::Hand)
    }

    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        match self.font_id.get() {
            Some(id) => Some(id),
            None => {
                let id = canvas
                    .add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR)
                    .ok();
                if let Some(id) = id {
                    self.font_id.set(Some(id));
                }
                id
            }
        }
    }
}

impl View for AbToggle {
    fn element(&self) -> Option<&'static str> {
        Some("ab-toggle")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let is_b = self.params.ab_active_is_b.load(Ordering::Relaxed);

                // Persist outgoing slot before switching
                if is_b {
                    // Leaving B → save B, switch to A
                    if let Ok(mut persist) = self.params.slot_b_persist.lock() {
                        *persist = SlotPersist::from_slot(&self.slot_b);
                    }
                    // Sync slot A from current nih-plug params
                    // (user may have changed params while in A before)
                    self.slot_a.load_from_params(&self.params);
                } else {
                    // Leaving A → save A, switch to B
                    if let Ok(mut persist) = self.params.slot_a_persist.lock() {
                        *persist = SlotPersist::from_slot(&self.slot_a);
                    }
                }

                // Toggle
                self.params.ab_active_is_b.store(!is_b, Ordering::Release);
                cx.needs_redraw();
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 1.0 || bounds.h < 1.0 {
            return;
        }

        let is_b = self.params.ab_active_is_b.load(Ordering::Relaxed);
        let mode = if self.glass_mode.load(Ordering::Relaxed) {
            ThemeMode::Glass
        } else {
            ThemeMode::Dark
        };
        let dpi = cx.scale_factor();

        // Button background
        let mut path = vg::Path::new();
        path.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpi);

        if is_b {
            // Slot B active: same color as segment button active (RATIO, etc.)
            let active_color = theme::seg_active_bg(mode);
            canvas.fill_path(&path, &vg::Paint::color(active_color));
            let mut border_paint = vg::Paint::color(theme::seg_border(mode));
            border_paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border_paint);
        } else {
            // Slot A active: default off state
            canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
            let mut border_paint = vg::Paint::color(theme::toggle_off_border(mode));
            border_paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border_paint);
        }

        // Label: "A" or "B"
        if let Some(font) = self.ensure_font(canvas) {
            let text = if is_b { "B" } else { "A" };
            let text_color = if is_b {
                theme::seg_active_text(mode)
            } else {
                theme::toggle_off_text(mode)
            };
            let mut paint = vg::Paint::color(text_color);
            paint.set_font_size(11.0 * dpi);
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
}

// ── INIT Button ──────────────────────────────────────────────

/// INIT button: resets all per-band compressor parameters in the active slot to defaults.
pub struct InitButton {
    params: Arc<IvylaceParams>,
    slot_a: Arc<SlotStorage>,
    slot_b: Arc<SlotStorage>,
    glass_mode: Arc<AtomicBool>,
    font_id: Cell<Option<vg::FontId>>,
}

impl InitButton {
    pub fn new(
        cx: &mut Context,
        params: Arc<IvylaceParams>,
        slot_a: Arc<SlotStorage>,
        slot_b: Arc<SlotStorage>,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        Self {
            params,
            slot_a,
            slot_b,
            glass_mode,
            font_id: Cell::new(None),
        }
        .build(cx, |_| {})
        .width(Pixels(44.0))
        .height(Pixels(22.0))
        .cursor(CursorIcon::Hand)
    }

    fn ensure_font(&self, canvas: &mut Canvas) -> Option<vg::FontId> {
        match self.font_id.get() {
            Some(id) => Some(id),
            None => {
                let id = canvas
                    .add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR)
                    .ok();
                if let Some(id) = id {
                    self.font_id.set(Some(id));
                }
                id
            }
        }
    }
}

impl View for InitButton {
    fn element(&self) -> Option<&'static str> {
        Some("init-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                // Reset the active slot to defaults
                let is_b = self.params.ab_active_is_b.load(Ordering::Relaxed);
                let slot = if is_b { &self.slot_b } else { &self.slot_a };
                slot.reset_to_defaults();

                // Notify host: sync all per-band params to default normalized values
                for i in 0..crate::NUM_BANDS {
                    let bp = &self.params.bands[i];

                    // Threshold: -20.0 dB → normalized = (-20+40)/40 = 0.5
                    let thr_ptr = bp.threshold.as_ptr();
                    cx.emit(RawParamEvent::BeginSetParameter(thr_ptr));
                    cx.emit(RawParamEvent::SetParameterNormalized(thr_ptr, 0.5));
                    cx.emit(RawParamEvent::EndSetParameter(thr_ptr));

                    // Ratio: Ratio4 = index 1 → normalized = 1/2 = 0.5
                    let rat_ptr = bp.ratio.as_ptr();
                    cx.emit(RawParamEvent::BeginSetParameter(rat_ptr));
                    cx.emit(RawParamEvent::SetParameterNormalized(rat_ptr, 0.5));
                    cx.emit(RawParamEvent::EndSetParameter(rat_ptr));

                    // Attack: A10 = index 4 → normalized = 4/5 = 0.8
                    let atk_ptr = bp.attack.as_ptr();
                    cx.emit(RawParamEvent::BeginSetParameter(atk_ptr));
                    cx.emit(RawParamEvent::SetParameterNormalized(atk_ptr, 0.8));
                    cx.emit(RawParamEvent::EndSetParameter(atk_ptr));

                    // Release: R300 = index 1 → normalized = 1/4 = 0.25
                    let rel_ptr = bp.release.as_ptr();
                    cx.emit(RawParamEvent::BeginSetParameter(rel_ptr));
                    cx.emit(RawParamEvent::SetParameterNormalized(rel_ptr, 0.25));
                    cx.emit(RawParamEvent::EndSetParameter(rel_ptr));

                    // Makeup: 0.0 dB → normalized = (0+12)/36 = 0.333...
                    let mkp_ptr = bp.makeup.as_ptr();
                    cx.emit(RawParamEvent::BeginSetParameter(mkp_ptr));
                    cx.emit(RawParamEvent::SetParameterNormalized(mkp_ptr, 12.0 / 36.0));
                    cx.emit(RawParamEvent::EndSetParameter(mkp_ptr));

                    // SC HPF: 0 Hz (band 0), 80 Hz (others)
                    // Skewed range: normalize = (plain/500)^factor, factor = 2^(-1.5)
                    let hpf_default = if i == 0 { 0.0_f32 } else { 80.0_f32 };
                    let skew_factor = 2.0f32.powf(-1.5);
                    let hpf_norm = (hpf_default / 500.0).powf(skew_factor);
                    let hpf_ptr = bp.sc_hpf.as_ptr();
                    cx.emit(RawParamEvent::BeginSetParameter(hpf_ptr));
                    cx.emit(RawParamEvent::SetParameterNormalized(hpf_ptr, hpf_norm));
                    cx.emit(RawParamEvent::EndSetParameter(hpf_ptr));
                }

                cx.needs_redraw();
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 1.0 || bounds.h < 1.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) {
            ThemeMode::Glass
        } else {
            ThemeMode::Dark
        };
        let dpi = cx.scale_factor();

        // Button background (always off-state appearance)
        let mut path = vg::Path::new();
        path.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpi);
        canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
        let mut border_paint = vg::Paint::color(theme::toggle_off_border(mode));
        border_paint.set_line_width(1.0 * dpi);
        canvas.stroke_path(&path, &border_paint);

        // "INIT" label
        if let Some(font) = self.ensure_font(canvas) {
            let mut paint = vg::Paint::color(theme::toggle_off_text(mode));
            paint.set_font_size(9.0 * dpi);
            paint.set_text_align(vg::Align::Center);
            paint.set_text_baseline(vg::Baseline::Middle);
            paint.set_font(&[font]);
            let _ = canvas.fill_text(
                bounds.x + bounds.w * 0.5,
                bounds.y + bounds.h * 0.5,
                "INIT",
                &paint,
            );
        }
    }
}
