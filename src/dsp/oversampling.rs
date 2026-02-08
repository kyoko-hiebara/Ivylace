/// Oversampling module for anti-aliased saturation and compression
///
/// Uses a simple FIR lowpass filter for 2x oversampling.
/// 4x is achieved by cascading two 2x stages.
///
/// The approach is straightforward:
/// - Upsample: zero-stuff → FIR lowpass filter (gain-of-2 to compensate)
/// - Downsample: FIR lowpass filter → decimate (keep every other sample)
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

impl OversamplingFactor {
    /// Returns the integer multiplier (1, 2, or 4).
    pub fn multiplier(self) -> u32 {
        match self {
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X4 => 4,
        }
    }
}

/// FIR lowpass filter for 2x oversampling.
///
/// 15-tap symmetric half-band FIR. At the 2x rate, the cutoff is at
/// 0.25 * fs_2x = 0.5 * fs_original (i.e., original Nyquist).
///
/// Full impulse response (symmetric, 15 taps, indices -7..+7):
///   h[n] for n = 0..14, center at n=7
///
/// Designed with windowed-sinc (Kaiser, beta=4.5) then normalized for
/// unity DC gain at the 2x rate.
const FIR_TAPS: usize = 15;
const FIR_HALF: usize = 7; // (FIR_TAPS - 1) / 2

/// The 15-tap FIR coefficients (symmetric: coeff[k] == coeff[14-k]).
/// Normalized so sum = 1.0 (unity DC gain). The *2 gain for zero-stuffing
/// compensation is applied to the input in the upsampler, not the filter.
const FIR_COEFFS: [f64; FIR_TAPS] = {
    // Start from windowed sinc for half-band lowpass (cutoff = π/2)
    // h_ideal[n] = sin(π/2 * (n - 7)) / (π * (n - 7)) * kaiser(n, 15, 4.5)
    // Then normalize sum to 0.5.
    //
    // Precomputed:
    [
        -0.002_077_586_698_498_6,  // n=0
         0.0,                       // n=1
         0.017_726_580_009_717_6,  // n=2
         0.0,                       // n=3
        -0.069_570_697_498_560_9,  // n=4
         0.0,                       // n=5
         0.303_921_704_187_342_0,  // n=6
         0.500_000_000_000_000_0,  // n=7 (center tap)
         0.303_921_704_187_342_0,  // n=8
         0.0,                       // n=9
        -0.069_570_697_498_560_9,  // n=10
         0.0,                       // n=11
         0.017_726_580_009_717_6,  // n=12
         0.0,                       // n=13
        -0.002_077_586_698_498_6,  // n=14
    ]
};

/// 2x-rate delay line length for the FIR filter
const DELAY_2X: usize = FIR_TAPS;

/// Single-channel 2x oversampler.
///
/// Upsample path: input → zero-stuff → FIR filter (*2) → 2x output
/// Downsample path: 2x input → FIR filter → decimate → 1x output
#[derive(Clone)]
pub struct Oversampler2x {
    /// Circular buffer for the upsampling FIR (at 2x rate)
    up_buf: [f64; DELAY_2X],
    up_pos: usize,
    /// Circular buffer for the downsampling FIR (at 2x rate)
    down_buf: [f64; DELAY_2X],
    down_pos: usize,
    /// Toggle for zero-stuffing: true = insert real sample, false = insert zero
    up_phase: bool,
}

impl Oversampler2x {
    pub fn new() -> Self {
        Self {
            up_buf: [0.0; DELAY_2X],
            up_pos: 0,
            down_buf: [0.0; DELAY_2X],
            down_pos: 0,
            up_phase: true,
        }
    }

    pub fn reset(&mut self) {
        self.up_buf = [0.0; DELAY_2X];
        self.up_pos = 0;
        self.down_buf = [0.0; DELAY_2X];
        self.down_pos = 0;
        self.up_phase = true;
    }

    /// Push one sample into the 2x-rate upsampling buffer and compute filtered output.
    #[inline(always)]
    fn up_push_and_filter(&mut self, sample: f64) -> f64 {
        self.up_buf[self.up_pos] = sample;
        // Convolve with FIR
        let mut out = 0.0;
        let mut idx = self.up_pos;
        for k in 0..FIR_TAPS {
            out += self.up_buf[idx] * FIR_COEFFS[k];
            if idx == 0 { idx = DELAY_2X - 1; } else { idx -= 1; }
        }
        self.up_pos = (self.up_pos + 1) % DELAY_2X;
        out
    }

    /// Upsample: produce 2 output samples from 1 input sample.
    /// Returns [sample_0, sample_1] at 2x rate.
    #[inline(always)]
    pub fn upsample(&mut self, input: f64) -> [f64; 2] {
        // Zero-stuffing: insert input, then zero (or vice versa).
        // The *2.0 compensates for the energy loss from zero-stuffing.
        let s0 = self.up_push_and_filter(input * 2.0);
        let s1 = self.up_push_and_filter(0.0);
        [s0, s1]
    }

    /// Push one sample into the downsampling buffer and compute filtered output.
    #[inline(always)]
    fn down_push_and_filter(&mut self, sample: f64) -> f64 {
        self.down_buf[self.down_pos] = sample;
        let mut out = 0.0;
        let mut idx = self.down_pos;
        for k in 0..FIR_TAPS {
            out += self.down_buf[idx] * FIR_COEFFS[k];
            if idx == 0 { idx = DELAY_2X - 1; } else { idx -= 1; }
        }
        self.down_pos = (self.down_pos + 1) % DELAY_2X;
        out
    }

