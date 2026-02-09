use nih_plug::prelude::*;
use nih_plug::formatters;
use nih_plug_vizia::ViziaState;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

mod dsp;
mod editor;
use dsp::compressor::{SslCompressor, ATTACK_TIMES_MS, RATIOS, RELEASE_TIMES_MS};
use dsp::crossover::FourBandCrossover;
use dsp::gr_meter::{AtomicF32, GrMeter, GrMeterOutputs};
use dsp::oversampling::{MultibandOversampler, OversamplingFactor};
use dsp::saturation::{MultibandSaturation, SaturationType};
use dsp::spectrum::SpectrumBuffer;
use dsp::trim_detect::TrimDetector;

pub(crate) const NUM_BANDS: usize = 4;
const BAND_NAMES: [&str; NUM_BANDS] = ["Low", "LowMid", "HighMid", "High"];

/// GR meter update interval (every N samples) to reduce atomic store overhead
const GR_METER_UPDATE_INTERVAL: u32 = 64;

// ============================================================
//  A/B Slot Storage — lock-free dual parameter slots
// ============================================================

/// One A/B slot storing all per-band compressor parameter values.
/// Stored in native units so audio thread can use values directly.
pub(crate) struct SlotStorage {
    /// Per-band threshold in dB (-40..0)
    pub threshold_db: [AtomicF32; NUM_BANDS],
    /// Per-band attack index (0..5, maps to ATTACK_TIMES_MS)
    pub attack_idx: [AtomicU32; NUM_BANDS],
    /// Per-band release index (0..4, maps to RELEASE_TIMES_MS)
    pub release_idx: [AtomicU32; NUM_BANDS],
    /// Per-band ratio index (0..2, maps to RATIOS)
    pub ratio_idx: [AtomicU32; NUM_BANDS],
    /// Per-band makeup gain in dB (-12..24)
    pub makeup_db: [AtomicF32; NUM_BANDS],
    /// Per-band SC HPF frequency in Hz (0..500)
    pub sc_hpf_hz: [AtomicF32; NUM_BANDS],
}

impl SlotStorage {
    /// Create with default compressor param values.
    pub fn new_with_defaults() -> Self {
        Self {
            threshold_db: [
                AtomicF32::new(-20.0), AtomicF32::new(-20.0),
                AtomicF32::new(-20.0), AtomicF32::new(-20.0),
            ],
            attack_idx: [
                AtomicU32::new(4), AtomicU32::new(4), // A10 = index 4
                AtomicU32::new(4), AtomicU32::new(4),
            ],
            release_idx: [
                AtomicU32::new(1), AtomicU32::new(1), // R300 = index 1
                AtomicU32::new(1), AtomicU32::new(1),
            ],
            ratio_idx: [
                AtomicU32::new(1), AtomicU32::new(1), // Ratio4 = index 1
                AtomicU32::new(1), AtomicU32::new(1),
            ],
            makeup_db: [
                AtomicF32::new(0.0), AtomicF32::new(0.0),
                AtomicF32::new(0.0), AtomicF32::new(0.0),
            ],
            sc_hpf_hz: [
                AtomicF32::new(0.0), AtomicF32::new(80.0),
                AtomicF32::new(80.0), AtomicF32::new(80.0),
            ],
        }
    }

    /// Snapshot current nih-plug parameter values into this slot.
    pub fn load_from_params(&self, params: &IvylaceParams) {
        for i in 0..NUM_BANDS {
            let bp = &params.bands[i];

            // threshold: Linear range -40..0 dB
            let thr_norm = bp.threshold.unmodulated_normalized_value();
            let thr_db = thr_norm * 40.0 - 40.0;
            self.threshold_db[i].store(thr_db);

            // Enum params: normalized → step index
            let atk_norm = bp.attack.unmodulated_normalized_value();
            self.attack_idx[i].store((atk_norm * 5.0).round() as u32, Ordering::Relaxed);

            let rel_norm = bp.release.unmodulated_normalized_value();
            self.release_idx[i].store((rel_norm * 4.0).round() as u32, Ordering::Relaxed);

            let rat_norm = bp.ratio.unmodulated_normalized_value();
            self.ratio_idx[i].store((rat_norm * 2.0).round() as u32, Ordering::Relaxed);

            // makeup: Linear range -12..24 dB
            let mkp_norm = bp.makeup.unmodulated_normalized_value();
            let mkp_db = mkp_norm * 36.0 - 12.0;
            self.makeup_db[i].store(mkp_db);

            // sc_hpf: Skewed range 0..500 Hz, factor = 2^(-1.5)
            // unnormalize: plain = norm^(1/factor) * (max-min) + min
            let hpf_norm = bp.sc_hpf.unmodulated_normalized_value();
            let skew_factor = 2.0f32.powf(-1.5); // same as FloatRange::skew_factor(-1.5)
            let hpf_hz = hpf_norm.powf(skew_factor.recip()) * 500.0;
            self.sc_hpf_hz[i].store(hpf_hz);
        }
    }

