/// Spectrum analyzer widget — delta display mode.
/// Shows the difference between pre- and post-processing spectra.
/// Gain reduction (cut) is drawn below the 0dB center line,
/// makeup/boost is drawn above.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::cell::RefCell;

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::theme::{self, ThemeMode};
use crate::dsp::spectrum::{self, FFT_SIZE, NUM_BINS, SpectrumBuffer};

const FREQ_MIN: f32 = 30.0;
const FREQ_MAX: f32 = 20000.0;
const DISPLAY_HEIGHT: f32 = 80.0;
/// Delta dB range (symmetrical around 0)
const DELTA_DB_MAX: f32 = 6.0;
/// Smoothing factor for attack (0..1). Lower = faster response to new peaks.
const SMOOTH_ATTACK: f32 = 0.6;
/// Smoothing factor for release (0..1). Higher = slower decay.
const SMOOTH_RELEASE: f32 = 0.94;

#[inline]
fn freq_to_x(freq: f32, width: f32) -> f32 {
    let log_min = FREQ_MIN.log10();
    let log_max = FREQ_MAX.log10();
    ((freq.log10() - log_min) / (log_max - log_min)) * width
}

#[inline]
fn x_to_freq(x: f32, width: f32) -> f32 {
    let log_min = FREQ_MIN.log10();
    let log_max = FREQ_MAX.log10();
    10.0_f32.powf(log_min + (x / width) * (log_max - log_min))
}

/// How many draw() calls to skip between FFT recomputes.
/// At 60fps, skip_count=2 means FFT runs at ~20fps — still smooth but 3x less CPU.
const FFT_SKIP_FRAMES: u32 = 2;

/// Heap-allocated FFT work buffers to avoid stack overflow on GUI thread.
struct FftWorkBuffers {
    real: Box<[f32; FFT_SIZE]>,
    imag: Box<[f32; FFT_SIZE]>,
    pre_mag: Box<[f32; NUM_BINS]>,
    post_mag: Box<[f32; NUM_BINS]>,
    smoothed_delta: Box<[f32; NUM_BINS]>,
    /// Frame counter for FFT skip logic
    frame_counter: u32,
}

impl FftWorkBuffers {
    fn new() -> Self {
        Self {
            real: Box::new([0.0f32; FFT_SIZE]),
            imag: Box::new([0.0f32; FFT_SIZE]),
            pre_mag: Box::new([-120.0f32; NUM_BINS]),
            post_mag: Box::new([-120.0f32; NUM_BINS]),
            smoothed_delta: Box::new([0.0f32; NUM_BINS]),
            frame_counter: 0,
        }
    }
}

/// Spectrum analyzer view — delta (difference) display.
pub struct SpectrumAnalyzer {
    spectrum_pre: Arc<SpectrumBuffer>,
    spectrum_post: Arc<SpectrumBuffer>,
    sample_rate: f32,
    glass_mode: Arc<AtomicBool>,
    /// Hann window (precomputed, heap-allocated)
    window: Box<[f32; FFT_SIZE]>,
    /// FFT work buffers (heap-allocated)
    work: RefCell<FftWorkBuffers>,
}

impl SpectrumAnalyzer {
    pub fn new(
        cx: &mut Context,
        spectrum_pre: Arc<SpectrumBuffer>,
        spectrum_post: Arc<SpectrumBuffer>,
        sample_rate: f32,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        let window = spectrum::hann_window();

        Self {
            spectrum_pre,
            spectrum_post,
            sample_rate,
            glass_mode,
            window,
            work: RefCell::new(FftWorkBuffers::new()),
        }
        .build(cx, |_| {})
        .height(Pixels(DISPLAY_HEIGHT))
        .width(Stretch(1.0))
    }

}

impl View for SpectrumAnalyzer {
    fn element(&self) -> Option<&'static str> {
        Some("spectrum-analyzer")
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

        // ── Background ──
        {
            let mut path = vg::Path::new();
            path.rect(bx, by, bw, bh);
            canvas.fill_path(&path, &vg::Paint::color(theme::spectrum_bg(mode)));
        }

        // ── Compute FFTs (throttled to reduce GUI CPU) ──
        let mut work = self.work.borrow_mut();

