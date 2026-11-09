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
use serde_json::Value;

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

fn config_value_exists(config: &Value, key: &str) -> bool {
    let snake_key = key
        .chars()
        .flat_map(|character| {
            if character.is_ascii_uppercase() {
                vec!['_', character.to_ascii_lowercase()]
            } else {
                vec![character]
            }
        })
        .collect::<String>();
    config
        .get(key)
        .or_else(|| config.get(snake_key))
        .is_some_and(|value| match value {
            Value::String(value) => !value.trim().is_empty(),
            Value::Null => false,
            _ => true,
        })
}

fn asr_slot_is_configured(config: &Value, slot: &str) -> bool {
    let selection = config
        .pointer(&format!("/asr/selections/{slot}"))
        .or_else(|| config.pointer(&format!("/asr/{slot}")));
    let selected_model_id = selection
        .and_then(|selection| {
            selection
                .get("modelId")
                .or_else(|| selection.get("model_id"))
        })
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let request = if let Some(selection) = selection {
        let mut slot_config = config.clone();
        if let Some(asr) = slot_config.get_mut("asr").and_then(Value::as_object_mut) {
            asr.insert("live".to_string(), selection.clone());
            if let Some(selections) = asr.get_mut("selections").and_then(Value::as_object_mut) {
                selections.insert("live".to_string(), selection.clone());
            }
        } else {
            slot_config["asr"] = serde_json::json!({"live": selection});
        }
        crate::platform::agent_control::facade::resolve_live_asr_request_from_config(&slot_config)
    } else {
        crate::platform::agent_control::facade::resolve_live_asr_request_from_config(config)
    };
    let Ok(request) = request else {
        return false;
    };

    match request.engine_config {
        sona_core::ports::asr::AsrEngineConfig::Local {
            model_id,
            model_path,
            vad_model,
            ..
        } => {
            let requires_vad = model_id
                .as_deref()
                .or(selected_model_id.as_deref())
                .and_then(sona_core::models::preset_models::find_preset_model)
                .is_some_and(|model| model.resolved_rules().requires_vad);
            !model_path.trim().is_empty()
                && (!requires_vad || vad_model.is_some_and(|path| !path.trim().is_empty()))
        }
        sona_core::ports::asr::AsrEngineConfig::Online { provider } => {
            let Some(provider_spec) =
                sona_core::ports::asr::find_online_asr_provider(&provider.provider_id)
            else {
                return false;
            };
            let capability = &provider_spec.streaming;
            let has_api_key = config_value_exists(&provider.config, "apiKey")
                || provider_spec
                    .default_api_key_env()
                    .and_then(|name| std::env::var(name).ok())
                    .is_some_and(|value| !value.trim().is_empty());
            let fields_configured = capability
                .required_config_fields
                .iter()
                .filter(|field| !field.eq_ignore_ascii_case("apiKey"))
                .all(|field| config_value_exists(&provider.config, field));

            capability.supported != Some(false)
                && (!capability.requires_api_key || has_api_key)
                && fields_configured
        }
    }
}

fn mcp_status(enabled: bool, running: bool, clients: usize) -> (String, ToolStatusTone) {
    if !enabled {
        ("Disabled".to_string(), ToolStatusTone::Inactive)
    } else if running {
        if clients > 0 {
            (
                format!("{clients} client{}", if clients == 1 { "" } else { "s" }),
                ToolStatusTone::Active,
            )
        } else {
            ("Running".to_string(), ToolStatusTone::Active)
        }
    } else {
        ("Stopped".to_string(), ToolStatusTone::Warning)
    }
}

