/// Crossover frequency display with draggable handles.
/// Log-scale frequency axis, band color fills, grid lines.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::theme::{self, ThemeMode};
use super::Data;

const DISPLAY_HEIGHT: f32 = 72.0;
const FREQ_MIN: f32 = 20.0;
const FREQ_MAX: f32 = 20000.0;
const GRID_FREQS: [f32; 8] = [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];

fn freq_to_x(freq: f32, width: f32) -> f32 {
    let log_min = FREQ_MIN.log10();
    let log_max = FREQ_MAX.log10();
    ((freq.log10() - log_min) / (log_max - log_min)) * width
}

fn x_to_freq(x: f32, width: f32) -> f32 {
    let log_min = FREQ_MIN.log10();
    let log_max = FREQ_MAX.log10();
    let log_freq = log_min + (x / width) * (log_max - log_min);
    10.0_f32.powf(log_freq)
}

fn format_freq(freq: f32) -> String {
    if freq >= 1000.0 {
        format!("{}k", (freq / 1000.0) as u32)
    } else {
        format!("{}", freq as u32)
    }
}

pub struct CrossoverDisplay {
    low_base: ParamWidgetBase,
    mid_base: ParamWidgetBase,
    high_base: ParamWidgetBase,
    glass_mode: Arc<AtomicBool>,
    drag_handle: Option<usize>, // 0=low, 1=mid, 2=high
    hover_handle: Option<usize>,
    /// Handle index for a pending reset gesture (double-click).
    /// begin_set + set_value happen on MouseDoubleClick, end_set on MouseUp.
    reset_handle: Option<usize>,
}

impl CrossoverDisplay {
    pub fn new(cx: &mut Context, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self> {
        let low_base = ParamWidgetBase::new(cx, Data::params, |p| &p.xover_low);
        let mid_base = ParamWidgetBase::new(cx, Data::params, |p| &p.xover_mid);
        let high_base = ParamWidgetBase::new(cx, Data::params, |p| &p.xover_high);

        Self {
            low_base,
            mid_base,
            high_base,
            glass_mode,
            drag_handle: None,
            hover_handle: None,
            reset_handle: None,
        }
        .build(cx, |_| {})
        .height(Pixels(DISPLAY_HEIGHT))
        .width(Stretch(1.0))
    }

    fn get_handle_freqs(&self) -> [f32; 3] {
        [
            self.low_base.unmodulated_plain_value(),
            self.mid_base.unmodulated_plain_value(),
            self.high_base.unmodulated_plain_value(),
        ]
    }

    fn handle_for_x(&self, mouse_x: f32, bounds: &BoundingBox) -> Option<usize> {
        let freqs = self.get_handle_freqs();
        let w = bounds.w;

        for (i, &freq) in freqs.iter().enumerate() {
            let hx = bounds.x + freq_to_x(freq, w);
            if (mouse_x - hx).abs() < 10.0 {
                return Some(i);
            }
        }
        None
    }
}

impl View for CrossoverDisplay {
    fn element(&self) -> Option<&'static str> {
        Some("crossover-display")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let bx = bounds.x;
        let by = bounds.y;
        let bw = bounds.w;
        let bh = bounds.h;

        let freqs = self.get_handle_freqs();
        let xover_positions = [
            freq_to_x(freqs[0], bw),
            freq_to_x(freqs[1], bw),
            freq_to_x(freqs[2], bw),
        ];

        // ── Background ──
        {
            let mut path = vg::Path::new();
            path.rect(bx, by, bw, bh);
            canvas.fill_path(&path, &vg::Paint::color(theme::xover_bg(mode)));
        }

        // ── Band fills ──
        {
            let regions = [
                (0.0, xover_positions[0]),
                (xover_positions[0], xover_positions[1]),
                (xover_positions[1], xover_positions[2]),
                (xover_positions[2], bw),
            ];

            for (i, &(start, end)) in regions.iter().enumerate() {
                if end <= start {
                    continue;
                }
                let color = theme::band_color(i, mode);
                let mut path = vg::Path::new();
                path.rect(bx + start, by, end - start, bh);
                let paint = vg::Paint::linear_gradient(
                    bx + start, by,
                    bx + start, by + bh,
                    theme::with_alpha(color, 0.07),
                    theme::with_alpha(color, 0.02),
                );
                canvas.fill_path(&path, &paint);
            }
        }

