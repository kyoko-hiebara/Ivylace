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

    /// Configure as 2nd-order Butterworth lowpass
    fn set_lowpass(&mut self, freq: f64, sample_rate: f64) {
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        // Q = 0.7071 (Butterworth)
        let alpha = sin_w0 / (2.0 * std::f64::consts::FRAC_1_SQRT_2.recip());
        let alpha = sin_w0 / (2.0 * 2.0_f64.sqrt());

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
        let alpha = sin_w0 / (2.0 * 2.0_f64.sqrt());

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

/// Complete 4-band crossover (stereo)
pub struct FourBandCrossover {
    /// [channel][crossover_index]
    crossovers: [[CrossoverPoint; NUM_CROSSOVERS]; 2],
    sample_rate: f64,
}

impl FourBandCrossover {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            crossovers: [[CrossoverPoint::default(); NUM_CROSSOVERS]; 2],
            sample_rate,
        }
    }

    pub fn reset(&mut self) {
        for ch in &mut self.crossovers {
            for xover in ch.iter_mut() {
                xover.reset();
            }
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    /// Set crossover frequencies: [low_freq, mid_freq, high_freq]
    /// e.g., [120.0, 1500.0, 8000.0]
    pub fn set_frequencies(&mut self, freqs: [f64; NUM_CROSSOVERS]) {
        for ch in &mut self.crossovers {
            for (i, &freq) in freqs.iter().enumerate() {
                ch[i].set_frequency(freq, self.sample_rate);
            }
        }
    }

    /// Process one stereo sample, returns [band0, band1, band2, band3] for each channel
    /// Input: (left, right)
    /// Output: ([L_low, L_lowmid, L_highmid, L_high], [R_low, R_lowmid, R_highmid, R_high])
    #[inline(always)]
    pub fn process(&mut self, left: f64, right: f64) -> ([f64; NUM_BANDS], [f64; NUM_BANDS]) {
        let mut out_l = [0.0f64; NUM_BANDS];
        let mut out_r = [0.0f64; NUM_BANDS];

        // Split topology for 4 bands:
        //
        //  input ─┬─ LP1 ─┬─ LP0 ─── Band 0 (Low)
        //         │       └─ HP0 ─── Band 1 (LowMid)
        //         └─ HP1 ─┬─ LP2 ─── Band 2 (HighMid)
        //                 └─ HP2 ─── Band 3 (High)
        //
        // Crossover 1 = mid frequency (splits low-half and high-half)
        // Crossover 0 = low frequency (splits low-half into Low and LowMid)
        // Crossover 2 = high frequency (splits high-half into HighMid and High)

        for (ch_idx, input) in [(0usize, left), (1usize, right)] {
            let xovers = &mut self.crossovers[ch_idx];

            // First split at mid frequency
            let (low_half, high_half) = xovers[1].process(input);

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
