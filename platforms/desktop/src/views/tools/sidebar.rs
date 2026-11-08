use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{
    COLOR_BG_ELEVATED_LIGHT, COLOR_BG_TERTIARY_LIGHT, COLOR_TEXT_MUTED_LIGHT,
    COLOR_TEXT_PRIMARY_LIGHT, COLOR_TEXT_SECONDARY_LIGHT, RADIUS_MD, RADIUS_SM,
};
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ToolStatusTone {
    Active,
    Warning,
    Inactive,
    Error,
}

impl ToolStatusTone {
    pub fn colors(self) -> (Hsla, Hsla) {
        match self {
            Self::Active => (
                hsla(142.0 / 360.0, 0.71, 0.45, 0.12),
                hsla(142.0 / 360.0, 0.76, 0.36, 1.0),
            ),
            Self::Warning => (
                hsla(45.0 / 360.0, 0.93, 0.47, 0.12),
                hsla(43.0 / 360.0, 0.96, 0.40, 1.0),
            ),
            Self::Inactive => (COLOR_BG_TERTIARY_LIGHT, COLOR_TEXT_MUTED_LIGHT),
            Self::Error => (hsla(0.0, 0.84, 0.60, 0.12), hsla(0.0, 0.72, 0.51, 1.0)),
        }
    }
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

    pub fn render(
        &mut self,
        media_server_name: Option<&str>,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let services = cx.global::<GlobalDesktopServices>().0.clone();

        // 1. Subtitles Status
        let is_caption_running = services
            .config
            .get_setting("captionMode")
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let subtitles_status = if is_caption_running {
            ("Running".to_string(), ToolStatusTone::Active)
        } else {
            ("Stopped".to_string(), ToolStatusTone::Inactive)
        };

        // 2. Voice Typing Status
        let vt_enabled = services
            .config
            .get_setting("voiceTypingEnabled")
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let vt_status = if vt_enabled {
            ("Ready".to_string(), ToolStatusTone::Active)
        } else {
            ("Disabled".to_string(), ToolStatusTone::Inactive)
        };

        // 3. API Server Status
        let api_enabled = services
            .config
            .get_setting("httpServerEnabled")
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let api_port = services
            .config
            .get_setting("httpServerPort")
            .ok()
            .flatten()
            .and_then(|v| v.as_u64())
            .unwrap_or(14200);
        let api_status = if api_enabled {
            (format!(":{api_port}"), ToolStatusTone::Active)
        } else {
            ("Disabled".to_string(), ToolStatusTone::Inactive)
        };

        // 4. MCP Agents Status
        let mcp_enabled = services
            .config
            .get_setting("mcpEnabled")
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let mcp_running = services.agent_control.is_running();
        let clients = services.agent_control.tracker().active_count();
        let mcp_status = if !mcp_enabled {
            ("Disabled".to_string(), ToolStatusTone::Inactive)
        } else if mcp_running {
            if clients > 0 {
                (
                    format!("{clients} client{}", if clients == 1 { "" } else { "s" }),
                    ToolStatusTone::Active,
                )
            } else {
                ("Running".to_string(), ToolStatusTone::Active)
            }
        } else {
            ("Stopped".to_string(), ToolStatusTone::Inactive)
        };

        // 5. Media Server Status
        let media_status = if let Some(name) = media_server_name {
            (name.to_string(), ToolStatusTone::Active)
        } else {
            ("Not Connected".to_string(), ToolStatusTone::Inactive)
        };
        let tools = [
            (
                ToolKind::Subtitles,
                "Live Subtitles",
                "Desktop floating bilingual subtitles",
                LucideIcon::Captions,
                subtitles_status,
            ),
            (
                ToolKind::VoiceTyping,
                "Voice Typing",
                "System-wide dictation and smart polishing",
                LucideIcon::Keyboard,
                vt_status,
            ),
            (
                ToolKind::ApiServer,
                "API Server",
                "Local HTTP audio transcription API",
                LucideIcon::Server,
                api_status,
            ),
            (
                ToolKind::Mcp,
                "MCP Agents",
                "Model Context Protocol agent server",
                LucideIcon::Bot,
                mcp_status,
            ),
            (
                ToolKind::MediaServer,
                "Media Server Subtitle Assistant",
                "Detect missing subtitles in Emby and Jellyfin",
                LucideIcon::Tv,
                media_status,
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
            .children(tools.into_iter().map(
                |(kind, label, description, icon, (status_text, status_tone))| {
                    let is_active = self.active_tool == kind;
                    let (badge_bg, badge_fg) = status_tone.colors();
                    div()
                        .id(ElementId::from(format!("tool-nav-{:?}", kind)))
                        .flex()
                        .items_start()
                        .gap_3()
                        .px_3()
                        .py(px(10.0))
                        .rounded(RADIUS_MD)
                        .bg(if is_active {
                            COLOR_BG_ELEVATED_LIGHT
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
                        .hover(|style| style.bg(COLOR_BG_TERTIARY_LIGHT))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tools_sidebar.set_tool(kind);
                            cx.notify();
                        }))
                        // 32x32px Icon Tile (accent blue on active)
                        .child(
                            div()
                                .flex()
                                .size(px(32.0))
                                .items_center()
                                .justify_center()
                                .rounded(RADIUS_SM)
                                .bg(if is_active {
                                    hsla(217.0 / 360.0, 0.91, 0.60, 0.12)
                                } else {
                                    COLOR_BG_TERTIARY_LIGHT
                                })
                                .text_color(if is_active {
                                    hsla(217.0 / 360.0, 0.91, 0.60, 1.0)
                                } else {
                                    COLOR_TEXT_SECONDARY_LIGHT
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
                                        .justify_between()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .text_size(px(14.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                                .child(label),
                                        )
                                        // Status Badge with indicator dot
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .px(px(6.0))
                                                .py(px(1.5))
                                                .rounded_full()
                                                .bg(badge_bg)
                                                .text_color(badge_fg)
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(
                                                    div().size(px(6.0)).rounded_full().bg(badge_fg),
                                                )
                                                .child(status_text),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                                        .child(description),
                                ),
                        )
                },
            ))
    }
}

impl Default for ToolsSidebar {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::{ToolKind, ToolStatusTone, ToolsSidebar};
    use crate::theme::tokens::{COLOR_BG_TERTIARY_LIGHT, COLOR_TEXT_MUTED_LIGHT};
    #[test]
    fn tool_status_tone_colors_match_design_tokens() {
        let (active_bg, active_fg) = ToolStatusTone::Active.colors();
        assert_eq!(active_bg.a, 0.12);
        assert_eq!(active_fg.a, 1.0);

        let (warning_bg, warning_fg) = ToolStatusTone::Warning.colors();
        assert_eq!(warning_bg.a, 0.12);
        assert_eq!(warning_fg.a, 1.0);

        let (inactive_bg, inactive_fg) = ToolStatusTone::Inactive.colors();
        assert_eq!(inactive_bg, COLOR_BG_TERTIARY_LIGHT);
        assert_eq!(inactive_fg, COLOR_TEXT_MUTED_LIGHT);

        let (error_bg, error_fg) = ToolStatusTone::Error.colors();
        assert_eq!(error_bg.a, 0.12);
        assert_eq!(error_fg.a, 1.0);
    }

    #[test]
    fn tools_sidebar_initializes_with_subtitles() {
        let sidebar = ToolsSidebar::default();
        assert_eq!(sidebar.active_tool, ToolKind::Subtitles);
    }
}
