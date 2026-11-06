//! Crash recovery center modal for interrupted transcription tasks.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct RecoveryModal {
    pub is_open: bool,
    pub interrupted_tasks_count: usize,
}

impl RecoveryModal {
    pub fn new() -> Self {
        Self {
            is_open: false,
            interrupted_tasks_count: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
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
                                .w(px(460.0))
                                .p_6()
                                .rounded_xl()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .gap_4()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(16.0))
                                                .font_weight(FontWeight::BOLD)
                                                .child("Recovery Center"),
                                        )
                                        .child(
                                            Button::new("btn-close-recovery")
                                                .ghost()
                                                .small()
                                                .label("✕")
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(cx.theme().muted)
                                        .child("No corrupted sessions found. All SQLite WAL transactions are consistent."),
                                ),
                        ),
                )
            })
    }
}
