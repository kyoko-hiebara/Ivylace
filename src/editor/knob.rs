/// Custom glass rotary knob widget.
/// Supports three sizes (sm/md/lg), arc indicator, drag interaction.
/// Optional threshold linking: when `link_params` is set, dragging propagates
/// delta changes to other linked bands' thresholds.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use super::theme::{self, ThemeMode};
use super::Data;
use crate::{GlobalOverrides, IvylaceParams, SlotStorage};

fn mode_lens() -> impl Lens<Target = ThemeMode> {
    Data::params.map(|p| {
        if p.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark }
    })
}

/// Sentinel value for `pending_display`: indicates no pending override.
/// Uses `f32::NAN` bits which can never be a valid normalized value.
const PENDING_NONE: u32 = f32::NAN.to_bits();

/// Identifies which global override field a non-slot knob writes to.
/// Used for immediate GUI→audio value bypass (VST3 deferred edit workaround).
#[derive(Clone, Copy)]
pub(crate) enum GlobalOverrideKind {
    InputGainDb,
    OutputGainDb,
    DryWet,
    DriveDb(usize), // band index
}

/// Describes which slot field a knob should read for display override.
#[derive(Clone, Copy)]
pub(crate) enum SlotParamKind {
    ThresholdDb,
    MakeupDb,
    ScHpfHz,
}

/// Optional slot value source for A/B display override.
pub(crate) struct SlotValueSource {
    pub slot_a: Arc<SlotStorage>,
    pub slot_b: Arc<SlotStorage>,
    pub is_b: Arc<AtomicBool>,
    pub kind: SlotParamKind,
    pub band_idx: usize,
}

impl SlotValueSource {
    /// Get the normalized value (0..1) from the active slot.
    #[inline]
    fn normalized(&self) -> f32 {
        let slot = if self.is_b.load(Ordering::Relaxed) { &self.slot_b } else { &self.slot_a };
        match self.kind {
            SlotParamKind::ThresholdDb => slot.threshold_normalized(self.band_idx),
            SlotParamKind::MakeupDb => slot.makeup_normalized(self.band_idx),
            SlotParamKind::ScHpfHz => slot.sc_hpf_normalized(self.band_idx),
        }
    }

    /// Write a plain value directly to the active slot's atomic.
    /// This enables immediate display update even when the host defers the
    /// parameter change (VST3 deferred edit during audio processing).
    #[inline]
    fn store_plain(&self, plain: f32) {
        let slot = if self.is_b.load(Ordering::Relaxed) { &self.slot_b } else { &self.slot_a };
        match self.kind {
            SlotParamKind::ThresholdDb => slot.threshold_db[self.band_idx].store(plain),
            SlotParamKind::MakeupDb => slot.makeup_db[self.band_idx].store(plain),
            SlotParamKind::ScHpfHz => slot.sc_hpf_hz[self.band_idx].store(plain),
        }
    }

    /// Get threshold normalized value for an arbitrary band from the active slot.
    /// Used by linked-drag to read other bands' threshold values.
    #[inline]
    fn threshold_normalized_for_band(&self, band: usize) -> f32 {
        let slot = if self.is_b.load(Ordering::Relaxed) { &self.slot_b } else { &self.slot_a };
        slot.threshold_normalized(band)
    }
}

/// Knob sizes matching Figma spec
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KnobSize {
    Sm, // outer=36, inner=28
    Md, // outer=48, inner=38
    Lg, // outer=68, inner=56
}