    /// Get threshold as normalized value (0..1) for a band.
    #[inline]
    pub fn threshold_normalized(&self, band: usize) -> f32 {
        // -40dB → 0.0, 0dB → 1.0
        (self.threshold_db[band].load() + 40.0) / 40.0
    }

    /// Get makeup as normalized value (0..1) for a band.
    #[inline]
    pub fn makeup_normalized(&self, band: usize) -> f32 {
        // -12dB → 0.0, 24dB → 1.0
        (self.makeup_db[band].load() + 12.0) / 36.0
    }

    /// Get attack as normalized value for a band.
    #[inline]
    pub fn attack_normalized(&self, band: usize) -> f32 {
        self.attack_idx[band].load(Ordering::Relaxed) as f32 / 5.0
    }

    /// Get release as normalized value for a band.
    #[inline]
    pub fn release_normalized(&self, band: usize) -> f32 {
        self.release_idx[band].load(Ordering::Relaxed) as f32 / 4.0
    }

    /// Get ratio as normalized value for a band.
    #[inline]
    pub fn ratio_normalized(&self, band: usize) -> f32 {
        self.ratio_idx[band].load(Ordering::Relaxed) as f32 / 2.0
    }

    /// Get SC HPF as normalized value for a band.
    #[inline]
    pub fn sc_hpf_normalized(&self, band: usize) -> f32 {
        // Skewed range: normalize(plain) = ((plain - min) / (max - min)).powf(factor)
        let hz = self.sc_hpf_hz[band].load();
        let skew_factor = 2.0f32.powf(-1.5);
        (hz / 500.0).clamp(0.0, 1.0).powf(skew_factor)
    }

    /// Reset all values to factory defaults (same as new_with_defaults).
    pub fn reset_to_defaults(&self) {
        for i in 0..NUM_BANDS {
            self.threshold_db[i].store(-20.0);
            self.attack_idx[i].store(4, Ordering::Relaxed);  // A10
            self.release_idx[i].store(1, Ordering::Relaxed);  // R300
            self.ratio_idx[i].store(1, Ordering::Relaxed);   // Ratio4
            self.makeup_db[i].store(0.0);
            self.sc_hpf_hz[i].store(if i == 0 { 0.0 } else { 80.0 });
        }
    }
}

/// Serializable snapshot of slot values for persistence.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SlotPersist {
    pub threshold_db: [f32; NUM_BANDS],
    pub attack_idx: [u32; NUM_BANDS],
    pub release_idx: [u32; NUM_BANDS],
    pub ratio_idx: [u32; NUM_BANDS],
    pub makeup_db: [f32; NUM_BANDS],
    pub sc_hpf_hz: [f32; NUM_BANDS],
}

impl SlotPersist {
    fn from_slot(slot: &SlotStorage) -> Self {
        let mut s = Self {
            threshold_db: [0.0; NUM_BANDS],
            attack_idx: [0; NUM_BANDS],
            release_idx: [0; NUM_BANDS],
            ratio_idx: [0; NUM_BANDS],
            makeup_db: [0.0; NUM_BANDS],
            sc_hpf_hz: [0.0; NUM_BANDS],
        };
        for i in 0..NUM_BANDS {
            s.threshold_db[i] = slot.threshold_db[i].load();
            s.attack_idx[i] = slot.attack_idx[i].load(Ordering::Relaxed);
            s.release_idx[i] = slot.release_idx[i].load(Ordering::Relaxed);
            s.ratio_idx[i] = slot.ratio_idx[i].load(Ordering::Relaxed);
            s.makeup_db[i] = slot.makeup_db[i].load();
            s.sc_hpf_hz[i] = slot.sc_hpf_hz[i].load();
        }
        s
    }

