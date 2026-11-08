use std::collections::HashMap;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::{SettingsModal, SettingsModalEvent};
use crate::app::GlobalDesktopServices;

pub struct ApiServerSettingsTab {
    pub field_states: HashMap<String, Entity<InputState>>,
    pub pending_key: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl ApiServerSettingsTab {
    pub fn new(
        initial_config: &Value,
        window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> Self {
        let mut field_states = HashMap::new();
        let mut subscriptions = Vec::new();

        for (key, config_key, placeholder, masked) in [
            ("host", "httpServerHost", "127.0.0.1", false),
            ("port", "httpServerPort", "14200", false),
            ("whitelist", "httpServerIpWhitelist", "127.0.0.1", false),
            ("apiKey", "httpServerApiKey", "Optional Bearer Token", true),
        ] {
            let initial_value = initial_config
                .get(config_key)
                .map(|val| match val {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => String::new(),
                })
                .unwrap_or_else(|| placeholder.to_string());

            let state = cx.new(|cx| {
                let mut input = InputState::new(window, cx)
                    .default_value(initial_value)
                    .placeholder(placeholder);
                if masked {
                    input = input.masked(true);
                }
                input
            });

            let sub = cx.subscribe(&state, move |_modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let val = state.read(cx).value().to_string();
                    let services = cx.global::<GlobalDesktopServices>().0.clone();
                    let config_key_owned = config_key.to_string();
                    cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                        let _ = services
                            .config
                            .set_setting(&config_key_owned, &Value::String(val));
                    })
                    .detach();
                }
            });

            field_states.insert(key.to_string(), state);
            subscriptions.push(sub);
        }

        for (key, config_key, _min, _max, default) in [
            ("maxConcurrent", "httpServerMaxConcurrent", 1, 32, 2),
            ("maxQueue", "httpServerMaxQueueSize", 0, 1000, 100),
            ("maxUpload", "httpServerMaxUploadSizeMB", 0, 10000, 50),
            ("jobTtl", "httpServerJobTtlMinutes", 0, 10080, 60),
        ] {
            let initial_value = initial_config
                .get(config_key)
                .and_then(Value::as_i64)
                .unwrap_or(default)
                .to_string();

            let state = cx.new(|cx| InputState::new(window, cx).default_value(initial_value));

            let sub = cx.subscribe(&state, move |_modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let val = state.read(cx).value().parse::<i64>().unwrap_or(default);
                    let services = cx.global::<GlobalDesktopServices>().0.clone();
                    let config_key_owned = config_key.to_string();
                    cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                        let _ = services.config.set_setting(&config_key_owned, &json!(val));
                    })
                    .detach();
                }
            });

            field_states.insert(key.to_string(), state);
            subscriptions.push(sub);
        }

        Self {
            field_states,
            pending_key: None,
            _subscriptions: subscriptions,
        }
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
                                .child(Switch::new("settings-api-server-enabled").checked(enabled).accessibility_label("Enable API Server").on_change(cx.listener(|_modal, value, _, cx| {
                                    set_setting("httpServerEnabled", Value::Bool(*value), cx);
                                })))),
                    )
                    .when(enabled, |panel| panel.child(
                        section_title("Server Status", LucideIcon::Activity, cx)
                            .child(div().flex().items_center().gap_3().p_3().rounded_md().bg(cx.theme().input)
                                .child(Icon::new(LucideIcon::Info).size(px(15.0)).text_color(muted))
                                .child(div().text_size(px(12.0)).text_color(muted).child("The API server runtime is not connected to the GPUI desktop services yet.")))
                            .child(row("Endpoint", "Configured bind address and port.", cx)
                                .child(div().flex().items_center().gap_2()
                                    .child(div().font_family("monospace").text_size(px(12.0)).child(endpoint))
                                    .child(Button::new("settings-api-copy-endpoint").ghost().small().icon(LucideIcon::Copy).tooltip("Copy Endpoint").accessibility_label("Copy Endpoint").on_click(cx.listener(move |_modal, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(endpoint_to_copy.clone()))))))))
                    ))
                    .child(section_title("Network & Security", LucideIcon::Globe, cx)
                        .child(row("Host", "127.0.0.1 restricts access to this computer; 0.0.0.0 allows remote access.", cx)
                            .child(div().w(px(200.0)).child(Input::new(&self.field_states["host"]).small())))
                        .child(numeric_row("Port", "TCP port used by the API server.", "httpServerPort", "port", port, 1, 65535, 14200, &self.field_states, cx))
                        .child(row("IP Whitelist", "Allowed IP addresses, CIDRs, or localhost.", cx)
                            .child(div().w(px(400.0)).child(Input::new(&self.field_states["whitelist"]).small())))
                        .child(row("API Key", "Optional Bearer token required by HTTP requests.", cx)
                            .child(div().flex().items_center().gap_2()
                                .child(div().w(px(280.0)).child(Input::new(&self.field_states["apiKey"]).small()))
                                .child(Button::new("settings-api-generate-key").ghost().small().icon(LucideIcon::RefreshCw).tooltip("Generate API Key").accessibility_label("Generate API Key").on_click(cx.listener(move |modal, _, window, cx| {
                                    let key = uuid::Uuid::new_v4().to_string();
                                    if modal.api_server.field_states["apiKey"].read(cx).value().is_empty() {
                                        set_setting("httpServerApiKey", json!(key.clone()), cx);
                                        modal.api_server.field_states["apiKey"].update(cx, |state, cx| state.set_value(key, window, cx));
                                    } else {
                                        modal.api_server.pending_key = Some(key);
                                        cx.emit(SettingsModalEvent::RequestApiKeyRegenConfirm);
                                        cx.notify();
                                    }
                                })))
                                .child(Button::new("settings-api-copy-key").ghost().small().icon(LucideIcon::Copy).tooltip("Copy API Key").accessibility_label("Copy API Key").disabled(api_key.is_empty()).on_click(cx.listener(move |_modal, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(key_to_copy.clone())))))),
                    )
                    .child(section_title("Task Limits", LucideIcon::SlidersHorizontal, cx)
                        .child(numeric_row("Max Concurrent Tasks", "Concurrent transcription tasks. Default: 2.", "httpServerMaxConcurrent", "maxConcurrent", max_concurrent, 1, 32, 2, &self.field_states, cx))
                        .child(numeric_row("Max Queue Size", "0 allows an unlimited queue. Default: 100.", "httpServerMaxQueueSize", "maxQueue", max_queue, 0, 1000, 100, &self.field_states, cx))
                        .child(numeric_row("Max Upload Size (MB)", "0 allows unlimited upload size. Default: 50 MB.", "httpServerMaxUploadSizeMB", "maxUpload", max_upload, 0, 10000, 50, &self.field_states, cx))
                        .child(numeric_row("Job TTL (Minutes)", "Retention time for completed or failed jobs. 0 disables cleanup.", "httpServerJobTtlMinutes", "jobTtl", job_ttl, 0, 10080, 60, &self.field_states, cx))
                    )
                    .when(enabled, |panel| panel.child(section_title("Job Queue", LucideIcon::List, cx)
                        .child(div().py_3().text_size(px(13.0)).text_color(muted).child("No active or recent jobs."))))
            )
            .into_any_element()
    }
}

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