impl KnobSize {
    pub fn outer(self) -> f32 {
        match self {
            KnobSize::Sm => 36.0,
            KnobSize::Md => 48.0,
            KnobSize::Lg => 68.0,
        }
    }
    pub fn inner(self) -> f32 {
        match self {
            KnobSize::Sm => 28.0,
            KnobSize::Md => 38.0,
            KnobSize::Lg => 56.0,
        }
    }
    pub fn indicator_len(self) -> f32 {
        match self {
            KnobSize::Sm => 10.0,
            KnobSize::Md => 14.0,
            KnobSize::Lg => 20.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GranularDragStatus {
    starting_y: f32,
    starting_value: f32,
}

/// State for linked bands during a drag gesture (delta mode).
/// Records each linked band's normalized threshold at drag start.
struct LinkedDragState {
    /// (band_index, start_normalized_value, ParamPtr)
    entries: Vec<(usize, f32, ParamPtr)>,
}

pub struct GlassKnob {
    param_base: ParamWidgetBase,
    size: KnobSize,
    color: vg::Color,
    glass_mode: Arc<AtomicBool>,

    drag_active: bool,
    granular_drag_status: Option<GranularDragStatus>,
    /// Whether a non-drag gesture is active (reset via alt-click or double-click)
    /// and needs end_set_parameter on MouseUp
    reset_gesture_active: bool,

    // --- Link support (optional, only for threshold knobs) ---
    /// Access to all params for reading link state and setting other thresholds
    link_params: Option<Arc<IvylaceParams>>,
    /// Which band index this knob belongs to
    link_band_idx: usize,
    /// The normalized value of this knob when a linked drag started
    my_start_value: f32,
    /// Active linked drag state (populated on drag start if any bands are linked)
    linked_drag: Option<LinkedDragState>,

    /// Optional A/B slot value source for display override
    slot_value: Option<SlotValueSource>,

    /// Whether the value label is in text editing mode (double-click to enter).
    /// Arc<AtomicBool> so child Labels can read it via lens and hide themselves.
    editing: Arc<AtomicBool>,
    /// Buffer for typed text during editing
    edit_buffer: String,

    /// Pending normalized value for display override (non-slot knobs only).
    /// Stores f32 bits in AtomicU32. Set after keyboard/drag/scroll edits to
    /// ensure immediate display update even when VST3 defers the parameter change.
    /// PENDING_NONE sentinel means "use param's internal value".
    pending_display: Arc<AtomicU32>,

    /// Optional global override for immediate audio-thread value bypass.
    /// Non-slot knobs set this so process() picks up the plain value instantly.
    global_override: Option<(Arc<GlobalOverrides>, GlobalOverrideKind)>,
}

impl GlassKnob {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        size: KnobSize,
        color: vg::Color,
        label: &str,
        glass_mode: Arc<AtomicBool>,
        global_override: Option<(Arc<GlobalOverrides>, GlobalOverrideKind)>,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::new_full(cx, params, params_to_param, size, color, label, glass_mode, None, 0, None, global_override)
    }

    /// Full constructor with all options (link support + slot value source).
    pub fn new_full<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        size: KnobSize,
        color: vg::Color,
        label: &str,
        glass_mode: Arc<AtomicBool>,
        link_params: Option<Arc<IvylaceParams>>,
        band_idx: usize,
        slot_value: Option<SlotValueSource>,
        global_override: Option<(Arc<GlobalOverrides>, GlobalOverrideKind)>,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let param_base = ParamWidgetBase::new(cx, params.clone(), params_to_param);

        // Clone slot info for the label lens (self takes ownership of the original)
        let slot_value_for_label: Option<(Arc<SlotStorage>, Arc<SlotStorage>, Arc<AtomicBool>, SlotParamKind, usize)> =
            slot_value.as_ref().map(|sv| {
                (sv.slot_a.clone(), sv.slot_b.clone(), sv.is_b.clone(), sv.kind, sv.band_idx)
            });

        let editing = Arc::new(AtomicBool::new(false));
        let editing_for_labels = editing.clone();
        let pending_display = Arc::new(AtomicU32::new(PENDING_NONE));
        let pending_display_for_labels = pending_display.clone();

        Self {
            param_base,
            size,
            color,
            glass_mode,
            drag_active: false,
            granular_drag_status: None,
            reset_gesture_active: false,
            link_params,
            link_band_idx: band_idx,
            my_start_value: 0.0,
            linked_drag: None,
            slot_value,
            editing,
            edit_buffer: String::new(),
            pending_display,
            global_override,
        }
        .build(cx, |cx| {
            Self::build_labels(cx, params, params_to_param, size, label, slot_value_for_label, editing_for_labels, pending_display_for_labels);
        })
        .width(Pixels(size.outer().max(50.0)))
        .height(Auto)
    }

    /// Shared label building logic for both constructors.
    /// `slot_info` is an optional tuple of (slot_a, slot_b, is_b, kind, band_idx)
    /// used to read the display value from the active A/B slot instead of nih-plug params.
    /// `editing` flag is used to hide labels during text input mode.
    fn build_labels<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        size: KnobSize,
        label: &str,
        slot_info: Option<(Arc<SlotStorage>, Arc<SlotStorage>, Arc<AtomicBool>, SlotParamKind, usize)>,
        editing: Arc<AtomicBool>,
        pending_display: Arc<AtomicU32>,
    ) where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        // Lens to hide labels while editing.
        // Use Visibility::Hidden (not Display::None) to preserve layout space.
        let editing_for_name = editing.clone();
        let editing_for_value = editing;

