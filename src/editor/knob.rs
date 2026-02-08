/// Custom glass rotary knob widget.
/// Supports three sizes (sm/md/lg), arc indicator, drag interaction.
/// Optional threshold linking: when `link_params` is set, dragging propagates
/// delta changes to other linked bands' thresholds.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use super::theme::{self, ThemeMode};
use super::Data;
use crate::{IvylaceParams, SlotStorage};

fn mode_lens() -> impl Lens<Target = ThemeMode> {
    Data::params.map(|p| {
        if p.glass_mode.load(Ordering::Relaxed) { ThemeMode::Glass } else { ThemeMode::Dark }
    })
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
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::new_full(cx, params, params_to_param, size, color, label, glass_mode, None, 0, None)
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
        }
        .build(cx, |cx| {
            Self::build_labels(cx, params, params_to_param, size, label, slot_value_for_label);
        })
        .width(Pixels(size.outer().max(50.0)))
        .height(Auto)
    }

    /// Shared label building logic for both constructors.
    /// `slot_info` is an optional tuple of (slot_a, slot_b, is_b, kind, band_idx)
    /// used to read the display value from the active A/B slot instead of nih-plug params.
    fn build_labels<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        size: KnobSize,
        label: &str,
        slot_info: Option<(Arc<SlotStorage>, Arc<SlotStorage>, Arc<AtomicBool>, SlotParamKind, usize)>,
    ) where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
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
                .hoverable(false);

            // Value display below the label (e.g., "-20.0 dB")
            // When slot_info is provided, read from the active slot atomics
            // instead of nih-plug's internal param value (which may not update during processing).
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
                            param.normalized_value_to_string(param.unmodulated_normalized_value(), true)
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
                .hoverable(false);
        })
        .row_between(Pixels(2.0))
        .width(Stretch(1.0))
        .height(Auto);
    }

    /// Get the effective normalized value for this knob.
    /// Reads from slot atomics when available (A/B mode), otherwise from nih-plug param.
    #[inline]
    fn effective_normalized(&self) -> f32 {
        match &self.slot_value {
            Some(sv) => sv.normalized(),
            None => self.param_base.unmodulated_normalized_value(),
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

        let normalized = match &self.slot_value {
            Some(sv) => sv.normalized(),
            None => self.param_base.unmodulated_normalized_value(),
        };
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
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if cx.modifiers().alt() || cx.modifiers().command() {
                    // Alt/Cmd+Click: reset to default
                    // Split gesture: begin+set on MouseDown, end on MouseUp
                    let current = self.effective_normalized();
                    let default = self.param_base.default_normalized_value();

                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, default);
                    self.reset_gesture_active = true;

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
                // Double-click: reset to default
                let current = self.effective_normalized();
                let default = self.param_base.default_normalized_value();

                // If a drag gesture is already active from the preceding MouseDown,
                // end it first before starting the reset gesture
                if self.drag_active {
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                    self.drag_active = false;
                }
                self.param_base.begin_set_parameter(cx);
                self.param_base.set_normalized_value(cx, default);
                self.reset_gesture_active = true;

                // Link: propagate delta reset
                self.my_start_value = current;
                self.begin_linked(cx);
                self.update_linked(cx, default);

                cx.capture();
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    self.drag_active = false;
                    cx.release();
                    cx.set_active(false);
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                    meta.consume();
                } else if self.reset_gesture_active {
                    // End the reset gesture (alt-click or double-click)
                    self.reset_gesture_active = false;
                    cx.release();
                    self.param_base.end_set_parameter(cx);
                    self.end_linked(cx);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
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
                }
            }
            WindowEvent::KeyUp(_, Some(Key::Shift)) => {
                if self.drag_active && self.granular_drag_status.is_some() {
                    self.granular_drag_status = Some(GranularDragStatus {
                        starting_y: cx.mouse().cursory,
                        starting_value: self.effective_normalized(),
                    });
                }
            }
            WindowEvent::MouseScroll(_sx, sy) => {
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
