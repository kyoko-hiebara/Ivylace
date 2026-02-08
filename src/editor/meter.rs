/// Gain Reduction meter widgets:
/// - GrMeterWidget: Vertical segmented digital-style bar meter
/// - AnalogGrMeter: Analog needle-style semicircular gauge
/// Reads from Arc<GrMeterOutputs> via lenses for lock-free display.
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::theme::{self, ThemeMode};
use super::Data;

fn mode_lens() -> impl Lens<Target = ThemeMode> {
    Data::params.map(|p| {
        if p.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark }
    })
}

const METER_WIDTH: f32 = 18.0;
const METER_HEIGHT: f32 = 60.0;
const NUM_SEGMENTS: usize = 12;
const MIN_DB: f32 = -6.0;
const MAX_DB: f32 = 0.0;
const SCALE_MARKS: [f32; 4] = [0.0, -2.0, -4.0, -6.0];

/// A simple vertical gain reduction meter
pub struct GrMeterWidget<L: Lens<Target = f32>, P: Lens<Target = f32>> {
    level_lens: L,
    peak_lens: P,
    glass_mode: Arc<AtomicBool>,
    /// Held peak value for the display
    held_peak: Cell<f32>,
    held_peak_time: Cell<Option<Instant>>,
}

impl<L: Lens<Target = f32>, P: Lens<Target = f32>> GrMeterWidget<L, P> {
    pub fn new(cx: &mut Context, level_lens: L, peak_lens: P, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self>
    where
        L: Clone + 'static,
        P: Clone + 'static,
    {
        let gr_text_lens = level_lens.clone().map(|&gr_db| {
            if gr_db < -0.1 {
                format!("{:.1}", gr_db)
            } else {
                "0.0".to_string()
            }
        });

        Self {
            level_lens,
            peak_lens,
            glass_mode,
            held_peak: Cell::new(0.0),
            held_peak_time: Cell::new(None),
        }
        .build(cx, move |cx| {
            // VStack: meter bar → value readout → "GR" label
            VStack::new(cx, |cx| {
                // Spacer for the meter bar area (drawn by View::draw)
                Element::new(cx)
                    .width(Pixels(METER_WIDTH))
                    .height(Pixels(METER_HEIGHT))
                    .hoverable(false);

                // Value readout (e.g., "-3.2" or "0.0")
                Label::new(cx, gr_text_lens)
                    .font_size(9.0)
                    .color(mode_lens().map(|m| theme::to_vizia(theme::meter_readout_text(*m))))
                    .font_family(vec![FamilyOwned::Name(String::from(nih_plug_vizia::assets::NOTO_SANS))])
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .height(Pixels(12.0))
                    .hoverable(false);

                // "GR" label
                Label::new(cx, "GR")
                    .font_size(8.0)
                    .font_weight(FontWeightKeyword::SemiBold)
                    .color(mode_lens().map(|m| theme::to_vizia(theme::meter_gr_label(*m))))
                    .font_family(vec![FamilyOwned::Name(String::from(nih_plug_vizia::assets::NOTO_SANS))])
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .height(Pixels(11.0))
                    .hoverable(false);
            })
            .row_between(Pixels(1.0))
            .width(Pixels(METER_WIDTH))
            .height(Auto);
        })
        .width(Pixels(METER_WIDTH))
        .height(Auto)
    }
}

impl<L: Lens<Target = f32>, P: Lens<Target = f32>> View for GrMeterWidget<L, P> {
    fn element(&self) -> Option<&'static str> {
        Some("gr-meter")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let _opacity = cx.opacity();

        let gr_db = self.level_lens.get(cx);
        let peak_db = self.peak_lens.get(cx);

        // Update held peak with hold time
        let now = Instant::now();
        let current_held = self.held_peak.get();
        if peak_db < current_held || self.held_peak_time.get().is_none() {
            self.held_peak.set(peak_db);
            self.held_peak_time.set(Some(now));
        } else if let Some(t) = self.held_peak_time.get() {
            if now.duration_since(t).as_millis() > 1000 {
                // Decay toward 0
                let decayed = current_held * 0.95;
                if decayed > -0.1 {
                    self.held_peak.set(0.0);
                    self.held_peak_time.set(None);
                } else {
                    self.held_peak.set(decayed);
                }
            }
        }

        // Meter area
        let mx = bounds.x;
        let my = bounds.y;
        let mw = METER_WIDTH * dpi;
        let mh = METER_HEIGHT * dpi;

