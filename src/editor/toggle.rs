/// Toggle button widget for BoolParam.
/// Variants: Normal (band color), Solo (yellow), Sat (amber).
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::theme;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToggleVariant {
    Normal,
    Solo,
    Sat,
    Power,
}

pub struct ToggleButton {
    param_base: ParamWidgetBase,
    variant: ToggleVariant,
    color: vg::Color,
    /// Whether a gesture is currently in progress (begin_set called, end_set pending)
    gesture_active: bool,
}

impl ToggleButton {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        label: &str,
        variant: ToggleVariant,
        color: vg::Color,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let param_base = ParamWidgetBase::new(cx, params.clone(), params_to_param);

        let active_lens = ParamWidgetBase::make_lens(
            params,
            params_to_param,
            |param| param.unmodulated_normalized_value() > 0.5,
        );

        Self {
            param_base,
            variant,
            color,
            gesture_active: false,
        }
        .build(cx, |cx| {
            Label::new(cx, label)
                .font_size(10.0)
                .text_align(TextAlign::Center)
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .hoverable(false);
        })
        .height(Pixels(24.0))
        .border_radius(Pixels(4.0))
        .cursor(CursorIcon::Hand)
        .toggle_class("toggle-active", active_lens)
    }
}

impl View for ToggleButton {
    fn element(&self) -> Option<&'static str> {
        Some("toggle-button")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 {
            return;
        }

        let dpi = cx.scale_factor();
        let active = self.param_base.unmodulated_normalized_value() > 0.5;

        let active_color = match self.variant {
            ToggleVariant::Solo => theme::solo_color(),
            ToggleVariant::Sat => theme::accent(),
            ToggleVariant::Normal | ToggleVariant::Power => self.color,
        };

        // ── Button background ──
        {
            let mut path = vg::Path::new();
            path.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpi);

            if active {
                let paint = vg::Paint::linear_gradient(
                    bounds.x, bounds.y,
                    bounds.x, bounds.y + bounds.h,
                    vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.93),
                    vg::Color::rgbaf(active_color.r, active_color.g, active_color.b, 0.80),
                );
                canvas.fill_path(&path, &paint);

                let mut border_paint = vg::Paint::color(theme::with_alpha(active_color, 0.53));
                border_paint.set_line_width(1.0 * dpi);
                canvas.stroke_path(&path, &border_paint);

                let glow = vg::Paint::box_gradient(
                    bounds.x - 2.0, bounds.y - 2.0,
                    bounds.w + 4.0, bounds.h + 4.0,
                    6.0, 8.0,
                    theme::with_alpha(active_color, 0.25),
                    vg::Color::rgba(0, 0, 0, 0),
                );
                let mut glow_path = vg::Path::new();
                glow_path.rect(bounds.x - 8.0, bounds.y - 8.0, bounds.w + 16.0, bounds.h + 16.0);
                canvas.fill_path(&glow_path, &glow);
            } else {
                canvas.fill_path(&path, &vg::Paint::color(theme::toggle_off_bg()));

                let mut border_paint = vg::Paint::color(theme::toggle_off_border());
                border_paint.set_line_width(1.0 * dpi);
                canvas.stroke_path(&path, &border_paint);
            }
        }

        // ── Power icon (drawn for Power variant) ──
        if self.variant == ToggleVariant::Power {
            let icon_cx = bounds.x + bounds.w * 0.5;
            let icon_cy = bounds.y + bounds.h * 0.5;
            let r = 6.0 * dpi;

            let icon_color = if active {
                vg::Color::rgba(255, 255, 255, 240)
            } else {
                vg::Color::rgba(255, 255, 255, 140)
            };

            // Arc (270° open at top)
            let mut arc_path = vg::Path::new();
            let start = (-60.0_f32).to_radians(); // ~300° → opening at top
            let end = (240.0_f32).to_radians();
            arc_path.arc(icon_cx, icon_cy, r, start, end, vg::Solidity::Hole);
            let mut arc_paint = vg::Paint::color(icon_color);
            arc_paint.set_line_width(1.8 * dpi);
            arc_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&arc_path, &arc_paint);

            // Vertical line at top center
            let mut line_path = vg::Path::new();
            line_path.move_to(icon_cx, icon_cy - r - 1.0 * dpi);
            line_path.line_to(icon_cx, icon_cy - 1.5 * dpi);
            let mut line_paint = vg::Paint::color(icon_color);
            line_paint.set_line_width(1.8 * dpi);
            line_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&line_path, &line_paint);
        }

        // Text is rendered by the child Label via VIZIA's cosmic-text pipeline.
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            match window_event {
                WindowEvent::MouseDown(MouseButton::Left)
                | WindowEvent::MouseDoubleClick(MouseButton::Left)
                | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                    let current = self.param_base.unmodulated_normalized_value();
                    let new_val = if current > 0.5 { 0.0 } else { 1.0 };

                    // Split gesture across event loop iterations:
                    // MouseDown: begin + set value (host sees begin_edit + perform_edit)
                    // MouseUp: end (host sees end_edit in a SEPARATE callback)
                    // This is necessary because some DAWs (Cubase) ignore perform_edit
                    // when begin_edit/perform_edit/end_edit all happen in one callback.
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, new_val);
                    self.gesture_active = true;

                    cx.capture();
                    cx.focus();

                    meta.consume();
                }
                WindowEvent::MouseUp(MouseButton::Left) => {
                    if self.gesture_active {
                        self.param_base.end_set_parameter(cx);
                        self.gesture_active = false;
                        cx.release();
                        meta.consume();
                    }
                }
                _ => {}
            }
        });
    }
}
