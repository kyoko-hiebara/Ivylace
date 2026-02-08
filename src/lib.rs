use nih_plug::prelude::*;
use nih_plug::formatters;
use nih_plug_vizia::ViziaState;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

mod dsp;
mod editor;
use dsp::compressor::{SslCompressor, ATTACK_TIMES_MS, RATIOS, RELEASE_TIMES_MS};
use dsp::crossover::FourBandCrossover;
use dsp::gr_meter::{GrMeter, GrMeterOutputs};
use dsp::oversampling::{MultibandOversampler, OversamplingFactor};
use dsp::saturation::{MultibandSaturation, SaturationType};
use dsp::spectrum::SpectrumBuffer;

pub(crate) const NUM_BANDS: usize = 4;
const BAND_NAMES: [&str; NUM_BANDS] = ["Low", "LowMid", "HighMid", "High"];

/// GR meter update interval (every N samples) to reduce atomic store overhead
const GR_METER_UPDATE_INTERVAL: u32 = 64;

// ============================================================
//  Plugin struct
// ============================================================

struct Ivylace {
    params: Arc<IvylaceParams>,
    crossover: FourBandCrossover,
    compressors: [SslCompressor; NUM_BANDS],
    saturation: MultibandSaturation,
    oversampler: MultibandOversampler,
    gr_meters: [GrMeter; NUM_BANDS],
    gr_meter_outputs: Arc<GrMeterOutputs>,
    /// Spectrum buffer (pre-processing) for lock-free audio→GUI FFT data
    spectrum_pre: Arc<SpectrumBuffer>,
    /// Spectrum buffer (post-processing) for lock-free audio→GUI FFT data
    spectrum_post: Arc<SpectrumBuffer>,
    /// Current sample rate (for spectrum display)
    current_sample_rate: f32,
    /// Sample counter for throttled GR meter output updates
    gr_update_counter: u32,
    /// Whether the host is currently rendering (offline bounce)
    is_rendering: bool,
}

// ============================================================
//  Parameters
// ============================================================

#[derive(Params)]
pub struct IvylaceParams {
    /// Editor state (window size/scale persistence)
    #[persist = "editor-state"]
    pub(crate) editor_state: Arc<ViziaState>,

    /// Glass theme mode (persisted but not a DAW parameter)
    #[persist = "glass-mode"]
    pub(crate) glass_mode: Arc<AtomicBool>,

    // --- Global ---
    #[id = "input_gain"]
    pub input_gain: FloatParam,
    #[id = "output_gain"]
    pub output_gain: FloatParam,
    #[id = "dry_wet"]
    pub dry_wet: FloatParam,

    // --- Crossover frequencies ---
    #[id = "xover_low"]
    pub xover_low: FloatParam,
    #[id = "xover_mid"]
    pub xover_mid: FloatParam,
    #[id = "xover_high"]
    pub xover_high: FloatParam,

    // --- Per-band compressor params ---
    #[nested(array, group = "Band")]
    pub bands: [BandParams; NUM_BANDS],

    // --- Saturation global ---
    #[id = "sat_type"]
    pub sat_type: EnumParam<SatTypeParam>,

    // --- Per-band saturation ---
    #[nested(array, group = "Saturation")]
    pub sat_bands: [SatBandParams; NUM_BANDS],

    // --- Oversampling ---
    #[id = "os_realtime"]
    pub os_realtime: EnumParam<OversamplingParam>,
    #[id = "os_render"]
    pub os_render: EnumParam<OversamplingParam>,
}

#[derive(Params)]
pub struct BandParams {
    #[id = "threshold"]
    pub threshold: FloatParam,
    #[id = "ratio"]
    pub ratio: EnumParam<RatioParam>,
    #[id = "attack"]
    pub attack: EnumParam<AttackParam>,
    #[id = "release"]
    pub release: EnumParam<ReleaseParam>,
    #[id = "makeup"]
    pub makeup: FloatParam,
    #[id = "sc_hpf"]
    pub sc_hpf: FloatParam,
    #[id = "enabled"]
    pub enabled: BoolParam,
    #[id = "solo"]
    pub solo: BoolParam,
    #[id = "link"]
    pub link: BoolParam,
}

#[derive(Params)]
pub struct SatBandParams {
    #[id = "sat_drive"]
    pub drive: FloatParam,
    #[id = "sat_enabled"]
    pub enabled: BoolParam,
}

// --- Enum parameters ---

#[derive(Enum, PartialEq)]
pub(crate) enum RatioParam {
    #[name = "2:1"]
    Ratio2,
    #[name = "4:1"]
    Ratio4,
    #[name = "10:1"]
    Ratio10,
}

