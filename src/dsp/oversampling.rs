/// Oversampling module for anti-aliased saturation and compression
///
/// Uses half-band polyphase FIR filters for efficient 2x oversampling.
/// 4x is achieved by cascading two 2x stages.
///
/// The FIR filter coefficients are designed for:
/// - Steep transition band near Nyquist/2
/// - Good stopband attenuation (>80dB)
/// - Linear phase (symmetric FIR)
///
/// No heap allocations — all buffers are fixed-size arrays.

/// Oversampling factor selection
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OversamplingFactor {
    /// No oversampling (1x)
    X1,
    /// 2x oversampling
    X2,
    /// 4x oversampling (two cascaded 2x stages)
    X4,
}

impl Default for OversamplingFactor {
    fn default() -> Self {
        Self::X1
    }
}

/// Half-band FIR filter coefficients for 2x oversampling
/// 16-tap half-band filter with ~80dB stopband attenuation
/// Only non-zero coefficients stored (half-band property: every other coeff is 0)
const HALFBAND_COEFFS: [f64; 8] = [
    -0.000_986_588_470_651_4,
     0.005_765_984_949_204_5,
    -0.021_352_573_498_587_4,
     0.069_513_028_169_622_3,
    // center tap is 0.5 (handled separately)
    // mirror of above
     0.069_513_028_169_622_3,
    -0.021_352_573_498_587_4,
     0.005_765_984_949_204_5,
    -0.000_986_588_470_651_4,
];

/// Half the filter length
const HALFBAND_HALF_LEN: usize = 8;
/// Full delay line length for the half-band filter
const HALFBAND_DELAY_LEN: usize = HALFBAND_HALF_LEN * 2 + 1; // 17

/// Single-channel 2x oversampler using half-band polyphase FIR
#[derive(Clone)]
pub struct Oversampler2x {
    /// Delay line for upsampling filter
    up_delay: [f64; HALFBAND_DELAY_LEN],
    up_pos: usize,
    /// Delay line for downsampling filter
    down_delay: [f64; HALFBAND_DELAY_LEN],
    down_pos: usize,
}

impl Oversampler2x {
    pub fn new() -> Self {
        Self {
            up_delay: [0.0; HALFBAND_DELAY_LEN],
            up_pos: 0,
            down_delay: [0.0; HALFBAND_DELAY_LEN],
            down_pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.up_delay = [0.0; HALFBAND_DELAY_LEN];
        self.up_pos = 0;
        self.down_delay = [0.0; HALFBAND_DELAY_LEN];
        self.down_pos = 0;
    }

    /// Upsample: produce 2 output samples from 1 input sample.
    /// Returns [sample_0, sample_1] at 2x rate.
    #[inline(always)]
    pub fn upsample(&mut self, input: f64) -> [f64; 2] {
        // Insert input with zero-stuffing:
        // Even samples = input, odd samples = 0
        // Then filter with half-band FIR

        // Push input into delay line (at even position)
        self.up_delay[self.up_pos] = input * 2.0; // *2 to compensate for zero-stuffing
        let pos = self.up_pos;

        // Compute filtered output for even sample (center tap + polyphase)
        let mut even_out = self.up_delay[pos] * 0.5; // center tap
        for k in 0..HALFBAND_HALF_LEN {
            let idx_neg = (pos + HALFBAND_DELAY_LEN - (2 * k + 1)) % HALFBAND_DELAY_LEN;
            let idx_pos = (pos + 2 * k + 1) % HALFBAND_DELAY_LEN;
            even_out += HALFBAND_COEFFS[k] * (self.up_delay[idx_neg] + self.up_delay[idx_pos]);
        }

        // Odd sample: interpolated value using the polyphase decomposition
        // For zero-stuffed signal, odd sample is purely from the filter wings
        let mut odd_out = 0.0;
        for k in 0..HALFBAND_HALF_LEN {
            let idx = (pos + HALFBAND_DELAY_LEN - 2 * k) % HALFBAND_DELAY_LEN;
            odd_out += HALFBAND_COEFFS[k] * self.up_delay[idx];
        }
        // Add center contribution for odd
        let center_idx = (pos + HALFBAND_DELAY_LEN - HALFBAND_HALF_LEN) % HALFBAND_DELAY_LEN;
        odd_out += self.up_delay[center_idx] * 0.5;

        self.up_pos = (self.up_pos + 1) % HALFBAND_DELAY_LEN;

        [even_out, odd_out]
    }

    /// Downsample: take 2 input samples (at 2x rate) and produce 1 output sample.
    #[inline(always)]
    pub fn downsample(&mut self, samples: [f64; 2]) -> f64 {
        // Anti-aliasing filter + decimation
        // Process both samples through the filter, output only every other

        // First sample
        self.down_delay[self.down_pos] = samples[0];
        self.down_pos = (self.down_pos + 1) % HALFBAND_DELAY_LEN;

        // Second sample
        self.down_delay[self.down_pos] = samples[1];

        // Compute filtered output (keeping every other sample)
        let pos = self.down_pos;
        let mut output = self.down_delay[pos] * 0.5;
        for k in 0..HALFBAND_HALF_LEN {
            let idx_neg = (pos + HALFBAND_DELAY_LEN - (2 * k + 1)) % HALFBAND_DELAY_LEN;
            let idx_pos = (pos + 2 * k + 1) % HALFBAND_DELAY_LEN;
            output += HALFBAND_COEFFS[k] * (self.down_delay[idx_neg] + self.down_delay[idx_pos]);
        }

        self.down_pos = (self.down_pos + 1) % HALFBAND_DELAY_LEN;

        output
    }
}

/// Stereo oversampler for one band (handles L+R)
#[derive(Clone)]
pub struct StereoOversampler2x {
    left: Oversampler2x,
    right: Oversampler2x,
}

impl StereoOversampler2x {
    pub fn new() -> Self {
        Self {
            left: Oversampler2x::new(),
            right: Oversampler2x::new(),
        }
    }

    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }

