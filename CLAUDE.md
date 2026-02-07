# CLAUDE.md — Ivylace

## Project Overview

Multiband glue compressor VST3/CLAP plugin built with nih-plug (Rust).
SSL 4000G-style VCA compressor DSP with per-band analog saturation.
EDM production向けに設計されており、Low bandのサチュレーションはデフォルトOFFでサブベースをクリーンに保つ。

## Tech Stack

- **Language:** Rust (edition 2021, stable 1.75+)
- **Framework:** [nih-plug](https://github.com/robbert-vdh/nih-plug) (git dependency)
- **GUI:** nih_plug_vizia (VIZIA, custom femtovg drawing, no CSS)
- **Plugin Formats:** VST3, CLAP
- **Audio:** Stereo only (2in/2out)
- **Processing:** f64 internal, f32 I/O

## Project Structure

```
ivylace/
├── CLAUDE.md               # This file
├── Cargo.toml
├── README.md
├── GUI_SPEC.md
└── src/
    ├── lib.rs              # Plugin entry: Ivylace struct, IvylaceParams, process()
    ├── editor/
    │   ├── mod.rs          # Editor entry: create(), default_state(), Data lens model
    │   ├── theme.rs        # Color constants (band colors, glass, text, meter)
    │   ├── knob.rs         # GlassKnob: custom rotary knob (3 sizes)
    │   ├── meter.rs        # GrMeterWidget: vertical GR meter
    │   ├── crossover.rs    # CrossoverDisplay: log-freq band display, draggable handles
    │   ├── segmented.rs    # SegmentedParam: stepped enum control
    │   ├── toggle.rs       # ToggleButton: bool param toggle (Normal/Solo/Sat)
    │   ├── band_strip.rs   # BandStrip: per-band control strip
    │   ├── header.rs       # Header: global controls bar
    │   └── spectrum.rs     # SpectrumAnalyzer: real-time FFT display
    └── dsp/
        ├── mod.rs          # Module declarations
        ├── crossover.rs    # 4-band LR4 (24dB/oct) Linkwitz-Riley crossover
        ├── compressor.rs   # SSL 4000G-style VCA compressor (per-band)
        ├── saturation.rs   # Analog saturation (Tube/Tape/Console, per-band)
        ├── oversampling.rs # 2x/4x oversampling for saturation
        ├── gr_meter.rs     # Gain reduction metering (lock-free AtomicF32)
        └── spectrum.rs     # FFT + lock-free triple-buffer for spectrum analyzer
```

## Architecture & Signal Flow

```
Input → Input Gain
  → FourBandCrossover (LR4, 3 crossover points)
    → Band 0 (Low):     DC–120Hz
    → Band 1 (LowMid):  120–1500Hz
    → Band 2 (HighMid): 1500–8000Hz
    → Band 3 (High):    8000Hz–Nyquist
  → [Per Band] SslCompressor → BandSaturation (oversampled)
  → Sum all bands (with solo logic)
  → Dry/Wet Mix
  → Output Gain
```

## Key DSP Modules

### crossover.rs — FourBandCrossover
- Biquad (Direct Form II Transposed) → Lr4Filter (2x cascaded Butterworth) → CrossoverPoint (LP+HP pair)
- Split topology: input → mid split → each half split again
- Phase-coherent reconstruction (LR4 property)

### compressor.rs — SslCompressor
- Feed-forward VCA topology
- Stepped attack/release/ratio matching SSL 4000G hardware
- Program-dependent auto-release (fast/slow time constant blend)
- Sidechain HPF (Butterworth 2nd order) — critical for "glue" character
- Soft knee adapts to ratio (wider at 2:1, narrower at 10:1)
- Peak detection, linked stereo

### saturation.rs — MultibandSaturation / BandSaturation
- Three models: Console (subtle polynomial), Tube (asymmetric, even harmonics), Tape (tanh, odd harmonics)
- Per-band drive + enable with auto-gain compensation
- DC blocker (~5Hz HPF) after saturation
- **Band 0 (Low) saturation is OFF by default** for EDM sub-bass integrity

## GUI Architecture

- **ViziaTheming::Custom** — all rendering via custom `View::draw()` with femtovg
- **Gesture split pattern** — `begin_set` + `set_value` on MouseDown, `end_set` on MouseUp (required for Cubase compatibility)
- **ParamWidgetBase** for all parameter bindings
- **Lock-free data flow** — AtomicF32 for GR meters, triple-buffer for spectrum

## Coding Conventions

- `#[inline(always)]` on all per-sample DSP functions
- f64 for all internal DSP arithmetic (precision matters for filter stability)
- Parameter smoothing via nih-plug's `.smoothed.next()` in process loop
- Enum parameters for stepped controls (attack, release, ratio) matching hardware
- Constants: `ATTACK_TIMES_MS`, `RELEASE_TIMES_MS`, `RATIOS` defined in compressor.rs
- Use `clamp()` for parameter bounds, avoid panics in audio thread
- **No allocations in audio thread** — `assert_process_allocs` feature enabled

## Build Commands

```bash
# Debug build
cargo build

# Release VST3/CLAP bundle
cargo xtask bundle ivylace --release

# Check without building
cargo check

# Run tests
cargo test
```

## Current State (v0.4.0)

### ✅ Implemented
- 4-band LR4 crossover with adjustable frequencies
- Per-band SSL 4000G-style compressor with all hardware parameters
- Per-band analog saturation (3 models) with low-band bypass
- 2x/4x oversampling for saturation stage
- Full parameter set with nih-plug integration
- Solo per band
- Global dry/wet, input/output gain
- VST3 + CLAP export
- Full VIZIA GUI with custom widgets:
  - GlassKnob (3 sizes), GR meters, crossover display
  - Segmented controls, toggle buttons
  - Spectrum analyzer (in-crate FFT)
  - Header with global controls

### 🔲 TODO

1. **Preset System**
   - Factory presets for common use cases
   - User preset save/load

2. **Sidechain External Input**
   - Additional audio input for external sidechain
   - Requires `AuxiliaryBuffers` configuration in nih-plug

3. **Stereo Width**
   - Mid/Side processing option per band

4. **Testing**
   - Unit tests for each DSP module
   - Frequency response verification for crossover

## Important Design Decisions

- **Why f64 internal?** IIR filter coefficient precision. Biquad filters accumulate rounding errors over time; f64 prevents audible artifacts especially at low frequencies.
- **Why LR4 not LR8?** LR4 (24dB/oct) gives sufficient band isolation with less phase rotation. LR8 would add latency from additional filter stages.
- **Why low-band saturation OFF?** EDM sub-bass (30-120Hz) needs phase coherence for club systems. Saturation harmonics in this range cause mud and translation issues on large PAs.
- **Why stepped attack/release/ratio?** Matches the SSL 4000G hardware. Stepped values also prevent users from dialing in problematic settings.
- **Why gesture split?** Cubase ignores `perform_edit` when `begin_edit`/`perform_edit`/`end_edit` happen in one callback. Splitting across MouseDown/MouseUp fixes this.

## Dependencies

- `nih_plug` (git) — plugin framework
- `nih_plug_vizia` (git) — VIZIA GUI integration

No other external crates. FFT is implemented in-crate.

## Notes

- VST3 Class ID: `IvylaceKyoko0001` (16 bytes)
- CLAP ID: `com.kyoko.ivylace`
- CLAP features: AudioEffect, Compressor, Mastering, Stereo
- Random seed convention: 114514
- Editor: vim (save with ZZ)