#[derive(Enum, PartialEq)]
pub(crate) enum AttackParam {
    #[name = "0.1"]
    A01,
    #[name = "0.3"]
    A03,
    #[name = "1"]
    A1,
    #[name = "3"]
    A3,
    #[name = "10"]
    A10,
    #[name = "30"]
    A30,
}

#[derive(Enum, PartialEq)]
pub(crate) enum ReleaseParam {
    #[name = "100"]
    R100,
    #[name = "300"]
    R300,
    #[name = "600"]
    R600,
    #[name = "1200"]
    R1200,
    #[name = "Auto"]
    Auto,
}

#[derive(Enum, PartialEq)]
pub(crate) enum SatTypeParam {
    #[name = "Console"]
    Console,
    #[name = "Tube"]
    Tube,
    #[name = "Tape"]
    Tape,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub(crate) enum OversamplingParam {
    #[name = "1x"]
    X1,
    #[name = "2x"]
    X2,
    #[name = "4x"]
    X4,
}

impl OversamplingParam {
    fn to_factor(self) -> OversamplingFactor {
        match self {
            OversamplingParam::X1 => OversamplingFactor::X1,
            OversamplingParam::X2 => OversamplingFactor::X2,
            OversamplingParam::X4 => OversamplingFactor::X4,
        }
    }
}

// ============================================================
//  Parameter construction helpers
// ============================================================

fn make_band_params(band_idx: usize) -> BandParams {
    let name = BAND_NAMES[band_idx];

    BandParams {
        threshold: FloatParam::new(
            format!("{name} Threshold"),
            -20.0,
            FloatRange::Linear {
                min: -40.0,
                max: 0.0,
            },
        )
        .with_unit(" dB")
        .with_step_size(0.1),

        ratio: EnumParam::new(format!("{name} Ratio"), RatioParam::Ratio4),

        attack: EnumParam::new(format!("{name} Attack"), AttackParam::A10),

        release: EnumParam::new(format!("{name} Release"), ReleaseParam::R300),

        makeup: FloatParam::new(
            format!("{name} Makeup"),
            0.0,
            FloatRange::Linear {
                min: -12.0,
                max: 24.0,
            },
        )
        .with_unit(" dB")
        .with_step_size(0.1),

        sc_hpf: FloatParam::new(
            format!("{name} SC HPF"),
            if band_idx == 0 { 0.0 } else { 80.0 },
            FloatRange::Skewed {
                min: 0.0,
                max: 500.0,
                factor: FloatRange::skew_factor(-1.5),
            },
        )
        .with_unit(" Hz")
        .with_step_size(1.0),

        enabled: BoolParam::new(format!("{name} Comp Enabled"), true),

        solo: BoolParam::new(format!("{name} Solo"), false),

        link: BoolParam::new(format!("{name} Link"), false),
    }
}

fn make_sat_band_params(band_idx: usize) -> SatBandParams {
    let name = BAND_NAMES[band_idx];

    SatBandParams {
        drive: FloatParam::new(
            format!("{name} Sat Drive"),
            if band_idx == 0 { 0.0 } else { 0.3 },
            FloatRange::Linear {
                min: 0.0,
                max: 1.0,
            },
        )
        .with_unit("")
        .with_step_size(0.01),

        // Low band saturation OFF by default for EDM!
        enabled: BoolParam::new(format!("{name} Sat Enabled"), band_idx != 0),
    }
}

// ============================================================
//  Plugin implementation
// ============================================================

impl Default for Ivylace {
    fn default() -> Self {
        Self {
            params: Arc::new(IvylaceParams {
                editor_state: editor::default_state(),
                glass_mode: Arc::new(AtomicBool::new(false)),

                input_gain: FloatParam::new(
                    "Input Gain",
                    0.0,
                    FloatRange::Linear {
                        min: -24.0,
                        max: 24.0,
                    },
                )
                .with_unit(" dB")
                .with_step_size(0.1),

                output_gain: FloatParam::new(
                    "Output Gain",
                    0.0,
                    FloatRange::Linear {
                        min: -24.0,
                        max: 24.0,
                    },
                )
                .with_unit(" dB")
                .with_step_size(0.1),

                dry_wet: FloatParam::new(
                    "Dry/Wet",
                    1.0,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_unit(" %")
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),

                xover_low: FloatParam::new(
                    "Crossover Low",
                    120.0,
                    FloatRange::Skewed {
                        min: 20.0,
                        max: 500.0,
                        factor: FloatRange::skew_factor(-1.5),
                    },
                )
                .with_unit(" Hz")
                .with_step_size(1.0),

                xover_mid: FloatParam::new(
                    "Crossover Mid",
                    1500.0,
                    FloatRange::Skewed {
                        min: 200.0,
                        max: 5000.0,
                        factor: FloatRange::skew_factor(-1.0),
                    },
                )
                .with_unit(" Hz")
                .with_step_size(1.0),

                xover_high: FloatParam::new(
                    "Crossover High",
                    8000.0,
                    FloatRange::Skewed {
                        min: 2000.0,
                        max: 18000.0,
                        factor: FloatRange::skew_factor(-1.0),
                    },
                )
                .with_unit(" Hz")
                .with_step_size(1.0),

                bands: [
                    make_band_params(0),
                    make_band_params(1),
                    make_band_params(2),
                    make_band_params(3),
                ],

                sat_type: EnumParam::new("Saturation Type", SatTypeParam::Console),

                sat_bands: [
                    make_sat_band_params(0),
                    make_sat_band_params(1),
                    make_sat_band_params(2),
                    make_sat_band_params(3),
                ],

                os_realtime: EnumParam::new("OS Realtime", OversamplingParam::X1),
                os_render: EnumParam::new("OS Render", OversamplingParam::X4),
            }),
            crossover: FourBandCrossover::new(44100.0),
            compressors: [
                SslCompressor::new(44100.0),
                SslCompressor::new(44100.0),
                SslCompressor::new(44100.0),
                SslCompressor::new(44100.0),
            ],
            saturation: MultibandSaturation::new(),
            oversampler: MultibandOversampler::new(),
            gr_meters: [
                GrMeter::new(44100.0),
                GrMeter::new(44100.0),
                GrMeter::new(44100.0),
                GrMeter::new(44100.0),
            ],
            gr_meter_outputs: Arc::new(GrMeterOutputs::new()),
            spectrum_pre: Arc::new(SpectrumBuffer::new()),
            spectrum_post: Arc::new(SpectrumBuffer::new()),
            current_sample_rate: 44100.0,
            gr_update_counter: 0,
            is_rendering: false,
        }
    }
}

impl Ivylace {
    /// Get the shared GR meter outputs (for GUI access)
    pub fn gr_meter_outputs(&self) -> Arc<GrMeterOutputs> {
        self.gr_meter_outputs.clone()
    }
}

impl Plugin for Ivylace {
    const NAME: &'static str = "Ivylace";
    const VENDOR: &'static str = "きょーこ";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.gr_meter_outputs.clone(),
            self.spectrum_pre.clone(),
            self.spectrum_post.clone(),
            self.current_sample_rate,
            self.params.editor_state.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let sr = buffer_config.sample_rate as f64;
        self.current_sample_rate = buffer_config.sample_rate;
        self.crossover.set_sample_rate(sr);
        for comp in &mut self.compressors {
            comp.set_sample_rate(sr);
        }
        for meter in &mut self.gr_meters {
            meter.set_sample_rate(sr);
        }
        self.is_rendering = buffer_config.process_mode == ProcessMode::Offline;
        true
    }

