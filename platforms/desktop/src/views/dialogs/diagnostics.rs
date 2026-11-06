//! Diagnostics modal for checking system health, audio hardware, and logs.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct DiagnosticsModal {
    pub is_open: bool,
    pub audio_health: bool,
    pub models_health: bool,
    pub storage_health: bool,
    pub network_health: bool,
}

impl DiagnosticsModal {
    pub fn new() -> Self {
        Self {
            is_open: false,
            audio_health: true,
            models_health: true,
            storage_health: true,
            network_health: true,
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        let items = [
            ("Audio Capture Engine", self.audio_health),
            ("Neural Models & VAD", self.models_health),
            ("SQLite WAL Storage", self.storage_health),
            ("Network & Cloud Sync", self.network_health),
        ];

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
                            .w(px(480.0))
                            .p_6()
                            .rounded_xl()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_xl()
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
                                            .child("System Diagnostics"),
                                    )
                                    .child(
                                        Button::new("btn-close-diagnostics")
                                            .ghost()
                                            .small()
                                            .label("✕"),
                                    ),
                            )
                            .child(div().flex().flex_col().gap_2().children(
                                items.into_iter().map(|(name, ok)| {
                                    div()
                                        .id(ElementId::from(format!("diag-item-{}", name)))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().input)
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(name),
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_full()
                                                .bg(if ok {
                                                    hsla(142.0 / 360.0, 0.71, 0.45, 0.15)
                                                } else {
                                                    hsla(0.0, 0.71, 0.45, 0.15)
                                                })
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(if ok {
                                                    hsla(142.0 / 360.0, 0.71, 0.45, 1.0)
                                                } else {
                                                    hsla(0.0, 0.71, 0.45, 1.0)
                                                })
                                                .child(if ok { "HEALTHY" } else { "DEGRADED" }),
                                        )
                                }),
                            )),
                    ),
            )
        })
    }
}