        VStack::new(cx, |cx| {
            // Spacer element for the knob drawing area (drawn by View::draw)
            Element::new(cx)
                .width(Pixels(size.outer()))
                .height(Pixels(size.outer()))
                .left(Stretch(1.0))
                .right(Stretch(1.0))
                .hoverable(false);

            // Label name below the knob (e.g., "THRESHOLD", "IN GAIN")
            Label::new(cx, &label.to_uppercase())
                .font_size(9.0)
                .font_weight(FontWeightKeyword::Medium)
                .color(mode_lens().map(|m| theme::to_vizia(theme::text_secondary(*m))))
                .font_family(vec![FamilyOwned::Name(String::from(
                    nih_plug_vizia::assets::NOTO_SANS,
                ))])
                .text_align(TextAlign::Center)
                .width(Stretch(1.0))
                .height(Pixels(13.0))
                .hoverable(false)
                .visibility(Data::params.map(move |_| {
                    if editing_for_name.load(Ordering::Relaxed) { Visibility::Hidden } else { Visibility::Visible }
                }));

            // Value display below the label (e.g., "-20.0 dB")
            // Priority:
            //   1. slot_info → read from active slot atomics (slot knobs)
            //   2. pending_display → read from per-knob atomic (non-slot knobs, set on keyboard/drag edit)
            //   3. param.unmodulated_normalized_value() → nih-plug internal (fallback)
            let pending_for_lens = pending_display;
            let display_lens = ParamWidgetBase::make_lens(
                params.clone(),
                params_to_param,
                move |param| {
                    match &slot_info {
                        Some((sa, sb, is_b, kind, idx)) => {
                            let slot = if is_b.load(Ordering::Relaxed) { sb } else { sa };
                            let normalized = match kind {
                                SlotParamKind::ThresholdDb => slot.threshold_normalized(*idx),
                                SlotParamKind::MakeupDb => slot.makeup_normalized(*idx),
                                SlotParamKind::ScHpfHz => slot.sc_hpf_normalized(*idx),
                            };
                            param.normalized_value_to_string(normalized, true)
                        }
                        None => {
                            // Check pending_display for non-slot knobs
                            let pending_bits = pending_for_lens.load(Ordering::Relaxed);
                            let normalized = if pending_bits != PENDING_NONE {
                                f32::from_bits(pending_bits)
                            } else {
                                param.unmodulated_normalized_value()
                            };
                            param.normalized_value_to_string(normalized, true)
                        }
                    }
                },
            );

            Label::new(cx, display_lens)
                .font_size(10.0)
                .color(mode_lens().map(|m| theme::to_vizia(theme::text_primary(*m))))
                .font_family(vec![FamilyOwned::Name(String::from(
                    nih_plug_vizia::assets::NOTO_SANS,
                ))])
                .text_align(TextAlign::Center)
                .width(Stretch(1.0))
                .height(Pixels(14.0))
                .hoverable(false)
                .visibility(Data::params.map(move |_| {
                    if editing_for_value.load(Ordering::Relaxed) { Visibility::Hidden } else { Visibility::Visible }
                }));
        })
        .row_between(Pixels(2.0))
        .width(Stretch(1.0))
        .height(Auto);
    }

    /// Get the effective normalized value for this knob.
    /// Priority: slot atomics > pending_display > nih-plug param internal value.
    #[inline]
    fn effective_normalized(&self) -> f32 {
        match &self.slot_value {
            Some(sv) => sv.normalized(),
            None => {
                let pending_bits = self.pending_display.load(Ordering::Relaxed);
                if pending_bits != PENDING_NONE {
                    f32::from_bits(pending_bits)
                } else {
                    self.param_base.unmodulated_normalized_value()
                }
            }
        }
    }

    /// Write a plain value to the global override for immediate audio-thread pickup.
    /// Also updates `pending_display` for immediate GUI display.
    #[inline]
    fn apply_override(&self, normalized: f32) {
        // Always update pending_display for GUI
        if self.slot_value.is_none() {
            self.pending_display.store(normalized.to_bits(), Ordering::Relaxed);
        }
        // Write to audio-thread override
        if let Some((ovr, kind)) = &self.global_override {
            let plain = self.param_base.preview_plain(normalized);
            match kind {
                GlobalOverrideKind::InputGainDb => ovr.input_gain_db.store(plain),
                GlobalOverrideKind::OutputGainDb => ovr.output_gain_db.store(plain),
                GlobalOverrideKind::DryWet => ovr.dry_wet.store(plain),
                GlobalOverrideKind::DriveDb(band) => ovr.drive_db[*band].store(plain),
            }
        }
    }

    /// Clear global override (revert to nih-plug smoothed value on audio thread).
    #[inline]
    fn clear_override(&self) {
        if let Some((ovr, kind)) = &self.global_override {
            match kind {
                GlobalOverrideKind::InputGainDb => ovr.clear_input_gain(),
                GlobalOverrideKind::OutputGainDb => ovr.clear_output_gain(),
                GlobalOverrideKind::DryWet => ovr.clear_dry_wet(),
                GlobalOverrideKind::DriveDb(band) => ovr.clear_drive(*band),
            }
        }
        // Also clear pending display
        if self.slot_value.is_none() {
            self.pending_display.store(PENDING_NONE, Ordering::Relaxed);
        }
    }

    // ── Link helpers ──

    /// Collect linked bands and begin their parameter gestures.
    /// Returns true if any linked bands were found.
    fn begin_linked(&mut self, cx: &mut EventContext) -> bool {
        let Some(all_params) = &self.link_params else {
            return false;
        };

        // This band must have link enabled
        if !all_params.bands[self.link_band_idx].link.value() {
            self.linked_drag = None;
            return false;
        }

        let mut entries = Vec::new();
        for i in 0..crate::NUM_BANDS {
            if i == self.link_band_idx {
                continue;
            }
            if all_params.bands[i].link.value() {
                let param = &all_params.bands[i].threshold;
                let ptr = param.as_ptr();
                // Read start value from slot atomics when available (A/B mode),
                // otherwise fall back to nih-plug's internal param value.
                let start_val = match &self.slot_value {
                    Some(sv) => sv.threshold_normalized_for_band(i),
                    None => unsafe { ptr.unmodulated_normalized_value() },
                };
                entries.push((i, start_val, ptr));
            }
        }

        if entries.is_empty() {
            self.linked_drag = None;
            return false;
        }

        // Begin gesture for each linked band
        for &(_, _, ptr) in &entries {
            cx.emit(RawParamEvent::BeginSetParameter(ptr));
        }

        // Read own start value from slot when available
        self.my_start_value = match &self.slot_value {
            Some(sv) => sv.normalized(),
            None => self.param_base.unmodulated_normalized_value(),
        };
        self.linked_drag = Some(LinkedDragState { entries });
        true
    }

    /// Propagate a delta change to all linked bands.
    fn update_linked(&self, cx: &mut EventContext, new_normalized: f32) {
        let Some(state) = &self.linked_drag else {
            return;
        };
        let delta = new_normalized - self.my_start_value;
        for &(_, start_val, ptr) in &state.entries {
            let linked_new = (start_val + delta).clamp(0.0, 1.0);
            cx.emit(RawParamEvent::SetParameterNormalized(ptr, linked_new));
        }
    }

    /// End gesture for all linked bands.
    fn end_linked(&mut self, cx: &mut EventContext) {
        if let Some(state) = self.linked_drag.take() {
            for (_, _, ptr) in state.entries {
                cx.emit(RawParamEvent::EndSetParameter(ptr));
            }
        }
    }
}

