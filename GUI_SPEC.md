# Ivylace — GUI Implementation Spec

> **Status**: Implemented with nih-plug VIZIA (native GPU-rendered)
> **Window**: 920 x 660px, resizable

---

## 1. Architecture

### 1.1 Framework

- **nih_plug_vizia** (native GPU via femtovg/OpenGL)
- Custom `View::draw()` widgets using `vizia::vg` (femtovg canvas API)
- `ParamWidgetBase` for parameter binding
- `ViziaTheming::Custom` (no CSS, all drawing in Rust)

### 1.2 File Structure

```
src/editor/
  mod.rs          -- Editor entry: create(), default_state(), Data lens model, title bar
  theme.rs        -- Color functions (band colors, glass, text, meter, etc.)
  knob.rs         -- GlassKnob: custom rotary knob (arc + glass body, 3 sizes)
  meter.rs        -- GrMeterWidget: vertical segmented GR meter (custom draw)
  crossover.rs    -- CrossoverDisplay: log-freq band visualization, draggable handles
  segmented.rs    -- SegmentedParam: stepped enum parameter control
  toggle.rs       -- ToggleButton: bool param toggle (Normal/Solo/Sat variants)
  band_strip.rs   -- BandStrip: per-band vertical strip composing all controls
  header.rs       -- Header: global controls bar (In/Out Gain, Mix, Sat Type, OS)
  spectrum.rs     -- SpectrumAnalyzer: real-time FFT spectrum display

src/dsp/
  spectrum.rs     -- SpectrumBuffer (lock-free triple-buffer), in-crate FFT (2048-pt)
```

### 1.3 Data Model

```rust
#[derive(Lens)]
struct Data {
    params: Arc<IvylaceParams>,
    gr_outputs: Arc<GrMeterOutputs>,
}
```

Additional shared state passed directly to widgets (not via lens):
- `Arc<SpectrumBuffer>` for spectrum analyzer
- `sample_rate: f32` for frequency bin calculation

### 1.4 Layout Hierarchy

```
VStack (root, background: #0A0A1A)
  HStack (title bar, 32px)
  Header (80px)
  CrossoverDisplay (96px)
  SpectrumAnalyzer (80px)
  HStack (band strips, Stretch)
    BandStrip x 4 (each Stretch(1.0))
  ResizeHandle
```

---

## 2. Custom Widgets

### 2.1 GlassKnob (`knob.rs`)

Glassmorphism-style rotary knob with colored arc indicator.

**Sizes**:
| Size | Outer | Inner | Indicator |
|------|-------|-------|-----------|
| Sm   | 36px  | 28px  | 10px      |
| Md   | 48px  | 38px  | 14px      |
| Lg   | 68px  | 56px  | 20px      |

**Drawing** (femtovg):
1. Background arc track: 270-degree sweep (-135 to +135), grey 2.5px stroke
2. Value arc: band color stroke + glow (6px feathered)
3. Glass body: radial gradient (white-tint top-left to dark bottom-right)
4. Indicator line: 2px colored line rotated to value angle + glow
5. Label text below (parameter name, uppercase, 9pt)
6. Value display below (monospace, 10pt)

**Interaction**:
- Vertical drag: 200px = full range
- Shift+drag: 2000px = full range (fine control)
- Double-click: reset to default
- Alt/Cmd+click: reset to default
- Mouse scroll: step increment/decrement (shift = finer)

### 2.2 GrMeterWidget (`meter.rs`)

Vertical gain reduction meter, 20x112px.

