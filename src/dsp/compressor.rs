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
        let alpha = sin_w0 / (2.0 * 2.0_f64.sqrt());

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

        // RMS-ish level detection (peak of abs, linked stereo)
        // SSL uses peak detection with some RMS-like averaging
        let peak = sc_l.abs().max(sc_r.abs());
        let input_db = if peak > 1e-10 {
            20.0 * peak.log10()
        } else {
            -96.0
        };

        // === Envelope follower ===
        let target_db = input_db;

        // Attack/release ballistics
        let coeff = if target_db > self.envelope_db {
            // Attack phase
            self.attack_coeff
        } else if self.auto_release {
            // Auto-release: blend between fast and slow based on how long we've been compressing
            // This is key to the SSL's musical behavior
            let fast_coeff = self.ms_to_coeff(self.auto_release_fast);
            let slow_coeff = self.ms_to_coeff(self.auto_release_slow);

            // Update blend: ramp toward slow during compression, toward fast during release
            if self.gain_reduction_db < -1.0 {
                // Compressing significantly - blend toward slow
                self.auto_release_blend =
                    self.auto_release_blend * 0.9999 + 0.0001;
            } else {
                // Light/no compression - blend toward fast
                self.auto_release_blend =
                    self.auto_release_blend * 0.999;
            }

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
        let gain_linear = 10.0_f64.powf(total_gain_db / 20.0);

        // === Apply gain with mix (parallel compression) ===
        let wet_l = left * gain_linear;
        let wet_r = right * gain_linear;

        let out_l = left * (1.0 - self.mix) + wet_l * self.mix;
        let out_r = right * (1.0 - self.mix) + wet_r * self.mix;

        (out_l, out_r)
    }
}
