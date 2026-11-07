use crate::app::GlobalDesktopServices;
use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

fn string_setting(config: &Value, key: &str, default: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn integer_setting(config: &Value, key: &str, default: i64) -> i64 {
    config.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn set_setting(key: &'static str, value: Value, cx: &mut Context<MainShell>) {
    let config = cx.global::<GlobalDesktopServices>().0.config.clone();
    if let Err(error) = config.set_setting(key, &value) {
        log::error!("Failed to update API server setting {key}: {error}");
    }
    cx.notify();
}

pub(crate) fn api_server_settings_panel(
    field_states: &std::collections::HashMap<String, Entity<InputState>>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let services = cx.global::<GlobalDesktopServices>().0.clone();
    let config = services
        .config
        .load()
        .ok()
        .flatten()
        .unwrap_or_else(|| json!({}));
    let enabled = config
        .get("httpServerEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let host = string_setting(&config, "httpServerHost", "127.0.0.1");
    let port = integer_setting(&config, "httpServerPort", 14200);
    let api_key = string_setting(&config, "httpServerApiKey", "");
    let max_concurrent = integer_setting(&config, "httpServerMaxConcurrent", 2);
    let max_queue = integer_setting(&config, "httpServerMaxQueueSize", 100);
    let max_upload = integer_setting(&config, "httpServerMaxUploadSizeMB", 50);
    let job_ttl = integer_setting(&config, "httpServerJobTtlMinutes", 60);
    let muted = cx.theme().muted_foreground;
    let border = cx.theme().border;
    let endpoint_host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.clone()
    };
    let endpoint = format!("http://{endpoint_host}:{port}");
    let endpoint_to_copy = endpoint.clone();
    let key_to_copy = api_key.clone();
    let api_key_state = field_states["apiKey"].clone();

    div()
        .id("settings-api-server-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .overflow_y_scroll()
        .px(px(32.0))
        .py(px(24.0))
        .child(
            div().w_full().max_w(px(900.0)).mx_auto().flex().flex_col().gap(px(28.0))
                .child(
                    div().flex().items_start().justify_between().gap_4().pb_5().border_b_1().border_color(border)
                        .child(div().flex().items_start().gap_4()
                            .child(div().flex().size(px(44.0)).items_center().justify_center().rounded_md().border_1().border_color(border).bg(cx.theme().group_box).text_color(cx.theme().primary).child(Icon::new(LucideIcon::Server).size(px(22.0))))
                            .child(div().flex().flex_col().gap_1()
                                .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("API Server"))
                                .child(div().text_size(px(13.0)).text_color(muted).child("Expose Sona transcription and live audio through an HTTP API."))))
                        .child(div().flex().items_center().gap_3()
                            .child(div().text_size(px(12.0)).text_color(if enabled { cx.theme().warning } else { muted }).child(if enabled { "Enabled · runtime status unavailable" } else { "Disabled" }))
                            .child(Switch::new("settings-api-server-enabled").checked(enabled).accessibility_label("Enable API Server").on_change(cx.listener(|_shell, value, _, cx| set_setting("httpServerEnabled", Value::Bool(*value), cx)))),
                )
                .when(enabled, |panel| panel.child(
                    section_title("Server Status", LucideIcon::Activity, cx)
                        .child(div().flex().items_center().gap_3().p_3().rounded_md().bg(cx.theme().input)
                            .child(Icon::new(LucideIcon::Info).size(px(15.0)).text_color(muted))
                            .child(div().text_size(px(12.0)).text_color(muted).child("The API server runtime is not connected to the GPUI desktop services yet.")))
                        .child(row("Endpoint", "Configured bind address and port.", cx)
                            .child(div().flex().items_center().gap_2()
                                .child(div().font_family("monospace").text_size(px(12.0)).child(endpoint))
                                .child(Button::new("settings-api-copy-endpoint").ghost().small().icon(LucideIcon::Copy).tooltip("Copy Endpoint").accessibility_label("Copy Endpoint").on_click(cx.listener(move |_shell, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(endpoint_to_copy.clone()))))))))
                ))
                .child(section_title("Network & Security", LucideIcon::Globe, cx)
                    .child(row("Host", "127.0.0.1 restricts access to this computer; 0.0.0.0 allows remote access.", cx)
                        .child(div().w(px(200.0)).child(Input::new(&field_states["host"]).small())))
                    .child(numeric_row("Port", "TCP port used by the API server.", "httpServerPort", "port", port, 1, 65535, 14200, field_states, cx))
                    .child(row("IP Whitelist", "Allowed IP addresses, CIDRs, or localhost.", cx)
                        .child(div().w(px(400.0)).child(Input::new(&field_states["whitelist"]).small())))
                    .child(row("API Key", "Optional Bearer token required by HTTP requests.", cx)
                        .child(div().flex().items_center().gap_2()
                            .child(div().w(px(280.0)).child(Input::new(&field_states["apiKey"]).small()))
                            .child(Button::new("settings-api-generate-key").ghost().small().icon(LucideIcon::RefreshCw).tooltip("Generate API Key").accessibility_label("Generate API Key").on_click(cx.listener(move |_shell, _, window, cx| {
                                let key = uuid::Uuid::new_v4().to_string();
                                if _shell.api_server_field_states["apiKey"]
                                    .read(cx)
                                    .value()
                                    .is_empty()
                                {
                                    set_setting("httpServerApiKey", json!(key.clone()), cx);
                                    api_key_state.update(cx, |state, cx| state.set_value(key, window, cx));
                                } else {
                                    _shell.settings_modal.api_server_pending_key = Some(key);
                                    _shell.global_dialog.show(
                                        "Regenerate API Key",
                                        "Regenerating the API key will invalidate the current key and require updating all external clients. Do you want to continue?",
                                        crate::views::dialogs::global_dialog::DialogKind::Confirm,
                                    );
                                    cx.notify();
                                }
                            })))
                            .child(Button::new("settings-api-copy-key").ghost().small().icon(LucideIcon::Copy).tooltip("Copy API Key").accessibility_label("Copy API Key").disabled(api_key.is_empty()).on_click(cx.listener(move |_shell, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(key_to_copy.clone())))))))
                )
                .child(section_title("Task Limits", LucideIcon::SlidersHorizontal, cx)
                    .child(numeric_row("Max Concurrent Tasks", "Concurrent transcription tasks. Default: 2.", "httpServerMaxConcurrent", "maxConcurrent", max_concurrent, 1, 32, 2, field_states, cx))
                    .child(numeric_row("Max Queue Size", "0 allows an unlimited queue. Default: 100.", "httpServerMaxQueueSize", "maxQueue", max_queue, 0, 1000, 100, field_states, cx))
                    .child(numeric_row("Max Upload Size (MB)", "0 allows unlimited upload size. Default: 50 MB.", "httpServerMaxUploadSizeMB", "maxUpload", max_upload, 0, 10000, 50, field_states, cx))
                    .child(numeric_row("Job TTL (Minutes)", "Retention time for completed or failed jobs. 0 disables cleanup.", "httpServerJobTtlMinutes", "jobTtl", job_ttl, 0, 10080, 60, field_states, cx))
                )
                .when(enabled, |panel| panel.child(section_title("Job Queue", LucideIcon::List, cx)
                    .child(div().py_3().text_size(px(13.0)).text_color(muted).child("No active or recent jobs."))))
        )
        .into_any_element()
}

