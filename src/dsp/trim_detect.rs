/// TRIM level measurement for dry/wet gain matching.
///
/// Uses simple RMS over a fixed window (3 seconds) to compute dry and wet levels,
/// then calculates trim_gain_db = dry_rms_db - wet_rms_db.
///
/// Simpler than LUFS (which requires K-weighting and gating) because we only
/// need the relative difference between dry and wet — measurement bias cancels out.

const ANALYSIS_DURATION_SECS: f64 = 3.0;

/// Maximum trim gain magnitude (dB). Prevents extreme values from silence or
/// very aggressive processing.
const MAX_TRIM_DB: f64 = 12.0;

/// Floor level for RMS (dB). If measured RMS is below this, treat as silence.
const SILENCE_FLOOR_DB: f64 = -60.0;

pub struct TrimDetector {
    sample_rate: f64,
    analysis_samples: u64,
    dry_sum_sq_l: f64,
    dry_sum_sq_r: f64,
    wet_sum_sq_l: f64,
    wet_sum_sq_r: f64,
    sample_count: u64,
    active: bool,
    done: bool,
    trim_gain_db: f64,
}

impl TrimDetector {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            analysis_samples: (sample_rate * ANALYSIS_DURATION_SECS) as u64,
            dry_sum_sq_l: 0.0,
            dry_sum_sq_r: 0.0,
            wet_sum_sq_l: 0.0,
            wet_sum_sq_r: 0.0,
            sample_count: 0,
            active: false,
            done: false,
            trim_gain_db: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.analysis_samples = (sample_rate * ANALYSIS_DURATION_SECS) as u64;
    }

    /// Start a new measurement. Resets accumulators.
    pub fn start(&mut self) {
        self.dry_sum_sq_l = 0.0;
        self.dry_sum_sq_r = 0.0;
        self.wet_sum_sq_l = 0.0;
        self.wet_sum_sq_r = 0.0;
        self.sample_count = 0;
        self.active = true;
        self.done = false;
        self.trim_gain_db = 0.0;
    }

    /// Reset to idle state.
    pub fn reset(&mut self) {
        self.active = false;
        self.done = false;
        self.dry_sum_sq_l = 0.0;
        self.dry_sum_sq_r = 0.0;
        self.wet_sum_sq_l = 0.0;
        self.wet_sum_sq_r = 0.0;
        self.sample_count = 0;
        self.trim_gain_db = 0.0;
    }

    /// Process one stereo sample pair.
    /// dry_l/dry_r: crossover output (before comp/sat)
    /// wet_l/wet_r: processed output (after comp/sat)
    #[inline(always)]
    pub fn process_sample(&mut self, dry_l: f64, dry_r: f64, wet_l: f64, wet_r: f64) {
        if !self.active || self.done {
            return;
        }

        self.dry_sum_sq_l += dry_l * dry_l;
        self.dry_sum_sq_r += dry_r * dry_r;
        self.wet_sum_sq_l += wet_l * wet_l;
        self.wet_sum_sq_r += wet_r * wet_r;

        self.sample_count += 1;

        if self.sample_count >= self.analysis_samples {
            self.compute_trim_gain();
        }
    }

    fn compute_trim_gain(&mut self) {
        self.active = false;
        self.done = true;

        if self.sample_count == 0 {
            self.trim_gain_db = 0.0;
            return;
        }

        let inv_count = 1.0 / self.sample_count as f64;

        // RMS for dry (stereo average)
        let dry_rms_l = (self.dry_sum_sq_l * inv_count).sqrt();
        let dry_rms_r = (self.dry_sum_sq_r * inv_count).sqrt();
        let dry_rms = (dry_rms_l + dry_rms_r) * 0.5;

        // RMS for wet (stereo average)
        let wet_rms_l = (self.wet_sum_sq_l * inv_count).sqrt();
        let wet_rms_r = (self.wet_sum_sq_r * inv_count).sqrt();
        let wet_rms = (wet_rms_l + wet_rms_r) * 0.5;

        // Compute dB levels (protect against silence)
        let dry_db = if dry_rms > 1e-9 {
            20.0 * dry_rms.log10()
        } else {
            SILENCE_FLOOR_DB
        };

        let wet_db = if wet_rms > 1e-9 {
            20.0 * wet_rms.log10()
        } else {
            SILENCE_FLOOR_DB
        };

        // Trim gain = dry - wet (how much to boost/cut wet to match dry)
        self.trim_gain_db = (dry_db - wet_db).clamp(-MAX_TRIM_DB, MAX_TRIM_DB);
    }

    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[inline(always)]
    pub fn is_done(&self) -> bool {
        self.done
    }

    #[inline(always)]
    pub fn trim_gain_db(&self) -> f64 {
        self.trim_gain_db
    }

    #[inline(always)]
    pub fn progress(&self) -> f32 {
        if self.analysis_samples == 0 {
            0.0
        } else {
            (self.sample_count as f32 / self.analysis_samples as f32).min(1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_silence() {
        let mut det = TrimDetector::new(44100.0);
        det.start();

        // Feed silence
        for _ in 0..((44100.0 * 3.0) as u64) {
            det.process_sample(0.0, 0.0, 0.0, 0.0);
        }

        assert!(det.is_done());
        // Both at floor → trim = 0
        assert!((det.trim_gain_db() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_trim_louder_wet() {
        let mut det = TrimDetector::new(44100.0);
        det.start();

        // Dry = 0.5, Wet = 1.0 → wet is ~6dB louder → trim ≈ -6dB
        let dry = 0.5;
        let wet = 1.0;
        for _ in 0..((44100.0 * 3.0) as u64) {
            det.process_sample(dry, dry, wet, wet);
        }

        assert!(det.is_done());
        let expected = 20.0 * (0.5_f64).log10() - 20.0 * (1.0_f64).log10();
        assert!((det.trim_gain_db() - expected).abs() < 0.01);
    }

    #[test]
    fn test_trim_clamp() {
        let mut det = TrimDetector::new(44100.0);
        det.start();

        // Extreme difference: dry=1.0, wet=0.001 → ~60dB diff → should clamp to 12dB
        for _ in 0..((44100.0 * 3.0) as u64) {
            det.process_sample(1.0, 1.0, 0.001, 0.001);
        }

        assert!(det.is_done());
        assert!((det.trim_gain_db() - 12.0).abs() < 0.01);
    }

    #[test]
    fn test_progress() {
        let mut det = TrimDetector::new(44100.0);
        det.start();

        let total = (44100.0 * 3.0) as u64;
        let half = total / 2;

        for _ in 0..half {
            det.process_sample(0.5, 0.5, 0.5, 0.5);
        }

        let p = det.progress();
        assert!((p - 0.5).abs() < 0.01);
        assert!(!det.is_done());
    }
}