        // ── Background ──
        {
            let mut path = vg::Path::new();
            path.rounded_rect(mx, my, mw, mh, 4.0 * dpi);
            canvas.fill_path(&path, &vg::Paint::color(theme::meter_bg(mode)));

            // Inset shadow
            let shadow = vg::Paint::box_gradient(
                mx, my + 1.0 * dpi, mw, mh, 4.0 * dpi, 4.0 * dpi,
                theme::meter_shadow(mode),
                vg::Color::rgba(0, 0, 0, 0),
            );
            canvas.fill_path(&path, &shadow);

            // Border
            let mut border_paint = vg::Paint::color(theme::meter_border(mode));
            border_paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border_paint);
        }

        // ── Segment lines ──
        {
            let seg_h = mh / NUM_SEGMENTS as f32;
            for i in 1..NUM_SEGMENTS {
                let sy = my + i as f32 * seg_h;
                let mut path = vg::Path::new();
                path.move_to(mx + 1.0, sy);
                path.line_to(mx + mw - 1.0, sy);
                let mut paint = vg::Paint::color(theme::meter_seg_line(mode));
                paint.set_line_width(1.0 * dpi);
                canvas.stroke_path(&path, &paint);
            }
        }

        // ── Meter fill ──
        let range = MAX_DB - MIN_DB; // 40
        let clamped_gr = gr_db.clamp(MIN_DB, MAX_DB);
        let fill_pct = (clamped_gr - MAX_DB).abs() / range; // 0 at 0dB, 1 at -40dB
        let fill_h = fill_pct * mh;

        if fill_h > 1.0 {
            let mut path = vg::Path::new();
            // Fill from top (0 dB side)
            path.rect(mx + 1.0, my + 1.0, mw - 2.0, fill_h.min(mh - 2.0));

            // Gradient: blue at low GR, red when approaching max range
            let paint = if fill_pct > 0.85 {
                vg::Paint::linear_gradient(
                    mx, my,
                    mx, my + fill_h,
                    theme::meter_fill_red(mode),
                    theme::meter_fill_blue(mode),
                )
            } else {
                vg::Paint::linear_gradient(
                    mx, my,
                    mx, my + fill_h,
                    theme::meter_fill_blue(mode),
                    theme::meter_fill_blue_light(mode),
                )
            };
            canvas.fill_path(&path, &paint);
        }

        // ── Peak indicator ──
        let held = self.held_peak.get();
        if held < -0.1 {
            let peak_pct = (held - MAX_DB).abs() / range;
            let peak_y = my + peak_pct * mh;
            if peak_y > my && peak_y < my + mh {
                let mut path = vg::Path::new();
                path.move_to(mx + 1.0, peak_y);
                path.line_to(mx + mw - 1.0, peak_y);
                let mut paint = vg::Paint::color(theme::meter_peak(mode));
                paint.set_line_width(2.0 * dpi);
                canvas.stroke_path(&path, &paint);
            }
        }

        // ── Scale markings ──
        {
            for &db in &SCALE_MARKS {
                let pos_pct = (db - MAX_DB).abs() / range;
                let mark_y = my + pos_pct * mh;
                let mut path = vg::Path::new();
                path.move_to(mx + mw - 4.0 * dpi, mark_y);
                path.line_to(mx + mw - 1.0, mark_y);
                let mut paint = vg::Paint::color(theme::meter_scale(mode));
                paint.set_line_width(1.0 * dpi);
                canvas.stroke_path(&path, &paint);
            }
        }

        // GR label + value readout are rendered as VIZIA child Labels (see build())
    }
}

// ── Analog Needle GR Meter ──────────────────────────────────

const ANALOG_WIDTH: f32 = 100.0;
const ANALOG_HEIGHT: f32 = 60.0;

/// Analog needle-style semicircular GR gauge.
/// Needle sweeps from right (0dB) to left (-6dB) across a 180° arc.
pub struct AnalogGrMeter<L: Lens<Target = f32>> {
    level_lens: L,
    glass_mode: Arc<AtomicBool>,
    font_id: Cell<Option<vg::FontId>>,
}

impl<L: Lens<Target = f32>> AnalogGrMeter<L> {
    pub fn new(cx: &mut Context, level_lens: L, glass_mode: Arc<AtomicBool>) -> Handle<'_, Self>
    where
        L: Clone + 'static,
    {
        Self {
            level_lens,
            glass_mode,
            font_id: Cell::new(None),
        }
        .build(cx, |_| {})
        .width(Pixels(ANALOG_WIDTH))
        .height(Pixels(ANALOG_HEIGHT))
    }

