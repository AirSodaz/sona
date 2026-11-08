//! MCP Agent server status and runtime controls.

use crate::app::GlobalDesktopServices;
use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::*;

pub struct McpToolPanel;

impl McpToolPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let is_enabled = services
            .config
            .get_setting("mcpEnabled")
            .ok()
            .flatten()
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let is_running = services.agent_control.is_running();
        let connected_clients = services.agent_control.tracker().active_count();
        let status = if !is_enabled {
            "Disabled"
        } else if is_running {
            "Listening"
        } else {
            "Stopped"
        };

        div()
            .id("tools-panel-mcp")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(48.0))
            .py(px(32.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(860.0))
                    .mx_auto()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .gap_4()
                            .pb_6()
                            .mb(px(28.0))
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex()
                                            .size(px(48.0))
                                            .items_center()
                                            .justify_center()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().group_box)
                                            .text_color(cx.theme().primary)
                                            .child(Icon::new(LucideIcon::Bot).size(px(24.0))),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(20.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child("MCP Agents"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Securely expose Sona tools to external AI agents via Model Context Protocol."),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .px_3()
                                            .py_1()
                                            .rounded_full()
                                            .bg(if is_running { cx.theme().success } else { cx.theme().muted })
                                            .text_size(px(12.0))
                                            .text_color(cx.theme().foreground)
                                            .child(status),
                                    )
                                    .child(
                                        Button::new("mcp-runtime-toggle")
                                            .outline()
                                            .small()
                                            .label(if is_running { "Disable" } else { "Enable" })
                                            .on_click(cx.listener(|_this, _, _, cx| {
                                                let services = cx.global::<GlobalDesktopServices>().0.clone();
                                                let should_enable = !services.agent_control.is_running();
                                                let _ = services.config.set_setting("mcpEnabled", &serde_json::json!(should_enable));
                                                if should_enable {
                                                    services.start_mcp();
                                                } else {
                                                    services.stop_mcp();
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(32.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Service Status"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .border_b_1()
                                            .border_color(cx.theme().border)
                                            .py_3()
                                            .child(div().text_size(px(14.0)).child("Connected Clients"))
                                            .child(
                                                div()
                                                    .text_size(px(14.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(format!("{connected_clients} active")),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl Default for McpToolPanel {
    fn default() -> Self {
        Self::new()
    }
}
