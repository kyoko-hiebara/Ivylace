/// AUTO button + full-screen overlay for automatic parameter setting.
///
/// Architecture:
///   - AutoButton: clickable toggle in header, opens BPM input overlay
///   - AutoOverlay: full-screen SelfDirected overlay with BPM input UI,
///     then threshold convergence with progress display
///   - SharedAutoPhase: lock-free phase state shared between button and overlay
///   - GuiContext: used from draw() to apply params (nih_plug_vizia has no timers)
///   - Audio thread: GR accumulation for threshold convergence
///
/// A/B slot integration:
///   - AUTO writes attack/release/threshold to slot B
///   - On Set, switches to slot B (ab_active_is_b = true)
///   - On cancel, switches back to slot A
///
/// State machine: Idle → BpmInput → MeasuringGR → Done → Idle

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::theme::{self, ThemeMode};
use super::Data;
use crate::{AutoAnalysisState, IvylaceParams, SlotStorage, NUM_BANDS};

/// Done flash duration (ms)
const DONE_FLASH_MS: u128 = 1500;
/// Target GR for convergence check
const TARGET_GR: f32 = -2.5;
/// Max convergence iterations
const MAX_ITERATIONS: u32 = 3;
/// BPM range
const BPM_MIN: u32 = 60;
const BPM_MAX: u32 = 300;
const BPM_DEFAULT: u32 = 128;

/// Auto-analysis phases
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AutoPhase {
    Idle,
    BpmInput,
    MeasuringGR,
    Done,
}

/// Lock-free shared phase state between AutoButton and AutoOverlay.
pub struct SharedAutoPhase {
    bpm_input: AtomicBool,
    measuring: AtomicBool,
    done: AtomicBool,
}

impl SharedAutoPhase {
    pub fn new() -> Self {
        Self {
            bpm_input: AtomicBool::new(false),
            measuring: AtomicBool::new(false),
            done: AtomicBool::new(false),
        }
    }

    pub fn load(&self) -> AutoPhase {
        if self.done.load(Ordering::Relaxed) {
            AutoPhase::Done
        } else if self.measuring.load(Ordering::Relaxed) {
            AutoPhase::MeasuringGR
        } else if self.bpm_input.load(Ordering::Relaxed) {
            AutoPhase::BpmInput
        } else {
            AutoPhase::Idle
        }
    }

