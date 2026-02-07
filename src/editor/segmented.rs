/// Segmented control widget for stepped EnumParam parameters.
/// Used for Ratio, Attack, Release selectors, and header controls.
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::theme;

/// A segmented parameter control that displays all enum variants as buttons.
pub struct SegmentedParam;

impl SegmentedParam {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        label: Option<&str>,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone + Send + Sync,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static + Send + Sync,
    {
        Self.build(cx, |cx| {
            // Optional label above the button row
            if let Some(label_text) = label {
                Label::new(cx, label_text)
                    .font_size(8.0)
                    .color(Color::rgba(255, 255, 255, 100))
                    .text_align(TextAlign::Center)
                    .width(Stretch(1.0))
                    .height(Pixels(11.0))
                    .hoverable(false);
            }

            // Container for the buttons
            HStack::new(cx, |cx| {
                let param_base_temp = ParamWidgetBase::new(cx, params.clone(), params_to_param);
                let step_count = param_base_temp.step_count().unwrap_or(0);
                if step_count == 0 {
                    return;
                }

                for i in 0..=step_count {
                    let normalized = i as f32 / step_count as f32;
                    let display = param_base_temp.normalized_value_to_string(normalized, false);

                    SegmentButton::new(
                        cx,
                        params.clone(),
                        params_to_param,
                        &display,
                        i,
                        step_count,
                        normalized,
                    );
                }
            })
            .height(Pixels(18.0))
            .width(Stretch(1.0))
            .background_color(Color::rgba(255, 255, 255, 10))
            .border_color(Color::rgba(255, 255, 255, 15))
            .border_width(Pixels(1.0))
            .border_radius(Pixels(3.0));
        })
        .width(Stretch(1.0))
    }
}

impl View for SegmentedParam {
    fn element(&self) -> Option<&'static str> {
        Some("segmented-param")
    }
}

// ── Individual segment button ────────────────────────────────

struct SegmentButton {
    param_base: ParamWidgetBase,
    step_index: usize,
    step_count: usize,
    normalized_value: f32,
    /// Whether a gesture is currently in progress (begin_set called, end_set pending)
    gesture_active: bool,
}

impl SegmentButton {
    fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        label: &str,
        step_index: usize,
        step_count: usize,
        normalized_value: f32,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone + Send + Sync,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static + Send + Sync,
    {
        let param_base = ParamWidgetBase::new(cx, params.clone(), params_to_param);

        let is_active_lens = ParamWidgetBase::make_lens(
            params.clone(),
            params_to_param,
            move |param| {
                let current = param.unmodulated_normalized_value();
                let current_step = (current * step_count as f32).round() as usize;
                current_step == step_index
            },
        );

        let text_color_lens = is_active_lens.map(|&is_active| {
            if is_active {
                Color::rgba(255, 255, 255, 242)
            } else {
                Color::rgba(255, 255, 255, 89)
            }
        });

        let label_owned = label.to_string();

        Self {
            param_base,
            step_index,
            step_count,
            normalized_value,
            gesture_active: false,
        }
        .build(cx, move |cx| {
            Label::new(cx, &label_owned)
                .font_size(9.0)
                .text_align(TextAlign::Center)
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .hoverable(false)
                .color(text_color_lens);
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .cursor(CursorIcon::Hand)
    }
}

impl View for SegmentButton {
    fn element(&self) -> Option<&'static str> {
        Some("segment-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            match window_event {
                WindowEvent::MouseDown(MouseButton::Left)
                | WindowEvent::MouseDoubleClick(MouseButton::Left)
                | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                    // Split gesture across event loop iterations:
                    // MouseDown: begin + set value
                    // MouseUp: end
                    // Cubase ignores perform_edit when begin/perform/end happen in one callback.
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, self.normalized_value);
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

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w < 1.0 || bounds.h < 1.0 {
            return;
        }

        let dpi = cx.scale_factor();

        let current_normalized = self.param_base.unmodulated_normalized_value();
        let current_step = (current_normalized * self.step_count as f32).round() as usize;
        let is_active = current_step == self.step_index;

        if is_active {
            let mut path = vg::Path::new();
            path.rounded_rect(
                bounds.x + 1.0, bounds.y + 1.0,
                bounds.w - 2.0, bounds.h - 2.0,
                2.0 * dpi,
            );
            canvas.fill_path(&path, &vg::Paint::color(theme::seg_active_bg()));
        }
    }
}