    #[inline]
    fn ensure_font(&self, canvas: &mut Canvas) -> vg::FontId {
        if let Some(fid) = self.font_id.get() {
            fid
        } else {
            let fid = canvas.add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR)
                .expect("failed to load font");
            self.font_id.set(Some(fid));
            fid
        }
    }
}

impl<L: Lens<Target = f32>> View for AnalogGrMeter<L> {
    fn element(&self) -> Option<&'static str> {
        Some("analog-gr-meter")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let font = self.ensure_font(canvas);

        let gr_db = self.level_lens.get(cx);

        // Gauge geometry: semicircle, pivot at bottom center
        let cx_pos = bounds.x + bounds.w * 0.5;          // pivot X
        let cy_pos = bounds.y + bounds.h - 4.0 * dpi;    // pivot Y (near bottom)
        let radius = (bounds.w * 0.5 - 4.0 * dpi).min(bounds.h - 8.0 * dpi);

        // Angle mapping: π (left, -6dB) → 0 (right, 0dB)
        // 0dB = right end (angle 0), -6dB = left end (angle π)
        let db_to_angle = |db: f32| -> f32 {
            let clamped = db.clamp(MIN_DB, MAX_DB);
            let t = (clamped - MAX_DB) / (MIN_DB - MAX_DB); // 0 at 0dB, 1 at -6dB
            std::f32::consts::PI * t
        };

        // ── Background arc ──
        {
            let mut path = vg::Path::new();
            path.arc(cx_pos, cy_pos, radius, std::f32::consts::PI, 0.0, vg::Solidity::Hole);
            let mut paint = vg::Paint::color(theme::analog_meter_arc(mode));
            paint.set_line_width(2.5 * dpi);
            canvas.stroke_path(&path, &paint);
        }

        // ── Tick marks ──
        let tick_dbs: [(f32, &str); 4] = [(0.0, "0"), (-2.0, "2"), (-4.0, "4"), (-6.0, "6")];
        for &(db, label) in &tick_dbs {
            let angle = db_to_angle(db);
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            // Outer tick
            let inner_r = radius - 4.0 * dpi;
            let outer_r = radius + 2.0 * dpi;
            let x0 = cx_pos - cos_a * inner_r;
            let y0 = cy_pos - sin_a * inner_r;
            let x1 = cx_pos - cos_a * outer_r;
            let y1 = cy_pos - sin_a * outer_r;

            let mut path = vg::Path::new();
            path.move_to(x0, y0);
            path.line_to(x1, y1);
            let mut paint = vg::Paint::color(theme::analog_meter_tick(mode));
            paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &paint);

            // Label text
            let label_r = radius + 8.0 * dpi;
            let lx = cx_pos - cos_a * label_r;
            let ly = cy_pos - sin_a * label_r;

            let mut text_paint = vg::Paint::color(theme::analog_meter_tick_label(mode));
            text_paint.set_font_size(9.5 * dpi);
            text_paint.set_font(&[font]);
            text_paint.set_text_align(vg::Align::Center);
            text_paint.set_text_baseline(vg::Baseline::Middle);
            let _ = canvas.fill_text(lx, ly, label, &text_paint);
        }

        // ── Needle ──
        {
            let angle = db_to_angle(gr_db);
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            let needle_len = radius - 2.0 * dpi;
            let nx = cx_pos - cos_a * needle_len;
            let ny = cy_pos - sin_a * needle_len;

            // Color: blue normally, blending toward red for heavy compression
            let intensity = (gr_db.clamp(MIN_DB, MAX_DB) - MAX_DB).abs() / (MAX_DB - MIN_DB).abs();
            let needle_color = if intensity > 0.75 {
                theme::meter_fill_red(mode)
            } else {
                theme::meter_fill_blue(mode)
            };

            let mut path = vg::Path::new();
            path.move_to(cx_pos, cy_pos);
            path.line_to(nx, ny);
            let mut paint = vg::Paint::color(needle_color);
            paint.set_line_width(1.5 * dpi);
            paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&path, &paint);
        }

        // ── Pivot circle ──
        {
            let mut path = vg::Path::new();
            path.circle(cx_pos, cy_pos, 2.5 * dpi);
            canvas.fill_path(&path, &vg::Paint::color(theme::analog_meter_pivot(mode)));
        }
    }
}
