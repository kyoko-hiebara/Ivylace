/// BPM Detection Engine — zero-allocation onset detection + IOI histogram
///
/// Designed for the audio thread: all arrays are fixed-size, no heap allocation.
/// Uses energy-envelope onset detection with adaptive threshold,
/// then inter-onset-interval (IOI) histogram analysis to find BPM.

/// Maximum number of onsets tracked in one analysis window.
/// At ~180 BPM with double-time subdivisions: ~6 onsets/sec × 5 = 30.
/// 256 provides generous headroom.
const MAX_ONSETS: usize = 256;

/// Analysis duration in seconds
const ANALYSIS_DURATION_SECS: f64 = 5.0;

/// BPM detection range
const BPM_MIN: f64 = 60.0;
const BPM_MAX: f64 = 200.0;
/// Histogram bins: one per integer BPM from BPM_MIN to BPM_MAX inclusive
const NUM_BPM_BINS: usize = (BPM_MAX as usize) - (BPM_MIN as usize) + 1; // 141

/// Onset detection: adaptive threshold ratio above running mean energy
const ONSET_THRESHOLD_RATIO: f64 = 3.0;

/// Minimum absolute energy to trigger an onset (prevents noise triggers in silence)
const ONSET_MIN_ENERGY: f64 = 1e-6;

/// Fallback BPM when detection fails (common EDM tempo)
const FALLBACK_BPM: f64 = 128.0;

/// Minimum onsets required for confident BPM detection
const MIN_ONSETS_FOR_CONFIDENCE: usize = 6;

/// BPM detection engine for the audio thread.
/// All fields are fixed-size — no heap allocation.
pub struct AutoDetector {
    // Configuration
    sample_rate: f64,
    analysis_samples: u64,

    // Onset detection envelope (half-wave rectified + attack/release)
    envelope: f64,
    env_attack_coeff: f64,  // fast attack ~0.5ms
    env_release_coeff: f64, // slow release ~30ms

    // Adaptive threshold: running mean energy
    mean_energy: f64,
    mean_coeff: f64, // very slow ~2s time constant

    // Rising-edge detection: was the previous sample below threshold?
    was_below_threshold: bool,

    // Cooldown to prevent double-triggering on noisy onsets
    cooldown_remaining: u32,
    cooldown_samples: u32, // ~100ms at sample rate

    // Onset accumulation
    sample_count: u64,
    onset_positions: [u64; MAX_ONSETS],
    onset_count: usize,

    // Peak level measurement (for threshold estimation)
    peak_level: f64,
    sum_squares: f64,
    level_sample_count: u64,

    // State
    active: bool,
    done: bool,
    detected_bpm: f64,
    /// RMS level in dB (valid after done)
    rms_db: f64,
    /// Peak level in dB (valid after done)
    peak_db: f64,
}

impl AutoDetector {
    pub fn new(sample_rate: f64) -> Self {
        let mut det = Self {
            sample_rate,
            analysis_samples: 0,
            envelope: 0.0,
            env_attack_coeff: 0.0,
            env_release_coeff: 0.0,
            mean_energy: 0.0,
            mean_coeff: 0.0,
            was_below_threshold: true,
            cooldown_remaining: 0,
            cooldown_samples: 0,
            sample_count: 0,
            onset_positions: [0u64; MAX_ONSETS],
            onset_count: 0,
            peak_level: 0.0,
            sum_squares: 0.0,
            level_sample_count: 0,
            active: false,
            done: false,
            detected_bpm: 0.0,
            rms_db: -60.0,
            peak_db: -60.0,
        };
        det.update_coefficients();
        det
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.update_coefficients();
    }

    fn update_coefficients(&mut self) {
        let sr = self.sample_rate;
        self.analysis_samples = (ANALYSIS_DURATION_SECS * sr) as u64;

        // Onset envelope: fast attack (0.5ms), slow release (30ms)
        self.env_attack_coeff = (-1.0 / (0.0005 * sr)).exp();
        self.env_release_coeff = (-1.0 / (0.030 * sr)).exp();

        // Running mean energy: very slow (~2s) for adaptive threshold
        self.mean_coeff = (-1.0 / (2.0 * sr)).exp();

        // Cooldown: ~100ms minimum between onsets (prevents re-trigger during decay)
        self.cooldown_samples = (0.100 * sr) as u32;
    }

    /// Start a new BPM detection analysis. Resets all accumulation state.
    pub fn start(&mut self) {
        self.envelope = 0.0;
        self.mean_energy = 0.0;
        self.was_below_threshold = true;
        self.cooldown_remaining = 0;
        self.sample_count = 0;
        self.onset_count = 0;
        self.peak_level = 0.0;
        self.sum_squares = 0.0;
        self.level_sample_count = 0;
        self.detected_bpm = 0.0;
        self.rms_db = -60.0;
        self.peak_db = -60.0;
        self.done = false;
        self.active = true;
    }