    pub fn store(&self, phase: AutoPhase) {
        match phase {
            AutoPhase::Idle => {
                self.bpm_input.store(false, Ordering::Relaxed);
                self.measuring.store(false, Ordering::Relaxed);
                self.done.store(false, Ordering::Relaxed);
            }
            AutoPhase::BpmInput => {
                self.measuring.store(false, Ordering::Relaxed);
                self.done.store(false, Ordering::Relaxed);
                self.bpm_input.store(true, Ordering::Relaxed);
            }
            AutoPhase::MeasuringGR => {
                self.bpm_input.store(false, Ordering::Relaxed);
                self.done.store(false, Ordering::Relaxed);
                self.measuring.store(true, Ordering::Relaxed);
            }
            AutoPhase::Done => {
                self.bpm_input.store(false, Ordering::Relaxed);
                self.measuring.store(false, Ordering::Relaxed);
                self.done.store(true, Ordering::Relaxed);
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.bpm_input.load(Ordering::Relaxed)
            || self.measuring.load(Ordering::Relaxed)
            || self.done.load(Ordering::Relaxed)
    }
}

// ══════════════════════════════════════════════════════════════
//  AUTO Button (header toggle)
// ══════════════════════════════════════════════════════════════

pub struct AutoButton {
    phase: Arc<SharedAutoPhase>,
    glass_mode: Arc<AtomicBool>,
    font_id: Cell<Option<vg::FontId>>,
}

impl AutoButton {
    pub fn new(
        cx: &mut Context,
        _auto_analysis: Arc<AutoAnalysisState>,
        phase: Arc<SharedAutoPhase>,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        Self {
            phase,
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

impl View for AutoButton {
    fn element(&self) -> Option<&'static str> {
        Some("auto-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let current = self.phase.load();
                if current == AutoPhase::Idle {
                    self.phase.store(AutoPhase::BpmInput);
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
        let current_phase = self.phase.load();
        let is_active = current_phase != AutoPhase::Idle;

        let mut path = vg::Path::new();
        path.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpi);

        if is_active && current_phase != AutoPhase::Done {
            let ac = theme::auto_button_active(mode);
            canvas.fill_path(&path, &vg::Paint::color(ac));
            let mut bp = vg::Paint::color(theme::with_alpha(ac, 0.6));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);
        } else if current_phase == AutoPhase::Done {
            let ac = theme::auto_button_active(mode);
            canvas.fill_path(&path, &vg::Paint::color(ac));
            let mut bp = vg::Paint::color(theme::with_alpha(ac, 0.6));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);
        } else {
            canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg(mode)));
            let mut bp = vg::Paint::color(theme::toggle_off_border(mode));
            bp.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &bp);
        }

        if let Some(font) = self.ensure_font(canvas) {
            let text = if current_phase == AutoPhase::Done { "OK!" } else { "AUTO" };
            let tc = if is_active {
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
                bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5, text, &paint,
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════
//  Auto Overlay (BPM input + threshold convergence)
// ══════════════════════════════════════════════════════════════

/// Hit test regions for femtovg-drawn interactive elements
#[derive(Clone, Copy)]
struct HitRegions {
    /// ◀ button rect
    left_arrow: (f32, f32, f32, f32),
    /// ▶ button rect
    right_arrow: (f32, f32, f32, f32),
    /// [Set] button rect
    set_button: (f32, f32, f32, f32),
}

impl HitRegions {
    fn new() -> Self {
        Self {
            left_arrow: (0.0, 0.0, 0.0, 0.0),
            right_arrow: (0.0, 0.0, 0.0, 0.0),
            set_button: (0.0, 0.0, 0.0, 0.0),
        }
    }

    fn contains(rect: (f32, f32, f32, f32), x: f32, y: f32) -> bool {
        x >= rect.0 && x <= rect.0 + rect.2 && y >= rect.1 && y <= rect.1 + rect.3
    }
}

pub struct AutoOverlay {
    auto_analysis: Arc<AutoAnalysisState>,
    params: Arc<IvylaceParams>,
    phase: Arc<SharedAutoPhase>,
    glass_mode: Arc<AtomicBool>,
    gui_context: Arc<dyn GuiContext>,
    font_id: Cell<Option<vg::FontId>>,
    /// BPM value for input
    bpm: Cell<u32>,
    /// Done timestamp
    done_time: Cell<Option<Instant>>,
    /// Current convergence iteration count
    local_iteration: Cell<u32>,
    /// Whether threshold was applied this iteration
    threshold_applied: Cell<bool>,
    /// Hit regions for click detection (updated by draw)
    hit_regions: Cell<HitRegions>,
    /// The BPM that was set (for display in Done phase)
    set_bpm: Cell<u32>,
    /// A/B slot references for writing AUTO values
    #[allow(dead_code)]
    slot_a: Arc<SlotStorage>,
    slot_b: Arc<SlotStorage>,
    /// BPM keyboard input mode
    bpm_editing: Cell<bool>,
    /// BPM keyboard input buffer
    bpm_edit_buffer: RefCell<String>,
}

impl AutoOverlay {
    pub fn new(
        cx: &mut Context,
        auto_analysis: Arc<AutoAnalysisState>,
        params: Arc<IvylaceParams>,
        phase: Arc<SharedAutoPhase>,
        glass_mode: Arc<AtomicBool>,
        gui_context: Arc<dyn GuiContext>,
        slot_a: Arc<SlotStorage>,
        slot_b: Arc<SlotStorage>,
    ) -> Handle<'_, Self> {
        let phase_for_hover = phase.clone();

        Self {
            auto_analysis,
            params,
            phase,
            glass_mode,
            gui_context,
            font_id: Cell::new(None),
            bpm: Cell::new(BPM_DEFAULT),
            done_time: Cell::new(None),
            local_iteration: Cell::new(0),
            threshold_applied: Cell::new(false),
            hit_regions: Cell::new(HitRegions::new()),
            set_bpm: Cell::new(BPM_DEFAULT),
            slot_a,
            slot_b,
            bpm_editing: Cell::new(false),
            bpm_edit_buffer: RefCell::new(String::new()),
        }
        .build(cx, |_| {})
        .position_type(PositionType::SelfDirected)
        .left(Pixels(0.0))
        .top(Pixels(0.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .hoverable(Data::params.map(move |_| phase_for_hover.is_active()))
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

    /// Set attack/release from BPM. Writes to slot B for immediate audio thread use,
    /// and also calls gui_context.raw_* for host notification (knob display, automation).
    fn apply_attack_release(&self, bpm: u32) {
        // Copy current params to slot B as starting point before AUTO modifies it
        self.slot_b.load_from_params(&self.params);

        // Bus comp attack: Low=30ms, LowMid=10ms, HighMid=3ms, High=1ms
        // AttackParam: 6 variants (0.1, 0.3, 1, 3, 10, 30ms) → indices 0-5
        // Normalized: index / 5.0
        let attack_indices: [u32; NUM_BANDS] = [5, 4, 3, 2]; // 30ms, 10ms, 3ms, 1ms
        let attack_step_max = 5.0_f32;

        for i in 0..NUM_BANDS {
            // Write to slot B (immediate effect on audio thread when B is active)
            self.slot_b.attack_idx[i].store(attack_indices[i], Ordering::Relaxed);

            // Also notify host via gui_context (for knob display / automation recording)
            let norm = attack_indices[i] as f32 / attack_step_max;
            let ptr = self.params.bands[i].attack.as_ptr();
            eprintln!("[AUTO] Setting attack band {} → idx={}, norm={:.3}", i, attack_indices[i], norm);
            unsafe {
                self.gui_context.raw_begin_set_parameter(ptr);
                self.gui_context.raw_set_parameter_normalized(ptr, norm);
                self.gui_context.raw_end_set_parameter(ptr);
            }
        }

        // Bus comp release: BPM-dependent
        // ReleaseParam: 5 variants (100, 300, 600, 1200, Auto) → indices 0-4
        // Bus comp: Low=2×quarter, LowMid=1×quarter, HighMid=0.5×quarter, High=0.25×quarter
        let quarter_ms = 60_000.0 / bpm as f64;
        let band_factors: [f64; NUM_BANDS] = [2.0, 1.0, 0.5, 0.25];
        let release_ms_options: [f64; 4] = [100.0, 300.0, 600.0, 1200.0];
        let release_step_max = 4.0_f32;

        for i in 0..NUM_BANDS {
            let release_idx = if bpm > 160 && i <= 1 {
                4usize // Auto for fast tempos on low bands
            } else {
                let target_ms = quarter_ms * band_factors[i];
                let mut best_idx = 0usize;
                let mut best_dist = f64::MAX;
                for (idx, &val) in release_ms_options.iter().enumerate() {
                    let dist = (val - target_ms).abs();
                    if dist < best_dist {
                        best_dist = dist;
                        best_idx = idx;
                    }
                }
                best_idx
            };

            // Write to slot B
            self.slot_b.release_idx[i].store(release_idx as u32, Ordering::Relaxed);

            // Also notify host via gui_context
            let norm = release_idx as f32 / release_step_max;
            let ptr = self.params.bands[i].release.as_ptr();
            eprintln!("[AUTO] Setting release band {} → idx={}, norm={:.3}", i, release_idx, norm);
            unsafe {
                self.gui_context.raw_begin_set_parameter(ptr);
                self.gui_context.raw_set_parameter_normalized(ptr, norm);
                self.gui_context.raw_end_set_parameter(ptr);
            }
        }

        // Initialize threshold in slot B from current param values
        for i in 0..NUM_BANDS {
            let current_norm = self.params.bands[i].threshold.modulated_normalized_value();
            let current_db = current_norm * 40.0 - 40.0;
            self.slot_b.threshold_db[i].store(current_db);
            eprintln!("[AUTO] Init threshold slot_b band {} → {:.1} dB (norm={:.4})", i, current_db, current_norm);
        }

        // Switch to slot B — audio thread will read from slot B
        self.params.ab_active_is_b.store(true, Ordering::Release);
    }

    /// Apply computed threshold from audio thread. Slot B is already updated
    /// by the audio thread directly. This additionally notifies the host via gui_context
    /// for knob display and automation recording.
    fn apply_threshold_notify_host(&self) {
        for i in 0..NUM_BANDS {
            let norm = self.auto_analysis.target_threshold[i].load();
            let slot_b_db = self.slot_b.threshold_db[i].load();
            let ptr = self.params.bands[i].threshold.as_ptr();
            eprintln!("[AUTO] Threshold band {} → norm={:.4}, slot_b_db={:.1}", i, norm, slot_b_db);
            unsafe {
                self.gui_context.raw_begin_set_parameter(ptr);
                self.gui_context.raw_set_parameter_normalized(ptr, norm);
                self.gui_context.raw_end_set_parameter(ptr);
            }
        }
        self.auto_analysis.params_applied.store(true, Ordering::Release);
        self.threshold_applied.set(true);
    }

    /// Start next GR measurement iteration
    fn start_measurement(&self) {
        self.auto_analysis.done.store(false, Ordering::Release);
        self.auto_analysis.params_ready.store(false, Ordering::Release);
        self.auto_analysis.params_applied.store(false, Ordering::Release);
        self.auto_analysis.progress.store(0.0);
        self.threshold_applied.set(false);
        self.auto_analysis.active.store(true, Ordering::Release);
    }

    /// Check if convergence is good enough to stop early
    fn check_convergence(&self) -> bool {
        for i in 0..NUM_BANDS {
            let gr = self.auto_analysis.avg_gr[i].load();
            if (gr - TARGET_GR).abs() > 0.5 {
                return false;
            }
        }
        true
    }

    /// Polling logic called from draw() — the only reliable per-frame callback.
    /// When audio thread finishes measurement, notifies host via gui_context.
    /// The actual parameter values are already applied via slot B atomics.
    fn poll_state_draw(&self) {
        let current = self.phase.load();

        if current == AutoPhase::MeasuringGR {
            // Check if audio thread finished a measurement
            if self.auto_analysis.params_ready.load(Ordering::Acquire)
                && !self.threshold_applied.get()
            {
                // Notify host about threshold changes (slot B already updated by audio thread)
                self.apply_threshold_notify_host();

                let iter = self.local_iteration.get() + 1;
                self.local_iteration.set(iter);
                self.auto_analysis.iteration.store(iter, Ordering::Relaxed);

                // Check if we should continue or stop
                if iter >= MAX_ITERATIONS || self.check_convergence() {
                    // Done! Slot B remains active with AUTO values
                    self.phase.store(AutoPhase::Done);
                    self.done_time.set(Some(Instant::now()));
                } else {
                    // Start next iteration
                    self.start_measurement();
                }
            }
        }

        if current == AutoPhase::Done {
            if let Some(t) = self.done_time.get() {
                if t.elapsed().as_millis() > DONE_FLASH_MS {
                    // AUTO complete — slot B stays active with the computed values.
                    // User can switch back to slot A via A/B toggle.
                    self.phase.store(AutoPhase::Idle);
                    self.local_iteration.set(0);

                    // Save slot B persist
                    if let Ok(mut persist) = self.params.slot_b_persist.lock() {
                        *persist = crate::SlotPersist::from_slot(&self.slot_b);
                    }
                }
            }
        }
    }

    fn cancel(&self) {
        self.auto_analysis.active.store(false, Ordering::Release);
        // Switch back to slot A (manual settings)
        self.params.ab_active_is_b.store(false, Ordering::Release);
        self.phase.store(AutoPhase::Idle);
        self.local_iteration.set(0);
    }
}

impl View for AutoOverlay {
    fn element(&self) -> Option<&'static str> {
        Some("auto-overlay")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let current = self.phase.load();
        if current == AutoPhase::Idle {
            return;
        }

        event.map(|window_event, meta| {
            match window_event {
                // ── Keyboard: BPM direct input ──
                WindowEvent::CharInput(c) => {
                    if current == AutoPhase::BpmInput && c.is_ascii_digit() {
                        if !self.bpm_editing.get() {
                            self.bpm_editing.set(true);
                            self.bpm_edit_buffer.borrow_mut().clear();
                        }
                        self.bpm_edit_buffer.borrow_mut().push(*c);
                        cx.needs_redraw();
                        meta.consume();
                    }
                }
                WindowEvent::KeyDown(code, _) => {
                    if current == AutoPhase::BpmInput {
                        match code {
                            Code::Enter | Code::NumpadEnter => {
                                if self.bpm_editing.get() {
                                    if let Ok(val) = self.bpm_edit_buffer.borrow().parse::<u32>() {
                                        self.bpm.set(val.clamp(BPM_MIN, BPM_MAX));
                                    }
                                    self.bpm_editing.set(false);
                                    self.bpm_edit_buffer.borrow_mut().clear();
                                    cx.needs_redraw();
                                    meta.consume();
                                }
                            }
                            Code::Escape => {
                                if self.bpm_editing.get() {
                                    // Cancel typing
                                    self.bpm_editing.set(false);
                                    self.bpm_edit_buffer.borrow_mut().clear();
                                    cx.needs_redraw();
                                } else {
                                    // Cancel entire BPM input
                                    self.cancel();
                                    cx.needs_redraw();
                                }
                                meta.consume();
                            }
                            Code::Backspace => {
                                if self.bpm_editing.get() {
                                    self.bpm_edit_buffer.borrow_mut().pop();
                                    if self.bpm_edit_buffer.borrow().is_empty() {
                                        self.bpm_editing.set(false);
                                    }
                                    cx.needs_redraw();
                                    meta.consume();
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // ── Mouse interactions ──
                WindowEvent::MouseDown(MouseButton::Left) => {
                    // Grab focus for keyboard events
                    cx.focus();

                    if current == AutoPhase::BpmInput {
                        // If was typing, confirm typed value first
                        if self.bpm_editing.get() {
                            if let Ok(val) = self.bpm_edit_buffer.borrow().parse::<u32>() {
                                self.bpm.set(val.clamp(BPM_MIN, BPM_MAX));
                            }
                            self.bpm_editing.set(false);
                            self.bpm_edit_buffer.borrow_mut().clear();
                        }

                        // Check hit regions
                        let mouse = cx.mouse();
                        let mx = mouse.cursorx;
                        let my = mouse.cursory;
                        let regions = self.hit_regions.get();

                        if HitRegions::contains(regions.left_arrow, mx, my) {
                            let bpm = self.bpm.get();
                            self.bpm.set(bpm.saturating_sub(1).max(BPM_MIN));
                            cx.needs_redraw();
                        } else if HitRegions::contains(regions.right_arrow, mx, my) {
                            let bpm = self.bpm.get();
                            self.bpm.set((bpm + 1).min(BPM_MAX));
                            cx.needs_redraw();
                        } else if HitRegions::contains(regions.set_button, mx, my) {
                            // Confirm BPM → set attack/release → start GR measurement
                            let bpm = self.bpm.get();
                            self.set_bpm.set(bpm);
                            eprintln!("[AUTO] Set clicked! BPM={}", bpm);
                            self.apply_attack_release(bpm);
                            self.local_iteration.set(0);
                            self.auto_analysis.iteration.store(0, Ordering::Relaxed);
                            self.phase.store(AutoPhase::MeasuringGR);
                            self.start_measurement();
                            cx.needs_redraw();
                        } else {
                            // Click outside → cancel
                            self.cancel();
                            cx.needs_redraw();
                        }
                    } else if current == AutoPhase::MeasuringGR {
                        // Cancel measurement
                        self.cancel();
                        cx.needs_redraw();
                    }
                    meta.consume();
                }
                WindowEvent::MouseScroll(_, y) => {
                    if current == AutoPhase::BpmInput {
                        // If was typing, confirm first
                        if self.bpm_editing.get() {
                            if let Ok(val) = self.bpm_edit_buffer.borrow().parse::<u32>() {
                                self.bpm.set(val.clamp(BPM_MIN, BPM_MAX));
                            }
                            self.bpm_editing.set(false);
                            self.bpm_edit_buffer.borrow_mut().clear();
                        }

                        let bpm = self.bpm.get();
                        if *y > 0.0 {
                            self.bpm.set((bpm + 1).min(BPM_MAX));
                        } else if *y < 0.0 {
                            self.bpm.set(bpm.saturating_sub(1).max(BPM_MIN));
                        }
                        cx.needs_redraw();
                    }
                    meta.consume();
                }
                WindowEvent::MouseDown(_)
                | WindowEvent::MouseUp(_)
                | WindowEvent::MouseMove(_, _)
                | WindowEvent::MouseDoubleClick(_) => {
                    meta.consume();
                }
                _ => {}
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // State polling in draw() — detect when audio thread completes measurement.
        self.poll_state_draw();

        let current = self.phase.load();
        if current == AutoPhase::Idle {
            return;
        }

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

        // Semi-transparent backdrop
        let mut bg = vg::Path::new();
        bg.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        let bg_color = match mode {
            ThemeMode::Dark => vg::Color::rgba(20, 15, 40, 180),
            ThemeMode::Glass => vg::Color::rgba(240, 244, 250, 180),
        };
        canvas.fill_path(&bg, &vg::Paint::color(bg_color));

        let Some(font) = self.ensure_font(canvas) else { return };

        let text_color = match mode {
            ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 230),
            ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 230),
        };
        let sub_color = match mode {
            ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 130),
            ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 130),
        };
        let dim_color = match mode {
            ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 80),
            ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 80),
        };
        let accent = theme::auto_button_active(mode);

        let cx_x = bounds.x + bounds.w * 0.5;
        let cy = bounds.y + bounds.h * 0.42;

        match current {
            AutoPhase::BpmInput => {
                self.draw_bpm_input(canvas, font, cx_x, cy, dpi, text_color, sub_color, dim_color, accent, mode);
            }
            AutoPhase::MeasuringGR => {
                self.draw_measuring(canvas, font, cx_x, cy, dpi, text_color, sub_color, dim_color, accent, mode, bounds);
            }
            AutoPhase::Done => {
                self.draw_done(canvas, font, cx_x, cy, dpi, text_color, sub_color, accent);
            }
            AutoPhase::Idle => {}
        }
    }
}

// ── Drawing helpers ──

impl AutoOverlay {
    fn draw_bpm_input(
        &self, canvas: &mut Canvas, font: vg::FontId,
        cx_x: f32, cy: f32, dpi: f32,
        text_color: vg::Color, sub_color: vg::Color, dim_color: vg::Color,
        accent: vg::Color, mode: ThemeMode,
    ) {
        // "Enter BPM" title
        let mut tp = vg::Paint::color(text_color);
        tp.set_font_size(16.0 * dpi);
        tp.set_text_align(vg::Align::Center);
        tp.set_text_baseline(vg::Baseline::Middle);
        tp.set_font(&[font]);
        let _ = canvas.fill_text(cx_x, cy - 30.0 * dpi, "Enter BPM", &tp);

        // ◀  128  ▶  row
        let bpm = self.bpm.get();
        let bpm_text = if self.bpm_editing.get() {
            let buf = self.bpm_edit_buffer.borrow();
            if buf.is_empty() { "_".to_string() } else { format!("{}_", buf) }
        } else {
            format!("{}", bpm)
        };
        let arrow_size = 28.0 * dpi;
        let num_width = 60.0 * dpi;
        let total_w = arrow_size * 2.0 + num_width;
        let left_x = cx_x - total_w * 0.5;

        // ◀ button
        let la_x = left_x;
        let la_y = cy - arrow_size * 0.5;
        {
            let mut p = vg::Path::new();
            p.rounded_rect(la_x, la_y, arrow_size, arrow_size, 4.0 * dpi);
            let btn_bg = match mode {
                ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 25),
                ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 20),
            };
            canvas.fill_path(&p, &vg::Paint::color(btn_bg));
            let mut ap = vg::Paint::color(text_color);
            ap.set_font_size(16.0 * dpi);
            ap.set_text_align(vg::Align::Center);
            ap.set_text_baseline(vg::Baseline::Middle);
            ap.set_font(&[font]);
            let _ = canvas.fill_text(la_x + arrow_size * 0.5, cy, "<", &ap);
        }

        // BPM number
        {
            let mut np = vg::Paint::color(accent);
            np.set_font_size(28.0 * dpi);
            np.set_text_align(vg::Align::Center);
            np.set_text_baseline(vg::Baseline::Middle);
            np.set_font(&[font]);
            let _ = canvas.fill_text(cx_x, cy, &bpm_text, &np);
        }

        // ▶ button
        let ra_x = left_x + arrow_size + num_width;
        let ra_y = cy - arrow_size * 0.5;
        {
            let mut p = vg::Path::new();
            p.rounded_rect(ra_x, ra_y, arrow_size, arrow_size, 4.0 * dpi);
            let btn_bg = match mode {
                ThemeMode::Dark => vg::Color::rgba(255, 255, 255, 25),
                ThemeMode::Glass => vg::Color::rgba(30, 40, 60, 20),
            };
            canvas.fill_path(&p, &vg::Paint::color(btn_bg));
            let mut ap = vg::Paint::color(text_color);
            ap.set_font_size(16.0 * dpi);
            ap.set_text_align(vg::Align::Center);
            ap.set_text_baseline(vg::Baseline::Middle);
            ap.set_font(&[font]);
            let _ = canvas.fill_text(ra_x + arrow_size * 0.5, cy, ">", &ap);
        }

        // [Set] button
        let set_w = 60.0 * dpi;
        let set_h = 26.0 * dpi;
        let set_x = cx_x - set_w * 0.5;
        let set_y = cy + 30.0 * dpi;
        {
            let mut p = vg::Path::new();
            p.rounded_rect(set_x, set_y, set_w, set_h, 6.0 * dpi);
            canvas.fill_path(&p, &vg::Paint::color(accent));
            let mut sp = vg::Paint::color(match mode {
                ThemeMode::Dark => vg::Color::rgba(0, 0, 0, 220),
                ThemeMode::Glass => vg::Color::rgba(255, 255, 255, 240),
            });
            sp.set_font_size(12.0 * dpi);
            sp.set_text_align(vg::Align::Center);
            sp.set_text_baseline(vg::Baseline::Middle);
            sp.set_font(&[font]);
            let _ = canvas.fill_text(cx_x, set_y + set_h * 0.5, "Set", &sp);
        }

        // Scroll hint
        {
            let mut hp = vg::Paint::color(dim_color);
            hp.set_font_size(9.0 * dpi);
            hp.set_text_align(vg::Align::Center);
            hp.set_text_baseline(vg::Baseline::Top);
            hp.set_font(&[font]);
            let _ = canvas.fill_text(cx_x, set_y + set_h + 10.0 * dpi, "Type / Scroll to adjust \u{00b7} Click outside to cancel", &hp);
        }

        // Sub info
        {
            let mut ip = vg::Paint::color(sub_color);
            ip.set_font_size(10.0 * dpi);
            ip.set_text_align(vg::Align::Center);
            ip.set_text_baseline(vg::Baseline::Bottom);
            ip.set_font(&[font]);
            let _ = canvas.fill_text(cx_x, cy - 44.0 * dpi, "Auto-set attack, release & threshold", &ip);
        }

        // Store hit regions for event handling
        self.hit_regions.set(HitRegions {
            left_arrow: (la_x, la_y, arrow_size, arrow_size),
            right_arrow: (ra_x, ra_y, arrow_size, arrow_size),
            set_button: (set_x, set_y, set_w, set_h),
        });
    }

    fn draw_measuring(
        &self, canvas: &mut Canvas, font: vg::FontId,
        cx_x: f32, cy: f32, dpi: f32,
        text_color: vg::Color, sub_color: vg::Color, dim_color: vg::Color,
        accent: vg::Color, _mode: ThemeMode,
        bounds: BoundingBox,
    ) {
        let iter = self.local_iteration.get();
        let status = format!("Adjusting... ({}/{})", iter + 1, MAX_ITERATIONS);

        let mut tp = vg::Paint::color(text_color);
        tp.set_font_size(18.0 * dpi);
        tp.set_text_align(vg::Align::Center);
        tp.set_text_baseline(vg::Baseline::Middle);
        tp.set_font(&[font]);
        let _ = canvas.fill_text(cx_x, cy, &status, &tp);

        // Sub text
        let mut sp = vg::Paint::color(sub_color);
        sp.set_font_size(11.0 * dpi);
        sp.set_text_align(vg::Align::Center);
        sp.set_text_baseline(vg::Baseline::Top);
        sp.set_font(&[font]);
        let _ = canvas.fill_text(cx_x, cy + 16.0 * dpi, "Listening to set threshold", &sp);

        // Progress bar
        let bar_w = bounds.w * 0.4;
        let bar_h = 4.0 * dpi;
        let bar_x = cx_x - bar_w * 0.5;
        let bar_y = cy + 40.0 * dpi;

        let mut track = vg::Path::new();
        track.rounded_rect(bar_x, bar_y, bar_w, bar_h, 2.0 * dpi);
        let tc = vg::Color::rgbaf(text_color.r, text_color.g, text_color.b, 0.15);
        canvas.fill_path(&track, &vg::Paint::color(tc));

        let progress = self.auto_analysis.progress.load();
        if progress > 0.0 {
            let mut fill = vg::Path::new();
            fill.rounded_rect(bar_x, bar_y, bar_w * progress, bar_h, 2.0 * dpi);
            canvas.fill_path(&fill, &vg::Paint::color(accent));
        }

        // Cancel hint
        let mut hp = vg::Paint::color(dim_color);
        hp.set_font_size(9.0 * dpi);
        hp.set_text_align(vg::Align::Center);
        hp.set_text_baseline(vg::Baseline::Top);
        hp.set_font(&[font]);
        let _ = canvas.fill_text(cx_x, bar_y + bar_h + 8.0 * dpi, "Click to cancel", &hp);
    }

    fn draw_done(
        &self, canvas: &mut Canvas, font: vg::FontId,
        cx_x: f32, cy: f32, dpi: f32,
        text_color: vg::Color, sub_color: vg::Color, accent: vg::Color,
    ) {
        let mut tp = vg::Paint::color(accent);
        tp.set_font_size(20.0 * dpi);
        tp.set_text_align(vg::Align::Center);
        tp.set_text_baseline(vg::Baseline::Middle);
        tp.set_font(&[font]);
        let _ = canvas.fill_text(cx_x, cy, "Done!", &tp);

        // BPM display
        let bpm = self.set_bpm.get();
        let bpm_text = format!("{} BPM", bpm);
        let mut bp = vg::Paint::color(text_color);
        bp.set_font_size(14.0 * dpi);
        bp.set_text_align(vg::Align::Center);
        bp.set_text_baseline(vg::Baseline::Top);
        bp.set_font(&[font]);
        let _ = canvas.fill_text(cx_x, cy + 18.0 * dpi, &bpm_text, &bp);

        // GR summary
        let mut gr_parts = Vec::new();
        for i in 0..NUM_BANDS {
            let gr = self.auto_analysis.avg_gr[i].load();
            gr_parts.push(format!("{:.1}", gr));
        }
        let gr_text = format!("GR: {} dB", gr_parts.join(" / "));
        let mut gp = vg::Paint::color(sub_color);
        gp.set_font_size(10.0 * dpi);
        gp.set_text_align(vg::Align::Center);
        gp.set_text_baseline(vg::Baseline::Top);
        gp.set_font(&[font]);
        let _ = canvas.fill_text(cx_x, cy + 38.0 * dpi, &gr_text, &gp);
    }
}
