# Ivylace — GUI Implementation Spec

> **Status**: Implemented with nih-plug VIZIA (native GPU-rendered)
> **Window**: 960 x 740px, resizable
> **Theme**: Dark / Glass dual mode (persisted per session)

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
  mod.rs          -- Editor entry: create(), default_state(), Data lens model, TitleBarLabel, BackgroundGradient
  theme.rs        -- ~45 color functions, ThemeMode enum, Dark/Glass dual theme
  knob.rs         -- GlassKnob: custom rotary knob (arc + glass body, 3 sizes)
  meter.rs        -- GrMeterWidget: vertical segmented GR meter (custom draw)
  crossover.rs    -- CrossoverDisplay: log-freq band visualization, draggable handles
  segmented.rs    -- SegmentedParam: stepped enum parameter control
  toggle.rs       -- ToggleButton: bool param toggle (Normal/Solo/Sat/Power variants)
  band_strip.rs   -- BandStrip: per-band vertical strip composing all controls
  header.rs       -- Header: global controls bar (In/Out Gain, Mix, Sat Type, OS) + ThemeToggle
  spectrum.rs     -- SpectrumAnalyzer: real-time FFT spectrum display (implemented, not yet in layout)
  about.rs        -- AboutDialog: full-screen overlay with theme-dependent logo, version, author, GitHub link

assets/
  noto_sans_jp_kana.ttf  -- Noto Sans JP subset (hiragana/katakana) for femtovg fallback

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
- `glass_mode: Arc<AtomicBool>` cloned from `IvylaceParams` into each widget

### 1.4 Layout Hierarchy

```
VStack (root)
  BackgroundGradient (SelfDirected overlay, hoverable=false, femtovg gradient)
  HStack (title bar, 28px) — TitleBarLabel (click → About dialog)
  Header (70px) — In/Out Gain, Mix, Sat Type, OS Realtime/Render, ThemeToggle
  CrossoverDisplay (96px)
  HStack (band strips, Stretch)
    BandStrip x 4 (each Stretch(1.0))
  AboutDialog (SelfDirected overlay, full screen, conditionally hoverable)
  ResizeHandle
```

---

## 2. Theme System

### 2.1 Dual Theme Architecture

- `ThemeMode { Dark, Glass }` enum
- `glass_mode: Arc<AtomicBool>` with `#[persist = "glass-mode"]` on IvylaceParams
- ThemeToggle button in header flips the AtomicBool + `cx.needs_redraw()`
- All ~45 color functions in `theme.rs` take `mode: ThemeMode`

### 2.2 Color Reactivity

Two mechanisms for theme-reactive colors:

**femtovg (custom draw):** widgets hold `glass_mode: Arc<AtomicBool>`, read in `View::draw()`:
```rust
let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
let color = theme::some_color(mode);
```

**VIZIA layout properties:** use lens-based reactive colors:
```rust
fn mode_lens() -> impl Lens<Target = ThemeMode> {
    Data::params.map(|p| {
        if p.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark }
    })
}
// Usage:
.background_color(mode_lens().map(|m| theme::to_vizia(theme::panel_bg(*m))))
```

### 2.3 Band Colors

| Band    | Dark Mode | Glass Mode |
|---------|-----------|------------|
| Low     | `#FF6B6B` (red) | `#E04545` (deeper red) |
| LowMid  | `#FFA94D` (orange) | `#E08A30` (deeper orange) |
| HighMid  | `#69DB7C` (green) | `#38A852` (deeper green) |
| High    | `#74C0FC` (blue) | `#3A8FD4` (deeper blue) |

### 2.4 Dark Theme Colors

