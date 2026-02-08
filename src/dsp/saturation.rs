/// Analog Saturation / Harmonic Exciter
///
/// Models the nonlinear characteristics of analog circuits:
/// - Even harmonics (tube-like warmth) via asymmetric soft clipping
/// - Odd harmonics (tape-like grit) via symmetric saturation
/// - Per-band drive control with explicit LOW BAND BYPASS for EDM production
///
/// The saturation is designed to be subtle at low drive settings (console coloring)
/// and more aggressive at higher settings (tape saturation territory).
///
/// For EDM production:
/// - Sub-bass (< 120Hz) benefits from staying CLEAN to maintain punch and translation
/// - Low-mids can take light saturation for warmth
/// - High-mids and highs benefit most from harmonic enhancement

/// Saturation model type
#[derive(Clone, Copy, PartialEq)]
pub enum SaturationType {
    /// Tube-like: primarily even harmonics, warm and musical
    /// Models the transfer function of a triode tube stage
    Tube,
    /// Tape-like: primarily odd harmonics with soft compression
    /// Models magnetic tape saturation characteristics
    Tape,
    /// Console channel: very subtle, mostly even harmonics
    /// Models the transformer and op-amp coloring of a large-format console
    Console,
}

impl Default for SaturationType {
    fn default() -> Self {
        Self::Console
    }
}

/// Per-band saturation processor
#[derive(Clone)]
pub struct BandSaturation {
    /// Drive amount (0.0 = bypass, 1.0 = full saturation)
    drive: f64,
    /// Saturation type
    sat_type: SaturationType,
    /// Output level compensation (auto-gain)
    compensation: f64,
    /// Whether this band's saturation is enabled
    enabled: bool,
    /// DC blocker state (remove DC offset from asymmetric clipping)
    dc_blocker_x1: f64,
    dc_blocker_y1: f64,
    /// DC blocker coefficient: R = exp(-2π * fc / sr), fc = 5Hz
    dc_blocker_r: f64,
}

/// DC blocker cutoff frequency in Hz
const DC_BLOCKER_FC: f64 = 5.0;

impl BandSaturation {
    pub fn new() -> Self {
        Self {
            drive: 0.0,
            sat_type: SaturationType::Console,
            compensation: 1.0,
            enabled: true,
            dc_blocker_x1: 0.0,
            dc_blocker_y1: 0.0,
            dc_blocker_r: Self::compute_dc_blocker_r(44100.0),
        }
    }

    pub fn reset(&mut self) {
        self.dc_blocker_x1 = 0.0;
        self.dc_blocker_y1 = 0.0;
    }