        // ── Grid lines ──
        {
            for &freq in &GRID_FREQS {
                let gx = bx + freq_to_x(freq, bw);
                let mut path = vg::Path::new();
                path.move_to(gx, by);
                path.line_to(gx, by + bh);
                let mut paint = vg::Paint::color(theme::xover_grid(mode));
                paint.set_line_width(1.0);
                canvas.stroke_path(&path, &paint);

                // Label
                let label = format_freq(freq);
                let mut text_paint = vg::Paint::color(theme::xover_grid_text(mode));
                text_paint.set_font_size(7.0 * dpi);
                text_paint.set_text_align(vg::Align::Center);
                text_paint.set_text_baseline(vg::Baseline::Bottom);
                let _ = canvas.fill_text(gx, by + bh - 2.0, &label, &text_paint);
            }
        }

        // ── Band name labels ──
        {
            let regions = [
                (0.0, xover_positions[0]),
                (xover_positions[0], xover_positions[1]),
                (xover_positions[1], xover_positions[2]),
                (xover_positions[2], bw),
            ];

            for (i, &(start, end)) in regions.iter().enumerate() {
                if end - start < 30.0 {
                    continue;
                }
                let center_x = bx + (start + end) * 0.5;
                let color = theme::band_color(i, mode);
                let mut paint = vg::Paint::color(theme::with_alpha(color, 0.25));
                paint.set_font_size(10.0 * dpi);
                paint.set_text_align(vg::Align::Center);
                paint.set_text_baseline(vg::Baseline::Top);
                let _ = canvas.fill_text(center_x, by + 8.0, theme::BAND_NAMES[i], &paint);
            }
        }

        // ── Crossover handles ──
        {
            let handle_colors = [
                theme::band_color(0, mode), // Low handle uses low band color
                theme::band_color(1, mode), // Mid handle
                theme::band_color(2, mode), // High handle
            ];

            for (i, &hx) in xover_positions.iter().enumerate() {
                let color = handle_colors[i];
                let is_active = self.drag_handle == Some(i);
                let is_hover = self.hover_handle == Some(i);
                let line_width = if is_active || is_hover { 2.0 } else { 1.0 };

                // Vertical line with gradient
                let mut path = vg::Path::new();
                path.move_to(bx + hx, by);
                path.line_to(bx + hx, by + bh);

                let paint = vg::Paint::linear_gradient(
                    bx + hx, by,
                    bx + hx, by + bh,
                    theme::with_alpha(color, 0.0),
                    theme::with_alpha(color, 0.6),
                );
                let mut stroke = paint;
                stroke.set_line_width(line_width * dpi);
                canvas.stroke_path(&path, &stroke);

                // Handle dot
                let dot_alpha = if is_active || is_hover { 0.8 } else { 0.3 };
                let mut dot_path = vg::Path::new();
                dot_path.rounded_rect(bx + hx - 3.0, by + bh * 0.35, 6.0, 30.0, 3.0);
                canvas.fill_path(&dot_path, &vg::Paint::color(theme::with_alpha(color, dot_alpha)));

                // Frequency label on hover/active
                if is_active || is_hover {
                    let freq = freqs[i];
                    let label = format!("{} Hz", freq as u32);
                    let mut bg_path = vg::Path::new();
                    let label_w = 50.0;
                    let label_h = 16.0;
                    bg_path.rounded_rect(
                        bx + hx - label_w * 0.5, by + 20.0,
                        label_w, label_h, 4.0,
                    );
                    canvas.fill_path(&bg_path, &vg::Paint::color(theme::xover_handle_label_bg(mode)));

                    let mut text_paint = vg::Paint::color(theme::text_primary(mode));
                    text_paint.set_font_size(9.0 * dpi);
                    text_paint.set_text_align(vg::Align::Center);
                    text_paint.set_text_baseline(vg::Baseline::Middle);
                    let _ = canvas.fill_text(bx + hx, by + 20.0 + label_h * 0.5, &label, &text_paint);
                }
            }
        }