    #[inline(always)]
    pub fn upsample(&mut self, l: f64, r: f64) -> ([f64; 2], [f64; 2]) {
        (self.left.upsample(l), self.right.upsample(r))
    }

    #[inline(always)]
    pub fn downsample(&mut self, l_samples: [f64; 2], r_samples: [f64; 2]) -> (f64, f64) {
        (self.left.downsample(l_samples), self.right.downsample(r_samples))
    }
}

/// Complete oversampling stage for one band
/// Supports 1x, 2x, and 4x oversampling
#[derive(Clone)]
pub struct BandOversampler {
    /// First 2x stage (used for 2x and 4x)
    stage1: StereoOversampler2x,
    /// Second 2x stage (used only for 4x)
    stage2_a: StereoOversampler2x,
    stage2_b: StereoOversampler2x,
    /// Current factor
    factor: OversamplingFactor,
}

impl BandOversampler {
    pub fn new() -> Self {
        Self {
            stage1: StereoOversampler2x::new(),
            stage2_a: StereoOversampler2x::new(),
            stage2_b: StereoOversampler2x::new(),
            factor: OversamplingFactor::X1,
        }
    }

    pub fn reset(&mut self) {
        self.stage1.reset();
        self.stage2_a.reset();
        self.stage2_b.reset();
    }

    pub fn set_factor(&mut self, factor: OversamplingFactor) {
        if self.factor != factor {
            self.factor = factor;
            self.reset();
        }
    }

    pub fn factor(&self) -> OversamplingFactor {
        self.factor
    }

    /// Process a stereo sample pair through the oversampled callback.
    ///
    /// The callback `f` processes a single stereo sample at the oversampled rate
    /// and returns the processed stereo pair.
    ///
    /// This function:
    /// 1. Upsamples to the target rate
    /// 2. Calls `f` for each oversampled sample
    /// 3. Downsamples back to the original rate
    #[inline(always)]
    pub fn process<F>(&mut self, left: f64, right: f64, mut f: F) -> (f64, f64)
    where
        F: FnMut(f64, f64) -> (f64, f64),
    {
        match self.factor {
            OversamplingFactor::X1 => {
                // No oversampling — direct processing
                f(left, right)
            }
            OversamplingFactor::X2 => {
                // 2x: upsample, process 2 samples, downsample
                let (up_l, up_r) = self.stage1.upsample(left, right);

                let (proc_l0, proc_r0) = f(up_l[0], up_r[0]);
                let (proc_l1, proc_r1) = f(up_l[1], up_r[1]);

                self.stage1.downsample([proc_l0, proc_l1], [proc_r0, proc_r1])
            }
            OversamplingFactor::X4 => {
                // 4x: cascade two 2x stages
                // First upsample 1x -> 2x
                let (up2_l, up2_r) = self.stage1.upsample(left, right);

                // Second upsample each 2x sample -> 4x (2 samples each)
                let (up4_l0, up4_r0) = self.stage2_a.upsample(up2_l[0], up2_r[0]);
                let (up4_l1, up4_r1) = self.stage2_b.upsample(up2_l[1], up2_r[1]);

                // Process all 4 samples
                let (p_l0, p_r0) = f(up4_l0[0], up4_r0[0]);
                let (p_l1, p_r1) = f(up4_l0[1], up4_r0[1]);
                let (p_l2, p_r2) = f(up4_l1[0], up4_r1[0]);
                let (p_l3, p_r3) = f(up4_l1[1], up4_r1[1]);

                // Downsample 4x -> 2x
                let (d2_l0, d2_r0) = self.stage2_a.downsample([p_l0, p_l1], [p_r0, p_r1]);
                let (d2_l1, d2_r1) = self.stage2_b.downsample([p_l2, p_l3], [p_r2, p_r3]);

                // Downsample 2x -> 1x
                self.stage1.downsample([d2_l0, d2_l1], [d2_r0, d2_r1])
            }
        }
    }
}

/// 4-band oversampler (one per band)
pub struct MultibandOversampler {
    pub bands: [BandOversampler; 4],
}

impl MultibandOversampler {
    pub fn new() -> Self {
        Self {
            bands: [
                BandOversampler::new(),
                BandOversampler::new(),
                BandOversampler::new(),
                BandOversampler::new(),
            ],
        }
    }

    pub fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
    }

    pub fn set_factor(&mut self, factor: OversamplingFactor) {
        for band in &mut self.bands {
            band.set_factor(factor);
        }
    }
}