    fn apply_to_slot(&self, slot: &SlotStorage) {
        for i in 0..NUM_BANDS {
            slot.threshold_db[i].store(self.threshold_db[i]);
            slot.attack_idx[i].store(self.attack_idx[i], Ordering::Relaxed);
            slot.release_idx[i].store(self.release_idx[i], Ordering::Relaxed);
            slot.ratio_idx[i].store(self.ratio_idx[i], Ordering::Relaxed);
            slot.makeup_db[i].store(self.makeup_db[i]);
            slot.sc_hpf_hz[i].store(self.sc_hpf_hz[i]);
        }
    }
}

/// Lock-free communication between audio thread (GR measurement) and GUI (AutoButton).
///
/// Flow: GUI sets attack/release from BPM input, then sets `active` →
///       audio thread measures GR for 1 second → computes threshold adjustment →
///       stores target_threshold + sets `params_ready` → GUI applies threshold.
///       Repeats up to 3 iterations for convergence.
pub(crate) struct AutoAnalysisState {
    /// GUI→Audio: start GR measurement
    pub active: AtomicBool,
    /// Audio→GUI: measurement complete
    pub done: AtomicBool,
    /// Progress 0.0..1.0 for GUI animation
    pub progress: AtomicF32,
    /// Audio thread sets true when target threshold is computed
    pub params_ready: AtomicBool,
    /// GUI sets true after applying threshold
    pub params_applied: AtomicBool,
    /// Per-band average GR measured (dB, negative = compression)
    pub avg_gr: [AtomicF32; NUM_BANDS],
    /// Per-band target threshold (normalized 0..1)
    pub target_threshold: [AtomicF32; NUM_BANDS],
    /// Current convergence iteration (0, 1, 2)
    pub iteration: AtomicU32,
}

impl AutoAnalysisState {
    pub fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            done: AtomicBool::new(false),
            progress: AtomicF32::new(0.0),
            params_ready: AtomicBool::new(false),
            params_applied: AtomicBool::new(false),
            avg_gr: [
                AtomicF32::new(0.0), AtomicF32::new(0.0),
                AtomicF32::new(0.0), AtomicF32::new(0.0),
            ],
            target_threshold: [
                AtomicF32::new(0.0), AtomicF32::new(0.0),
                AtomicF32::new(0.0), AtomicF32::new(0.0),
            ],
            iteration: AtomicU32::new(0),
        }
    }
}

// ============================================================
//  TRIM State — lock-free audio↔GUI for dry/wet gain matching
// ============================================================

pub(crate) struct TrimState {
    /// GUI→Audio: start RMS measurement
    pub active: AtomicBool,
    /// Audio→GUI: measurement complete
    pub done: AtomicBool,
    /// Progress 0.0..1.0 for GUI animation
    pub progress: AtomicF32,
    /// Computed trim gain in dB (audio→GUI display)
    pub trim_gain_db: AtomicF32,
}

impl TrimState {
    pub fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            done: AtomicBool::new(false),
            progress: AtomicF32::new(0.0),
            trim_gain_db: AtomicF32::new(0.0),
        }
    }
}

// ============================================================
//  Global parameter overrides — immediate GUI→audio bypass
// ============================================================

/// Lock-free override storage for non-slot parameters.
/// When the GUI sets a value via keyboard input, the host may defer the
/// parameter change (VST3 `is_processing`). These atomics let the audio
/// thread pick up the new value immediately. NAN sentinel = "no override,
/// use nih-plug smoothed value".
pub(crate) struct GlobalOverrides {
    pub input_gain_db: AtomicF32,
    pub output_gain_db: AtomicF32,
    pub dry_wet: AtomicF32,
    pub drive_db: [AtomicF32; NUM_BANDS],
}

impl GlobalOverrides {
    fn new() -> Self {
        Self {
            input_gain_db: AtomicF32::new(f32::NAN),
            output_gain_db: AtomicF32::new(f32::NAN),
            dry_wet: AtomicF32::new(f32::NAN),
            drive_db: [
                AtomicF32::new(f32::NAN), AtomicF32::new(f32::NAN),
                AtomicF32::new(f32::NAN), AtomicF32::new(f32::NAN),
            ],
        }
    }

    /// Clear an override (revert to nih-plug smoothed value).
    #[inline]
    pub fn clear_input_gain(&self) { self.input_gain_db.store(f32::NAN); }
    #[inline]
    pub fn clear_output_gain(&self) { self.output_gain_db.store(f32::NAN); }
    #[inline]
    pub fn clear_dry_wet(&self) { self.dry_wet.store(f32::NAN); }
    #[inline]
    pub fn clear_drive(&self, band: usize) { self.drive_db[band].store(f32::NAN); }
}