    fn reset(&mut self) {
        self.crossover.reset();
        for comp in &mut self.compressors {
            comp.reset();
        }
        self.saturation.reset();
        self.oversampler.reset();
        for meter in &mut self.gr_meters {
            meter.reset();
        }
        self.gr_update_counter = 0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Select oversampling factor based on realtime vs render
        // is_rendering is set in initialize() from buffer_config.process_mode
        let os_factor = if self.is_rendering {
            self.params.os_render.value().to_factor()
        } else {
            self.params.os_realtime.value().to_factor()
        };
        self.oversampler.set_factor(os_factor);

        // Read parameters
        let input_gain_db = self.params.input_gain.smoothed.next();
        let output_gain_db = self.params.output_gain.smoothed.next();
        let dry_wet = self.params.dry_wet.smoothed.next();

        let input_gain = 10.0_f64.powf(input_gain_db as f64 / 20.0);
        let output_gain = 10.0_f64.powf(output_gain_db as f64 / 20.0);

        // Update crossover frequencies
        let xover_freqs = [
            self.params.xover_low.smoothed.next() as f64,
            self.params.xover_mid.smoothed.next() as f64,
            self.params.xover_high.smoothed.next() as f64,
        ];
        self.crossover.set_frequencies(xover_freqs);

        // Update compressor parameters per band
        for (i, band_params) in self.params.bands.iter().enumerate() {
            let comp = &mut self.compressors[i];
            comp.set_threshold(band_params.threshold.smoothed.next() as f64);
            comp.set_ratio(RATIOS[band_params.ratio.value() as usize]);
            comp.set_attack_ms(ATTACK_TIMES_MS[band_params.attack.value() as usize]);
            comp.set_release_ms(RELEASE_TIMES_MS[band_params.release.value() as usize]);
            comp.set_makeup_db(band_params.makeup.smoothed.next() as f64);
            comp.set_sidechain_hpf(band_params.sc_hpf.smoothed.next() as f64);
            comp.set_enabled(band_params.enabled.value());
        }

        // Update saturation parameters
        let sat_type = match self.params.sat_type.value() {
            SatTypeParam::Console => SaturationType::Console,
            SatTypeParam::Tube => SaturationType::Tube,
            SatTypeParam::Tape => SaturationType::Tape,
        };
        self.saturation.set_global_type(sat_type);

        for (i, sat_params) in self.params.sat_bands.iter().enumerate() {
            self.saturation.bands[i].set_drive(sat_params.drive.smoothed.next() as f64);
            self.saturation.bands[i].set_enabled(sat_params.enabled.value());
        }

        // Check for solo
        let any_solo = self.params.bands.iter().any(|b| b.solo.value());

        // Process audio
        for mut channel_samples in buffer.iter_samples() {
            let left_in = *channel_samples.get_mut(0).unwrap() as f64 * input_gain;
            let right_in = *channel_samples.get_mut(1).unwrap() as f64 * input_gain;

            // Store dry signal
            let dry_l = left_in;
            let dry_r = right_in;

            // Feed pre-processing spectrum buffer (mono sum, only when GUI is open)
            if self.params.editor_state.is_open() {
                self.spectrum_pre.push((left_in as f32 + right_in as f32) * 0.5);
            }

            // Split into 4 bands
            let (bands_l, bands_r) = self.crossover.process(left_in, right_in);

            // Process each band through oversampled compressor + saturation
            let mut out_l = 0.0f64;
            let mut out_r = 0.0f64;

            // Destructure to avoid borrow checker issues with multiple mutable borrows
            let compressors = &mut self.compressors;
            let sat_bands = &mut self.saturation.bands;
            let os_bands = &mut self.oversampler.bands;
            let gr_meters = &mut self.gr_meters;

            for i in 0..NUM_BANDS {
                // Process through oversampler (wraps compressor + saturation)
                let comp = &mut compressors[i];
                let sat = &mut sat_bands[i];

                let (proc_l, proc_r) = os_bands[i].process(
                    bands_l[i],
                    bands_r[i],
                    |l, r| {
                        let (cl, cr) = comp.process(l, r);
                        (sat.process(cl), sat.process(cr))
                    },
                );

                // Feed GR meter (from compressor's current gain reduction)
                gr_meters[i].push(comp.gain_reduction_db());

                // Solo logic
                let should_output = if any_solo {
                    self.params.bands[i].solo.value()
                } else {
                    true
                };

                if should_output {
                    out_l += proc_l;
                    out_r += proc_r;
                }
            }

            // Dry/wet mix
            let final_l = dry_l * (1.0 - dry_wet as f64) + out_l * dry_wet as f64;
            let final_r = dry_r * (1.0 - dry_wet as f64) + out_r * dry_wet as f64;

            // Output gain
            let out_sample_l = (final_l * output_gain) as f32;
            let out_sample_r = (final_r * output_gain) as f32;
            *channel_samples.get_mut(0).unwrap() = out_sample_l;
            *channel_samples.get_mut(1).unwrap() = out_sample_r;

            // Feed post-processing spectrum buffer (mono sum, only when GUI is open)
            if self.params.editor_state.is_open() {
                self.spectrum_post.push((out_sample_l + out_sample_r) * 0.5);
            }

            // Update GR meter outputs at reduced rate
            self.gr_update_counter += 1;
            if self.gr_update_counter >= GR_METER_UPDATE_INTERVAL {
                self.gr_update_counter = 0;
                self.gr_meter_outputs.update_from_meters(&self.gr_meters);
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Ivylace {
    const CLAP_ID: &'static str = "com.kyoko.ivylace";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Multiband glue compressor with per-band analog saturation");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Compressor,
        ClapFeature::Mastering,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for Ivylace {
    const VST3_CLASS_ID: [u8; 16] = *b"IvylaceKyoko0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Dynamics,
        Vst3SubCategory::Mastering,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_clap!(Ivylace);
nih_export_vst3!(Ivylace);