    /// Update the DC blocker coefficient for the given sample rate.
    /// Must be called when sample rate changes (including oversampling).
    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        self.dc_blocker_r = Self::compute_dc_blocker_r(sample_rate);
    }

    /// Compute DC blocker R coefficient: R = exp(-2π * fc / sr)
    #[inline(always)]
    fn compute_dc_blocker_r(sample_rate: f64) -> f64 {
        (-2.0 * std::f64::consts::PI * DC_BLOCKER_FC / sample_rate).exp()
    }

    pub fn set_drive(&mut self, drive: f64) {
        self.drive = drive.clamp(0.0, 1.0);
        self.update_compensation();
    }

    pub fn set_type(&mut self, sat_type: SaturationType) {
        self.sat_type = sat_type;
        self.update_compensation();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn update_compensation(&mut self) {
        // Auto-gain compensation based on drive and type
        // Higher drive = more level increase = more compensation needed
        let base = match self.sat_type {
            SaturationType::Tube => 1.0 + self.drive * 0.3,
            SaturationType::Tape => 1.0 + self.drive * 0.2,
            SaturationType::Console => 1.0 + self.drive * 0.1,
        };
        self.compensation = 1.0 / base;
    }

    /// DC blocker (1st order highpass at ~5Hz, sample-rate aware)
    #[inline(always)]
    fn dc_block(&mut self, input: f64) -> f64 {
        let r = self.dc_blocker_r;
        let output = input - self.dc_blocker_x1 + r * self.dc_blocker_y1;
        self.dc_blocker_x1 = input;
        self.dc_blocker_y1 = output;
        output
    }

    /// Tube-style saturation (asymmetric, even harmonics)
    /// Based on a simplified triode transfer curve
    #[inline(always)]
    fn saturate_tube(x: f64, drive: f64) -> f64 {
        let input = x * (1.0 + drive * 3.0);

        // Asymmetric waveshaper:
        // Positive half: soft clip with tube-like curve
        // Negative half: harder clip (cathode cutoff)
        if input >= 0.0 {
            // Warm positive saturation
            let t = input.min(4.0);
            t * (27.0 + t * t) / (27.0 + 9.0 * t * t)
        } else {
            // Sharper negative clip (adds even harmonics through asymmetry)
            let t = (-input).min(4.0);
            -(t / (1.0 + t * 0.7))
        }
    }

    /// Tape-style saturation (symmetric, odd harmonics + compression)
    /// Based on the Langevin function approximation for magnetic hysteresis
    #[inline(always)]
    fn saturate_tape(x: f64, drive: f64) -> f64 {
        let input = x * (1.0 + drive * 4.0);

        // Full tanh — classic tape saturation.
        // Provides symmetric soft clipping (odd harmonics)
        // with natural compression behavior at high levels.
        // Previously used a Padé approximation for |x|<1 but it caused
        // a ~0.1 value discontinuity at the crossover point |x|=1.
        input.tanh()
    }

    /// Console-style saturation (very subtle, transformer + op-amp coloring)
    #[inline(always)]
    fn saturate_console(x: f64, drive: f64) -> f64 {
        let input = x * (1.0 + drive * 1.5);

        // Subtle soft clipping - mostly linear with gentle nonlinearity
        // This is the "air" and "depth" that large-format consoles are known for
        let x2 = input * input;
        let x3 = x2 * input;

        // 3rd order polynomial with slight asymmetry
        input - 0.1667 * x3 * drive + 0.05 * x2 * drive
    }

    /// Process one sample
    #[inline(always)]
    pub fn process(&mut self, input: f64) -> f64 {
        if !self.enabled || self.drive <= 0.001 {
            return input;
        }

        let saturated = match self.sat_type {
            SaturationType::Tube => Self::saturate_tube(input, self.drive),
            SaturationType::Tape => Self::saturate_tape(input, self.drive),
            SaturationType::Console => Self::saturate_console(input, self.drive),
        };

        // Apply compensation (auto-gain to keep perceived level constant)
        let compensated = saturated * self.compensation;

        // DC blocker (especially important for tube mode)
        self.dc_block(compensated)
    }
}

/// 4-band saturation processor with per-band control
pub struct MultibandSaturation {
    pub bands: [BandSaturation; 4],
}

impl MultibandSaturation {
    pub fn new() -> Self {
        let mut bands = [
            BandSaturation::new(),
            BandSaturation::new(),
            BandSaturation::new(),
            BandSaturation::new(),
        ];

        // Default: Low band disabled (EDM-friendly!)
        bands[0].set_enabled(false);
        bands[0].set_drive(0.0);

        // Default drives for other bands (1.8dB/6 = 0.3, 2.4dB/6 = 0.4)
        bands[1].set_drive(0.3); // LowMid: 1.8dB — light warmth
        bands[2].set_drive(0.4); // HighMid: 2.4dB — moderate presence
        bands[3].set_drive(0.3); // High: 1.8dB — light air

        Self { bands }
    }

    pub fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
    }

    /// Update sample rate for all bands' DC blockers
    pub fn set_sample_rate(&mut self, sample_rate: f64) {
        for band in &mut self.bands {
            band.set_sample_rate(sample_rate);
        }
    }

    /// Set global saturation type for all bands
    pub fn set_global_type(&mut self, sat_type: SaturationType) {
        for band in &mut self.bands {
            band.set_type(sat_type);
        }
    }

    /// Process 4 bands of audio
    /// Input and output: [band0, band1, band2, band3]
    #[inline(always)]
    pub fn process(&mut self, bands_in: [f64; 4]) -> [f64; 4] {
        [
            self.bands[0].process(bands_in[0]),
            self.bands[1].process(bands_in[1]),
            self.bands[2].process(bands_in[2]),
            self.bands[3].process(bands_in[3]),
        ]
    }
}