// ============================================================
//  Plugin struct
// ============================================================

struct Ivylace {
    params: Arc<IvylaceParams>,
    /// Immediate GUI→audio overrides for non-slot parameters
    global_overrides: Arc<GlobalOverrides>,
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
    /// Shared state for auto-analysis (audio thread ↔ GUI)
    auto_state: Arc<AutoAnalysisState>,
    /// GR accumulation for auto-threshold: sum of GR per band (reset each measurement)
    auto_gr_sum: [f64; NUM_BANDS],
    /// GR accumulation: sample count
    auto_gr_count: u64,
    /// GR measurement duration in samples (1 second)
    auto_measure_samples: u64,
    /// A/B slot A (manual settings)
    slot_a: Arc<SlotStorage>,
    /// A/B slot B (AUTO results)
    slot_b: Arc<SlotStorage>,
    /// TRIM detector (dry/wet RMS measurement)
    trim_detector: TrimDetector,
    /// Shared state for TRIM measurement (audio thread ↔ GUI)
    trim_state: Arc<TrimState>,
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

    /// Delta monitor mode (listen to wet−dry difference)
    #[persist = "delta-monitor"]
    pub(crate) delta_monitor: Arc<AtomicBool>,

    /// TRIM enabled (auto gain matching wet to dry level)
    #[persist = "trim-enabled"]
    pub(crate) trim_enabled: Arc<AtomicBool>,

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

    // --- Bypass ---
    #[id = "bypass"]
    pub bypass: BoolParam,

    // --- A/B slot state ---
    /// A/B mode: which slot is active (false=A, true=B)
    #[persist = "ab-active-is-b"]
    pub(crate) ab_active_is_b: Arc<AtomicBool>,

    /// Slot A persistence
    #[persist = "slot-a-data"]
    pub(crate) slot_a_persist: std::sync::Mutex<SlotPersist>,

    /// Slot B persistence
    #[persist = "slot-b-data"]
    pub(crate) slot_b_persist: std::sync::Mutex<SlotPersist>,

    /// Immediate GUI→audio overrides for non-slot params (not a DAW parameter)
    pub(crate) global_overrides: Arc<GlobalOverrides>,
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

fn make_band_params(
    band_idx: usize,
    slot_a: &Arc<SlotStorage>,
    slot_b: &Arc<SlotStorage>,
    ab_is_b: &Arc<AtomicBool>,
) -> BandParams {
    let name = BAND_NAMES[band_idx];

    // Callback helper: write plain value to active slot's field
    macro_rules! float_cb {
        ($sa:expr, $sb:expr, $flag:expr, $idx:expr, $field:ident) => {{
            let sa = $sa.clone();
            let sb = $sb.clone();
            let flag = $flag.clone();
            let idx = $idx;
            Arc::new(move |val: f32| {
                let slot = if flag.load(Ordering::Relaxed) { &sb } else { &sa };
                slot.$field[idx].store(val);
            })
        }};
    }
    macro_rules! enum_cb {
        ($sa:expr, $sb:expr, $flag:expr, $idx:expr, $field:ident, $variant_to_idx:expr) => {{
            let sa = $sa.clone();
            let sb = $sb.clone();
            let flag = $flag.clone();
            let idx = $idx;
            Arc::new(move |val| {
                let slot = if flag.load(Ordering::Relaxed) { &sb } else { &sa };
                slot.$field[idx].store($variant_to_idx(val), Ordering::Relaxed);
            })
        }};
    }

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
        .with_step_size(0.1)
        .with_callback(float_cb!(slot_a, slot_b, ab_is_b, band_idx, threshold_db)),

        ratio: EnumParam::new(format!("{name} Ratio"), RatioParam::Ratio4)
            .with_callback(enum_cb!(slot_a, slot_b, ab_is_b, band_idx, ratio_idx,
                |v: RatioParam| v as u32)),

        attack: EnumParam::new(format!("{name} Attack"), AttackParam::A10)
            .with_callback(enum_cb!(slot_a, slot_b, ab_is_b, band_idx, attack_idx,
                |v: AttackParam| v as u32)),

        release: EnumParam::new(format!("{name} Release"), ReleaseParam::R300)
            .with_callback(enum_cb!(slot_a, slot_b, ab_is_b, band_idx, release_idx,
                |v: ReleaseParam| v as u32)),

        makeup: FloatParam::new(
            format!("{name} Makeup"),
            0.0,
            FloatRange::Linear {
                min: -12.0,
                max: 24.0,
            },
        )
        .with_unit(" dB")
        .with_step_size(0.1)
        .with_callback(float_cb!(slot_a, slot_b, ab_is_b, band_idx, makeup_db)),

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
        .with_step_size(1.0)
        .with_callback(float_cb!(slot_a, slot_b, ab_is_b, band_idx, sc_hpf_hz)),

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
            if band_idx == 0 { 0.0 } else { 1.8 },
            FloatRange::Linear {
                min: 0.0,
                max: 6.0,
            },
        )
        .with_unit(" dB")
        .with_step_size(0.1),

