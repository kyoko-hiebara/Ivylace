/// SSL 4000G-style VCA Bus Compressor
///
/// Key characteristics modeled:
/// - Feed-forward VCA topology (level detection before gain reduction)
/// - Stepped attack/release/ratio (like hardware)
/// - Program-dependent auto-release
/// - Sidechain HPF (critical for "glue" - prevents low-end pumping)
/// - Soft-knee at low ratios for musical compression
/// - Makeup gain
///
/// The "glue" character comes from:
/// 1. Relatively slow attack letting transients through
/// 2. Auto-release tracking the program material
/// 3. The subtle harmonic coloring of the VCA gain element
/// 4. Sidechain HPF preventing bass-induced pumping

use std::f64::consts::PI;

/// Available attack times (ms) - matches SSL hardware
pub const ATTACK_TIMES_MS: [f64; 6] = [0.1, 0.3, 1.0, 3.0, 10.0, 30.0];

/// Available release times (ms) - matches SSL hardware
/// Last value is sentinel for Auto mode
pub const RELEASE_TIMES_MS: [f64; 5] = [100.0, 300.0, 600.0, 1200.0, f64::INFINITY];

/// Available ratios
pub const RATIOS: [f64; 3] = [2.0, 4.0, 10.0];

/// Sidechain highpass filter (1st order for simplicity, 2nd order Butterworth)
#[derive(Clone, Copy)]
struct SidechainHpf {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
}

impl Default for SidechainHpf {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            s1: 0.0,
            s2: 0.0,
        }
    }
}

impl SidechainHpf {
    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    fn set_frequency(&mut self, freq: f64, sample_rate: f64) {
        if freq <= 0.0 {
            // Bypass
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.b2 = 0.0;
            self.a1 = 0.0;
            self.a2 = 0.0;
            return;
        }
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        // Q = 1/sqrt(2) (Butterworth): alpha = sin(w0) / (2*Q) = sin(w0) * sqrt(2) / 2
        let alpha = sin_w0 * std::f64::consts::FRAC_1_SQRT_2;

        let a0 = 1.0 + alpha;
        self.b0 = ((1.0 + cos_w0) / 2.0) / a0;
        self.b1 = (-(1.0 + cos_w0)) / a0;
        self.b2 = ((1.0 + cos_w0) / 2.0) / a0;
        self.a1 = (-2.0 * cos_w0) / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline(always)]
    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.s1;
        self.s1 = self.b1 * input - self.a1 * output + self.s2;
        self.s2 = self.b2 * input - self.a2 * output;
        output
    }
}

/// Single-band SSL-style compressor
#[derive(Clone)]
pub struct SslCompressor {
    sample_rate: f64,

    // Parameters
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    auto_release: bool,
    makeup_db: f64,
    mix: f64, // 0.0 = dry, 1.0 = wet (parallel compression)

    // Sidechain HPF
    sc_hpf_l: SidechainHpf,
    sc_hpf_r: SidechainHpf,
    sc_hpf_freq: f64,

    // Envelope follower state
    envelope_db: f64,
    gain_reduction_db: f64,

    // Auto-release state
    auto_release_fast: f64, // fast time constant (ms)
    auto_release_slow: f64, // slow time constant (ms)
    auto_release_blend: f64, // current blend factor

    // Sample-rate-aware auto-release blend coefficients
    // Ramp toward slow ~500ms time constant, toward fast ~100ms
    blend_ramp_up_coeff: f64,
    blend_ramp_down_coeff: f64,

    // Smoothed coefficients
    attack_coeff: f64,
    release_coeff: f64,

    // Band enable
    enabled: bool,
}

impl SslCompressor {
    pub fn new(sample_rate: f64) -> Self {
        let mut comp = Self {
            sample_rate,
            threshold_db: 0.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 300.0,
            auto_release: false,
            makeup_db: 0.0,
            mix: 1.0,
            sc_hpf_l: SidechainHpf::default(),
            sc_hpf_r: SidechainHpf::default(),
            sc_hpf_freq: 0.0,
            envelope_db: -96.0,
            gain_reduction_db: 0.0,
            auto_release_fast: 50.0,
            auto_release_slow: 1200.0,
            auto_release_blend: 0.0,
            blend_ramp_up_coeff: 0.0,
            blend_ramp_down_coeff: 0.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            enabled: true,
        };
        comp.update_coefficients();
        comp
    }

