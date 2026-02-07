/// Custom glass rotary knob widget.
/// Supports three sizes (sm/md/lg), arc indicator, drag interaction.
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::theme;

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

pub struct GlassKnob {
    param_base: ParamWidgetBase,
    size: KnobSize,
    color: vg::Color,

    drag_active: bool,
    granular_drag_status: Option<GranularDragStatus>,
    /// Whether a non-drag gesture is active (reset via alt-click or double-click)
    /// and needs end_set_parameter on MouseUp
    reset_gesture_active: bool,
}

impl GlassKnob {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        size: KnobSize,
        color: vg::Color,
        label: &str,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let param_base = ParamWidgetBase::new(cx, params.clone(), params_to_param);

        Self {
            param_base,
            size,
            color,
            drag_active: false,
            granular_drag_status: None,
            reset_gesture_active: false,
        }
        .build(cx, |cx| {
            // VStack layout: knob drawing area → label name → value display
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
                    .color(Color::rgba(255, 255, 255, 128))
                    .font_family(vec![FamilyOwned::Name(String::from(nih_plug_vizia::assets::NOTO_SANS))])
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .height(Pixels(13.0))
                    .hoverable(false);

                // Value display below the label (e.g., "-20.0 dB")
                let display_lens = ParamWidgetBase::make_lens(
                    params.clone(),
                    params_to_param,
                    |param| param.normalized_value_to_string(param.unmodulated_normalized_value(), true),
                );

                Label::new(cx, display_lens)
                    .font_size(10.0)
                    .color(Color::rgba(255, 255, 255, 230))
                    .font_family(vec![FamilyOwned::Name(String::from(nih_plug_vizia::assets::NOTO_SANS))])
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .height(Pixels(14.0))
                    .hoverable(false);
            })
            .row_between(Pixels(1.0))
            .width(Stretch(1.0))
            .height(Auto);
        })
        .width(Pixels(size.outer().max(50.0)))
        .height(Auto)
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

        let dpi = cx.scale_factor();
        let outer = self.size.outer() * dpi;
        let inner = self.size.inner() * dpi;
        let indicator_len = self.size.indicator_len() * dpi;

        // Center the knob within bounds
        let cx_x = bounds.x + bounds.w * 0.5;
        let cy_y = bounds.y + outer * 0.5;

        let normalized = self.param_base.unmodulated_normalized_value();
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
            let mut paint = vg::Paint::color(theme::knob_track());
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
                theme::knob_body_light(),
                theme::knob_body_dark(),
            );
            canvas.fill_path(&path, &paint);

            // Border
            let mut border_paint = vg::Paint::color(theme::knob_border());
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

        // Label and value are rendered as VIZIA child Labels (see build())
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if cx.modifiers().alt() || cx.modifiers().command() {
                    // Alt/Cmd+Click: reset to default
                    // Split gesture: begin+set on MouseDown, end on MouseUp
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, self.param_base.default_normalized_value());
                    self.reset_gesture_active = true;
                    cx.capture();
                    cx.focus();
                } else {
                    self.drag_active = true;
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);

                    self.param_base.begin_set_parameter(cx);

                    if cx.modifiers().shift() {
                        self.granular_drag_status = Some(GranularDragStatus {
                            starting_y: cx.mouse().cursory,
                            starting_value: self.param_base.unmodulated_normalized_value(),
                        });
                    } else {
                        self.granular_drag_status = None;
                    }
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                // Double-click: reset to default
                // Split gesture: begin+set on DoubleClick, end on MouseUp
                let default = self.param_base.default_normalized_value();
                // If a drag gesture is already active from the preceding MouseDown,
                // end it first before starting the reset gesture
                if self.drag_active {
                    self.param_base.end_set_parameter(cx);
                    self.drag_active = false;
                }
                self.param_base.begin_set_parameter(cx);
                self.param_base.set_normalized_value(cx, default);
                self.reset_gesture_active = true;
                cx.capture();
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    self.drag_active = false;
                    cx.release();
                    cx.set_active(false);
                    self.param_base.end_set_parameter(cx);
                    meta.consume();
                } else if self.reset_gesture_active {
                    // End the reset gesture (alt-click or double-click)
                    self.reset_gesture_active = false;
                    cx.release();
                    self.param_base.end_set_parameter(cx);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
                if self.drag_active {
                    let sensitivity = if cx.modifiers().shift() { 2000.0 } else { 200.0 };

                    if cx.modifiers().shift() {
                        let status = self.granular_drag_status.get_or_insert(GranularDragStatus {
                            starting_y: *y,
                            starting_value: self.param_base.unmodulated_normalized_value(),
                        });

                        let delta_y = status.starting_y - *y;
                        let delta_normalized = delta_y / sensitivity;
                        let new_value = (status.starting_value + delta_normalized).clamp(0.0, 1.0);
                        self.param_base.set_normalized_value(cx, new_value);
                    } else {
                        if let Some(status) = &self.granular_drag_status {
                            let delta_y = status.starting_y - *y;
                            let delta_normalized = delta_y / sensitivity;
                            let new_value = (status.starting_value + delta_normalized).clamp(0.0, 1.0);
                            self.param_base.set_normalized_value(cx, new_value);
                        } else {
                            self.granular_drag_status = Some(GranularDragStatus {
                                starting_y: *y,
                                starting_value: self.param_base.unmodulated_normalized_value(),
                            });
                        }
                    }
                }
            }
            WindowEvent::KeyUp(_, Some(Key::Shift)) => {
                if self.drag_active && self.granular_drag_status.is_some() {
                    self.granular_drag_status = Some(GranularDragStatus {
                        starting_y: cx.mouse().cursory,
                        starting_value: self.param_base.unmodulated_normalized_value(),
                    });
                }
            }
            WindowEvent::MouseScroll(_sx, sy) => {
                let use_finer = cx.modifiers().shift();
                if !self.drag_active {
                    self.param_base.begin_set_parameter(cx);
                }

                let current = self.param_base.unmodulated_normalized_value();
                let new_val = if *sy > 0.0 {
                    self.param_base.next_normalized_step(current, use_finer)
                } else {
                    self.param_base.previous_normalized_step(current, use_finer)
                };
                self.param_base.set_normalized_value(cx, new_val);

                if !self.drag_active {
                    self.param_base.end_set_parameter(cx);
                }
                meta.consume();
            }
            _ => {}
        });
    }
}
