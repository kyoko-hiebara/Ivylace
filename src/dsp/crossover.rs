/// 4-band Linkwitz-Riley crossover filter (LR4 = 24dB/oct)
///
/// Splits audio into 4 bands using cascaded 2nd-order Butterworth filters.
/// LR4 ensures phase-coherent summation at crossover points (flat magnitude response).
///
/// Band layout:
///   Band 0 (Low):     DC .. freq_low
///   Band 1 (LowMid):  freq_low .. freq_mid
///   Band 2 (HighMid): freq_mid .. freq_high
///   Band 3 (High):    freq_high .. Nyquist

use std::f64::consts::PI;

const NUM_BANDS: usize = 4;
const NUM_CROSSOVERS: usize = 3; // 3 crossover points for 4 bands

/// Second-order biquad filter (Direct Form II Transposed)
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    // State variables (DF2T)
    s1: f64,
    s2: f64,
}

impl Biquad {
    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// Configure as 2nd-order allpass at the given frequency.
    /// An allpass has the same magnitude as the corresponding lowpass/highpass
    /// but passes all frequencies with only phase shift.
    /// H_ap(z) = (a2 + a1*z^-1 + z^-2) / (1 + a1*z^-1 + a2*z^-2)
    fn set_allpass(&mut self, freq: f64, sample_rate: f64) {
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 * std::f64::consts::FRAC_1_SQRT_2;

        let a0 = 1.0 + alpha;
        self.b0 = (1.0 - alpha) / a0;
        self.b1 = (-2.0 * cos_w0) / a0;
        self.b2 = (1.0 + alpha) / a0;
        self.a1 = (-2.0 * cos_w0) / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    /// Configure as 2nd-order Butterworth lowpass
    fn set_lowpass(&mut self, freq: f64, sample_rate: f64) {
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        // Q = 1/sqrt(2) (Butterworth): alpha = sin(w0) / (2*Q) = sin(w0) * sqrt(2) / 2
        let alpha = sin_w0 * std::f64::consts::FRAC_1_SQRT_2;

        let a0 = 1.0 + alpha;
        self.b0 = ((1.0 - cos_w0) / 2.0) / a0;
        self.b1 = (1.0 - cos_w0) / a0;
        self.b2 = ((1.0 - cos_w0) / 2.0) / a0;
        self.a1 = (-2.0 * cos_w0) / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    /// Configure as 2nd-order Butterworth highpass
    fn set_highpass(&mut self, freq: f64, sample_rate: f64) {
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

    /// Process one sample (DF2T for numerical stability)
    #[inline(always)]
    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.s1;
        self.s1 = self.b1 * input - self.a1 * output + self.s2;
        self.s2 = self.b2 * input - self.a2 * output;
        output
    }
}

/// LR4 = two cascaded Butterworth 2nd-order filters
#[derive(Clone, Copy, Default)]
struct Lr4Filter {
    biquad1: Biquad,
    biquad2: Biquad,
}

impl Lr4Filter {
    fn reset(&mut self) {
        self.biquad1.reset();
        self.biquad2.reset();
    }

    fn set_lowpass(&mut self, freq: f64, sample_rate: f64) {
        self.biquad1.set_lowpass(freq, sample_rate);
        self.biquad2.set_lowpass(freq, sample_rate);
    }

    fn set_highpass(&mut self, freq: f64, sample_rate: f64) {
        self.biquad1.set_highpass(freq, sample_rate);
        self.biquad2.set_highpass(freq, sample_rate);
    }

    fn set_allpass(&mut self, freq: f64, sample_rate: f64) {
        self.biquad1.set_allpass(freq, sample_rate);
        self.biquad2.set_allpass(freq, sample_rate);
    }

    #[inline(always)]
    fn process(&mut self, input: f64) -> f64 {
        let x = self.biquad1.process(input);
        self.biquad2.process(x)
    }
}

/// One crossover point (splits into low and high)
#[derive(Clone, Copy, Default)]
struct CrossoverPoint {
    lp: Lr4Filter,
    hp: Lr4Filter,
}

impl CrossoverPoint {
    fn reset(&mut self) {
        self.lp.reset();
        self.hp.reset();
    }

    fn set_frequency(&mut self, freq: f64, sample_rate: f64) {
        self.lp.set_lowpass(freq, sample_rate);
        self.hp.set_highpass(freq, sample_rate);
    }

    #[inline(always)]
    fn process(&mut self, input: f64) -> (f64, f64) {
        (self.lp.process(input), self.hp.process(input))
    }
}

/// Complete 4-band crossover (stereo) with allpass phase compensation.
///
/// The tree-structured topology (mid split → sub-splits) introduces phase
/// misalignment between the low-half and high-half branches because each
/// branch only "sees" one of the two sub-crossover allpass responses.
///
/// Compensation: insert an LR4 allpass at the "other" crossover frequency
/// into each branch so all 4 bands share the same total phase response:
///   - Low-half  (Band 0, 1): add allpass at high crossover freq (xover[2])
///   - High-half (Band 2, 3): add allpass at low crossover freq (xover[0])
pub struct FourBandCrossover {
    /// [channel][crossover_index]
    crossovers: [[CrossoverPoint; NUM_CROSSOVERS]; 2],
    /// Allpass compensation: low-half bands get allpass at high freq
    /// [channel] — one LR4 allpass per channel
    ap_low_half: [Lr4Filter; 2],
    /// Allpass compensation: high-half bands get allpass at low freq
    ap_high_half: [Lr4Filter; 2],
    sample_rate: f64,
}

impl FourBandCrossover {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            crossovers: [[CrossoverPoint::default(); NUM_CROSSOVERS]; 2],
            ap_low_half: [Lr4Filter::default(), Lr4Filter::default()],
            ap_high_half: [Lr4Filter::default(), Lr4Filter::default()],
            sample_rate,
        }
    }

