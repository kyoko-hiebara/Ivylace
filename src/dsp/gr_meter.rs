/// Gain Reduction Meter
///
/// Provides smoothed gain reduction readback for GUI/host display.
/// Uses peak-hold with exponential decay for readable metering.
///
/// No heap allocations — suitable for audio thread.

use std::sync::atomic::{AtomicU32, Ordering};

/// Smoothing time constants
const ATTACK_MS: f64 = 0.1;   // Fast attack to catch peaks
const RELEASE_MS: f64 = 300.0; // Slow release for readability
const PEAK_HOLD_MS: f64 = 500.0; // Peak hold time before decay

/// Per-band gain reduction meter
#[derive(Clone)]
pub struct GrMeter {
    /// Current smoothed GR value (dB, negative = compression)
    current_gr_db: f64,
    /// Peak GR value (dB, most negative = deepest compression)
    peak_gr_db: f64,
    /// Peak hold counter (samples)
    peak_hold_counter: u32,
    /// Peak hold duration (samples)
    peak_hold_samples: u32,
    /// Attack coefficient
    attack_coeff: f64,
    /// Release coefficient
    release_coeff: f64,
    /// Sample rate
    sample_rate: f64,
}

impl GrMeter {
    pub fn new(sample_rate: f64) -> Self {
        let mut meter = Self {
            current_gr_db: 0.0,
            peak_gr_db: 0.0,
            peak_hold_counter: 0,
            peak_hold_samples: 0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sample_rate,
        };
        meter.update_coefficients();
        meter
    }

    pub fn reset(&mut self) {
        self.current_gr_db = 0.0;
        self.peak_gr_db = 0.0;
        self.peak_hold_counter = 0;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.update_coefficients();
    }

    fn update_coefficients(&mut self) {
        self.attack_coeff = (-1.0 / (ATTACK_MS * 0.001 * self.sample_rate)).exp();
        self.release_coeff = (-1.0 / (RELEASE_MS * 0.001 * self.sample_rate)).exp();
        self.peak_hold_samples = (PEAK_HOLD_MS * 0.001 * self.sample_rate) as u32;
    }

    /// Feed a gain reduction value (dB, typically negative or zero)
    #[inline(always)]
    pub fn push(&mut self, gr_db: f64) {
        // Smoothed current value with attack/release ballistics
        if gr_db < self.current_gr_db {
            // GR increasing (more compression) — fast attack
            self.current_gr_db =
                self.attack_coeff * self.current_gr_db + (1.0 - self.attack_coeff) * gr_db;
        } else {
            // GR decreasing (less compression) — slow release
            self.current_gr_db =
                self.release_coeff * self.current_gr_db + (1.0 - self.release_coeff) * gr_db;
        }

        // Peak hold
        if gr_db < self.peak_gr_db {
            self.peak_gr_db = gr_db;
            self.peak_hold_counter = self.peak_hold_samples;
        } else if self.peak_hold_counter > 0 {
            self.peak_hold_counter -= 1;
        } else {
            // Decay peak toward current
            self.peak_gr_db =
                self.release_coeff * self.peak_gr_db + (1.0 - self.release_coeff) * gr_db;
        }
    }

    /// Get current smoothed gain reduction (dB)
    #[inline(always)]
    pub fn current_gr_db(&self) -> f64 {
        self.current_gr_db
    }

    /// Get peak gain reduction (dB)
    #[inline(always)]
    pub fn peak_gr_db(&self) -> f64 {
        self.peak_gr_db
    }
}

/// Atomic float for thread-safe GR value sharing between audio and GUI threads
pub struct AtomicF32 {
    bits: AtomicU32,
}

impl AtomicF32 {
    pub fn new(val: f32) -> Self {
        Self {
            bits: AtomicU32::new(val.to_bits()),
        }
    }

    #[inline(always)]
    pub fn store(&self, val: f32) {
        self.bits.store(val.to_bits(), Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn load(&self) -> f32 {
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }
}

/// Per-band GR meter outputs for sharing with the GUI/host
/// These are placed in an Arc and shared between audio thread and GUI
pub struct GrMeterOutputs {
    /// Current smoothed GR per band (dB, negative = compression)
    pub band_gr: [AtomicF32; 4],
    /// Peak GR per band (dB)
    pub band_peak_gr: [AtomicF32; 4],
}

impl GrMeterOutputs {
    pub fn new() -> Self {
        Self {
            band_gr: [
                AtomicF32::new(0.0),
                AtomicF32::new(0.0),
                AtomicF32::new(0.0),
                AtomicF32::new(0.0),
            ],
            band_peak_gr: [
                AtomicF32::new(0.0),
                AtomicF32::new(0.0),
                AtomicF32::new(0.0),
                AtomicF32::new(0.0),
            ],
        }
    }

    /// Update outputs from meter state (call from audio thread at reduced rate)
    #[inline(always)]
    pub fn update_from_meters(&self, meters: &[GrMeter; 4]) {
        for i in 0..4 {
            self.band_gr[i].store(meters[i].current_gr_db() as f32);
            self.band_peak_gr[i].store(meters[i].peak_gr_db() as f32);
        }
    }
}