        // Only recompute FFT every FFT_SKIP_FRAMES frames.
        // Smoothed delta is always updated (using cached magnitudes) for smooth animation.
        let should_recompute = work.frame_counter == 0;
        work.frame_counter = (work.frame_counter + 1) % (FFT_SKIP_FRAMES + 1);

        if should_recompute {
            // Pre-processing spectrum
            {
                let samples = self.spectrum_pre.read();
                for i in 0..FFT_SIZE {
                    work.real[i] = samples[i] * self.window[i];
                    work.imag[i] = 0.0;
                }
                let w = &mut *work;
                spectrum::fft_in_place(&mut *w.real, &mut *w.imag);
                let norm = 2.0 / FFT_SIZE as f32;
                for i in 0..NUM_BINS {
                    let re = w.real[i] * norm;
                    let im = w.imag[i] * norm;
                    let mag = (re * re + im * im).sqrt();
                    w.pre_mag[i] = if mag > 1e-12 { 20.0 * mag.log10() } else { -120.0 };
                }
            }

            // Post-processing spectrum
            {
                let samples = self.spectrum_post.read();
                for i in 0..FFT_SIZE {
                    work.real[i] = samples[i] * self.window[i];
                    work.imag[i] = 0.0;
                }
                let w = &mut *work;
                spectrum::fft_in_place(&mut *w.real, &mut *w.imag);
                let norm = 2.0 / FFT_SIZE as f32;
                for i in 0..NUM_BINS {
                    let re = w.real[i] * norm;
                    let im = w.imag[i] * norm;
                    let mag = (re * re + im * im).sqrt();
                    w.post_mag[i] = if mag > 1e-12 { 20.0 * mag.log10() } else { -120.0 };
                }
            }
        }

        // ── Compute smoothed delta (asymmetric attack/release, always runs) ──
        // Noise gate: ignore delta when both pre and post are below this threshold.
        // This prevents phantom bumps at crossover boundaries during silence.
        const NOISE_FLOOR_DB: f32 = -90.0;
        for i in 0..NUM_BINS {
            let delta = if work.pre_mag[i] < NOISE_FLOOR_DB && work.post_mag[i] < NOISE_FLOOR_DB {
                0.0
            } else {
                work.post_mag[i] - work.pre_mag[i]
            };
            let clamped = delta.clamp(-DELTA_DB_MAX, DELTA_DB_MAX);
            let prev = work.smoothed_delta[i];
            // Use faster attack when magnitude is increasing, slower release when decaying
            let coeff = if clamped.abs() > prev.abs() { SMOOTH_ATTACK } else { SMOOTH_RELEASE };
            work.smoothed_delta[i] = coeff * prev + (1.0 - coeff) * clamped;
        }

        // ── 0dB center line ──
        let zero_y = by + bh * 0.5;
        {
            let mut path = vg::Path::new();
            path.move_to(bx, zero_y);
            path.line_to(bx + bw, zero_y);
            let mut paint = vg::Paint::color(theme::spectrum_zero_line(mode));
            paint.set_line_width(1.0);
            canvas.stroke_path(&path, &paint);
        }

        // ── Build delta path (pixel-based with log-frequency interpolation) ──
        let sr = self.sample_rate;
        let bin_hz = sr / FFT_SIZE as f32; // Hz per bin

        // Separate fill + stroke paths for cut (blue) and boost (orange).
        let mut cut_fill = vg::Path::new();
        let mut cut_stroke = vg::Path::new();
        let mut boost_fill = vg::Path::new();
        let mut boost_stroke = vg::Path::new();

        // Helper: delta dB to Y position (0dB = center, +DELTA_DB_MAX = top, -DELTA_DB_MAX = bottom)
        let delta_to_y = |db: f32| -> f32 {
            let normalized = (db / DELTA_DB_MAX).clamp(-1.0, 1.0);
            zero_y - normalized * (bh * 0.5)
        };