        // Low band saturation OFF by default for EDM!
        enabled: BoolParam::new(format!("{name} Sat Enabled"), band_idx != 0),
    }
}

// ============================================================
//  Plugin implementation
// ============================================================

impl Default for Ivylace {
    fn default() -> Self {
        // Create A/B slots and state FIRST so param callbacks can reference them
        let slot_a = Arc::new(SlotStorage::new_with_defaults());
        let slot_b = Arc::new(SlotStorage::new_with_defaults());
        let ab_is_b = Arc::new(AtomicBool::new(false));
        let auto_state = Arc::new(AutoAnalysisState::new());
        let global_overrides = Arc::new(GlobalOverrides::new());

        // Default slot persist values (match SlotStorage::new_with_defaults)
        let default_persist = SlotPersist {
            threshold_db: [-20.0; NUM_BANDS],
            attack_idx: [4; NUM_BANDS], // A10
            release_idx: [1; NUM_BANDS], // R300
            ratio_idx: [1; NUM_BANDS], // Ratio4
            makeup_db: [0.0; NUM_BANDS],
            sc_hpf_hz: [0.0, 80.0, 80.0, 80.0],
        };

        Self {
            params: Arc::new(IvylaceParams {
                editor_state: editor::default_state(),
                glass_mode: Arc::new(AtomicBool::new(false)),
                delta_monitor: Arc::new(AtomicBool::new(false)),
                trim_enabled: Arc::new(AtomicBool::new(false)),

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
                        min: 30.0,
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
                    make_band_params(0, &slot_a, &slot_b, &ab_is_b),
                    make_band_params(1, &slot_a, &slot_b, &ab_is_b),
                    make_band_params(2, &slot_a, &slot_b, &ab_is_b),
                    make_band_params(3, &slot_a, &slot_b, &ab_is_b),
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

                bypass: BoolParam::new("Bypass", false)
                    .make_bypass(),

                ab_active_is_b: ab_is_b,
                slot_a_persist: std::sync::Mutex::new(default_persist.clone()),
                slot_b_persist: std::sync::Mutex::new(default_persist),
                global_overrides: global_overrides.clone(),
            }),
            global_overrides,
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
            auto_state,
            auto_gr_sum: [0.0; NUM_BANDS],
            auto_gr_count: 0,
            auto_measure_samples: 44100,
            slot_a,
            slot_b,
            trim_detector: TrimDetector::new(44100.0),
            trim_state: Arc::new(TrimState::new()),
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
            self.auto_state.clone(),
            self.params.editor_state.clone(),
            self.slot_a.clone(),
            self.slot_b.clone(),
            self.trim_state.clone(),
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
        self.auto_measure_samples = sr as u64; // 1 second
        self.trim_detector.set_sample_rate(sr);
        self.is_rendering = buffer_config.process_mode == ProcessMode::Offline;

        // Restore A/B slot data from persistence (nih-plug deserializes #[persist] fields
        // before calling initialize)
        if let Ok(persist) = self.params.slot_a_persist.lock() {
            persist.apply_to_slot(&self.slot_a);
        }
        if let Ok(persist) = self.params.slot_b_persist.lock() {
            persist.apply_to_slot(&self.slot_b);
        }

        // If slot A is active and this is the first load (slot might be defaults),
        // sync slot A from current nih-plug params (which may have been restored by the host)
        if !self.params.ab_active_is_b.load(Ordering::Relaxed) {
            self.slot_a.load_from_params(&self.params);
        }

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
        self.auto_gr_sum = [0.0; NUM_BANDS];
        self.auto_gr_count = 0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Bypass: pass audio through unprocessed
        if self.params.bypass.value() {
            return ProcessStatus::Normal;
        }

        // Select oversampling factor based on realtime vs render
        // is_rendering is set in initialize() from buffer_config.process_mode
        let os_factor = if self.is_rendering {
            self.params.os_render.value().to_factor()
        } else {
            self.params.os_realtime.value().to_factor()
        };
        self.oversampler.set_factor(os_factor);

        // Update compressor sample rate to match effective oversampled rate.
        // Without this, attack/release coefficients are wrong at 2x/4x
        // (e.g., 10ms attack becomes 5ms at 2x, 2.5ms at 4x).
        let effective_sr = self.current_sample_rate as f64 * os_factor.multiplier() as f64;
        for comp in &mut self.compressors {
            comp.set_sample_rate(effective_sr);
        }
        // Saturation DC blocker also needs effective (oversampled) sample rate
        self.saturation.set_sample_rate(effective_sr);

        // Read non-smoothed parameters once per block
        let delta_monitor = self.params.delta_monitor.load(std::sync::atomic::Ordering::Relaxed);

        // Stepped enum params (don't need per-sample smoothing)
        let sat_type = match self.params.sat_type.value() {
            SatTypeParam::Console => SaturationType::Console,
            SatTypeParam::Tube => SaturationType::Tube,
            SatTypeParam::Tape => SaturationType::Tape,
        };
        self.saturation.set_global_type(sat_type);

        // Read A/B slot — all per-band compressor params come from the active slot
        let use_slot_b = self.params.ab_active_is_b.load(Ordering::Relaxed);
        let active_slot = if use_slot_b { &self.slot_b } else { &self.slot_a };

        // Per-band non-smoothed params from active slot
        for (i, band_params) in self.params.bands.iter().enumerate() {
            let atk_idx = active_slot.attack_idx[i].load(Ordering::Relaxed) as usize;
            self.compressors[i].set_attack_ms(ATTACK_TIMES_MS[atk_idx.min(ATTACK_TIMES_MS.len() - 1)]);

            let rel_idx = active_slot.release_idx[i].load(Ordering::Relaxed) as usize;
            self.compressors[i].set_release_ms(RELEASE_TIMES_MS[rel_idx.min(RELEASE_TIMES_MS.len() - 1)]);

            let rat_idx = active_slot.ratio_idx[i].load(Ordering::Relaxed) as usize;
            self.compressors[i].set_ratio(RATIOS[rat_idx.min(RATIOS.len() - 1)]);

            self.compressors[i].set_enabled(band_params.enabled.value());
        }
        for (i, sat_params) in self.params.sat_bands.iter().enumerate() {
            self.saturation.bands[i].set_enabled(sat_params.enabled.value());
        }

        // Check for solo
        let any_solo = self.params.bands.iter().any(|b| b.solo.value());

        // Crossover frequencies: update once per block (per-sample is too expensive —
        // set_frequencies() calls sin/cos for each biquad, and coefficient jitter can
        // cause filter instability). Crossover freqs change slowly during automation,
        // so block-rate updates are sufficient.
        // Use next_step(block_len) to advance the smoother by the entire block length
        // in one call, so it stays synchronized with the audio clock.
        let block_len = buffer.samples() as u32;
        let xover_freqs = [
            self.params.xover_low.smoothed.next_step(block_len) as f64,
            self.params.xover_mid.smoothed.next_step(block_len) as f64,
            self.params.xover_high.smoothed.next_step(block_len) as f64,
        ];
        self.crossover.set_frequencies(xover_freqs);

        // Process audio — smoothed parameters are read per-sample inside the loop
        let ovr = &self.global_overrides;
        for mut channel_samples in buffer.iter_samples() {
            // Per-sample smoothed parameters (prevents zipper noise at large block sizes)
            // Global overrides take priority when set (GUI keyboard input bypass).
            let smoothed_ig = self.params.input_gain.smoothed.next();
            let smoothed_og = self.params.output_gain.smoothed.next();
            let smoothed_dw = self.params.dry_wet.smoothed.next();

            let ovr_ig = ovr.input_gain_db.load();
            let ovr_og = ovr.output_gain_db.load();
            let ovr_dw = ovr.dry_wet.load();

            let input_gain_db = if ovr_ig.is_nan() { smoothed_ig } else { ovr_ig };
            let output_gain_db = if ovr_og.is_nan() { smoothed_og } else { ovr_og };
            let dry_wet = if ovr_dw.is_nan() { smoothed_dw } else { ovr_dw };

            let input_gain = (input_gain_db as f64 * std::f64::consts::LN_10 / 20.0).exp();
            let output_gain = (output_gain_db as f64 * std::f64::consts::LN_10 / 20.0).exp();

            // Per-band params from active slot (threshold, makeup, sc_hpf)
            for (i, band_params) in self.params.bands.iter().enumerate() {
                self.compressors[i].set_threshold(active_slot.threshold_db[i].load() as f64);
                self.compressors[i].set_makeup_db(active_slot.makeup_db[i].load() as f64);
                self.compressors[i].set_sidechain_hpf(active_slot.sc_hpf_hz[i].load() as f64);
                // Advance nih-plug smoothers to keep them in sync with audio clock
                let _ = band_params.threshold.smoothed.next();
                let _ = band_params.makeup.smoothed.next();
                let _ = band_params.sc_hpf.smoothed.next();
            }
            for (i, sat_params) in self.params.sat_bands.iter().enumerate() {
                let smoothed_drive = sat_params.drive.smoothed.next() as f64;
                let ovr_drive = ovr.drive_db[i].load();
                let drive_db = if ovr_drive.is_nan() { smoothed_drive } else { ovr_drive as f64 };
                self.saturation.bands[i].set_drive(drive_db / 6.0);
            }

            let left_in = *channel_samples.get_mut(0).unwrap() as f64 * input_gain;
            let right_in = *channel_samples.get_mut(1).unwrap() as f64 * input_gain;

            // Split into 4 bands
            let (bands_l, bands_r) = self.crossover.process(left_in, right_in);

            // Process each band through oversampled compressor + saturation.
            // The oversampler returns both wet (comp+sat) and dry (identity)
            // outputs that share the same FIR latency, enabling phase-coherent
            // dry/wet mixing at any oversampling factor.
            let mut out_l = 0.0f64;
            let mut out_r = 0.0f64;
            let mut dry_l = 0.0f64;
            let mut dry_r = 0.0f64;
            // Accumulators for spectrum pre/post inside the oversampled domain.
            // Both go through the same FIR up/down path, so the delta only
            // shows comp+sat effects without oversampler rolloff artifacts.
            let mut spectrum_pre_acc = 0.0f64;
            let mut spectrum_post_acc = 0.0f64;

            // Destructure to avoid borrow checker issues with multiple mutable borrows
            let compressors = &mut self.compressors;
            let sat_bands = &mut self.saturation.bands;
            let os_bands = &mut self.oversampler.bands;
            let gr_meters = &mut self.gr_meters;

            let editor_open = self.params.editor_state.is_open();

            for i in 0..NUM_BANDS {
                let comp = &mut compressors[i];
                let sat = &mut sat_bands[i];

                // Per-band accumulators for oversampled-domain pre/post
                let mut band_pre_sum = 0.0f64;
                let mut band_post_sum = 0.0f64;
                let mut band_os_count = 0u32;

                let (proc_l, proc_r, band_dry_l, band_dry_r) = os_bands[i].process(
                    bands_l[i],
                    bands_r[i],
                    |l, r| {
                        // Pre: input to comp/sat (inside oversampled domain)
                        band_pre_sum += (l + r) * 0.5;
                        let (cl, cr) = comp.process(l, r);
                        let sl = sat.process(cl);
                        let sr = sat.process(cr);
                        // Post: output of comp/sat (inside oversampled domain)
                        band_post_sum += (sl + sr) * 0.5;
                        band_os_count += 1;
                        (sl, sr)
                    },
                );

                // Average the oversampled pre/post values back to 1x rate
                if editor_open && band_os_count > 0 {
                    let inv = 1.0 / band_os_count as f64;
                    spectrum_pre_acc += band_pre_sum * inv;
                    spectrum_post_acc += band_post_sum * inv;
                }

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
                    dry_l += band_dry_l;
                    dry_r += band_dry_r;
                }
            }

            // TRIM measurement: accumulate dry/wet RMS over 3 seconds
            if self.trim_state.active.load(Ordering::Relaxed) {
                if !self.trim_detector.is_active() {
                    self.trim_detector.start();
                }
                self.trim_detector.process_sample(dry_l, dry_r, out_l, out_r);
                self.trim_state.progress.store(self.trim_detector.progress());
                if self.trim_detector.is_done() {
                    self.trim_state.trim_gain_db.store(self.trim_detector.trim_gain_db() as f32);
                    self.trim_state.done.store(true, Ordering::Release);
                    self.trim_state.active.store(false, Ordering::Release);
                    // Auto-enable trim after successful measurement
                    self.params.trim_enabled.store(true, Ordering::Relaxed);
                }
            }

            // Apply TRIM gain to wet signal (before dry/wet mix)
            let (out_l, out_r) = if self.params.trim_enabled.load(Ordering::Relaxed) {
                let db = self.trim_state.trim_gain_db.load() as f64;
                let gain = 10.0_f64.powf(db / 20.0);
                (out_l * gain, out_r * gain)
            } else {
                (out_l, out_r)
            };

            // Delta monitor or normal dry/wet mix
            let (final_l, final_r) = if delta_monitor {
                // Delta mode: output = wet − dry (what the processing is doing)
                // Both wet and dry share the same FIR latency, so the delta
                // cleanly isolates compressor + saturation artifacts.
                (out_l - dry_l, out_r - dry_r)
            } else {
                // Normal dry/wet mix (both dry and wet share the same FIR latency)
                let mix = dry_wet as f64;
                (dry_l * (1.0 - mix) + out_l * mix,
                 dry_r * (1.0 - mix) + out_r * mix)
            };

            // Output gain (output_gain already computed per-sample above)
            let out_sample_l = (final_l * output_gain) as f32;
            let out_sample_r = (final_r * output_gain) as f32;
            *channel_samples.get_mut(0).unwrap() = out_sample_l;
            *channel_samples.get_mut(1).unwrap() = out_sample_r;

            // Feed spectrum buffers (only when GUI is open).
            // Both pre and post are accumulated inside the oversampled domain,
            // so they share the same FIR filter path and the delta shows only
            // comp+sat effects without oversampler high-frequency rolloff artifacts.
            if editor_open {
                self.spectrum_pre.push(spectrum_pre_acc as f32);
                self.spectrum_post.push(spectrum_post_acc as f32);
            }

            // Update GR meter outputs at reduced rate
            self.gr_update_counter += 1;
            if self.gr_update_counter >= GR_METER_UPDATE_INTERVAL {
                self.gr_update_counter = 0;
                self.gr_meter_outputs.update_from_meters(&self.gr_meters);
            }

            // Auto-threshold: accumulate GR for 1 second, then compute threshold adjustment.
            // GUI sets active after setting attack/release from BPM input.
            if self.auto_state.active.load(Ordering::Relaxed) {
                // Accumulate per-band GR
                for i in 0..NUM_BANDS {
                    self.auto_gr_sum[i] += self.compressors[i].gain_reduction_db();
                }
                self.auto_gr_count += 1;

                // Progress update
                let progress = (self.auto_gr_count as f32 / self.auto_measure_samples as f32).min(1.0);
                self.auto_state.progress.store(progress);

                // 1 second measured → compute threshold adjustment
                if self.auto_gr_count >= self.auto_measure_samples {
                    const TARGET_GR: f64 = -2.5;

                    for i in 0..NUM_BANDS {
                        let avg_gr = self.auto_gr_sum[i] / self.auto_gr_count as f64;
                        self.auto_state.avg_gr[i].store(avg_gr as f32);

                        // error > 0 → not enough compression → lower threshold
                        // error < 0 → too much compression → raise threshold
                        let error = avg_gr - TARGET_GR;

                        // Read current threshold from slot B (AUTO target)
                        let current_threshold_db = self.slot_b.threshold_db[i].load() as f64;

                        // Adjust: damped step (0.7× error)
                        let new_threshold_db = (current_threshold_db - error * 0.7).clamp(-40.0, 0.0);
                        let threshold_norm = ((new_threshold_db + 40.0) / 40.0) as f32;
                        self.auto_state.target_threshold[i].store(threshold_norm);
                        // Update slot B directly so next iteration reads the new value
                        self.slot_b.threshold_db[i].store(new_threshold_db as f32);
                    }

                    // Reset accumulators
                    self.auto_gr_sum = [0.0; NUM_BANDS];
                    self.auto_gr_count = 0;

                    // Signal completion
                    self.auto_state.params_ready.store(true, Ordering::Release);
                    self.auto_state.done.store(true, Ordering::Release);
                    self.auto_state.active.store(false, Ordering::Release);
                }
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
