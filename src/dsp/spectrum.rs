/// Spectrum analyzer buffer for lock-free audio→GUI FFT data sharing.
///
/// Triple-buffer pattern: audio thread writes to one buffer, GUI reads another.
/// In-crate radix-2 Cooley-Tukey FFT (4096-point, f32).
/// No heap allocations on the audio thread path.

use std::sync::atomic::{AtomicUsize, Ordering};

/// FFT size (must be power of 2)
pub const FFT_SIZE: usize = 8192;
/// Number of magnitude bins the GUI reads
pub const NUM_BINS: usize = FFT_SIZE / 2;

/// Lock-free ring buffer for passing audio samples from audio→GUI thread.
/// Audio thread writes into current write slot; GUI thread reads from a
/// different slot. Triple-buffer ensures no contention.
pub struct SpectrumBuffer {
    /// Three sample buffers for triple-buffering
    buffers: [Box<[f32; FFT_SIZE]>; 3],
    /// Write position within current write buffer
    write_pos: AtomicUsize,
    /// Index of the buffer currently being written to (0, 1, or 2)
    write_idx: AtomicUsize,
    /// Index of the most recently completed buffer (for GUI to read)
    ready_idx: AtomicUsize,
}

// SAFETY: The atomic operations ensure safe cross-thread access
unsafe impl Send for SpectrumBuffer {}
unsafe impl Sync for SpectrumBuffer {}

impl SpectrumBuffer {
    pub fn new() -> Self {
        // Use Vec to avoid placing large arrays on the stack
        fn make_buffer() -> Box<[f32; FFT_SIZE]> {
            let v = vec![0.0f32; FFT_SIZE];
            let boxed_slice = v.into_boxed_slice();
            unsafe {
                let ptr = Box::into_raw(boxed_slice) as *mut [f32; FFT_SIZE];
                Box::from_raw(ptr)
            }
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

        // SAFETY: we only write within bounds, and wi is always 0..2
        let buf = unsafe {
            let ptr = self.buffers[wi].as_ptr() as *mut f32;
            &mut *ptr.add(pos)
        };
        *buf = sample;

        let next_pos = pos + 1;
        if next_pos >= FFT_SIZE {
            // Buffer full — swap
            self.write_pos.store(0, Ordering::Relaxed);
            // Mark current as ready
            self.ready_idx.store(wi, Ordering::Release);
            // Move to next write buffer (skip the ready one)
            // No iterator — plain arithmetic to avoid any potential allocation
            let ri = self.ready_idx.load(Ordering::Relaxed);
            let next_wi = match (wi, ri) {
                (0, 1) | (1, 0) => 2,
                (0, 2) | (2, 0) => 1,
                (1, 2) | (2, 1) => 0,
                _ => (wi + 1) % 3,
            };
            self.write_idx.store(next_wi, Ordering::Relaxed);
        } else {
            self.write_pos.store(next_pos, Ordering::Relaxed);
        }
    }

    /// Read the most recently completed buffer (called from GUI thread).
    /// Returns a reference to FFT_SIZE samples.
    pub fn read(&self) -> &[f32; FFT_SIZE] {
        let ri = self.ready_idx.load(Ordering::Acquire);
        &self.buffers[ri]
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
    // Convert Vec to Box<[f32; FFT_SIZE]> without going through stack
    let boxed_slice = w.into_boxed_slice();
    // SAFETY: Vec was exactly FFT_SIZE elements
    unsafe {
        let ptr = Box::into_raw(boxed_slice) as *mut [f32; FFT_SIZE];
        Box::from_raw(ptr)
    }
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