        // Helper: frequency-proportional smoothed lookup.
        // At low frequencies, average over a wider kernel (±1/3 octave)
        // to smooth out the sparse FFT bins. At high frequencies the kernel
        // shrinks to ~1 bin so detail is preserved.
        let interp_delta = |freq: f32| -> f32 {
            let center_bin = freq / bin_hz;
            // Kernel radius: 1/3 octave worth of bins (minimum 1 bin)
            let oct_third = freq * 0.26 / bin_hz; // 2^(1/3)-1 ≈ 0.26
            let radius = oct_third.max(1.0);
            let lo = ((center_bin - radius).floor() as usize).max(1);
            let hi = ((center_bin + radius).ceil() as usize).min(NUM_BINS - 1);
            let mut sum = 0.0_f32;
            let mut weight_sum = 0.0_f32;
            for b in lo..=hi {
                let dist = (b as f32 - center_bin).abs();
                let w = 1.0 - (dist / (radius + 0.5)).min(1.0); // triangular window
                sum += work.smoothed_delta[b] * w;
                weight_sum += w;
            }
            if weight_sum > 0.0 { sum / weight_sum } else { 0.0 }
        };

        // Step every 2 pixels for smooth curves without excessive path points
        let step = 2.0_f32;
        let num_steps = (bw / step).ceil() as usize;
        let mut cut_started = false;
        let mut boost_started = false;
        let mut prev_x = bx;

        for i in 0..=num_steps {
            let px = (i as f32 * step).min(bw);
            let x = bx + px;
            let freq = x_to_freq(px, bw);

            // Skip if frequency is outside displayable FFT range
            if freq < FREQ_MIN || freq > sr * 0.5 {
                continue;
            }

            let delta = interp_delta(freq);
            let y = delta_to_y(delta);

            // Cut path (below zero line)
            if delta < -0.01 {
                if !cut_started {
                    cut_fill.move_to(x, zero_y);
                    cut_fill.line_to(x, y);
                    cut_stroke.move_to(x, y);
                    cut_started = true;
                } else {
                    cut_fill.line_to(x, y);
                    cut_stroke.line_to(x, y);
                }
            } else if cut_started {
                cut_fill.line_to(x, zero_y);
                cut_fill.close();
                cut_stroke.line_to(x, zero_y);
                cut_started = false;
            }

            // Boost path (above zero line)
            if delta > 0.01 {
                if !boost_started {
                    boost_fill.move_to(x, zero_y);
                    boost_fill.line_to(x, y);
                    boost_stroke.move_to(x, y);
                    boost_started = true;
                } else {
                    boost_fill.line_to(x, y);
                    boost_stroke.line_to(x, y);
                }
            } else if boost_started {
                boost_fill.line_to(x, zero_y);
                boost_fill.close();
                boost_stroke.line_to(x, zero_y);
                boost_started = false;
            }

            prev_x = x;
        }

        // Close any open fill paths at the right edge
        if cut_started {
            cut_fill.line_to(prev_x, zero_y);
            cut_fill.close();
        }
        if boost_started {
            boost_fill.line_to(prev_x, zero_y);
            boost_fill.close();
        }

        // Drop borrow before drawing
        drop(work);

        // ── Draw cut (blue) ──
        {
            let fill_paint = vg::Paint::linear_gradient(
                bx, zero_y,
                bx, by + bh,
                theme::spectrum_delta_cut_top(mode),
                theme::spectrum_delta_cut_bottom(mode),
            );
            canvas.fill_path(&cut_fill, &fill_paint);

            let mut stroke_paint = vg::Paint::color(theme::spectrum_delta_cut_stroke(mode));
            stroke_paint.set_line_width(1.5 * dpi);
            canvas.stroke_path(&cut_stroke, &stroke_paint);
        }

        // ── Draw boost (orange) ──
        {
            let fill_paint = vg::Paint::linear_gradient(
                bx, by,
                bx, zero_y,
                theme::spectrum_delta_boost_top(mode),
                theme::spectrum_delta_boost_bottom(mode),
            );
            canvas.fill_path(&boost_fill, &fill_paint);

            let mut stroke_paint = vg::Paint::color(theme::spectrum_delta_boost_stroke(mode));
            stroke_paint.set_line_width(1.5 * dpi);
            canvas.stroke_path(&boost_stroke, &stroke_paint);
        }

        // ── Top border ──
        {
            let mut path = vg::Path::new();
            path.move_to(bx, by);
            path.line_to(bx + bw, by);
            let mut paint = vg::Paint::color(theme::spectrum_top_border(mode));
            paint.set_line_width(1.0);
            canvas.stroke_path(&path, &paint);
        }
    }
}
