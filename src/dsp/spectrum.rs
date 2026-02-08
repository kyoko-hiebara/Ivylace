/// Spectrum analyzer buffer for lock-free audio→GUI FFT data sharing.
///
/// Triple-buffer pattern: audio thread writes to one buffer, GUI reads another.
/// In-crate radix-2 Cooley-Tukey FFT (4096-point, f32).
/// No heap allocations on the audio thread path.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// FFT size (must be power of 2)
pub const FFT_SIZE: usize = 8192;
/// Number of magnitude bins the GUI reads
pub const NUM_BINS: usize = FFT_SIZE / 2;

/// Lock-free ring buffer for passing audio samples from audio→GUI thread.
/// Audio thread writes into current write slot; GUI thread reads from a
/// different slot. Triple-buffer ensures no contention.
///
/// Uses `UnsafeCell` for interior mutability to satisfy Rust aliasing rules.
/// The audio thread only writes to the current `write_idx` buffer,
/// and the GUI thread only reads from the `ready_idx` buffer.
/// The triple-buffer pattern with atomic swaps ensures these are always
/// different buffers, so no data races occur.
pub struct SpectrumBuffer {
    /// Three sample buffers for triple-buffering, wrapped in UnsafeCell
    /// for interior mutability (audio thread writes through &self).
    buffers: [UnsafeCell<Box<[f32; FFT_SIZE]>>; 3],
    /// Write position within current write buffer
    write_pos: AtomicUsize,
    /// Index of the buffer currently being written to (0, 1, or 2)
    write_idx: AtomicUsize,
    /// Index of the most recently completed buffer (for GUI to read)
    ready_idx: AtomicUsize,
}

// SAFETY: The triple-buffer protocol with atomic operations ensures that
// the audio thread (writer) and GUI thread (reader) never access the same
// buffer simultaneously. write_idx and ready_idx are always distinct,
// and buffer swaps are atomic.
unsafe impl Send for SpectrumBuffer {}
unsafe impl Sync for SpectrumBuffer {}

impl SpectrumBuffer {
    pub fn new() -> Self {
        fn make_buffer() -> UnsafeCell<Box<[f32; FFT_SIZE]>> {
            let v: Vec<f32> = vec![0.0f32; FFT_SIZE];
            // SAFETY: Vec is exactly FFT_SIZE elements, matching the array size.
            let boxed_array: Box<[f32; FFT_SIZE]> = v.into_boxed_slice()
                .try_into()
                .expect("Vec length matches FFT_SIZE");
            UnsafeCell::new(boxed_array)
        }
        Self {
            buffers: [make_buffer(), make_buffer(), make_buffer()],
            write_pos: AtomicUsize::new(0),
            write_idx: AtomicUsize::new(0),
            ready_idx: AtomicUsize::new(1),
        }
    }

    /// Push a single sample (called from audio thread).
    /// When the buffer is full, it swaps to the next write buffer.
    #[inline(always)]
    pub fn push(&self, sample: f32) {
        let wi = self.write_idx.load(Ordering::Relaxed);
        let pos = self.write_pos.load(Ordering::Relaxed);

        // SAFETY: Only the audio thread writes to buffers[wi], and wi != ready_idx
        // (the GUI thread only reads from buffers[ready_idx]).
        // UnsafeCell provides the interior mutability contract.
        unsafe {
            let buf = &mut *self.buffers[wi].get();
            buf[pos] = sample;
        }

        let next_pos = pos + 1;
        if next_pos >= FFT_SIZE {
            // Buffer full — swap
            self.write_pos.store(0, Ordering::Relaxed);
            // Mark current as ready
            self.ready_idx.store(wi, Ordering::Release);
            // Move to next write buffer.
            // Simple round-robin: advance to next buffer, skipping the one we just
            // marked as ready (= wi). Since (wi + 1) % 3 != wi, this always works.
            let next_wi = (wi + 1) % 3;
            self.write_idx.store(next_wi, Ordering::Relaxed);
        } else {
            self.write_pos.store(next_pos, Ordering::Relaxed);
        }
    }

    /// Read the most recently completed buffer (called from GUI thread).
    /// Returns a reference to FFT_SIZE samples.
    pub fn read(&self) -> &[f32; FFT_SIZE] {
        let ri = self.ready_idx.load(Ordering::Acquire);
        // SAFETY: The GUI thread only reads from buffers[ri], and ri != write_idx
        // (the audio thread only writes to buffers[write_idx]).
        unsafe { &**self.buffers[ri].get() }
    }
}

// ── In-crate FFT ────────────────────────────────────────────

/// Hann window coefficients (precomputed, heap-allocated to avoid large stack frames)
pub fn hann_window() -> Box<[f32; FFT_SIZE]> {
    let mut w = vec![0.0f32; FFT_SIZE];
    let n = FFT_SIZE as f32;
    for i in 0..FFT_SIZE {
        w[i] = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / n).cos());
    }
    w.into_boxed_slice()
        .try_into()
        .expect("Vec length matches FFT_SIZE")
}

/// Radix-2 Cooley-Tukey FFT (in-place, decimation-in-time).
/// `real` and `imag` must be of length FFT_SIZE (power of 2).
pub fn fft_in_place(real: &mut [f32], imag: &mut [f32]) {
    let n = real.len();
    assert!(n.is_power_of_two());
    assert_eq!(n, imag.len());

    // Bit-reversal permutation
    let mut j = 0usize;
    for i in 0..n {
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
        let mut m = n >> 1;
        while m >= 1 && j >= m {
            j -= m;
            m >>= 1;
        }
        j += m;
    }

    // Butterfly stages
    let mut size = 2;
    while size <= n {
        let half = size / 2;
        let angle_step = -2.0 * std::f32::consts::PI / size as f32;

        for k in (0..n).step_by(size) {
            for j in 0..half {
                let angle = angle_step * j as f32;
                let wr = angle.cos();
                let wi = angle.sin();

                let idx1 = k + j;
                let idx2 = k + j + half;

                let tr = wr * real[idx2] - wi * imag[idx2];
                let ti = wr * imag[idx2] + wi * real[idx2];

                real[idx2] = real[idx1] - tr;
                imag[idx2] = imag[idx1] - ti;
                real[idx1] += tr;
                imag[idx1] += ti;
            }
        }
        size <<= 1;
    }
}

/// Compute magnitude spectrum in dB from time-domain samples.
/// Applies Hann window → FFT → magnitude (dB).
/// Output: `magnitudes` must be at least NUM_BINS long.
pub fn compute_spectrum_db(samples: &[f32; FFT_SIZE], magnitudes: &mut [f32; NUM_BINS], window: &[f32; FFT_SIZE]) {
    let mut real = [0.0f32; FFT_SIZE];
    let mut imag = [0.0f32; FFT_SIZE];

    // Apply window
    for i in 0..FFT_SIZE {
        real[i] = samples[i] * window[i];
        imag[i] = 0.0;
    }

    fft_in_place(&mut real, &mut imag);

    // Compute magnitude in dB
    let norm = 2.0 / FFT_SIZE as f32;
    for i in 0..NUM_BINS {
        let re = real[i] * norm;
        let im = imag[i] * norm;
        let mag = (re * re + im * im).sqrt();
        // Convert to dB, floor at -120 dB
        magnitudes[i] = if mag > 1e-12 {
            20.0 * mag.log10()
        } else {
            -120.0
        };
    }
}
