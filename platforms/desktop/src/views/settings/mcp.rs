use crate::app::GlobalDesktopServices;
use crate::platform::agent_control::facade::AgentControlFacade;
use crate::platform::agent_control::server::dispatch::get_tools_catalog;
use crate::platform::agent_control::server::{McpActivityLogEntry, get_ipc_endpoint};
use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone)]
struct ClientGuide {
    id: &'static str,
    name: &'static str,
    path: String,
    content: String,
    format: &'static str,
}

fn binary_path() -> (Option<String>, bool) {
    let name = if cfg!(windows) {
        "sona-mcp.exe"
    } else {
        "sona-mcp"
    };
    if let Ok(path) = std::env::var("SONA_MCP_PATH") {
        let path = PathBuf::from(path);
        let candidate = if path.is_dir() { path.join(name) } else { path };
        if candidate.is_file() {
            return (Some(candidate.to_string_lossy().into_owned()), true);
        }
    }
    if let Some(parent) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(std::path::Path::parent)
    {
        let candidate = parent.join(name);
        if candidate.is_file() {
            return (Some(candidate.to_string_lossy().into_owned()), true);
        }
        for relative in [
            "../debug",
            "../release",
            "../../target/debug",
            "../../target/release",
        ] {
            if let Ok(candidate) = parent.join(relative).join(name).canonicalize() {
                return (Some(candidate.to_string_lossy().into_owned()), true);
            }
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return (Some(candidate.to_string_lossy().into_owned()), true);
            }
        }
    }
    let fallback = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(name)))
        .map(|path| path.to_string_lossy().into_owned());
    (fallback, false)
}

fn client_guides(binary: &str) -> Vec<ClientGuide> {
    let json_config = serde_json::to_string_pretty(&json!({
        "mcpServers": { "sona": { "command": binary, "args": [] } }
    }))
    .unwrap_or_default();
    let pi_config = serde_json::to_string_pretty(&json!({
        "mcpServers": { "sona": { "type": "stdio", "command": binary, "args": [] } }
    }))
    .unwrap_or_default();
    let codex_command = serde_json::to_string(binary).unwrap_or_else(|_| format!("\"{binary}\""));
    let claude_path = if cfg!(windows) {
        r"%APPDATA%\Claude\claude_desktop_config.json".to_string()
    } else if cfg!(target_os = "macos") {
        "~/Library/Application Support/Claude/claude_desktop_config.json".to_string()
    } else {
        "~/.config/Claude/claude_desktop_config.json".to_string()
    };
    let path = |windows: &str, unix: &str| -> String {
        if cfg!(windows) {
            windows.to_string()
        } else {
            unix.to_string()
        }
    };
    vec![
        ClientGuide {
            id: "claude-desktop",
            name: "Claude Desktop",
            path: claude_path,
            content: json_config.clone(),
            format: "JSON",
        },
        ClientGuide {
            id: "cursor",
            name: "Cursor",
            path: path(r"%USERPROFILE%\.cursor\mcp.json", "~/.cursor/mcp.json"),
            content: json_config.clone(),
            format: "JSON",
        },
        ClientGuide {
            id: "codex",
            name: "Codex",
            path: path(r"%USERPROFILE%\.codex\config.toml", "~/.codex/config.toml"),
            content: format!("[mcp_servers.sona]\ncommand = {codex_command}\nargs = []\n"),
            format: "TOML",
        },
        ClientGuide {
            id: "antigravity",
            name: "Antigravity",
            path: path(
                r"%USERPROFILE%\.gemini\antigravity\mcp_config.json",
                "~/.gemini/antigravity/mcp_config.json",
            ),
            content: json_config.clone(),
            format: "JSON",
        },
        ClientGuide {
            id: "claude-code",
            name: "Claude Code CLI",
            path: "Terminal CLI Command".into(),
            content: format!("claude mcp add sona \"{binary}\""),
            format: "BASH",
        },
        ClientGuide {
            id: "pi-omp",
            name: "pi / omp",
            path: path(
                r"%USERPROFILE%\.omp\agent\mcp.json",
                "~/.omp/agent/mcp.json",
            ),
            content: pi_config,
            format: "JSON",
        },
        ClientGuide {
            id: "windsurf",
            name: "Windsurf",
            path: path(
                r"%USERPROFILE%\.codeium\windsurf\mcp_config.json",
                "~/.codeium/windsurf/mcp_config.json",
            ),
            content: json_config.clone(),
            format: "JSON",
        },
        ClientGuide {
            id: "generic",
            name: "Generic / Stdio",
            path: "Custom Agent Client MCP Config".into(),
            content: json_config,
            format: "JSON",
        },
    ]
}

