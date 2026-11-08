//! Error diagnostic dialog for inspecting stack traces and reporting issues.
//! Strict 1:1 parity with React `ErrorDialog.tsx`.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::super::shell::MainShell;
use crate::theme::tokens::RADIUS_LG;

pub struct ErrorDialog {
    pub is_open: bool,
    pub title: String,
    pub error_message: String,
    pub stack_trace: String,
}

impl ErrorDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            title: "Error".to_string(),
            error_message: String::new(),
            stack_trace: String::new(),
        }
    }

    pub fn show(&mut self, msg: impl Into<String>, stack: impl Into<String>) {
        self.title = "Diagnostic Error".to_string();
        self.error_message = msg.into();
        self.stack_trace = stack.into();
        self.is_open = true;
    }

    pub fn show_with_title(
        &mut self,
        title: impl Into<String>,
        msg: impl Into<String>,
        stack: impl Into<String>,
    ) {
        self.title = title.into();
        self.error_message = msg.into();
        self.stack_trace = stack.into();
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let has_details = !self.stack_trace.trim().is_empty();

        div().when(self.is_open, |this| {
            this.child(
                div()
                    .id("error-dialog-backdrop")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.error_dialog.close();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id("error-dialog-panel")
                            .flex()
                            .flex_col()
                            .w(px(440.0))
                            .max_w(relative(0.9))
                            .p_6()
                            .rounded(RADIUS_LG)
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_2xl()
                            .gap_4()
                            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            // Header: Icon + Title + Description
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_none()
                                            .pt(px(2.0))
                                            .text_color(cx.theme().danger)
                                            .child(
                                                Icon::new(LucideIcon::CircleAlert).size(px(22.0)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(16.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(cx.theme().foreground)
                                                    .child(self.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .line_height(relative(1.45))
                                                    .text_color(cx.theme().muted)
                                                    .child(self.error_message.clone()),
                                            ),
                                    ),
                            )
                            // Optional details / stack trace block
                            .when(has_details, |this| {
                                this.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1p5()
                                        .pt_2()
                                        .border_t_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().muted)
                                                .child("Details"),
                                        )
                                        .child(
                                            div()
                                                .id("error-dialog-details-scroll")
                                                .max_h(px(160.0))
                                                .overflow_y_scroll()
                                                .p_2p5()
                                                .rounded_md()
                                                .bg(cx.theme().secondary)
                                                .text_size(px(11.0))
                                                .font_family("Consolas, monospace")
                                                .text_color(cx.theme().foreground)
                                                .child(self.stack_trace.clone()),
                                        ),
                                )
                            })
                            // Footer action buttons
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap_2()
                                    .when(has_details, |actions| {
                                        actions.child(
                                            Button::new("btn-copy-error")
                                                .outline()
                                                .small()
                                                .label("Copy Report")
                                                .on_click(cx.listener(|shell, _, _, cx| {
                                                    let text = format!(
                                                        "{}\n\n{}",
                                                        shell.error_dialog.error_message,
                                                        shell.error_dialog.stack_trace
                                                    );
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(text),
                                                    );
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("btn-dismiss-error")
                                            .primary()
                                            .small()
                                            .label("Dismiss")
                                            .on_click(cx.listener(|shell, _, _, cx| {
                                                shell.error_dialog.close();
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
        })
    }
}