    pub fn reset(&mut self) {
        self.envelope_db = -96.0;
        self.gain_reduction_db = 0.0;
        self.auto_release_blend = 0.0;
        self.sc_hpf_l.reset();
        self.sc_hpf_r.reset();
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        self.sc_hpf_l.set_frequency(self.sc_hpf_freq, sample_rate);
        self.sc_hpf_r.set_frequency(self.sc_hpf_freq, sample_rate);
    }

    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = db;
    }

    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = ratio;
    }

    pub fn set_attack_ms(&mut self, ms: f64) {
        self.attack_ms = ms;
        self.update_coefficients();
    }

    pub fn set_release_ms(&mut self, ms: f64) {
        if ms == f64::INFINITY {
            self.auto_release = true;
            self.release_ms = 300.0; // default for auto
        } else {
            self.auto_release = false;
            self.release_ms = ms;
        }
        self.update_coefficients();
    }

    pub fn set_makeup_db(&mut self, db: f64) {
        self.makeup_db = db;
    }

    pub fn set_mix(&mut self, mix: f64) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn set_sidechain_hpf(&mut self, freq: f64) {
        self.sc_hpf_freq = freq;
        self.sc_hpf_l.set_frequency(freq, self.sample_rate);
        self.sc_hpf_r.set_frequency(freq, self.sample_rate);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn gain_reduction_db(&self) -> f64 {
        self.gain_reduction_db
    }

    fn update_coefficients(&mut self) {
        // Time constant: coeff = exp(-1 / (time_sec * sample_rate))
        self.attack_coeff = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate)).exp();
        self.release_coeff = (-1.0 / (self.release_ms * 0.001 * self.sample_rate)).exp();

        // Auto-release blend ramp coefficients (sample-rate aware).
        // Ramp-up (toward slow): ~500ms time constant — slow transition prevents abrupt
        // switching between fast and slow release, which causes audible pumping
        // Ramp-down (toward fast): ~100ms time constant — gradual return to fast release
        self.blend_ramp_up_coeff = (-1.0 / (0.500 * self.sample_rate)).exp();
        self.blend_ramp_down_coeff = (-1.0 / (0.100 * self.sample_rate)).exp();
    }

    fn ms_to_coeff(&self, ms: f64) -> f64 {
        (-1.0 / (ms * 0.001 * self.sample_rate)).exp()
    }

    /// Compute gain curve with soft knee
    /// Returns gain reduction in dB (negative value)
    #[inline(always)]
    fn compute_gain_reduction(&self, input_db: f64) -> f64 {
        let over_db = input_db - self.threshold_db;

        if over_db <= 0.0 {
            return 0.0;
        }

        // Soft knee width (dB) - wider at lower ratios for more musical compression
        let knee_width = match self.ratio as u32 {
            2 => 6.0,
            4 => 4.0,
            _ => 2.0, // 10:1
        };

        let half_knee = knee_width / 2.0;

        if over_db < half_knee {
            // In the knee region - quadratic interpolation
            let knee_factor = over_db / knee_width;
            let slope = 1.0 - 1.0 / self.ratio;
            -(slope * knee_factor * over_db)
        } else {
            // Above knee - standard compression
            -(over_db * (1.0 - 1.0 / self.ratio))
        }
    }

    /// Process stereo sample pair
    /// Returns (left_out, right_out)
    #[inline(always)]
    pub fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        if !self.enabled {
            return (left, right);
        }

        // === Sidechain path ===
        // Apply sidechain HPF (the secret sauce for glue)
        let sc_l = self.sc_hpf_l.process(left);
        let sc_r = self.sc_hpf_r.process(right);

        // Level detection: peak (max of |L|, |R|) — matches SSL 4000G hardware.
        // Linked stereo peak detection preserves transient accuracy,
        // which is critical for the "punch through" character of the SSL.
        let peak = sc_l.abs().max(sc_r.abs());
        let input_db = if peak > 1e-10 {
            20.0 * peak.log10()
        } else {
            -96.0
        };

        // === Envelope follower ===
        let target_db = input_db;

        // Attack/release ballistics
        // The feed-forward topology detects the input level and applies gain.
        // Attack: fast ramp up to input level (lets transients through).
        // Release: exponential decay back to rest — the speed adapts to
        // compression depth for auto-release, giving musical "breathing".
        let coeff = if target_db > self.envelope_db {
            // Attack phase
            self.attack_coeff
        } else if self.auto_release {
            // Auto-release: blend between fast and slow based on how long we've been compressing
            // This is key to the SSL's musical behavior
            let fast_coeff = self.ms_to_coeff(self.auto_release_fast);
            let slow_coeff = self.ms_to_coeff(self.auto_release_slow);

            // Update blend: ramp toward slow during compression, toward fast during release.
            // Uses sample-rate-aware exponential coefficients (computed in update_coefficients).
            // Hysteresis: ramp-up triggers at -2dB GR, ramp-down only when GR is above -0.5dB.
            // This prevents rapid oscillation between fast and slow release when GR hovers
            // near a single threshold, which is a major cause of audible pumping.
            if self.gain_reduction_db < -2.0 {
                // Compressing significantly - blend toward slow (target = 1.0)
                self.auto_release_blend = self.blend_ramp_up_coeff * self.auto_release_blend
                    + (1.0 - self.blend_ramp_up_coeff);
            } else if self.gain_reduction_db > -0.5 {
                // Barely compressing - blend toward fast (target = 0.0)
                self.auto_release_blend =
                    self.blend_ramp_down_coeff * self.auto_release_blend;
            }
            // Between -2dB and -0.5dB: hold current blend (hysteresis zone)

            let blend = self.auto_release_blend.clamp(0.0, 1.0);
            fast_coeff * (1.0 - blend) + slow_coeff * blend
        } else {
            self.release_coeff
        };

        // Smoothed envelope
        self.envelope_db = coeff * self.envelope_db + (1.0 - coeff) * target_db;

        // === Gain computation ===
        self.gain_reduction_db = self.compute_gain_reduction(self.envelope_db);

        let total_gain_db = self.gain_reduction_db + self.makeup_db;
        // exp(x * ln10/20) is equivalent to 10^(x/20) but ~3x faster than powf
        let gain_linear = (total_gain_db * (std::f64::consts::LN_10 / 20.0)).exp();

        // === Apply gain with mix (parallel compression) ===
        let wet_l = left * gain_linear;
        let wet_r = right * gain_linear;

        let out_l = left * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = right * (1.0 - self.mix) + wet_r * self.mix;

        (out_l, out_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test compressor gain at various frequencies with SC HPF at 80Hz.
    /// The bugged alpha (Q=√2 resonant) in SC HPF causes the sidechain to see
    /// a resonant boost around 80Hz, leading to MORE compression there and
    /// LESS compression at very low freqs → apparent low-end boost.
    #[test]
    fn test_compressor_with_sc_hpf_80() {
        let sr = 44100.0;
        let test_freqs = [10.0, 20.0, 25.0, 30.0, 50.0, 80.0, 100.0, 500.0, 1000.0];

        eprintln!("--- SC HPF = 80Hz (with current bugged alpha) ---");
        for &test_freq in &test_freqs {
            let mut comp = SslCompressor::new(sr);
            comp.set_threshold(-20.0);
            comp.set_ratio(4.0);
            comp.set_attack_ms(10.0);
            comp.set_release_ms(300.0);
            comp.set_makeup_db(0.0);
            comp.set_sidechain_hpf(80.0); // SC HPF at 80Hz
            comp.set_enabled(true);

            let amplitude = 0.5;
            let settle = 88200;
            for i in 0..settle {
                let t = i as f64 / sr;
                let input = amplitude * (2.0 * PI * test_freq * t).sin();
                comp.process(input, input);
            }

            let measure = 88200;
            let mut sum_sq_in = 0.0f64;
            let mut sum_sq_out = 0.0f64;
            for i in 0..measure {
                let t = (settle + i) as f64 / sr;
                let input = amplitude * (2.0 * PI * test_freq * t).sin();
                let (out_l, _) = comp.process(input, input);
                sum_sq_in += input * input;
                sum_sq_out += out_l * out_l;
            }

            let gain_db = 10.0 * (sum_sq_out / sum_sq_in).log10();
            eprintln!(
                "  SC HPF 80Hz, comp gain at {:>6.1}Hz: {:>+.3} dB  (GR: {:.3} dB)",
                test_freq, gain_db, comp.gain_reduction_db()
            );
        }

        eprintln!("--- SC HPF = 0Hz (bypass) ---");
        for &test_freq in &test_freqs {
            let mut comp = SslCompressor::new(sr);
            comp.set_threshold(-20.0);
            comp.set_ratio(4.0);
            comp.set_attack_ms(10.0);
            comp.set_release_ms(300.0);
            comp.set_makeup_db(0.0);
            comp.set_sidechain_hpf(0.0); // bypass SC HPF
            comp.set_enabled(true);

            let amplitude = 0.5;
            let settle = 88200;
            for i in 0..settle {
                let t = i as f64 / sr;
                let input = amplitude * (2.0 * PI * test_freq * t).sin();
                comp.process(input, input);
            }

            let measure = 88200;
            let mut sum_sq_in = 0.0f64;
            let mut sum_sq_out = 0.0f64;
            for i in 0..measure {
                let t = (settle + i) as f64 / sr;
                let input = amplitude * (2.0 * PI * test_freq * t).sin();
                let (out_l, _) = comp.process(input, input);
                sum_sq_in += input * input;
                sum_sq_out += out_l * out_l;
            }

            let gain_db = 10.0 * (sum_sq_out / sum_sq_in).log10();
            eprintln!(
                "  SC HPF OFF, comp gain at {:>6.1}Hz: {:>+.3} dB  (GR: {:.3} dB)",
                test_freq, gain_db, comp.gain_reduction_db()
            );
        }
    }
}