fn permission_value(config: &sona_core::config::AppConfig, key: &str) -> bool {
    match key {
        "mcpAllowRecording" => config.mcp_allow_recording.unwrap_or(true),
        "mcpAllowTranscriptEdit" => config.mcp_allow_transcript_edit.unwrap_or(true),
        "mcpAllowDestructive" => config.mcp_allow_destructive.unwrap_or(false),
        "mcpAllowSettingsUpdate" => config.mcp_allow_settings_update.unwrap_or(false),
        "mcpAllowModelManagement" => config.mcp_allow_model_management.unwrap_or(true),
        "mcpNotifyOnMutation" => config.mcp_notify_on_mutation.unwrap_or(true),
        "mcpAuditLogEnabled" => config.mcp_audit_log_enabled.unwrap_or(true),
        _ => false,
    }
}

fn permission_toggle(
    key: &'static str,
    label: &'static str,
    checked: bool,
    cx: &Context<MainShell>,
) -> AnyElement {
    Checkbox::new(ElementId::Name(format!("mcp-setting-{key}").into()))
        .checked(checked)
        .label(label)
        .on_click(cx.listener(move |_shell, checked: &bool, _, cx| {
            let config = cx.global::<GlobalDesktopServices>().0.config.clone();
            if let Err(error) = config.set_setting(key, &Value::Bool(*checked)) {
                log::error!("Failed to update MCP setting {key}: {error}");
            }
            cx.notify();
        }))
        .into_any_element()
}