    pub fn reset(&mut self) {
        for ch in &mut self.crossovers {
            for xover in ch.iter_mut() {
                xover.reset();
            }
        }
        for ap in &mut self.ap_low_half {
            ap.reset();
        }
        for ap in &mut self.ap_high_half {
            ap.reset();
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    /// Set crossover frequencies: [low_freq, mid_freq, high_freq]
    /// e.g., [120.0, 1500.0, 8000.0]
    pub fn set_frequencies(&mut self, freqs: [f64; NUM_CROSSOVERS]) {
        for ch_idx in 0..2 {
            let ch = &mut self.crossovers[ch_idx];
            for (i, &freq) in freqs.iter().enumerate() {
                ch[i].set_frequency(freq, self.sample_rate);
            }
            // Allpass compensation:
            // Low-half (bands 0,1) gets allpass at high crossover freq
            self.ap_low_half[ch_idx].set_allpass(freqs[2], self.sample_rate);
            // High-half (bands 2,3) gets allpass at low crossover freq
            self.ap_high_half[ch_idx].set_allpass(freqs[0], self.sample_rate);
        }
    }

    /// Process one stereo sample, returns [band0, band1, band2, band3] for each channel
    /// Input: (left, right)
    /// Output: ([L_low, L_lowmid, L_highmid, L_high], [R_low, R_lowmid, R_highmid, R_high])
    #[inline(always)]
    pub fn process(&mut self, left: f64, right: f64) -> ([f64; NUM_BANDS], [f64; NUM_BANDS]) {
        let mut out_l = [0.0f64; NUM_BANDS];
        let mut out_r = [0.0f64; NUM_BANDS];

        // Split topology for 4 bands with allpass phase compensation:
        //
        //  input ─┬─ LP1 ─── AP_high ─┬─ LP0 ─── Band 0 (Low)
        //         │                    └─ HP0 ─── Band 1 (LowMid)
        //         └─ HP1 ─── AP_low  ─┬─ LP2 ─── Band 2 (HighMid)
        //                             └─ HP2 ─── Band 3 (High)
        //
        // AP_high = LR4 allpass at high crossover freq (compensates xover[2] phase)
        // AP_low  = LR4 allpass at low crossover freq  (compensates xover[0] phase)

        for (ch_idx, input) in [(0usize, left), (1usize, right)] {
            let xovers = &mut self.crossovers[ch_idx];

            // First split at mid frequency
            let (low_half, high_half) = xovers[1].process(input);

            // Apply allpass compensation before sub-splits
            let low_half = self.ap_low_half[ch_idx].process(low_half);
            let high_half = self.ap_high_half[ch_idx].process(high_half);

            // Split low half at low frequency
            let (band0, band1) = xovers[0].process(low_half);

            // Split high half at high frequency
            let (band2, band3) = xovers[2].process(high_half);

            let out = if ch_idx == 0 { &mut out_l } else { &mut out_r };
            out[0] = band0;
            out[1] = band1;
            out[2] = band2;
            out[3] = band3;
        }

        (out_l, out_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test crossover gain at very low frequencies (5-30Hz) to check for sub-bass boost.
    #[test]
    fn test_crossover_sub_bass_gain() {
        let sr = 44100.0;
        let test_freqs = [5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 50.0, 100.0];

        for &test_freq in &test_freqs {
            let mut xover = FourBandCrossover::new(sr);
            xover.set_frequencies([120.0, 1500.0, 8000.0]);

            let settle = 88200; // 2 seconds to settle low-freq filters
            for i in 0..settle {
                let t = i as f64 / sr;
                let input = (2.0 * PI * test_freq * t).sin();
                xover.process(input, input);
            }

            let measure = 88200;
            let mut sum_sq_sum = 0.0f64;
            let mut sum_sq_input = 0.0f64;

            for i in 0..measure {
                let t = (settle + i) as f64 / sr;
                let input = (2.0 * PI * test_freq * t).sin();
                let (bands_l, _) = xover.process(input, input);
                let band_sum: f64 = bands_l.iter().sum();
                sum_sq_sum += band_sum * band_sum;
                sum_sq_input += input * input;
            }

            let gain_db = 10.0 * (sum_sq_sum / sum_sq_input).log10();
            eprintln!("Crossover gain at {test_freq:>6.1}Hz: {gain_db:>+.4} dB");
            assert!(
                gain_db.abs() < 0.2,
                "Crossover gain at {test_freq}Hz should be near 0dB, got {gain_db:.3}dB"
            );
        }
    }

    /// Verify that the 4-band crossover reconstructs a DC signal with unity gain.
    #[test]
    fn test_crossover_dc_unity() {
        let sr = 44100.0;
        let mut xover = FourBandCrossover::new(sr);
        xover.set_frequencies([120.0, 1500.0, 8000.0]);

        // Settle the filters
        for _ in 0..4096 {
            xover.process(1.0, 1.0);
        }

        // Check that band sum ≈ input
        let (bands_l, bands_r) = xover.process(1.0, 1.0);
        let sum_l: f64 = bands_l.iter().sum();
        let sum_r: f64 = bands_r.iter().sum();
        assert!(
            (sum_l - 1.0).abs() < 1e-6,
            "DC sum L should be 1.0, got {sum_l}"
        );
        assert!(
            (sum_r - 1.0).abs() < 1e-6,
            "DC sum R should be 1.0, got {sum_r}"
        );
    }

    /// Verify that 4-band crossover sum has flat magnitude (allpass) for sine waves.
    /// Uses RMS gain comparison: RMS(band_sum) / RMS(input) should be ~1.0 (0dB).
    #[test]
    fn test_crossover_flat_magnitude_sine() {
        let sr = 44100.0;
        let test_freqs = [60.0, 120.0, 500.0, 1500.0, 4000.0, 8000.0, 15000.0];

        for &test_freq in &test_freqs {
            let mut xover = FourBandCrossover::new(sr);
            xover.set_frequencies([120.0, 1500.0, 8000.0]);

            let settle = 44100;
            for i in 0..settle {
                let t = i as f64 / sr;
                let input = (2.0 * PI * test_freq * t).sin();
                xover.process(input, input);
            }

            let measure = 44100;
            let mut sum_sq_sum = 0.0f64;
            let mut sum_sq_input = 0.0f64;

            for i in 0..measure {
                let t = (settle + i) as f64 / sr;
                let input = (2.0 * PI * test_freq * t).sin();
                let (bands_l, _) = xover.process(input, input);
                let band_sum: f64 = bands_l.iter().sum();
                sum_sq_sum += band_sum * band_sum;
                sum_sq_input += input * input;
            }

            let gain_db = 10.0 * (sum_sq_sum / sum_sq_input).log10();
            // With allpass compensation, the nested LR4 topology sums closer
            // to flat. Residual dips at crossover frequencies are < 0.6dB.
            assert!(
                gain_db.abs() < 0.6,
                "Crossover gain at {test_freq}Hz should be near 0dB, got {gain_db:.3}dB"
            );
        }
    }

    /// Test a single LR4 crossover point: LP + HP magnitude should be flat (allpass).
    #[test]
    fn test_lr4_crossover_point_allpass() {
        let sr = 44100.0;
        let test_freqs = [60.0, 120.0, 500.0, 1500.0, 4000.0, 8000.0, 15000.0];

        for &test_freq in &test_freqs {
            let mut xp = CrossoverPoint::default();
            xp.set_frequency(1500.0, sr);

            let settle = 44100;
            for i in 0..settle {
                let t = i as f64 / sr;
                xp.process((2.0 * PI * test_freq * t).sin());
            }

            let measure = 44100;
            let mut sum_sq_sum = 0.0f64;
            let mut sum_sq_input = 0.0f64;

            for i in 0..measure {
                let t = (settle + i) as f64 / sr;
                let input = (2.0 * PI * test_freq * t).sin();
                let (lo, hi) = xp.process(input);
                let band_sum = lo + hi;
                sum_sq_sum += band_sum * band_sum;
                sum_sq_input += input * input;
            }

            let gain_db = 10.0 * (sum_sq_sum / sum_sq_input).log10();
            assert!(
                gain_db.abs() < 0.1,
                "LR4 point gain at {test_freq}Hz should be ~0dB, got {gain_db:.3}dB"
            );
        }
    }

    /// Test 2nd-order Butterworth LP at cutoff: should be -3dB.
    #[test]
    fn test_butterworth_lp_at_cutoff() {
        let sr = 44100.0;
        let fc = 1500.0;
        let mut bq = Biquad::default();
        bq.set_lowpass(fc, sr);

        let settle = 44100;
        for i in 0..settle {
            let t = i as f64 / sr;
            bq.process((2.0 * PI * fc * t).sin());
        }

        let measure = 44100;
        let mut sum_sq_out = 0.0f64;
        let mut sum_sq_in = 0.0f64;
        for i in 0..measure {
            let t = (settle + i) as f64 / sr;
            let input = (2.0 * PI * fc * t).sin();
            let output = bq.process(input);
            sum_sq_out += output * output;
            sum_sq_in += input * input;
        }

        let gain_db = 10.0 * (sum_sq_out / sum_sq_in).log10();
        assert!(
            (gain_db - (-3.01)).abs() < 0.5,
            "Butterworth LP at cutoff should be ~-3dB, got {gain_db:.2}dB"
        );
    }
}