fn set_setting(key: &'static str, value: Value, cx: &mut Context<SettingsModal>) {
    let config = cx.global::<GlobalDesktopServices>().0.config.clone();
    if let Err(error) = config.set_setting(key, &value) {
        log::error!("Failed to update API server setting {key}: {error}");
    }
    cx.notify();
}

fn section_title(
    title: &'static str,
    icon: LucideIcon,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div().flex().flex_col().gap(px(12.0)).child(
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                Icon::new(icon)
                    .size(px(16.0))
                    .text_color(cx.theme().primary),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            ),
    )
}

fn row(title: &'static str, hint: &'static str, cx: &Context<SettingsModal>) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .py(px(4.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
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
    config_key: &'static str,
    field_key: &'static str,
    current: i64,
    min: i64,
    max: i64,
    default: i64,
    field_states: &HashMap<String, Entity<InputState>>,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    let state = field_states.get(field_key).cloned();
    row(title, hint, cx).child(
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                Button::new(format!("settings-api-{field_key}-dec"))
                    .outline()
                    .small()
                    .label("-")
                    .disabled(current <= min)
                    .on_click(cx.listener(move |modal, _, window, cx| {
                        let next = (current - 1).max(min);
                        set_setting(config_key, json!(next), cx);
                        if let Some(state) = modal.api_server.field_states.get(field_key) {
                            state.update(cx, |s, cx| s.set_value(next.to_string(), window, cx));
                        }
                    })),
            )
            .when_some(state, |d, state| {
                d.child(div().w(px(72.0)).child(Input::new(&state).small()))
            })
            .child(
                Button::new(format!("settings-api-{field_key}-inc"))
                    .outline()
                    .small()
                    .label("+")
                    .disabled(current >= max)
                    .on_click(cx.listener(move |modal, _, window, cx| {
                        let next = (current + 1).min(max);
                        set_setting(config_key, json!(next), cx);
                        if let Some(state) = modal.api_server.field_states.get(field_key) {
                            state.update(cx, |s, cx| s.set_value(next.to_string(), window, cx));
                        }
                    })),
            )
            .when(current != default, |d| {
                d.child(
                    Button::new(format!("settings-api-{field_key}-reset"))
                        .ghost()
                        .small()
                        .label("Default")
                        .on_click(cx.listener(move |modal, _, window, cx| {
                            set_setting(config_key, json!(default), cx);
                            if let Some(state) = modal.api_server.field_states.get(field_key) {
                                state.update(cx, |s, cx| {
                                    s.set_value(default.to_string(), window, cx)
                                });
                            }
                        })),
                )
            }),
    )
}
