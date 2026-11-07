//! Global dialog component for alerts, confirmations, and prompts.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::super::shell::MainShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let kind = self.kind;
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
                                    .when(
                                        kind == DialogKind::Confirm || kind == DialogKind::Prompt,
                                        |actions| {
                                            actions.child(
                                                Button::new("btn-dialog-cancel")
                                                    .ghost()
                                                    .small()
                                                    .label("Cancel")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.global_dialog.close();
                                                        cx.notify();
                                                    })),
                                            )
                                        },
                                    )
                                    .child(
                                        Button::new("btn-dialog-ok")
                                            .primary()
                                            .small()
                                            .label(if kind == DialogKind::Confirm {
                                                "Download"
                                            } else {
                                                "OK"
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let confirm_cuda = this.global_dialog.kind
                                                    == DialogKind::Confirm
                                                    && this.global_dialog.title
                                                        == "Download CUDA Add-on";
                                                this.global_dialog.close();
                                                if confirm_cuda {
                                                    this.settings_modal
                                                        .begin_cuda_addon_download(cx);
                                                } else {
                                                    cx.notify();
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
        })
    }
}
