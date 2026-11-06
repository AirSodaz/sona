//! Error diagnostic dialog for inspecting stack traces and reporting issues.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct ErrorDialog {
    pub is_open: bool,
    pub error_message: String,
    pub stack_trace: String,
}

impl ErrorDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            error_message: String::new(),
            stack_trace: String::new(),
        }
    }

    pub fn show(&mut self, msg: impl Into<String>, stack: impl Into<String>) {
        self.error_message = msg.into();
        self.stack_trace = stack.into();
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div().when(self.is_open, |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(520.0))
                            .p_6()
                            .rounded_xl()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().danger)
                            .shadow_2xl()
                            .gap_4()
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().danger)
                                    .child("Diagnostic Error"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(cx.theme().foreground)
                                    .child(self.error_message.clone()),
                            )
                            .child(
                                div()
                                    .p_3()
                                    .rounded_md()
                                    .bg(cx.theme().input)
                                    .text_size(px(11.0))
                                    .text_color(cx.theme().muted)
                                    .child(self.stack_trace.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("btn-copy-error")
                                            .outline()
                                            .small()
                                            .label("Copy Report"),
                                    )
                                    .child(
                                        Button::new("btn-dismiss-error")
                                            .primary()
                                            .small()
                                            .label("Dismiss"),
                                    ),
                            ),
                    ),
            )
        })
    }
}