    /// Reset all state (called on plugin reset).
    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.mean_energy = 0.0;
        self.was_below_threshold = true;
        self.cooldown_remaining = 0;
        self.sample_count = 0;
        self.onset_count = 0;
        self.peak_level = 0.0;
        self.sum_squares = 0.0;
        self.level_sample_count = 0;
        self.detected_bpm = 0.0;
        self.rms_db = -60.0;
        self.peak_db = -60.0;
        self.done = false;
        self.active = false;
    }

    /// Process one stereo sample pair. Called per-sample from the audio thread.
    #[inline(always)]
    pub fn process_sample(&mut self, left: f64, right: f64) {
        if !self.active || self.done {
            return;
        }

        // Mono energy (half-wave rectified absolute value)
        let energy = (left.abs() + right.abs()) * 0.5;

        // Peak and RMS level measurement (for threshold estimation)
        let mono = (left + right) * 0.5;
        let abs_mono = mono.abs();
        if abs_mono > self.peak_level {
            self.peak_level = abs_mono;
        }
        self.sum_squares += mono * mono;
        self.level_sample_count += 1;

        // Update envelope with attack/release ballistics
        if energy > self.envelope {
            self.envelope =
                self.env_attack_coeff * self.envelope + (1.0 - self.env_attack_coeff) * energy;
        } else {
            self.envelope =
                self.env_release_coeff * self.envelope + (1.0 - self.env_release_coeff) * energy;
        }

        // Update running mean energy (very slow, for adaptive threshold)
        self.mean_energy =
            self.mean_coeff * self.mean_energy + (1.0 - self.mean_coeff) * energy;

        // Onset detection: rising-edge — trigger only when envelope crosses
        // above threshold while previously below (prevents re-triggering during decay)
        let threshold = (self.mean_energy * ONSET_THRESHOLD_RATIO).max(ONSET_MIN_ENERGY);
        let is_above = self.envelope > threshold;

        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        } else if is_above && self.was_below_threshold && self.onset_count < MAX_ONSETS {
            self.onset_positions[self.onset_count] = self.sample_count;
            self.onset_count += 1;
            self.cooldown_remaining = self.cooldown_samples;
        }

        self.was_below_threshold = !is_above;

        self.sample_count += 1;

        // Check if analysis window is complete
        if self.sample_count >= self.analysis_samples {
            self.compute_bpm();
        }
    }

    /// Compute BPM from collected onset positions using IOI histogram.
    /// Uses only stack-allocated arrays — no heap allocation.
    fn compute_bpm(&mut self) {
        self.active = false;
        self.done = true;

        // Compute peak and RMS levels in dB
        if self.level_sample_count > 0 && self.peak_level > 0.0 {
            self.peak_db = (20.0 * self.peak_level.log10()).max(-60.0);
            let rms = (self.sum_squares / self.level_sample_count as f64).sqrt();
            self.rms_db = if rms > 0.0 {
                (20.0 * rms.log10()).max(-60.0)
            } else {
                -60.0
            };
        }

        if self.onset_count < MIN_ONSETS_FOR_CONFIDENCE {
            // Not enough onsets detected — fall back to default BPM
            self.detected_bpm = FALLBACK_BPM;
            return;
        }

        // Build IOI histogram using multiple span widths (consecutive, skip-1, skip-2).
        // This makes detection robust against occasional missed or extra onsets.
        let mut histogram = [0u32; NUM_BPM_BINS];

        // Weight: consecutive pairs get highest weight, wider spans less
        let max_skip = 3usize; // skip=1 (consecutive), skip=2, skip=3
        for skip in 1..=max_skip {
            let weight = (max_skip + 1 - skip) as u32; // 3, 2, 1
            let mut idx = skip;
            while idx < self.onset_count {
                let ioi_samples =
                    self.onset_positions[idx] - self.onset_positions[idx - skip];
                if ioi_samples == 0 {
                    idx += 1;
                    continue;
                }
                let ioi_secs = ioi_samples as f64 / self.sample_rate;

                // For wider spans, divide by skip count to get per-beat interval
                let per_beat_secs = ioi_secs / skip as f64;
                let raw_bpm = 60.0 / per_beat_secs;

                // Vote for raw BPM and ×2 (for 8th-note patterns).
                // ×0.5 is NOT used — it creates phantom half-tempo aliases.
                // Half-tempo is already handled by wider skip spans.
                let candidates = [raw_bpm, raw_bpm * 2.0];

                for &bpm in &candidates {
                    if bpm >= BPM_MIN && bpm <= BPM_MAX {
                        let bin = (bpm.round() as usize).saturating_sub(BPM_MIN as usize);
                        if bin < NUM_BPM_BINS {
                            histogram[bin] += weight;
                        }
                    }
                }
                idx += 1;
            }
        }

        // Smooth the histogram with a ±2 BPM window to handle jitter
        let mut smoothed = [0u32; NUM_BPM_BINS];
        for i in 0..NUM_BPM_BINS {
            let start = if i >= 2 { i - 2 } else { 0 };
            let end = if i + 2 < NUM_BPM_BINS { i + 2 } else { NUM_BPM_BINS - 1 };
            let mut sum = 0u32;
            let mut j = start;
            while j <= end {
                sum += histogram[j];
                j += 1;
            }
            smoothed[i] = sum;
        }

        // Find peak bin
        let mut best_bin = 0usize;
        let mut best_count = 0u32;
        for (i, &count) in smoothed.iter().enumerate() {
            if count > best_count {
                best_count = count;
                best_bin = i;
            }
        }

        if best_count == 0 {
            self.detected_bpm = FALLBACK_BPM;
        } else {
            self.detected_bpm = BPM_MIN + best_bin as f64;
        }
    }

    /// Whether the detector is currently accumulating.
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Whether detection is complete.
    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// The detected BPM (only valid when `is_done()` returns true).
    #[inline(always)]
    pub fn detected_bpm(&self) -> f64 {
        self.detected_bpm
    }

    /// Peak level in dB (valid after `is_done()`).
    #[inline(always)]
    pub fn peak_db(&self) -> f64 {
        self.peak_db
    }

    /// RMS level in dB (valid after `is_done()`).
    #[inline(always)]
    pub fn rms_db(&self) -> f64 {
        self.rms_db
    }

    /// Analysis progress from 0.0 to 1.0.
    #[inline(always)]
    pub fn progress(&self) -> f32 {
        if self.analysis_samples == 0 {
            return 0.0;
        }
        (self.sample_count as f32 / self.analysis_samples as f32).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Test BPM detection with a synthetic kick drum at 128 BPM.
    #[test]
    fn test_bpm_detection_128() {
        let sr = 44100.0;
        let mut det = AutoDetector::new(sr);
        det.start();

        let bpm = 128.0;
        let beat_interval_samples = (sr * 60.0 / bpm) as u64;
        let total_samples = (sr * ANALYSIS_DURATION_SECS) as u64;

        for s in 0..total_samples {
            // Synthetic kick: short burst every beat
            let pos_in_beat = s % beat_interval_samples;
            let kick = if pos_in_beat < 200 {
                // ~4.5ms burst at 44.1kHz
                let t = pos_in_beat as f64 / sr;
                0.8 * (2.0 * PI * 80.0 * t).sin() * (-t * 200.0).exp()
            } else {
                0.0
            };
            // Add some noise floor
            let noise = ((s as f64 * 0.1234).sin()) * 0.001;
            let sample = kick + noise;

            det.process_sample(sample, sample);
        }

        assert!(det.is_done());
        let detected = det.detected_bpm();
        eprintln!("Target BPM: {}, Detected BPM: {}", bpm, detected);
        // Allow ±3 BPM tolerance
        assert!(
            (detected - bpm).abs() <= 3.0,
            "Detected BPM {} is too far from target {}",
            detected,
            bpm
        );
    }

    /// Test BPM detection with 140 BPM.
    #[test]
    fn test_bpm_detection_140() {
        let sr = 44100.0;
        let mut det = AutoDetector::new(sr);
        det.start();

        let bpm = 140.0;
        let beat_interval_samples = (sr * 60.0 / bpm) as u64;
        let total_samples = (sr * ANALYSIS_DURATION_SECS) as u64;

        for s in 0..total_samples {
            let pos_in_beat = s % beat_interval_samples;
            let kick = if pos_in_beat < 200 {
                let t = pos_in_beat as f64 / sr;
                0.8 * (2.0 * PI * 80.0 * t).sin() * (-t * 200.0).exp()
            } else {
                0.0
            };
            let noise = ((s as f64 * 0.1234).sin()) * 0.001;
            let sample = kick + noise;
            det.process_sample(sample, sample);
        }

        assert!(det.is_done());
        let detected = det.detected_bpm();
        eprintln!("Target BPM: {}, Detected BPM: {}", bpm, detected);
        assert!(
            (detected - bpm).abs() <= 3.0,
            "Detected BPM {} is too far from target {}",
            detected,
            bpm
        );
    }

    /// Test that silence results in fallback BPM.
    #[test]
    fn test_bpm_detection_silence() {
        let sr = 44100.0;
        let mut det = AutoDetector::new(sr);
        det.start();

        let total_samples = (sr * ANALYSIS_DURATION_SECS) as u64;
        for _ in 0..total_samples {
            det.process_sample(0.0, 0.0);
        }

        assert!(det.is_done());
        assert_eq!(det.detected_bpm(), FALLBACK_BPM);
    }

    /// Test progress reporting.
    #[test]
    fn test_progress() {
        let sr = 44100.0;
        let mut det = AutoDetector::new(sr);
        det.start();

        assert!((det.progress() - 0.0).abs() < 0.01);

        let half = (sr * ANALYSIS_DURATION_SECS * 0.5) as u64;
        for _ in 0..half {
            det.process_sample(0.0, 0.0);
        }

        assert!((det.progress() - 0.5).abs() < 0.02);
    }
}
