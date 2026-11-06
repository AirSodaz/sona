//! Transcript editor formatting toolbar.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

pub struct EditorToolbar {
    pub is_saving: bool,
}

impl EditorToolbar {
    pub fn new() -> Self {
        Self { is_saving: false }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(40.0))
            .w_full()
            .px_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(Button::new("btn-undo").ghost().xsmall().label("Undo"))
                    .child(Button::new("btn-redo").ghost().xsmall().label("Redo"))
                    .child(div().w(px(1.0)).h(px(14.0)).bg(cx.theme().border))
                    .child(Button::new("btn-bold").ghost().xsmall().label("B"))
                    .child(Button::new("btn-italic").ghost().xsmall().label("I"))
                    .child(div().w(px(1.0)).h(px(14.0)).bg(cx.theme().border))
                    .child(
                        Button::new("btn-split")
                            .outline()
                            .xsmall()
                            .label("Split Segment"),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().muted)
                    .child(if self.is_saving { "Saving..." } else { "Saved" }),
            )
    }
}