impl View for GlassKnob {
    fn element(&self) -> Option<&'static str> {
        Some("glass-knob")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let mode = if self.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark };
        let dpi = cx.scale_factor();
        let outer = self.size.outer() * dpi;
        let inner = self.size.inner() * dpi;
        let indicator_len = self.size.indicator_len() * dpi;

        // Center the knob within bounds
        let cx_x = bounds.x + bounds.w * 0.5;
        let cy_y = bounds.y + outer * 0.5;

        let normalized = self.effective_normalized();
        let pct = normalized.clamp(0.0, 1.0);

        // Arc geometry: 270° sweep from -135° to +135°
        let start_angle = -225.0_f32.to_radians();
        let end_angle = 45.0_f32.to_radians();
        let value_angle = start_angle + pct * (end_angle - start_angle);
        let arc_radius = inner * 0.5 + 1.0 * dpi;

        // ── Background arc track ──
        {
            let mut path = vg::Path::new();
            path.arc(cx_x, cy_y, arc_radius, start_angle, end_angle, vg::Solidity::Hole);
            let mut paint = vg::Paint::color(theme::knob_track(mode));
            paint.set_line_width(2.5 * dpi);
            paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&path, &paint);
        }

        // ── Value arc ──
        if pct > 0.005 {
            let mut path = vg::Path::new();
            path.arc(cx_x, cy_y, arc_radius, start_angle, value_angle, vg::Solidity::Hole);

            let mut paint = vg::Paint::color(self.color);
            paint.set_line_width(2.5 * dpi);
            paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&path, &paint);

            // Glow effect
            let glow_color = theme::with_alpha(self.color, 0.3);
            let mut glow_paint = vg::Paint::color(glow_color);
            glow_paint.set_line_width(6.0 * dpi);
            glow_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&path, &glow_paint);
        }

        // ── Glass knob body ──
        {
            let mut path = vg::Path::new();
            path.circle(cx_x, cy_y, inner * 0.5);

            // Multi-stop radial gradient to simulate glass
            let paint = vg::Paint::radial_gradient(
                cx_x - inner * 0.15,
                cy_y - inner * 0.15,
                inner * 0.1,
                inner * 0.6,
                theme::knob_body_light(mode),
                theme::knob_body_dark(mode),
            );
            canvas.fill_path(&path, &paint);

            // Border
            let mut border_paint = vg::Paint::color(theme::knob_border(mode));
            border_paint.set_line_width(1.0 * dpi);
            canvas.stroke_path(&path, &border_paint);
        }

        // ── Indicator line ──
        {
            let indicator_angle = -std::f32::consts::FRAC_PI_2 + (-135.0_f32 + pct * 270.0).to_radians();
            let ind_start_r = inner * 0.5 - indicator_len;
            let ind_end_r = inner * 0.5 - 3.0 * dpi;

            let x1 = cx_x + ind_start_r * indicator_angle.cos();
            let y1 = cy_y + ind_start_r * indicator_angle.sin();
            let x2 = cx_x + ind_end_r * indicator_angle.cos();
            let y2 = cy_y + ind_end_r * indicator_angle.sin();

            let mut path = vg::Path::new();
            path.move_to(x1, y1);
            path.line_to(x2, y2);

            let mut paint = vg::Paint::color(self.color);
            paint.set_line_width(2.0 * dpi);
            paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&path, &paint);

            // Indicator glow
            let glow = theme::with_alpha(self.color, 0.5);
            let mut glow_paint = vg::Paint::color(glow);
            glow_paint.set_line_width(4.0 * dpi);
            glow_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&path, &glow_paint);
        }

        // Label and value are rendered as VIZIA child Labels (see build_labels())

        // ── Inline text editing overlay ──
        // Labels are hidden via Display::None (driven by editing AtomicBool lens).
        // Draw the text input box in their place below the knob.
        if self.editing.load(Ordering::Relaxed) {
            // Input box: centered below the knob circle, same area as the hidden labels
            let edit_w = bounds.w.max(56.0 * dpi);
            let edit_h = 20.0 * dpi;
            let edit_x = bounds.x + (bounds.w - edit_w) * 0.5;
            // Position: vertically center in the label area below knob
            // Label area starts at bounds.y + outer, total height ~31px
            let label_area_h = 31.0 * dpi;
            let edit_y = bounds.y + outer + (label_area_h - edit_h) * 0.5;

            // Background: glass panel style
            let mut bg_path = vg::Path::new();
            bg_path.rounded_rect(edit_x, edit_y, edit_w, edit_h, 3.0 * dpi);
            let bg_color = match mode {
                ThemeMode::Dark => vg::Color::rgba(80, 65, 130, 200),  // semi-opaque purple
                ThemeMode::Glass => vg::Color::rgba(220, 235, 250, 220), // semi-opaque light blue
            };
            canvas.fill_path(&bg_path, &vg::Paint::color(bg_color));

            // Accent border using the knob's color
            let mut bp = vg::Paint::color(self.color);
            bp.set_line_width(1.5 * dpi);
            canvas.stroke_path(&bg_path, &bp);

            // Text content — centered in the box
            let font = canvas.add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR).ok();
            if let Some(font) = font {
                if self.edit_buffer.is_empty() {
                    // Placeholder: show current value dimmed + cursor
                    let placeholder = format!(
                        "{}|",
                        self.param_base.normalized_value_to_string(self.effective_normalized(), false)
                    );
                    let mut paint = vg::Paint::color(theme::with_alpha(theme::text_primary(mode), 0.35));
                    paint.set_font_size(11.0 * dpi);
                    paint.set_text_align(vg::Align::Center);
                    paint.set_text_baseline(vg::Baseline::Middle);
                    paint.set_font(&[font]);
                    let _ = canvas.fill_text(
                        edit_x + edit_w * 0.5,
                        edit_y + edit_h * 0.5,
                        &placeholder,
                        &paint,
                    );
                } else {
                    // Active input: typed text + cursor
                    let display = format!("{}|", self.edit_buffer);
                    let mut paint = vg::Paint::color(theme::text_primary(mode));
                    paint.set_font_size(11.0 * dpi);
                    paint.set_text_align(vg::Align::Center);
                    paint.set_text_baseline(vg::Baseline::Middle);
                    paint.set_font(&[font]);
                    let _ = canvas.fill_text(
                        edit_x + edit_w * 0.5,
                        edit_y + edit_h * 0.5,
                        &display,
                        &paint,
                    );
                }
            }
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            // ── Keyboard: text editing ──
            WindowEvent::CharInput(c) => {
                if self.editing.load(Ordering::Relaxed) {
                    // Accept numeric characters relevant to param values
                    if c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '%' || *c == ' ' {
                        self.edit_buffer.push(*c);
                        cx.needs_redraw();
                    }
                    meta.consume();
                }
            }
            WindowEvent::KeyDown(code, _) => {
                if self.editing.load(Ordering::Relaxed) {
                    match code {
                        Code::Enter | Code::NumpadEnter => {
                            // Try to parse and apply the value
                            let parsed = self.param_base.string_to_normalized_value(&self.edit_buffer);
                            if let Some(normalized) = parsed {
                                self.param_base.begin_set_parameter(cx);
                                self.param_base.set_normalized_value(cx, normalized);
                                self.param_base.end_set_parameter(cx);
                                // Immediate display + audio override
                                if let Some(sv) = &self.slot_value {
                                    let plain = self.param_base.preview_plain(normalized);
                                    sv.store_plain(plain);
                                }
                                self.apply_override(normalized);
                            }
                            self.editing.store(false, Ordering::Relaxed);
                            self.edit_buffer.clear();
                            cx.release();
                            cx.set_active(false);
                            cx.needs_redraw();
                            meta.consume();
                        }
                        Code::Escape => {
                            // Cancel editing without changing value
                            self.editing.store(false, Ordering::Relaxed);
                            self.edit_buffer.clear();
                            cx.release();
                            cx.set_active(false);
                            cx.needs_redraw();
                            meta.consume();
                        }
                        Code::Backspace => {
                            self.edit_buffer.pop();
                            cx.needs_redraw();
                            meta.consume();
                        }
                        _ => { meta.consume(); }
                    }
                }
            }
            WindowEvent::FocusOut => {
                if self.editing.load(Ordering::Relaxed) {
                    // Lost focus: cancel editing
                    self.editing.store(false, Ordering::Relaxed);
                    self.edit_buffer.clear();
                    cx.needs_redraw();
                }
            }

            // ── Mouse interactions ──
            WindowEvent::MouseDown(MouseButton::Left) => {
                if self.editing.load(Ordering::Relaxed) {
                    // Click while editing: confirm the edit first
                    if let Some(normalized) = self.param_base.string_to_normalized_value(&self.edit_buffer) {
                        self.param_base.begin_set_parameter(cx);
                        self.param_base.set_normalized_value(cx, normalized);
                        self.param_base.end_set_parameter(cx);
                        // Immediate display + audio override
                        if let Some(sv) = &self.slot_value {
                            let plain = self.param_base.preview_plain(normalized);
                            sv.store_plain(plain);
                        }
                        self.apply_override(normalized);
                    }
                    self.editing.store(false, Ordering::Relaxed);
                    self.edit_buffer.clear();
                    cx.release();
                    cx.set_active(false);
                    cx.needs_redraw();
                    meta.consume();
                    return;
                }

                if cx.modifiers().alt() || cx.modifiers().command() {
                    // Alt/Cmd+Click: reset to default
                    // Split gesture: begin+set on MouseDown, end on MouseUp
                    let current = self.effective_normalized();
                    let default = self.param_base.default_normalized_value();

                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, default);
                    self.reset_gesture_active = true;
                    self.apply_override(default);

                    // Link: propagate delta to linked bands
                    self.my_start_value = current;
                    self.begin_linked(cx);
                    self.update_linked(cx, default);

                    cx.capture();
                    cx.focus();
                } else {
                    self.drag_active = true;
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);

                    self.param_base.begin_set_parameter(cx);

                    // Link: begin linked gesture
                    self.begin_linked(cx);

                    if cx.modifiers().shift() {
                        self.granular_drag_status = Some(GranularDragStatus {
                            starting_y: cx.mouse().cursory,
                            starting_value: self.effective_normalized(),
                        });
                    } else {
                        self.granular_drag_status = None;
                    }
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                // Double-click: enter text editing mode
                // If a drag gesture is already active from the preceding MouseDown,
                // end it first
                if self.drag_active {
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                    self.drag_active = false;
                }
                if self.reset_gesture_active {
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                    self.reset_gesture_active = false;
                }

                // Start with empty buffer — user types the desired value from scratch
                // Show placeholder with current value in draw() as hint
                self.edit_buffer.clear();
                self.editing.store(true, Ordering::Relaxed);
                cx.capture();
                cx.focus();
                cx.set_active(true);
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.editing.load(Ordering::Relaxed) {
                    // Don't end editing on mouse up
                    return;
                }
                if self.drag_active {
                    self.drag_active = false;
                    cx.release();
                    cx.set_active(false);
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                    meta.consume();
                } else if self.reset_gesture_active {
                    // End the reset gesture (alt-click)
                    self.reset_gesture_active = false;
                    cx.release();
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
                if self.editing.load(Ordering::Relaxed) { return; }
                if self.drag_active {
                    let sensitivity = if cx.modifiers().shift() { 2000.0 } else { 200.0 };

                    let new_value = if cx.modifiers().shift() {
                        let status = self.granular_drag_status.get_or_insert(GranularDragStatus {
                            starting_y: *y,
                            starting_value: self.effective_normalized(),
                        });

                        let delta_y = status.starting_y - *y;
                        let delta_normalized = delta_y / sensitivity;
                        (status.starting_value + delta_normalized).clamp(0.0, 1.0)
                    } else if let Some(status) = &self.granular_drag_status {
                        let delta_y = status.starting_y - *y;
                        let delta_normalized = delta_y / sensitivity;
                        (status.starting_value + delta_normalized).clamp(0.0, 1.0)
                    } else {
                        self.granular_drag_status = Some(GranularDragStatus {
                            starting_y: *y,
                            starting_value: self.effective_normalized(),
                        });
                        return; // First move just records starting position
                    };

                    self.param_base.set_normalized_value(cx, new_value);
                    self.update_linked(cx, new_value);
                    self.apply_override(new_value);
                }
            }
            WindowEvent::KeyUp(_, Some(Key::Shift)) => {
                if self.editing.load(Ordering::Relaxed) { return; }
                if self.drag_active && self.granular_drag_status.is_some() {
                    self.granular_drag_status = Some(GranularDragStatus {
                        starting_y: cx.mouse().cursory,
                        starting_value: self.effective_normalized(),
                    });
                }
            }
            WindowEvent::MouseScroll(_sx, sy) => {
                if self.editing.load(Ordering::Relaxed) { return; }
                let use_finer = cx.modifiers().shift();
                let current = self.effective_normalized();

                if !self.drag_active {
                    self.param_base.begin_set_parameter(cx);
                    // Link: begin for scroll
                    self.my_start_value = current;
                    self.begin_linked(cx);
                }

                let new_val = if *sy > 0.0 {
                    self.param_base.next_normalized_step(current, use_finer)
                } else {
                    self.param_base.previous_normalized_step(current, use_finer)
                };
                self.param_base.set_normalized_value(cx, new_val);
                self.update_linked(cx, new_val);
                self.apply_override(new_val);

                if !self.drag_active {
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                }
                meta.consume();
            }
            _ => {}
        });
    }
}
