//! Tools suite navigation sidebar.

use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
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

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let tools = [
            (
                ToolKind::Subtitles,
                "Live Subtitles",
                "Desktop floating bilingual subtitles",
                LucideIcon::Captions,
            ),
            (
                ToolKind::VoiceTyping,
                "Voice Typing",
                "System-wide dictation and smart polishing",
                LucideIcon::Keyboard,
            ),
            (
                ToolKind::ApiServer,
                "API Server",
                "Local HTTP audio transcription API",
                LucideIcon::Server,
            ),
            (
                ToolKind::Mcp,
                "MCP Agents",
                "Model Context Protocol agent server",
                LucideIcon::Bot,
            ),
            (
                ToolKind::MediaServer,
                "Media Server Subtitle Assistant",
                "Detect missing subtitles in Emby and Jellyfin",
                LucideIcon::Tv,
            ),
        ];

        div()
            .flex()
            .flex_col()
            .w(px(350.0))
            .min_w(px(350.0))
            .max_w(px(350.0))
            .h_full()
            .p_3()
            .gap_1()
            .bg(cx.theme().secondary)
            .children(tools.into_iter().map(|(kind, label, description, icon)| {
                let is_active = self.active_tool == kind;
                let hover_bg = cx.theme().muted;
                div()
                    .id(ElementId::from(format!("tool-nav-{:?}", kind)))
                    .flex()
                    .items_start()
                    .gap_3()
                    .px_3()
                    .py(px(10.0))
                    .rounded_md()
                    .bg(if is_active {
                        cx.theme().group_box
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .border_1()
                    .border_color(if is_active {
                        cx.theme().border
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover_bg))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tools_sidebar.set_tool(kind);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .size(px(32.0))
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .bg(cx.theme().muted)
                            .text_color(if is_active {
                                cx.theme().primary
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(Icon::new(icon).size(px(18.0))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_start()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child(label),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(description),
                            ),
                    )
            }))
    }
}
