/// Spectrum analyzer widget.
/// Reads from SpectrumBuffer, computes FFT, renders log-frequency magnitude plot.
/// Overlaid below the crossover display with matching frequency axis.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::cell::RefCell;

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::theme::{self, ThemeMode};
use crate::dsp::spectrum::{self, FFT_SIZE, NUM_BINS, SpectrumBuffer};

const FREQ_MIN: f32 = 20.0;
const FREQ_MAX: f32 = 20000.0;
const DB_MIN: f32 = -90.0;
const DB_MAX: f32 = 0.0;
const DISPLAY_HEIGHT: f32 = 80.0;
/// Smoothing factor (0..1). Higher = more smoothing.
const SMOOTHING: f32 = 0.8;

#[inline]
fn freq_to_x(freq: f32, width: f32) -> f32 {
    let log_min = FREQ_MIN.log10();
    let log_max = FREQ_MAX.log10();
    ((freq.log10() - log_min) / (log_max - log_min)) * width
}

/// Heap-allocated FFT work buffers to avoid stack overflow on GUI thread.
/// DAW GUI threads may have small stacks (32-64 KB).
struct FftWorkBuffers {
    real: Box<[f32; FFT_SIZE]>,
    imag: Box<[f32; FFT_SIZE]>,
    magnitudes: Box<[f32; NUM_BINS]>,
    smoothed: Box<[f32; NUM_BINS]>,
}

impl FftWorkBuffers {
    fn new() -> Self {
        Self {
            real: Box::new([0.0f32; FFT_SIZE]),
            imag: Box::new([0.0f32; FFT_SIZE]),
            magnitudes: Box::new([0.0f32; NUM_BINS]),
            smoothed: Box::new([-120.0f32; NUM_BINS]),
        }
    }
}

/// Spectrum analyzer view.
pub struct SpectrumAnalyzer {
    spectrum_buf: Arc<SpectrumBuffer>,
    sample_rate: f32,
    glass_mode: Arc<AtomicBool>,
    /// Hann window (precomputed, heap-allocated)
    window: Box<[f32; FFT_SIZE]>,
    /// FFT work buffers (heap-allocated to avoid stack overflow)
    work: RefCell<FftWorkBuffers>,
}

impl SpectrumAnalyzer {
    pub fn new(
        cx: &mut Context,
        spectrum_buf: Arc<SpectrumBuffer>,
        sample_rate: f32,
        glass_mode: Arc<AtomicBool>,
    ) -> Handle<'_, Self> {
        let window = spectrum::hann_window();

        Self {
            spectrum_buf,
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

        // Read samples and compute FFT using heap-allocated work buffers
        let samples = self.spectrum_buf.read();
        let mut work = self.work.borrow_mut();

        // Apply window and prepare FFT input
        for i in 0..FFT_SIZE {
            work.real[i] = samples[i] * self.window[i];
            work.imag[i] = 0.0;
        }

        // Split borrow: get raw pointers to avoid double-mutable-borrow issue
        let work_ref = &mut *work;
        spectrum::fft_in_place(&mut *work_ref.real, &mut *work_ref.imag);

        // Compute magnitude in dB
        let norm = 2.0 / FFT_SIZE as f32;
        for i in 0..NUM_BINS {
            let re = work.real[i] * norm;
            let im = work.imag[i] * norm;
            let mag = (re * re + im * im).sqrt();
            work.magnitudes[i] = if mag > 1e-12 {
                20.0 * mag.log10()
            } else {
                -120.0
            };
        }

        // Apply smoothing
        for i in 0..NUM_BINS {
            work.smoothed[i] = SMOOTHING * work.smoothed[i] + (1.0 - SMOOTHING) * work.magnitudes[i];
        }

        // ── Draw spectrum path ──
        let db_range = DB_MAX - DB_MIN;
        let sr = self.sample_rate;
        let bin_freq = |bin: usize| -> f32 { bin as f32 * sr / FFT_SIZE as f32 };

        // Build the filled path
        let mut fill_path = vg::Path::new();
        let mut stroke_path = vg::Path::new();
        let mut started = false;

        for bin in 1..NUM_BINS {
            let freq = bin_freq(bin);
            if freq < FREQ_MIN || freq > FREQ_MAX {
                continue;
            }

            let x = bx + freq_to_x(freq, bw);
            let db = work.smoothed[bin].clamp(DB_MIN, DB_MAX);
            let y = by + bh * (1.0 - (db - DB_MIN) / db_range);

            if !started {
                fill_path.move_to(x, by + bh);
                fill_path.line_to(x, y);
                stroke_path.move_to(x, y);
                started = true;
            } else {
                fill_path.line_to(x, y);
                stroke_path.line_to(x, y);
            }
        }

        // Need to drop borrow before any potential early return
        drop(work);

        if started {
            // Close fill path
            let last_freq = bin_freq(NUM_BINS - 1).min(FREQ_MAX);
            let last_x = bx + freq_to_x(last_freq, bw);
            fill_path.line_to(last_x, by + bh);
            fill_path.close();

            // Fill with gradient
            let fill_paint = vg::Paint::linear_gradient(
                bx, by,
                bx, by + bh,
                theme::spectrum_fill_top(mode),
                theme::spectrum_fill_bottom(mode),
            );
            canvas.fill_path(&fill_path, &fill_paint);

            // Stroke line
            let mut stroke_paint = vg::Paint::color(theme::spectrum_stroke(mode));
            stroke_paint.set_line_width(1.5 * dpi);
            canvas.stroke_path(&stroke_path, &stroke_paint);
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
