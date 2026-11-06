//! Tool: HTTP API Server controller panel.

use gpui_kit::component::button::Button;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct ApiServerToolPanel {
    pub is_running: bool,
    pub port: u16,
    pub address: String,
    pub bearer_auth_enabled: bool,
}

impl ApiServerToolPanel {
    pub fn new() -> Self {
        Self {
            is_running: true,
            port: 39281,
            address: "127.0.0.1".to_string(),
            bearer_auth_enabled: false,
        }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .child("HTTP API Server"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted)
                    .child("Embedded Axum REST & WebSocket service exposing local speech recognition to other tools."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(420.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Server Status")
                            .child(
                                Button::new("toggle-api-status")
                                    .outline()
                                    .small()
                                    .label(if self.is_running { "Running (Active)" } else { "Stopped" }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Listening Endpoint")
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("http://{}:{}", self.address, self.port)),
                            ),
                    ),
            )
    }
}
