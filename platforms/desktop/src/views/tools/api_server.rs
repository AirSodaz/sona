//! Tool: HTTP API Server controller panel.

use crate::views::shell::MainShell;
use gpui_kit::component::input::InputState;
use gpui_kit::*;

pub struct ApiServerToolPanel;

impl ApiServerToolPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &mut self,
        field_states: &std::collections::HashMap<String, Entity<InputState>>,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        crate::views::settings::api_server::api_server_settings_panel(field_states, cx)
    }
}

impl Default for ApiServerToolPanel {
    fn default() -> Self {
        Self::new()
    }
}