    /// Downsample: take 2 input samples (at 2x rate) and produce 1 output sample.
    #[inline(always)]
    pub fn downsample(&mut self, samples: [f64; 2]) -> f64 {
        // Filter both samples, but only keep every other output (decimation)
        let _discard = self.down_push_and_filter(samples[0]);
        let output = self.down_push_and_filter(samples[1]);
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
    #[inline(always)]
    pub fn process<F>(&mut self, left: f64, right: f64, mut f: F) -> (f64, f64)
    where
        F: FnMut(f64, f64) -> (f64, f64),
    {
        match self.factor {
            OversamplingFactor::X1 => {
                f(left, right)
            }
            OversamplingFactor::X2 => {
                let (up_l, up_r) = self.stage1.upsample(left, right);

                let (proc_l0, proc_r0) = f(up_l[0], up_r[0]);
                let (proc_l1, proc_r1) = f(up_l[1], up_r[1]);

                self.stage1.downsample([proc_l0, proc_l1], [proc_r0, proc_r1])
            }
            OversamplingFactor::X4 => {
                let (up2_l, up2_r) = self.stage1.upsample(left, right);

                let (up4_l0, up4_r0) = self.stage2_a.upsample(up2_l[0], up2_r[0]);
                let (up4_l1, up4_r1) = self.stage2_b.upsample(up2_l[1], up2_r[1]);

                let (p_l0, p_r0) = f(up4_l0[0], up4_r0[0]);
                let (p_l1, p_r1) = f(up4_l0[1], up4_r0[1]);
                let (p_l2, p_r2) = f(up4_l1[0], up4_r1[0]);
                let (p_l3, p_r3) = f(up4_l1[1], up4_r1[1]);

                let (d2_l0, d2_r0) = self.stage2_a.downsample([p_l0, p_l1], [p_r0, p_r1]);
                let (d2_l1, d2_r1) = self.stage2_b.downsample([p_l2, p_l3], [p_r2, p_r3]);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fir_coeffs_sum() {
        let sum: f64 = FIR_COEFFS.iter().sum();
        let error = (sum - 1.0).abs();
        assert!(error < 1e-10, "FIR sum should be 1.0, got {}", sum);
    }

    #[test]
    fn test_2x_roundtrip_dc_unity() {
        let mut os = Oversampler2x::new();
        let dc_value = 0.5;
        let mut last_output = 0.0;
        for _ in 0..200 {
            let up = os.upsample(dc_value);
            last_output = os.downsample(up);
        }
        let error = (last_output - dc_value).abs();
        assert!(
            error < 0.001,
            "DC roundtrip error: input={}, output={}, error={}",
            dc_value, last_output, error
        );
    }

    #[test]
    fn test_2x_upsample_dc_level() {
        let mut os = Oversampler2x::new();
        let dc_value = 1.0;
        let mut last_s0 = 0.0;
        let mut last_s1 = 0.0;
        for _ in 0..200 {
            let [s0, s1] = os.upsample(dc_value);
            last_s0 = s0;
            last_s1 = s1;
        }
        // Both phases should be close to DC value at steady state
        let error_s0 = (last_s0 - dc_value).abs();
        let error_s1 = (last_s1 - dc_value).abs();
        assert!(
            error_s0 < 0.01,
            "Phase 0 DC error: expected={}, got={}, error={}",
            dc_value, last_s0, error_s0
        );
        assert!(
            error_s1 < 0.01,
            "Phase 1 DC error: expected={}, got={}, error={}",
            dc_value, last_s1, error_s1
        );
    }

    #[test]
    fn test_4x_roundtrip_dc_unity() {
        let mut band = BandOversampler::new();
        band.set_factor(OversamplingFactor::X4);
        let dc_value = 0.5;
        let mut last_l = 0.0;
        for _ in 0..500 {
            let (l, _r) = band.process(dc_value, dc_value, |l, r| (l, r));
            last_l = l;
        }
        let error = (last_l - dc_value).abs();
        assert!(
            error < 0.002,
            "4x DC roundtrip error: input={}, output={}, error={}",
            dc_value, last_l, error
        );
    }

    /// Test that a low-frequency sine wave passes through 2x at near unity gain.
    #[test]
    fn test_2x_roundtrip_sine_1k() {
        let mut os = Oversampler2x::new();
        let sr = 44100.0_f64;
        let freq = 1000.0_f64;
        let mut max_in = 0.0_f64;
        let mut max_out = 0.0_f64;

        // Warmup
        for i in 0..1000 {
            let t = i as f64 / sr;
            let input = (2.0 * std::f64::consts::PI * freq * t).sin();
            let up = os.upsample(input);
            let _out = os.downsample(up);
        }
        // Measure
        for i in 1000..2000 {
            let t = i as f64 / sr;
            let input = (2.0 * std::f64::consts::PI * freq * t).sin();
            let up = os.upsample(input);
            let out = os.downsample(up);
            max_in = max_in.max(input.abs());
            max_out = max_out.max(out.abs());
        }

        let gain_db = 20.0 * (max_out / max_in).log10();
        assert!(
            gain_db.abs() < 0.5,
            "1kHz sine roundtrip gain: {:.2} dB (should be ~0 dB)",
            gain_db
        );
    }
}