fn voice_typing_status(
    enabled: bool,
    configured: bool,
    runtime: &crate::services::VoiceTypingRuntimeReadiness,
) -> (String, ToolStatusTone) {
    if !enabled {
        ("Disabled".to_string(), ToolStatusTone::Inactive)
    } else if !configured {
        ("Needs Config".to_string(), ToolStatusTone::Warning)
    } else if runtime.shortcut_error.is_some() || runtime.warmup_error.is_some() {
        ("Error".to_string(), ToolStatusTone::Error)
    } else if runtime.preparing || !runtime.warmup_ready {
        ("Preparing".to_string(), ToolStatusTone::Warning)
    } else {
        ("Ready".to_string(), ToolStatusTone::Active)
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
        let config = services.config.load().ok().flatten().unwrap_or(Value::Null);

        // 1. Subtitles Status
        let is_caption_running = config
            .get("captionMode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let subtitles_configured = asr_slot_is_configured(&config, "caption");
        let subtitles_status = if !is_caption_running {
            ("Stopped".to_string(), ToolStatusTone::Inactive)
        } else if !subtitles_configured {
            ("Needs Config".to_string(), ToolStatusTone::Warning)
        } else {
            ("Running".to_string(), ToolStatusTone::Active)
        };

        // 2. Voice Typing Status
        let vt_enabled = config
            .get("voiceTypingEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let voice_typing_configured = config_value_exists(&config, "voiceTypingShortcut")
            && asr_slot_is_configured(&config, "voiceTyping");
        let runtime_readiness = services.voice_typing_readiness.read().clone();
        let vt_status =
            voice_typing_status(vt_enabled, voice_typing_configured, &runtime_readiness);

        // 3. API Server Status
        let api_enabled = config
            .get("httpServerEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let api_port = config
            .get("httpServerPort")
            .and_then(|v| v.as_u64())
            .unwrap_or(14200);
        let api_status = if api_enabled {
            (format!(":{api_port}"), ToolStatusTone::Active)
        } else {
            ("Disabled".to_string(), ToolStatusTone::Inactive)
        };

        // 4. MCP Agents Status
        let mcp_enabled = config
            .get("mcpEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let mcp_running = services.agent_control.is_running();
        let clients = services.agent_control.tracker().active_count();
        let mcp_status = mcp_status(mcp_enabled, mcp_running, clients);

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
    use super::{
        ToolKind, ToolStatusTone, ToolsSidebar, asr_slot_is_configured, mcp_status,
        voice_typing_status,
    };
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

    #[test]
    fn asr_status_uses_slot_selection_and_requires_a_model_path() {
        assert!(!asr_slot_is_configured(&serde_json::Value::Null, "caption"));

        let caption_config = serde_json::json!({
            "asr": {
                "selections": {
                    "caption": {
                        "engine": "local",
                        "modelPath": "C:/models/caption.onnx"
                    }
                }
            }
        });
        assert!(asr_slot_is_configured(&caption_config, "caption"));
        assert!(!asr_slot_is_configured(&caption_config, "voiceTyping"));

        let model_needing_vad = serde_json::json!({
            "asr": {
                "selections": {
                    "caption": {
                        "engine": "local",
                        "modelId": "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
                        "modelPath": "C:/models/asr.onnx"
                    }
                }
            }
        });
        assert!(!asr_slot_is_configured(&model_needing_vad, "caption"));

        let model_with_vad = serde_json::json!({
            "asr": {
                "selections": {
                    "caption": {
                        "engine": "local",
                        "modelId": "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
                        "modelPath": "C:/models/asr.onnx",
                        "vadModel": "C:/models/vad.onnx"
                    }
                }
            }
        });
        assert!(asr_slot_is_configured(&model_with_vad, "caption"));
    }

    #[test]
    fn online_asr_status_requires_credentials_and_streaming_fields() {
        let missing_credentials = serde_json::json!({
            "asr": {
                "selections": {
                    "caption": {
                        "engine": "online",
                        "providerId": "volcengine-doubao",
                        "modelId": "volc.bigasr.sauc.duration"
                    }
                },
                "providers": {
                    "online": {
                        "volcengine-doubao": {
                            "streamingEndpoint": "wss://asr.example/stream",
                            "streamingResourceId": "volc.seedasr.sauc.duration"
                        }
                    }
                }
            }
        });
        assert!(!asr_slot_is_configured(&missing_credentials, "caption"));

        let configured = serde_json::json!({
            "asr": {
                "selections": {
                    "caption": {
                        "engine": "online",
                        "providerId": "volcengine-doubao",
                        "modelId": "volc.bigasr.sauc.duration"
                    }
                },
                "providers": {
                    "online": {
                        "volcengine-doubao": {
                            "apiKey": "configured-key",
                            "streamingEndpoint": "wss://asr.example/stream",
                            "streamingResourceId": "volc.seedasr.sauc.duration"
                        }
                    }
                }
            }
        });
        assert!(asr_slot_is_configured(&configured, "caption"));
    }

    #[test]
    fn enabled_but_stopped_mcp_is_a_warning() {
        assert_eq!(
            mcp_status(true, false, 0),
            ("Stopped".to_string(), ToolStatusTone::Warning)
        );
        assert_eq!(
            mcp_status(false, false, 0),
            ("Disabled".to_string(), ToolStatusTone::Inactive)
        );
        assert_eq!(
            mcp_status(true, true, 2),
            ("2 clients".to_string(), ToolStatusTone::Active)
        );
    }

    #[test]
    fn voice_typing_status_tracks_configuration_and_runtime() {
        let mut runtime = crate::services::VoiceTypingRuntimeReadiness::default();
        assert_eq!(
            voice_typing_status(false, true, &runtime),
            ("Disabled".to_string(), ToolStatusTone::Inactive)
        );
        assert_eq!(
            voice_typing_status(true, false, &runtime),
            ("Needs Config".to_string(), ToolStatusTone::Warning)
        );
        runtime.preparing = true;
        assert_eq!(
            voice_typing_status(true, true, &runtime),
            ("Preparing".to_string(), ToolStatusTone::Warning)
        );
        runtime.preparing = false;
        runtime.warmup_error = Some("device unavailable".to_string());
        assert_eq!(
            voice_typing_status(true, true, &runtime),
            ("Error".to_string(), ToolStatusTone::Error)
        );
        runtime.warmup_error = None;
        assert_eq!(
            voice_typing_status(true, true, &runtime),
            ("Ready".to_string(), ToolStatusTone::Active)
        );
    }
}
