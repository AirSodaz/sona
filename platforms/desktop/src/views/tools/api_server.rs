//! Tool: HTTP API Server controller panel.

use crate::views::shell::MainShell;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct ApiServerToolPanel;

impl ApiServerToolPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let config = services
            .config
            .load()
            .ok()
            .flatten()
            .unwrap_or_else(|| serde_json::json!({}));
        let enabled = config
            .get("httpServerEnabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let host = config
            .get("httpServerHost")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("127.0.0.1");
        let port = config
            .get("httpServerPort")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(14200);

        div()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_size(px(18.0)).font_weight(FontWeight::BOLD).child("API Server"))
            .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child(format!("Endpoint: http://{host}:{port} ({})", if enabled { "Enabled" } else { "Disabled" })))
            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Configure server host, port, IP whitelist, and security tokens in Settings > API Server."))
    }
}

impl Default for ApiServerToolPanel {
    fn default() -> Self {
        Self::new()
    }
}
