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
├── ivylace_logo.png        # Glass/Light mode logo (embedded in About dialog)
├── ivylace_logo_dark.png   # Dark mode logo (embedded in About dialog)
├── assets/
│   └── noto_sans_jp_kana.ttf  # Noto Sans JP subset for Japanese text
├── .github/
│   └── workflows/
│       └── build.yml       # CI: macOS Universal + Windows (no Linux)
└── src/
    ├── lib.rs              # Plugin entry: Ivylace struct, IvylaceParams, process()
    ├── editor/
    │   ├── mod.rs          # Editor entry: create(), default_state(), Data lens model, title bar, BackgroundGradient
    │   ├── theme.rs        # Color functions (~45 functions), Dark/Glass dual theme
    │   ├── knob.rs         # GlassKnob: custom rotary knob (3 sizes)
    │   ├── meter.rs        # GrMeterWidget: vertical GR meter
    │   ├── crossover.rs    # CrossoverDisplay: log-freq band display, draggable handles
    │   ├── segmented.rs    # SegmentedParam: stepped enum control
    │   ├── toggle.rs       # ToggleButton: bool param toggle (Normal/Solo/Sat/Power)
    │   ├── band_strip.rs   # BandStrip: per-band control strip
    │   ├── header.rs       # Header: global controls bar + ThemeToggle
    │   ├── spectrum.rs     # SpectrumAnalyzer: real-time FFT display (currently unused)
    │   └── about.rs        # AboutDialog: overlay with theme-dependent logo, version, author, GitHub link
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
- **Dual theme** — Dark / Glass mode, toggled via header button, persisted with `#[persist = "glass-mode"]`
- **Background gradient** — `BackgroundGradient` custom View with SelfDirected positioning, draws top-to-bottom gradient using femtovg `linear_gradient`
- **Lens-based reactive colors** — `Data::params.map(|p| ...)` pattern for theme-aware VIZIA layout colors
- **femtovg custom draw** — all widgets use `View::draw()` with direct canvas API
- **Gesture split pattern** — `begin_set` + `set_value` on MouseDown, `end_set` on MouseUp (required for Cubase compatibility)
- **ParamWidgetBase** for all parameter bindings
- **Lock-free data flow** — AtomicF32 for GR meters, triple-buffer for spectrum
- **About dialog** — overlay triggered by title bar click, with theme-dependent PNG logos and Japanese font fallback

### Theme System Details

- `ThemeMode { Dark, Glass }` enum in `theme.rs`
- ~45 color functions all take `mode: ThemeMode` parameter
- `glass_mode: Arc<AtomicBool>` on `IvylaceParams`, `#[persist]` for session persistence
- All widget structs hold `glass_mode: Arc<AtomicBool>`, read in `View::draw()`
- VIZIA layout properties (background, border, text color) use lens-based reactivity:
  ```rust
  fn mode_lens() -> impl Lens<Target = ThemeMode> {
      Data::params.map(|p| {
          if p.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark }
      })
  }
  ```
- `theme::to_vizia(femtovg_color)` converts `vg::Color` → VIZIA `Color`

### Theme Color Design

**Dark mode:**
- Base tint: purple `#cc7eb1` / deep purple `#663f58`
- Background gradient: `#2B2652` (top) → `#1E1A40` (bottom)
- Text: pure white with varying alpha
- Glass/panel backgrounds: `rgba(204, 126, 177, alpha)` (purple tint)

