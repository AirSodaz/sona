//! Tools Workspace View combining Sidebar and Tool Detail panels.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use std::collections::HashSet;

use super::api_server::ApiServerToolPanel;
use super::mcp::McpToolPanel;
use super::media_server::MediaServerToolPanel;
use super::sidebar::{ToolKind, ToolsSidebar};
use super::subtitles::SubtitlesToolPanel;
use super::voice_typing::VoiceTypingToolPanel;
use crate::views::shell::MainShell;

pub struct ToolsWorkspace {
    pub tools_sidebar: ToolsSidebar,
    pub subtitles_tool: SubtitlesToolPanel,
    pub voice_typing_tool: VoiceTypingToolPanel,
    pub api_server_tool: ApiServerToolPanel,
    pub mcp_tool: McpToolPanel,
    pub media_server_tool: MediaServerToolPanel,
}

impl ToolsWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
        Self {
            tools_sidebar: ToolsSidebar::new(),
            subtitles_tool: SubtitlesToolPanel::new(),
            voice_typing_tool: VoiceTypingToolPanel::new(),
            api_server_tool: ApiServerToolPanel::new(),
            mcp_tool: McpToolPanel::new(),
            media_server_tool: MediaServerToolPanel::new(window, cx),
        }
    }

    pub fn render(
        &mut self,
        is_recording_shortcut: bool,
        shortcut_target: Option<crate::views::settings::ShortcutTarget>,
        queued_media_ids: &HashSet<String>,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let media_server_name = self.media_server_tool.server_name();
        let tool_panel: AnyElement = match self.tools_sidebar.active_tool {
            ToolKind::Subtitles => self.subtitles_tool.render(cx).into_any_element(),
            ToolKind::VoiceTyping => self
                .voice_typing_tool
                .render(
                    is_recording_shortcut,
                    shortcut_target,
                    cx,
                    |this, target, cx| {
                        this.settings_modal.update(cx, |m, _| {
                            m.shortcuts.is_recording_shortcut = true;
                            m.shortcuts.shortcut_recording_target = Some(target);
                        });
                        cx.notify();
                    },
                )
                .into_any_element(),
            ToolKind::ApiServer => self.api_server_tool.render(cx).into_any_element(),
            ToolKind::Mcp => self.mcp_tool.render(cx).into_any_element(),
            ToolKind::MediaServer => self
                .media_server_tool
                .render(queued_media_ids, cx)
                .into_any_element(),
        };

        div()
            .id("tools-mode-shell")
            .flex()
            .size_full()
            .bg(cx.theme().secondary)
            .child(self.tools_sidebar.render(media_server_name.as_deref(), cx))
            .child(
                div()
                    .id("tools-detail-pane")
                    .flex_1()
                    .min_w(crate::theme::tokens::PANEL_RIGHT_MIN_WIDTH)
                    .h_full()
                    .bg(cx.theme().background)
                    .border_t_1()
                    .border_l_1()
                    .border_color(cx.theme().border)
                    .rounded_tl(crate::theme::tokens::RADIUS_LG)
                    .overflow_y_scroll()
                    .p_8()
                    .px(px(48.0))
                    .child(tool_panel),
            )
    }
}
