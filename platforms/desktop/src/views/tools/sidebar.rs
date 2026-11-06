//! Tools suite navigation sidebar.

use crate::theme::tokens::*;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ToolKind {
    Subtitles,
    VoiceTyping,
    ApiServer,
    Mcp,
    MediaServer,
}

pub struct ToolsSidebar {
    pub active_tool: ToolKind,
}

impl ToolsSidebar {
    pub fn new() -> Self {
        Self {
            active_tool: ToolKind::Subtitles,
        }
    }

    pub fn set_tool(&mut self, tool: ToolKind) {
        self.active_tool = tool;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        let tools = [
            (ToolKind::Subtitles, "Live Subtitles"),
            (ToolKind::VoiceTyping, "Voice Typing"),
            (ToolKind::ApiServer, "HTTP API Server"),
            (ToolKind::Mcp, "MCP Agents"),
            (ToolKind::MediaServer, "Media Server (Emby)"),
        ];

        div()
            .flex()
            .flex_col()
            .w(TOOLS_SIDEBAR_WIDTH)
            .h_full()
            .p_3()
            .gap_1()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted)
                    .child("TOOLS SUITE"),
            )
            .children(tools.into_iter().map(|(kind, label)| {
                let is_active = self.active_tool == kind;
                div()
                    .id(ElementId::from(format!("tool-nav-{:?}", kind)))
                    .flex()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if is_active {
                        cx.theme().accent
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .cursor_pointer()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if is_active {
                                cx.theme().primary
                            } else {
                                cx.theme().foreground
                            })
                            .child(label),
                    )
            }))
    }
}