**Glass mode:**
- Base tint: sky blue `#89c3eb`
- Background gradient: `#F2F6FB` (top) → `#FAFBFD` (bottom, near-white)
- Text: dark navy with varying alpha
- Glass/panel backgrounds: `rgba(137, 195, 235, alpha)` (#89c3eb tint)

### About Dialog Details

- `AboutDialog` in `about.rs` — full-screen overlay with centered panel
- **Dual logo:** `ivylace_logo.png` (Glass mode) / `ivylace_logo_dark.png` (Dark mode)
  - Both embedded via `include_bytes!` → decoded by `image` crate at widget creation
  - Lazy GPU upload via `Cell<Option<vg::ImageId>>` per logo (separate glass/dark cache)
  - `ensure_logo(canvas, mode)` returns `(ImageId, width, height)` for current theme
- Font: Noto Sans Regular (Latin) + Noto Sans JP kana subset (Japanese fallback), `paint.set_font(&[latin, jp])`
- Event: click anywhere to close; `hoverable()` uses lens to only intercept events when visible
- SelfDirected positioning (absolute overlay on top of all widgets)

### Header Layout Details

- Header bar: `height(Pixels(70.0))`, contains left/center/right groups
- Center group (SAT TYPE / OS REALTIME / OS RENDER):
  - Parent HStack has `child_top(Stretch(1.0))` + `child_bottom(Stretch(1.0))` for vertical centering
  - Each `SegmentedParam` directly placed (no VStack wrapper) with `.height(Auto)` so it sizes to content
  - Parent's child centering places the Auto-sized SegmentedParam in the center of available height
  - Dividers are `Element` with `height(Pixels(32.0))` + `top/bottom(Stretch(1.0))` for self-centering

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

## CI/CD

GitHub Actions (`.github/workflows/build.yml`):
- **macOS Universal** (x86_64 + aarch64 via `lipo`)
- **Windows** (x86_64-pc-windows-msvc)
- Linux は対象外
- タグ `v*` プッシュで GitHub Release (draft) 自動作成

## Current State (v0.5.1)

### Implemented
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
  - Header with global controls + theme toggle
- **Delta spectrum analyzer** (v0.5.0)
  - Pre/post processing difference display (cut=blue, boost=orange)
  - 8192-point in-crate FFT, 1/3 octave smoothing, asymmetric attack/release
  - Lock-free triple-buffer audio→GUI data flow
  - Noise gate to suppress phantom display during silence
  - Oversampler-aware pre/post capture (v0.5.1): both captured inside oversampled domain to prevent FIR rolloff artifacts
- **Dark / Glass dual theme** with lens-based reactive colors
  - Dark: purple-tinted glassmorphism (`#cc7eb1` / `#663f58`), gradient background `#2B2652` → `#1E1A40`
  - Glass: sky blue tint (`#89c3eb`), near-white gradient background `#F2F6FB` → `#FAFBFD`
- **About dialog** with theme-dependent logo (glass/dark), version, author name, GitHub link
- **Band threshold linking** (v0.4.2)
- **GitHub Actions CI** (macOS Universal + Windows)
- **Improved label visibility** for segmented controls (v0.4.4)
- **Header controls vertically centered** (v0.4.4)
- **DAW bypass support** (v0.5.1): VST3 kIsBypass flag for Cubase bypass button
- **Oversampling fixes** (v0.5.1): rewritten halfband FIR, correct compressor sample rate at 2x/4x, phase-coherent dry/wet mix
- **GR meter/spectrum range** (v0.5.1): ±6dB range for practical mastering use

### TODO

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

5. **Linear Phase Crossover Mode**
   - FIR-based linear phase crossover option for mastering use cases
   - Zero phase rotation at crossover boundaries, better transient preservation
   - Trade-off: increased latency (hundreds of samples) + pre-ringing

## Important Design Decisions

- **Why f64 internal?** IIR filter coefficient precision. Biquad filters accumulate rounding errors over time; f64 prevents audible artifacts especially at low frequencies.
- **Why LR4 not LR8?** LR4 (24dB/oct) gives sufficient band isolation with less phase rotation. LR8 would add latency from additional filter stages.
- **Why low-band saturation OFF?** EDM sub-bass (30-120Hz) needs phase coherence for club systems. Saturation harmonics in this range cause mud and translation issues on large PAs.
- **Why stepped attack/release/ratio?** Matches the SSL 4000G hardware. Stepped values also prevent users from dialing in problematic settings.
- **Why gesture split?** Cubase ignores `perform_edit` when `begin_edit`/`perform_edit`/`end_edit` happen in one callback. Splitting across MouseDown/MouseUp fixes this.
- **Why dynamic hoverable for About dialog?** VIZIA's hit-testing selects the topmost hoverable element; a full-screen overlay with `hoverable(true)` blocks all events below. Using `Data::params.map(...)` lens for `hoverable()` makes the overlay only intercept events when visible.
- **Why embed a JP font subset?** Noto Sans Regular (bundled with nih-plug) is Latin-only. femtovg supports font fallback via `paint.set_font(&[font1, font2])`. A 92KB Noto Sans JP kana subset covers all needed Japanese characters.
- **Why BackgroundGradient as SelfDirected View?** VIZIA doesn't support gradient backgrounds via layout properties. A custom View with `position_type(SelfDirected)` and `hoverable(false)` draws a femtovg gradient behind all other content without interfering with event handling.
- **Why `height(Auto)` on header SegmentedParam?** When placed directly in an HStack with `child_top/child_bottom(Stretch(1.0))`, `height(Auto)` lets the widget shrink to its content size (label + button row ≈ 29px), allowing the parent HStack to center it vertically. Previous approaches with VStack wrappers or `height(Stretch(1.0))` caused layout issues (content spreading, labels separating from buttons).
- **Why dual logo in About dialog?** The Glass mode logo uses dark text on light background; the Dark mode logo uses light text on dark background. Both are compiled-in via `include_bytes!` and lazily uploaded to GPU with separate `Cell<Option<vg::ImageId>>` caches.

## Dependencies

- `nih_plug` (git) — plugin framework
- `nih_plug_vizia` (git) — VIZIA GUI integration
- `image` (0.25, png feature only) — PNG logo decoding for About dialog

## Notes

- VST3 Class ID: `IvylaceKyoko0001` (16 bytes)
- CLAP ID: `com.kyoko.ivylace`
- CLAP features: AudioEffect, Compressor, Mastering, Stereo
- Random seed convention: 114514
- Editor: vim (save with ZZ)