**Drawing**:
- Dark background with inset shadow
- 20 segment lines
- Fill from top (0dB) down: blue (#4A9EFF) gradient, red (#FF4466) at high GR
- Peak hold line: red (#FF6B6B), 2px, 1s hold + exponential decay
- Scale marks at 0, -6, -12, -20, -30 dB
- Value readout below (-40 to 0 dB range)

**Data source**: `Data::gr_outputs.map(move |o| o.band_gr[band_idx].load())`

### 2.3 CrossoverDisplay (`crossover.rs`)

Full-width, 96px height, log-frequency display.

**Drawing**:
- Background: `rgba(0,0,0,51)`
- 4 band color fill regions (gradient top-to-bottom, 7% to 2% alpha)
- Grid lines at 50, 100, 200, 500, 1k, 2k, 5k, 10k Hz
- Frequency labels at bottom (7pt)
- Band name labels centered in each region (10pt, 25% alpha)
- 3 vertical handle lines (gradient fade, band colored)
- Handle dots: rounded rect (6x30px, 3px radius)
- Hover/active: frequency label pill (e.g., "1500 Hz")

**Interaction**:
- Mouse down: hit-test 3 handles (10px tolerance)
- Drag: convert pixel X to frequency via inverse log scale
- Constraints: handles can't cross (1.1x minimum spacing)
- Double-click: reset handle to default

### 2.4 SegmentedParam (`segmented.rs`)

Stepped segmented control for EnumParam (Ratio, Attack, Release).

- HStack of SegmentButton views
- Active state via `toggle_class("seg-active", lens)`
- Click emits `RawParamEvent` (BeginSet, SetNormalized, EndSet)
- Glass-style background: `rgba(255,255,255,15)` with border

### 2.5 ToggleButton (`toggle.rs`)

Boolean parameter toggle with 3 variants:

| Variant | Active Color | Use |
|---------|-------------|-----|
| Normal  | Band color  | IN (compressor enable) |
| Solo    | Yellow (#FFD43B) | Solo |
| Sat     | Gold (#E8A838) | Saturation enable |

**Drawing**:
- Active: linear gradient fill + border + glow (box_gradient)
- Inactive: dark glass (`rgba(255,255,255,15)`) + border
- Text: dark on active, grey on inactive

### 2.6 SpectrumAnalyzer (`spectrum.rs`)

Real-time FFT spectrum display, 80px height.

**DSP** (`dsp/spectrum.rs`):
- Triple-buffer `SpectrumBuffer`: lock-free audio-to-GUI sample transfer
- In-crate radix-2 Cooley-Tukey FFT, 2048-point
- Hann window applied before FFT
- Magnitude in dB (-120 to 0 dB range)
- Audio thread pushes mono sum `(L+R)/2` (only when GUI is open)

**Drawing**:
- Background: semi-transparent dark
- Log-frequency X axis (20Hz-20kHz, matching crossover display)
- dB Y axis (-90 to 0 dB)
- Filled path: gradient fill (`rgba(100,180,255,25)` to `rgba(100,180,255,5)`)
- Stroke: `rgba(120,200,255,100)`, 1.5px
- Exponential smoothing (0.8 factor) between frames

---

## 3. Color Theme

Glassmorphism dark theme. All colors defined as functions in `theme.rs`.

### 3.1 Band Colors

| Band    | Color   | Hex       |
|---------|---------|-----------|
| Low     | Red     | `#FF6B6B` |
| LowMid  | Orange  | `#FFA94D` |
| HighMid  | Green   | `#69DB7C` |
| High    | Blue    | `#74C0FC` |

### 3.2 Special Colors

| Purpose       | Hex       |
|---------------|-----------|
| Accent gold       | `#E8A838` |
| Solo yellow   | `#FFD43B` |
| Page background | `#0A0A1A` |

### 3.3 Glass/Panel Colors

Semi-transparent white overlays on dark background:

| Element | Alpha |
|---------|-------|
| Glass panel bg | 7% |
| Glass border | 12% |
| Knob body light | 18% |
| Knob body dark | 15% |
| Toggle off bg | 6% |
| Meter bg | `rgba(0,0,0,76)` |

### 3.4 Text Colors

| Role | RGBA |
|------|------|
| Primary | `rgba(255,255,255,230)` |
| Secondary | `rgba(255,255,255,128)` |
| Dim | `rgba(255,255,255,77)` |
| On active bg | `rgba(0,0,0,216)` |

---

## 4. Audio-to-GUI Data Flow

### 4.1 GR Meters

```
[Audio Thread]                     [GUI Thread]
Compressor.gain_reduction_db()
       |
  GrMeter.push(gr_db)
       | (every 64 samples)
  GrMeterOutputs
    .band_gr[i].store()    --->   .band_gr[i].load()
    .band_peak_gr[i].store() -->  .band_peak_gr[i].load()
                                       |
                                  GrMeterWidget::draw()
```

- `AtomicF32` lock-free sharing
- ~689 Hz update rate @ 44.1kHz

### 4.2 Spectrum Analyzer

```
[Audio Thread]                     [GUI Thread]
Output samples (L+R)/2
       |
  SpectrumBuffer.push(sample)
       | (triple-buffer, per-sample)
       | (buffer full: swap to next)

  SpectrumBuffer.read()  <---  SpectrumAnalyzer::draw()
                                       |
                               Hann window + FFT
                                       |
                               Magnitude dB + smoothing
                                       |
                               femtovg path drawing
```

- Triple-buffer pattern (no locks)
- 2048-sample FFT window
- Only active when GUI is open (`editor_state.is_open()`)

---

## 5. Band Strip Layout

Each `BandStrip` (vertical, 1/4 width) contains:

1. **Accent line** (2px, band color)
2. **Band name** (11pt bold, band color)
3. **GR Meter** (20x112px, centered)
4. **Threshold knob** (Lg, band color)
5. **Ratio segmented** (3 steps: 2:1 / 4:1 / 10:1)
6. **Attack segmented** (6 steps: 0.1-30 ms)
7. **Release segmented** (5 steps: 100ms-Auto)
8. **Makeup knob** (Md) + **SC HPF knob** (Sm) in HStack
9. **Saturation section** (glass panel):
   - Drive knob (Sm, accent color)
   - SAT toggle (Sat variant)
10. **Footer**: IN toggle (Normal) + Solo toggle (Solo)

---

## 6. Build & Bundle

```bash
# Debug check
cargo check

# Release VST3/CLAP bundle
cargo xtask bundle ivylace --release
```

Output:
- `target/bundled/ivylace.vst3`
- `target/bundled/ivylace.clap`