| Element | Value |
|---------|-------|
| Page bg gradient top | `#2B2652` |
| Page bg gradient bottom | `#1E1A40` |
| Glass/panel tint base | `rgba(204, 126, 177, alpha)` (#cc7eb1) |
| Panel bg | `rgba(204, 126, 177, 22)` |
| Panel border | `rgba(204, 126, 177, 40)` |
| Text primary | `rgba(255, 255, 255, 230)` |
| Text secondary | `rgba(255, 255, 255, 150)` |
| Text dim | `rgba(255, 255, 255, 170)` |

### 2.5 Glass Theme Colors

| Element | Value |
|---------|-------|
| Page bg gradient top | `#F2F6FB` |
| Page bg gradient bottom | `#FAFBFD` (near-white) |
| Glass/panel tint base | `rgba(137, 195, 235, alpha)` (#89c3eb) |
| Panel bg | `rgba(137, 195, 235, 30)` |
| Panel border | `rgba(137, 195, 235, 50)` |
| Text primary | `rgba(30, 40, 60, 230)` |
| Text secondary | `rgba(50, 60, 85, 160)` |
| Text dim | `rgba(40, 55, 80, 190)` |

### 2.6 Special Colors

| Purpose | Dark | Glass |
|---------|------|-------|
| Accent gold | `#E8A838` | `#E8A838` |
| Solo yellow | `#FFD43B` | `#FFD43B` |
| About link | `rgba(160, 200, 255, 200)` | `rgba(40, 120, 200, 200)` |
| About overlay bg | `rgba(40, 35, 80, 200)` | `rgba(0, 0, 0, 100)` |
| About panel bg | `rgba(60, 52, 120, 240)` | `rgba(230, 242, 252, 240)` |

---

## 3. Custom Widgets

### 3.1 GlassKnob (`knob.rs`)

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
5. Label text below (parameter name, uppercase, 9pt) — lens-reactive color
6. Value display below (monospace, 10pt) — lens-reactive color

**Interaction**:
- Vertical drag: 200px = full range
- Shift+drag: 2000px = full range (fine control)
- Double-click: reset to default
- Alt/Cmd+click: reset to default
- Mouse scroll: step increment/decrement (shift = finer)

### 3.2 GrMeterWidget (`meter.rs`)

Vertical gain reduction meter, 20x112px.

**Drawing**:
- Dark background with inset shadow
- 20 segment lines
- Fill from top (0dB) down: blue (#4A9EFF) gradient, red (#FF4466) at high GR
- Peak hold line: red (#FF6B6B), 2px, 1s hold + exponential decay
- Scale marks at 0, -6, -12, -20, -30 dB
- Value readout below (-40 to 0 dB range) — lens-reactive color
- GR label — lens-reactive color

**Data source**: `Data::gr_outputs.map(move |o| o.band_gr[band_idx].load())`

### 3.3 CrossoverDisplay (`crossover.rs`)

Full-width, 96px height, log-frequency display.

**Drawing**:
- Background: theme-reactive
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

### 3.4 SegmentedParam (`segmented.rs`)

Stepped segmented control for EnumParam (Ratio, Attack, Release, Sat Type, OS).

**Structure**: `SegmentedParam` builds Label (optional, 8pt, 11px height) + HStack of `SegmentButton` views (18px height)

**Active state**: combined lens (param active state + theme mode) via `ParamWidgetBase::make_lens` with captured `Arc<AtomicBool>`

**Interaction**:
- Click: `begin_set` + `set_normalized_value` on MouseDown, `end_set` on MouseUp (gesture split for Cubase)
- `gesture_active: bool` tracks whether `end_set` is pending

**Styling**:
- Glass-style background: lens-reactive `seg_bg` / `seg_border`
- Active segment: filled `seg_active_bg` rounded rect
- Label color: lens-reactive `text_dim`
- Active text: `seg_active_text`, inactive: `seg_inactive_text`

**Header usage**: placed directly in HStack with `.height(Auto)` (no VStack wrapper), parent centers via `child_top/child_bottom(Stretch(1.0))`

### 3.5 ToggleButton (`toggle.rs`)

Boolean parameter toggle with 4 variants:

| Variant | Active Color | Use |
|---------|-------------|-----|
| Normal  | Band color  | LINK |
| Power   | Band color (icon only) | Band enable |
| Solo    | Yellow (#FFD43B) | Solo |
| Sat     | Gold (#E8A838) | Saturation enable |

**Drawing**:
- Active: linear gradient fill + border + glow (box_gradient)
- Inactive: dark glass + border (theme-reactive)
- Text: dark on active, grey on inactive
- Power variant: draws power icon (circle + line) instead of text

### 3.6 SpectrumAnalyzer (`spectrum.rs`)

Real-time FFT spectrum display (implemented but not yet wired into editor layout).

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
- Filled path: gradient fill
- Stroke: 1.5px
- Exponential smoothing (0.8 factor) between frames

### 3.7 AboutDialog (`about.rs`)

Full-screen overlay dialog with centered info panel.

**Trigger**: Click on title bar label (TitleBarLabel in mod.rs)

**Resources** (compile-time embedded):
- Logos: `ivylace_logo.png` (Glass mode) + `ivylace_logo_dark.png` (Dark mode) via `include_bytes!`
- Both decoded by `image` crate at widget creation (not on every draw)
- Font: Noto Sans Regular (Latin, from nih-plug assets) + `assets/noto_sans_jp_kana.ttf` (JP fallback)

**GPU resources** (lazy init via `Cell<Option<...>>`):
- `logo_glass_id: Cell<Option<vg::ImageId>>` — Glass logo texture
- `logo_dark_id: Cell<Option<vg::ImageId>>` — Dark logo texture
- `font_ids: Cell<Option<(vg::FontId, vg::FontId)>>` — Latin + JP fonts
- `ensure_logo(canvas, mode)` → selects correct logo based on ThemeMode

**Drawing**:
- Full-screen dimming overlay (Dark: `rgba(40,35,80,200)`, Glass: `rgba(0,0,0,100)`)
- Centered panel (340x380px, 16px corner radius)
- Panel background: theme-reactive solid color
- Logo image (260px width, aspect-preserved, rounded corners, theme-dependent)
- Text stack: "Ivylace", version, "Multiband Glue Compressor", "by きょーこ", GitHub URL, "Built with nih-plug"
- Font fallback: `paint.set_font(&[latin_id, jp_id])` for mixed Latin/Japanese text

**Event handling**:
- Click anywhere → close (`visible.store(false)`)
- `hoverable()` uses lens: `Data::params.map(move |_| vis.load(Ordering::Relaxed))`
  - When visible=false, VIZIA's hit-test skips the overlay entirely
  - This prevents blocking events to widgets below when the dialog is hidden

**Positioning**: `PositionType::SelfDirected` with full-screen bounds (left/top=0, width/height=Stretch(1.0))

### 3.8 ThemeToggle (`header.rs`)

Theme mode toggle button in the header.

- Custom View with `glass_mode: Arc<AtomicBool>`
- Drawing: Sun icon (Dark mode) / Moon icon (Glass mode) with theme-reactive colors
- Click: flips `glass_mode` AtomicBool + `cx.needs_redraw()`
- Size: 28x28px

### 3.9 BackgroundGradient (`mod.rs`)

Full-screen gradient background drawn behind all content.

- Custom View with `glass_mode: Arc<AtomicBool>`
- `PositionType::SelfDirected` + `hoverable(false)` — doesn't interfere with event handling
- Draws top-to-bottom linear gradient using femtovg:
  - Dark: `#2B2652` → `#1E1A40` (deep purple)
  - Glass: `#F2F6FB` → `#FAFBFD` (near-white with subtle blue)

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

1. **Accent line** (2px, band color) — lens-reactive
2. **Band name** (11pt bold, band color) — lens-reactive
3. **GR Meter** (20x112px, centered)
4. **Threshold knob** (Lg, band color) — supports linked mode
5. **Ratio segmented** (3 steps: 2:1 / 4:1 / 10:1)
6. **Attack segmented** (6 steps: 0.1-30 ms)
7. **Release segmented** (5 steps: 100ms-Auto)
8. **Makeup knob** (Sm) + **SC HPF knob** (Sm) in HStack
9. **Saturation section** (glass panel, lens-reactive bg/border):
   - SATURATION label (8pt)
   - Drive knob (Sm, accent color) + SAT toggle (Sat variant)
10. **Footer**: LINK toggle (Normal) + Power toggle (Power) + SOLO toggle (Solo)

**Spacing**:
- RATIO/ATTACK/RELEASE: `row_between(15.0)`
- RELEASE-to-MAKEUP gap: `top(Pixels(24.0))`
- SATURATION section: `height(Auto)` on both VStack and inner HStack
- Footer: `top(Pixels(6.0))`, `bottom(Pixels(10.0))`

---

## 6. Header Layout

Header bar (70px height) with three groups:

**Left group** (`width(Auto)`):
- IN GAIN knob (Sm, accent)
- MIX knob (Sm, accent)
- `col_between(16.0)`

**Center group** (`width(Stretch(1.0))`):
- Glass panel background with border + corner radius
- SAT TYPE segmented (`.height(Auto)`)
- Divider (1px × 32px, centered)
- OS REALTIME segmented (`.height(Auto)`)
- Divider (1px × 32px, centered)
- OS RENDER segmented (`.height(Auto)`)
- `child_top/child_bottom(Stretch(1.0))` on parent HStack centers Auto-height children
- `col_between(8.0)`, padding `left/right(8.0)`

**Right group** (`width(Auto)`):
- OUT GAIN knob (Sm, accent)
- ThemeToggle (28x28px, sun/moon icon)
- `col_between(12.0)`

---

## 7. Build & Bundle

```bash
# Debug check
cargo check

# Release VST3/CLAP bundle
cargo xtask bundle ivylace --release
```

Output:
- `target/bundled/ivylace.vst3`
- `target/bundled/ivylace.clap`