pub(super) fn mcp_settings_panel(
    logs: &[McpActivityLogEntry],
    logs_loading: bool,
    filter_status: &str,
    filter_query: &str,
    selected_guide_id: &str,
    search_state: &Entity<InputState>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let services = cx.global::<GlobalDesktopServices>().0.clone();
    let config = AgentControlFacade::new((*services).clone()).load_app_config();
    let enabled = config.mcp_enabled.unwrap_or(true);
    let running = services.agent_control.is_running();
    let connected_clients = services.agent_control.tracker().active_count();
    let permission_mode = config.mcp_permission_mode.as_deref().unwrap_or("full");
    let endpoint = get_ipc_endpoint();
    let (binary_path, binary_exists) = binary_path();
    let binary = binary_path.clone().unwrap_or_else(|| {
        if cfg!(windows) {
            r"C:\Program Files\Sona\sona-mcp.exe".to_string()
        } else {
            "/usr/local/bin/sona-mcp".to_string()
        }
    });
    let guides = client_guides(&binary);
    let active_guide = guides
        .iter()
        .find(|guide| guide.id == selected_guide_id)
        .unwrap_or(&guides[0]);
    let active_guide_path = active_guide.path.clone();
    let active_guide_content = active_guide.content.clone();
    let active_guide_content_to_copy = active_guide_content.clone();
    let active_guide_format = active_guide.format;
    let catalog = get_tools_catalog(&config);
    let query = filter_query.trim().to_lowercase();
    let filtered_logs = logs
        .iter()
        .filter(|entry| filter_status == "all" || entry.status == filter_status)
        .filter(|entry| {
            query.is_empty()
                || entry.method.to_lowercase().contains(&query)
                || entry
                    .params_summary
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&query))
                || entry
                    .error_message
                    .as_deref()
                    .is_some_and(|value| value.to_lowercase().contains(&query))
        })
        .collect::<Vec<_>>();
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;

    div()
        .id("settings-mcp-panel")
        .flex().flex_col().gap(px(28.0)).size_full().min_h_0().overflow_y_scroll().px(px(32.0)).py(px(24.0))
        .child(div().w_full().max_w(px(900.0)).mx_auto().flex().flex_col().gap(px(28.0))
            .child(div().flex().items_start().justify_between().gap_4().pb_5().border_b_1().border_color(border)
                .child(div().flex().items_start().gap_4()
                    .child(div().flex().size(px(44.0)).items_center().justify_center().rounded_md().border_1().border_color(border).bg(cx.theme().group_box).text_color(cx.theme().primary).child(Icon::new(LucideIcon::Bot).size(px(22.0))))
                    .child(div().flex().flex_col().gap_1().child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("MCP Agents")).child(div().text_size(px(13.0)).text_color(muted).child("Connect external agent clients to Sona through the Model Context Protocol."))))
                .child(div().flex().items_center().gap_3()
                    .child(div().px_3().py_1().rounded_full().bg(if running { cx.theme().success } else { cx.theme().muted }).text_size(px(12.0)).child(if !enabled { "Disabled" } else if running { "Listening" } else { "Stopped" }))
                    .child(Button::new("settings-mcp-enabled").when(enabled, |button| button.primary()).when(!enabled, |button| button.outline()).small().label(if enabled { "Disable" } else { "Enable" }).on_click(cx.listener(move |_shell, _, _, cx| {
                        let services = cx.global::<GlobalDesktopServices>().0.clone();
                        let current = services.config.get_setting("mcpEnabled").ok().flatten().and_then(|value| value.as_bool()).unwrap_or(true);
                        let next = !current;
                        if let Err(error) = services.config.set_setting("mcpEnabled", &json!(next)) { log::error!("Failed to save MCP state: {error}"); }
                        if next { services.start_mcp(); } else { services.stop_mcp(); }
                        cx.notify();
                    })))))
            .child(div().flex().flex_col().gap_3()
                .child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Service Lifecycle & Status"))
                .child(div().flex().items_center().justify_between().py_3().border_b_1().border_color(border).child("Connected clients").child(format!("{connected_clients} active")))
                .child(div().flex().items_center().justify_between().py_3().border_b_1().border_color(border).child("IPC endpoint").child(div().flex().items_center().gap_2().child(endpoint.clone()).child(Button::new("settings-mcp-copy-endpoint").ghost().small().label("Copy").on_click(cx.listener(move |_shell, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(endpoint.clone())))))))
                .child(div().flex().items_center().justify_between().py_3().border_b_1().border_color(border).child(div().flex().flex_col().gap_1().child("Bridge executable (sona-mcp)").child(div().text_size(px(12.0)).text_color(muted).child(binary_path.as_deref().unwrap_or("Executable path unavailable").to_string()))).child(if binary_exists { "Available" } else { "Missing" }))
                .child(div().flex().justify_end().gap_2()
                    .child(Button::new("settings-mcp-disconnect-all").outline().small().label("Disconnect All").disabled(connected_clients == 0).on_click(cx.listener(|_shell, _, _, cx| { cx.global::<GlobalDesktopServices>().0.agent_control.disconnect_all(); cx.notify(); })))
                    .child(Button::new("settings-mcp-restart").outline().small().label("Restart Service").on_click(cx.listener(|_shell, _, _, cx| { let services = cx.global::<GlobalDesktopServices>().0.clone(); services.stop_mcp(); services.start_mcp(); cx.notify(); }))))
            )
            .child(div().flex().flex_col().gap_3().child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Security & Permissions"))
                .child(div().flex().gap_2().children([("full", "Full access"), ("read_only", "Read only"), ("granular", "Granular")].into_iter().map(|(mode, label)| {
                    let selected = permission_mode == mode;
                    Button::new(ElementId::Name(format!("mcp-mode-{mode}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(label).on_click(cx.listener(move |_shell, _, _, cx| {
                        let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                        if let Err(error) = config.set_setting("mcpPermissionMode", &json!(mode)) { log::error!("Failed to save MCP permission mode: {error}"); }
                        cx.notify();
                    }))
                })))
                .when(permission_mode == "granular", |section| section.child(div().flex().flex_col().gap_2().children([
                    ("mcpAllowRecording", "Allow recording controls"), ("mcpAllowTranscriptEdit", "Allow transcript edits"), ("mcpAllowDestructive", "Allow destructive actions"), ("mcpAllowSettingsUpdate", "Allow settings updates"), ("mcpAllowModelManagement", "Allow model management")
                ].into_iter().map(|(key, label)| permission_toggle(key, label, permission_value(&config, key), cx)))))
                .child(div().flex().gap_4().children([
                    permission_toggle("mcpNotifyOnMutation", "Notify on successful changes", permission_value(&config, "mcpNotifyOnMutation"), cx),
                    permission_toggle("mcpAuditLogEnabled", "Record audit activity", permission_value(&config, "mcpAuditLogEnabled"), cx),
                ])))
            .child(div().flex().flex_col().gap_3().child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Client Configuration"))
                .child(div().flex().flex_wrap().gap_2().children(guides.iter().map(|guide| {
                    let selected = guide.id == active_guide.id;
                    let id = guide.id;
                    Button::new(ElementId::Name(format!("mcp-client-{id}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(guide.name).on_click(cx.listener(move |shell, _, _, cx| { shell.settings_modal.mcp_selected_guide = id.to_string(); cx.notify(); }))
                })))
                .child(div().flex().items_center().justify_between().gap_3().child(div().text_size(px(12.0)).text_color(muted).child(active_guide_path.clone())).child(Button::new("settings-mcp-copy-guide-path").ghost().small().label("Copy path").on_click(cx.listener(move |_shell, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(active_guide_path.clone()))))))
                .child(div().flex().flex_col().gap_2().p_3().rounded_md().border_1().border_color(border).bg(cx.theme().input)
                    .child(div().flex().items_center().justify_between().child(active_guide_format).child(Button::new("settings-mcp-copy-config").outline().small().label("Copy Configuration").on_click(cx.listener(move |_shell, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(active_guide_content_to_copy.clone())))))
                    .child(div().text_size(px(12.0)).font_family("monospace").child(active_guide_content.clone())))
            )
            .child(div().flex().flex_col().gap_3()
                .child(div().flex().items_center().justify_between().child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Audit activity")).child(Button::new("settings-mcp-clear-logs").ghost().small().label("Clear").disabled(logs.is_empty()).on_click(cx.listener(|_shell, _, _, cx| {
                    let services = cx.global::<GlobalDesktopServices>().0.clone();
                    cx.spawn(async move |this, cx: &mut AsyncApp| {
                        services.agent_control.clear_activity_logs().await;
                        this.update(cx, |shell, cx| {
                            shell.settings_modal.mcp_logs.clear();
                            cx.notify();
                        }).ok();
                    }).detach();
                }))))
                .child(div().flex().items_center().justify_between().gap_2().flex_wrap()
                    .child(div().flex().items_center().gap_2().flex_1().min_w(px(200.0)).max_w(px(320.0)).px_2().h(px(32.0)).rounded_md().border_1().border_color(border).bg(cx.theme().input)
                        .on_key_down(cx.listener(|shell, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key.eq_ignore_ascii_case("escape") && !shell.settings_modal.mcp_log_query.is_empty() {
                                shell.mcp_search_state.update(cx, |state, cx| state.set_value("", window, cx));
                                shell.settings_modal.mcp_log_query.clear();
                                window.prevent_default();
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }))
                        .child(Icon::new(LucideIcon::Search).size(px(14.0)).text_color(muted))
                        .child(Input::new(search_state).small())
                        .when(!filter_query.is_empty(), |search| search.child(Button::new("settings-mcp-search-clear").ghost().small().label("×").on_click(cx.listener(|shell, _, window, cx| {
                            shell.mcp_search_state.update(cx, |state, cx| state.set_value("", window, cx));
                            shell.settings_modal.mcp_log_query.clear();
                            cx.notify();
                        }))))
                    )
                    .child(div().flex().gap_2().children(["all", "success", "error", "blocked"].into_iter().map(|status| {
                        let selected = filter_status == status;
                        let status_value = status.to_string();
                        Button::new(ElementId::Name(format!("mcp-log-filter-{status}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(match status { "all" => "All", "success" => "Success", "error" => "Error", _ => "Blocked" }).on_click(cx.listener(move |shell, _, _, cx| { shell.settings_modal.mcp_log_filter_status = status_value.clone(); cx.notify(); }))
                    })))
                )
                .when(logs_loading, |section| section.child(div().text_color(muted).child("Loading activity…")))
                .when(!logs_loading && filtered_logs.is_empty(), |section| section.child(div().p_3().rounded_md().bg(cx.theme().input).text_color(muted).child("No activity recorded yet.")))
                .children(filtered_logs.into_iter().map(|entry| {
                    let color = match entry.status.as_str() { "success" => cx.theme().success, "blocked" => cx.theme().warning, _ => cx.theme().danger };
                    div().flex().items_start().justify_between().gap_3().py_2().border_b_1().border_color(border)
                        .child(div().flex().flex_col().gap_1().child(div().flex().items_center().gap_2().child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(entry.method.clone())).child(div().px_2().py(px(1.0)).rounded_full().bg(color).text_size(px(10.0)).child(entry.status.clone()))).child(div().text_size(px(11.0)).text_color(muted).child(format!("{} · {} ms{}", entry.category, entry.duration_ms, entry.params_summary.as_ref().map(|summary| format!(" · {summary}")).unwrap_or_default()))).when_some(entry.error_message.clone(), |row, message| row.child(div().text_size(px(11.0)).text_color(cx.theme().danger).child(message))))
                        .child(div().text_size(px(11.0)).text_color(muted).child(entry.timestamp.to_string()))
                }))
            )
            .child(div().flex().flex_col().gap_3().child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Available tools")).children(catalog.iter().map(|tool| {
                let category = tool.category.replace('_', " ");
                div().flex().items_start().justify_between().gap_3().py_2().border_b_1().border_color(border)
                    .child(div().flex().flex_col().gap_1().child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(tool.title.clone())).child(div().text_size(px(11.0)).text_color(muted).child(tool.description.clone())).child(div().text_size(px(10.0)).text_color(muted).child(category)))
                    .child(div().text_size(px(11.0)).text_color(if tool.allowed { cx.theme().success } else { cx.theme().muted }).child(if tool.allowed { "Allowed" } else { "Restricted" }))
            }))))
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::client_guides;

    #[test]
    fn client_guides_match_the_supported_react_client_set() {
        let guides = client_guides("sona-mcp");
        let ids = guides.iter().map(|guide| guide.id).collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "claude-desktop",
                "cursor",
                "codex",
                "antigravity",
                "claude-code",
                "pi-omp",
                "windsurf",
                "generic",
            ]
        );
        assert!(guides[0].content.contains("mcpServers"));
        assert!(guides[2].content.contains("[mcp_servers.sona]"));
        assert_eq!(guides[2].format, "TOML");
        assert!(guides[4].content.starts_with("claude mcp add sona"));
        assert!(guides[5].content.contains("\"type\": \"stdio\""));
    }
}
