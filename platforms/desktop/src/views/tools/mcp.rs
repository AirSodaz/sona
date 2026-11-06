//! Tool: Model Context Protocol (MCP) Agent server controller panel.

use gpui_kit::component::button::Button;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct McpToolPanel {
    pub is_enabled: bool,
    pub connected_clients: usize,
    pub allow_transcription: bool,
    pub allow_history_read: bool,
}

impl McpToolPanel {
    pub fn new() -> Self {
        Self {
            is_enabled: true,
            connected_clients: 1,
            allow_transcription: true,
            allow_history_read: true,
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
                    .child("MCP Agent Server"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted)
                    .child("Connect AI agents (Claude, Antigravity, Cursor) directly to Sona via Model Context Protocol."),
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
                            .child("MCP Server Bridge")
                            .child(
                                Button::new("toggle-mcp-enabled")
                                    .outline()
                                    .small()
                                    .label(if self.is_enabled { "Active" } else { "Inactive" }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Connected Clients")
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{} active", self.connected_clients)),
                            ),
                    ),
            )
    }
}