        // ── Bottom border ──
        {
            let mut path = vg::Path::new();
            path.move_to(bx, by + bh);
            path.line_to(bx + bw, by + bh);
            let mut paint = vg::Paint::color(theme::xover_border(mode));
            paint.set_line_width(1.0);
            canvas.stroke_path(&path, &paint);
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            let bounds = cx.bounds();

            match window_event {
                WindowEvent::MouseDown(MouseButton::Left) => {
                    let mouse_x = cx.mouse().cursorx;
                    if let Some(handle) = self.handle_for_x(mouse_x, &bounds) {
                        self.drag_handle = Some(handle);
                        cx.capture();
                        cx.set_active(true);

                        match handle {
                            0 => self.low_base.begin_set_parameter(cx),
                            1 => self.mid_base.begin_set_parameter(cx),
                            2 => self.high_base.begin_set_parameter(cx),
                            _ => {}
                        }
                        meta.consume();
                    }
                }
                WindowEvent::MouseUp(MouseButton::Left) => {
                    if let Some(handle) = self.drag_handle {
                        match handle {
                            0 => self.low_base.end_set_parameter(cx),
                            1 => self.mid_base.end_set_parameter(cx),
                            2 => self.high_base.end_set_parameter(cx),
                            _ => {}
                        }
                        self.drag_handle = None;
                        cx.release();
                        cx.set_active(false);
                        meta.consume();
                    } else if let Some(handle) = self.reset_handle {
                        // End the reset gesture (double-click)
                        match handle {
                            0 => self.low_base.end_set_parameter(cx),
                            1 => self.mid_base.end_set_parameter(cx),
                            2 => self.high_base.end_set_parameter(cx),
                            _ => {}
                        }
                        self.reset_handle = None;
                        cx.release();
                        meta.consume();
                    }
                }
                WindowEvent::MouseMove(x, _y) => {
                    // Update hover
                    self.hover_handle = self.handle_for_x(*x, &bounds);

                    if let Some(handle) = self.drag_handle {
                        let local_x = *x - bounds.x;
                        let freq = x_to_freq(local_x.clamp(0.0, bounds.w), bounds.w);

                        // Get current freqs for constraint checking
                        let freqs = self.get_handle_freqs();

                        let base = match handle {
                            0 => &self.low_base,
                            1 => &self.mid_base,
                            2 => &self.high_base,
                            _ => return,
                        };

                        // Clamp freq to valid range with spacing constraints
                        let clamped_freq = match handle {
                            0 => freq.clamp(FREQ_MIN, freqs[1] / 1.1),
                            1 => freq.clamp(freqs[0] * 1.1, freqs[2] / 1.1),
                            2 => freq.clamp(freqs[1] * 1.1, FREQ_MAX),
                            _ => freq,
                        };

                        let normalized = base.preview_normalized(clamped_freq);
                        base.set_normalized_value(cx, normalized);
                        meta.consume();
                    }
                }
                WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                    let mouse_x = cx.mouse().cursorx;
                    if let Some(handle) = self.handle_for_x(mouse_x, &bounds) {
                        // If a drag gesture is active from the preceding MouseDown,
                        // end it first before starting the reset gesture
                        if self.drag_handle.is_some() {
                            match self.drag_handle.unwrap() {
                                0 => self.low_base.end_set_parameter(cx),
                                1 => self.mid_base.end_set_parameter(cx),
                                2 => self.high_base.end_set_parameter(cx),
                                _ => {}
                            }
                            self.drag_handle = None;
                        }

                        // Split gesture: begin + set on DoubleClick, end on MouseUp.
                        // This is necessary because some DAWs (Cubase) ignore perform_edit
                        // when begin_edit/perform_edit/end_edit happen in one callback.
                        let base = match handle {
                            0 => &self.low_base,
                            1 => &self.mid_base,
                            2 => &self.high_base,
                            _ => return,
                        };
                        base.begin_set_parameter(cx);
                        base.set_normalized_value(cx, base.default_normalized_value());
                        self.reset_handle = Some(handle);
                        cx.capture();
                        meta.consume();
                    }
                }
                _ => {}
            }
        });
    }
}
