//! Global dialog component for alerts, confirmations, and prompts.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

#[derive(Clone, Debug)]
pub enum DialogKind {
    Alert,
    Confirm,
    Prompt,
}

pub struct GlobalDialog {
    pub is_open: bool,
    pub title: String,
    pub message: String,
    pub kind: DialogKind,
}

impl GlobalDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            title: String::new(),
            message: String::new(),
            kind: DialogKind::Alert,
        }
    }

    pub fn show(&mut self, title: impl Into<String>, msg: impl Into<String>, kind: DialogKind) {
        self.title = title.into();
        self.message = msg.into();
        self.kind = kind;
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .when(self.is_open, |this| {
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
                                .w(px(420.0))
                                .p_6()
                                .rounded_xl()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_xl()
                                .gap_4()
                                .child(
                                    div()
                                        .text_size(px(16.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child(self.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(cx.theme().muted)
                                        .child(self.message.clone()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("btn-dialog-ok")
                                                .primary()
                                                .small()
                                                .label("OK")
                                        ),
                                ),
                        ),
                )
            })
    }
}
