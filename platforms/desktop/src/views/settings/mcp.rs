use std::path::PathBuf;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::SettingsModal;
use crate::app::GlobalDesktopServices;
use crate::platform::agent_control::server::dispatch::get_tools_catalog;
use crate::platform::agent_control::server::{McpActivityLogEntry, get_ipc_endpoint};

#[derive(Clone)]
struct ClientGuide {
    id: &'static str,
    name: &'static str,
    path: String,
    content: String,
    format: &'static str,
}

pub struct McpSettingsTab {
    pub search_state: Entity<InputState>,
    pub logs: Vec<McpActivityLogEntry>,
    pub logs_loading: bool,
    pub logs_loaded: bool,
    pub log_filter_status: String,
    pub log_query: String,
    pub selected_guide: String,
    _search_subscription: Subscription,
}

impl McpSettingsTab {
    pub fn new(window: &mut Window, cx: &mut Context<SettingsModal>) -> Self {
        let search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search activity..."));
        let _search_subscription =
            cx.subscribe(&search_state, |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    modal.mcp.log_query = state.read(cx).value().to_string();
                    cx.notify();
                }
            });

        Self {
            search_state,
            logs: Vec::new(),
            logs_loading: false,
            logs_loaded: false,
            log_filter_status: "all".to_string(),
            log_query: String::new(),
            selected_guide: "claude-desktop".to_string(),
            _search_subscription,
        }
    }

    pub fn load_logs(&mut self, cx: &mut Context<SettingsModal>) {
        self.logs_loading = true;
        self.logs_loaded = true;
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let logs = services.agent_control.get_activity_logs().await;
            this.update(cx, |modal, cx| {
                modal.mcp.logs = logs;
                modal.mcp.logs_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn render(&mut self, cx: &mut Context<SettingsModal>) -> impl IntoElement {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let config = services
            .config
            .load()
            .ok()
            .flatten()
            .unwrap_or_else(|| json!({}));
        let enabled = config
            .get("mcpEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let permission_mode = config
            .get("mcpPermissionMode")
            .and_then(Value::as_str)
            .unwrap_or("full");
        let (binary_path, binary_exists) = binary_path();
        let endpoint = get_ipc_endpoint();
        let connected_clients = services.agent_control.tracker().active_count();
        let running = enabled && binary_exists;
        let app_config: sona_core::config::AppConfig =
            serde_json::from_value(config.clone()).unwrap_or_default();
        let catalog = get_tools_catalog(&app_config);
        let guides = client_guides(binary_path.as_deref().unwrap_or("sona-mcp"));
        let selected_guide = self.selected_guide.clone();
        let active_guide = guides
            .iter()
            .find(|guide| guide.id == selected_guide)
            .unwrap_or(&guides[0])
            .clone();
        let active_guide_path = active_guide.path.clone();
        let active_guide_content = active_guide.content.clone();
        let active_guide_content_to_copy = active_guide.content.clone();
        let active_guide_format = active_guide.format;
        let query = self.log_query.trim().to_lowercase();
        let filter_status = self.log_filter_status.clone();
        let filter_query = self.log_query.clone();
        let logs_loading = self.logs_loading;
        let logs = self.logs.clone();
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
                        .child(Button::new("settings-mcp-enabled").when(enabled, |button| button.primary()).when(!enabled, |button| button.outline()).small().label(if enabled { "Disable" } else { "Enable" }).on_click(cx.listener(move |_modal, _, _, cx| {
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
                    .child(div().flex().items_center().justify_between().py_3().border_b_1().border_color(border).child("IPC endpoint").child(div().flex().items_center().gap_2().child(endpoint.clone()).child(Button::new("settings-mcp-copy-endpoint").ghost().small().label("Copy").on_click(cx.listener(move |_modal, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(endpoint.clone())))))))
                    .child(div().flex().items_center().justify_between().py_3().border_b_1().border_color(border).child(div().flex().flex_col().gap_1().child("Bridge executable (sona-mcp)").child(div().text_size(px(12.0)).text_color(muted).child(binary_path.as_deref().unwrap_or("Executable path unavailable").to_string()))).child(if binary_exists { "Available" } else { "Missing" }))
                    .child(div().flex().justify_end().gap_2()
                        .child(Button::new("settings-mcp-disconnect-all").outline().small().label("Disconnect All").disabled(connected_clients == 0).on_click(cx.listener(|_modal, _, _, cx| { cx.global::<GlobalDesktopServices>().0.agent_control.disconnect_all(); cx.notify(); })))
                        .child(Button::new("settings-mcp-restart").outline().small().label("Restart Service").on_click(cx.listener(|_modal, _, _, cx| { let services = cx.global::<GlobalDesktopServices>().0.clone(); services.stop_mcp(); services.start_mcp(); cx.notify(); }))))
                )
                .child(div().flex().flex_col().gap_3().child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Security & Permissions"))
                    .child(div().flex().gap_2().children([("full", "Full access"), ("read_only", "Read only"), ("granular", "Granular")].into_iter().map(|(mode, label)| {
                        let selected = permission_mode == mode;
                        Button::new(ElementId::Name(format!("mcp-mode-{mode}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(label).on_click(cx.listener(move |_modal, _, _, cx| {
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
                        Button::new(ElementId::Name(format!("mcp-client-{id}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(guide.name).on_click(cx.listener(move |modal, _, _, cx| { modal.mcp.selected_guide = id.to_string(); cx.notify(); }))
                    })))
                    .child(div().flex().items_center().justify_between().gap_3().child(div().text_size(px(12.0)).text_color(muted).child(active_guide_path.clone())).child(Button::new("settings-mcp-copy-guide-path").ghost().small().label("Copy path").on_click(cx.listener(move |_modal, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(active_guide_path.clone()))))))
                    .child(div().flex().flex_col().gap_2().p_3().rounded_md().border_1().border_color(border).bg(cx.theme().input)
                        .child(div().flex().items_center().justify_between().child(active_guide_format).child(Button::new("settings-mcp-copy-config").outline().small().label("Copy Configuration").on_click(cx.listener(move |_modal, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(active_guide_content_to_copy.clone()))))))
                        .child(div().text_size(px(12.0)).font_family("monospace").child(active_guide_content.clone())))
                )
                .child(div().flex().flex_col().gap_3()
                    .child(div().flex().items_center().justify_between().child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Audit activity")).child(Button::new("settings-mcp-clear-logs").ghost().small().label("Clear").disabled(logs.is_empty()).on_click(cx.listener(|_modal, _, _, cx| {
                        let services = cx.global::<GlobalDesktopServices>().0.clone();
                        cx.spawn(async move |this, cx: &mut AsyncApp| {
                            services.agent_control.clear_activity_logs().await;
                            this.update(cx, |modal, cx| {
                                modal.mcp.logs.clear();
                                cx.notify();
                            }).ok();
                        }).detach();
                    }))))
                    .child(div().flex().items_center().justify_between().gap_2().flex_wrap()
                        .child(div().flex().items_center().gap_2().flex_1().min_w(px(200.0)).max_w(px(320.0)).px_2().h(px(32.0)).rounded_md().border_1().border_color(border).bg(cx.theme().input)
                            .on_key_down(cx.listener(|modal, event: &KeyDownEvent, window, cx| {
                                if event.keystroke.key.eq_ignore_ascii_case("escape") && !modal.mcp.log_query.is_empty() {
                                    modal.mcp.search_state.update(cx, |state, cx| state.set_value("", window, cx));
                                    modal.mcp.log_query.clear();
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                            }))
                            .child(Icon::new(LucideIcon::Search).size(px(14.0)).text_color(muted))
                            .child(Input::new(&self.search_state).small())
                            .when(!filter_query.is_empty(), |search| search.child(Button::new("settings-mcp-search-clear").ghost().small().label("×").on_click(cx.listener(|modal, _, window, cx| {
                                modal.mcp.search_state.update(cx, |state, cx| state.set_value("", window, cx));
                                modal.mcp.log_query.clear();
                                cx.notify();
                            }))))
                        )
                        .child(div().flex().gap_2().children(["all", "success", "error", "blocked"].into_iter().map(|status| {
                            let selected = filter_status == status;
                            let status_value = status.to_string();
                            Button::new(ElementId::Name(format!("mcp-log-filter-{status}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(match status { "all" => "All", "success" => "Success", "error" => "Error", _ => "Blocked" }).on_click(cx.listener(move |modal, _, _, cx| { modal.mcp.log_filter_status = status_value.clone(); cx.notify(); }))
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
                    let category = format!("{:?}", tool.category).to_lowercase();
                    div().flex().items_start().justify_between().gap_3().py_2().border_b_1().border_color(border)
                        .child(div().flex().flex_col().gap_1().child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(tool.title.clone())).child(div().text_size(px(11.0)).text_color(muted).child(tool.description.clone())).child(div().text_size(px(10.0)).text_color(muted).child(category)))
                        .child(div().text_size(px(11.0)).text_color(if tool.allowed { cx.theme().success } else { cx.theme().muted }).child(if tool.allowed { "Allowed" } else { "Restricted" }))
                })))
            )
            .into_any_element()
    }
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

fn permission_value(config: &Value, key: &str) -> bool {
    config.get(key).and_then(Value::as_bool).unwrap_or(true)
}

fn permission_toggle(
    key: &'static str,
    label: &'static str,
    checked: bool,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            Checkbox::new(ElementId::Name(format!("mcp-perm-{key}").into()))
                .checked(checked)
                .on_click(cx.listener(move |_modal, value, _, cx| {
                    let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                    if let Err(error) = config.set_setting(key, &Value::Bool(*value)) {
                        log::error!("Failed to save MCP permission {key}: {error}");
                    }
                    cx.notify();
                })),
        )
        .child(div().text_size(px(13.0)).child(label))
}

fn client_guides(binary_path: &str) -> Vec<ClientGuide> {
    let endpoint = get_ipc_endpoint();
    let claude_config = json!({
        "mcpServers": {
            "sona": {
                "command": binary_path,
                "args": ["--pipe", endpoint]
            }
        }
    });
    let cursor_config = json!({
        "mcpServers": {
            "sona": {
                "type": "command",
                "command": binary_path,
                "args": ["--pipe", endpoint]
            }
        }
    });
    vec![
        ClientGuide {
            id: "claude-desktop",
            name: "Claude Desktop",
            path: if cfg!(windows) {
                "%APPDATA%\\Claude\\claude_desktop_config.json".into()
            } else {
                "~/Library/Application Support/Claude/claude_desktop_config.json".into()
            },
            content: serde_json::to_string_pretty(&claude_config).unwrap_or_default(),
            format: "JSON (Configuration File)",
        },
        ClientGuide {
            id: "cursor",
            name: "Cursor",
            path: "~/.cursor/mcp.json".into(),
            content: serde_json::to_string_pretty(&cursor_config).unwrap_or_default(),
            format: "JSON (Cursor Settings)",
        },
    ]
}