fn section_title(title: &'static str, icon: LucideIcon, cx: &Context<MainShell>) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(
                    Icon::new(icon)
                        .size(px(18.0))
                        .text_color(cx.theme().muted_foreground),
                )
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(cx.theme().border),
        )
}

fn row(title: &'static str, hint: &'static str, cx: &Context<MainShell>) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                ),
        )
}

fn numeric_row(
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    field_key: &'static str,
    value: i64,
    min: i64,
    max: i64,
    default: i64,
    field_states: &std::collections::HashMap<String, Entity<InputState>>,
    cx: &Context<MainShell>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(76.0))
                        .child(Input::new(&field_states[field_key]).small()),
                )
                .child(
                    Button::new(ElementId::Name(format!("api-setting-dec-{key}").into()))
                        .outline()
                        .small()
                        .label("-")
                        .disabled(value <= min)
                        .on_click(cx.listener(move |_shell, _, _, cx| {
                            set_setting(key, json!((value - 1).max(min)), cx)
                        })),
                )
                .child(
                    div()
                        .min_w(px(44.0))
                        .text_center()
                        .text_size(px(13.0))
                        .child(value.to_string()),
                )
                .child(
                    Button::new(ElementId::Name(format!("api-setting-inc-{key}").into()))
                        .outline()
                        .small()
                        .label("+")
                        .disabled(value >= max)
                        .on_click(cx.listener(move |_shell, _, _, cx| {
                            set_setting(key, json!((value + 1).min(max)), cx)
                        })),
                )
                .child(
                    Button::new(ElementId::Name(format!("api-setting-reset-{key}").into()))
                        .ghost()
                        .small()
                        .label("Reset")
                        .disabled(value == default)
                        .on_click(cx.listener(move |_shell, _, _, cx| {
                            set_setting(key, json!(default), cx)
                        })),
                ),
        )
}
