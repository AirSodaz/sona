use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sona_application::live_transcription::{LiveInputTransform, LiveSourceEpoch};
use sona_core::ports::asr::{AsrRuntimeObserver, AsrStreamingErrorEvent, AsrTranscriptUpdateEvent};
use sona_core::ports::llm::LlmTranslationPort;
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::TranscriptSegment;

use crate::event_bus::AppEventBus;
use crate::platform::event::EventEmitterPort;
use crate::services::DesktopServices;

const CONSUMER_ID: &str = "voice-typing";

struct VoiceTypingObserver {
    emitter: Arc<dyn EventEmitterPort>,
    segments: Mutex<BTreeMap<String, TranscriptSegment>>,
    cjk_spacing: bool,
}

impl VoiceTypingObserver {
    fn new(emitter: Arc<dyn EventEmitterPort>, cjk_spacing: bool) -> Self {
        Self {
            emitter,
            segments: Mutex::new(BTreeMap::new()),
            cjk_spacing,
        }
    }

    fn transcript(&self, final_only: bool) -> String {
        let Ok(segments) = self.segments.lock() else {
            return String::new();
        };
        let mut segments = segments.values().collect::<Vec<_>>();
        segments.sort_by(|left, right| {
            left.start
                .total_cmp(&right.start)
                .then_with(|| left.id.cmp(&right.id))
        });
        let selected = segments
            .iter()
            .copied()
            .filter(|segment| !final_only || segment.is_final)
            .collect::<Vec<_>>();
        let selected = if final_only && selected.is_empty() {
            segments
        } else {
            selected
        };
        selected
            .into_iter()
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn emit_state(&self, text: &str, phase: &str, is_final: bool) {
        let _ = self.emitter.emit(
            "voice-typing:text",
            serde_json::json!({
                "text": text,
                "phase": phase,
                "isFinal": is_final,
            }),
        );
    }
}

impl AsrRuntimeObserver for VoiceTypingObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        if let Ok(mut segments) = self.segments.lock() {
            for id in &event.update.remove_ids {
                segments.remove(id);
            }
            for segment in &event.update.upsert_segments {
                segments.insert(segment.id.clone(), segment.clone());
            }
        }
        let text = self.transcript(false);
        let text = if self.cjk_spacing {
            format_cjk_typography(&text)
        } else {
            text
        };
        self.emit_state(&text, "segment", false);
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}

    fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
        self.emit_state(&event.message, "error", true);
    }
}

struct ActiveVoiceTypingSession {
    observer: Arc<VoiceTypingObserver>,
    shortcut: String,
    is_translate: bool,
    selection: Option<String>,
    context: Option<VoiceTypingContext>,
}

#[derive(Clone, Debug)]
struct VoiceTypingContext {
    mode: String,
    name: String,
    icon: Option<String>,
    directive: String,
    strip_trailing_punctuation: bool,
}

pub fn spawn_voice_typing_event_loop(services: Arc<DesktopServices>, event_bus: Arc<AppEventBus>) {
    let mut receiver = event_bus.subscribe();
    tokio::spawn(async move {
        let warmup_consumer = "voice-typing-warmup".to_string();
        let mut warmup_active = false;
        let initial_config = load_config(&services);
        if setting_bool(&services, &initial_config, "voiceTypingEnabled", false)
            && setting_bool(&services, &initial_config, "keepMicrophoneActive", false)
        {
            let app_data_dir = services
                .sqlite
                .current_context()
                .map(|context| context.app_data_dir().to_path_buf())
                .unwrap_or_else(|_| std::env::temp_dir());
            let microphone_id =
                setting_string(&services, &initial_config, "microphoneId", "default");
            let device_name = (microphone_id != "default").then_some(microphone_id);
            warmup_active = crate::integrations::audio::start_native_live_capture(
                services.audio.clone(),
                services.asr.clone(),
                Arc::new(event_bus.clone()),
                app_data_dir,
                "microphone",
                device_name,
                warmup_consumer.clone(),
                None,
            )
            .await
            .is_ok();
        }
        let mut active: Option<ActiveVoiceTypingSession> = None;
        loop {
            let event = match receiver.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    log::warn!("Voice typing runtime skipped {skipped} queued events");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };

            if event.is("hotkey:voice_typing") {
                let pressed = event.payload["pressed"].as_bool().unwrap_or(false);
                let config = load_config(&services);
                if !setting_bool(&services, &config, "voiceTypingEnabled", false) {
                    continue;
                }
                let mode = setting_string(&services, &config, "voiceTypingMode", "hold");
                match shortcut_transition(&mode, pressed, active.is_some()) {
                    Some(true) => {
                        let _ = event_bus.emit("voice-typing:show", serde_json::json!({}));
                        if let Err(error) =
                            start_session(&services, &event_bus, &config, &mut active, false).await
                        {
                            emit_error(event_bus.as_ref(), &error);
                        }
                    }
                    Some(false) => {
                        finish_session(&services, &event_bus, &mut active, false).await;
                    }
                    None => {}
                }
            } else if event.is("hotkey:voice_typing_translate") {
                let pressed = event.payload["pressed"].as_bool().unwrap_or(false);
                let config = load_config(&services);
                if !setting_bool(&services, &config, "voiceTypingEnabled", false) {
                    continue;
                }
                let mode = setting_string(&services, &config, "voiceTypingMode", "hold");
                match shortcut_transition(&mode, pressed, active.is_some()) {
                    Some(true) => {
                        let _ = event_bus.emit("voice-typing:show", serde_json::json!({}));
                        if let Err(error) =
                            start_session(&services, &event_bus, &config, &mut active, true).await
                        {
                            emit_error(event_bus.as_ref(), &error);
                        }
                    }
                    Some(false) => {
                        finish_session(&services, &event_bus, &mut active, false).await;
                    }
                    None => {}
                }
            } else if event.is("voice-typing:cancel") {
                finish_session(&services, &event_bus, &mut active, true).await;
            } else if event.is("voice-typing:reinject") {
                let text = event.payload["text"].as_str().unwrap_or_default().trim();
                if !text.is_empty() {
                    finish_session(&services, &event_bus, &mut active, true).await;
                    if let Err(error) =
                        crate::platform::system::host::inject_text(text.to_string(), None)
                    {
                        emit_error(event_bus.as_ref(), &error);
                    }
                }
            } else if event.is("hotkey:toggle_quick_recall") {
                let _ = event_bus.emit("voice-typing:show", serde_json::json!({"recall": true}));
            }
        }
        if warmup_active {
            let _ = crate::integrations::audio::stop_native_live_capture(
                &services.audio,
                "microphone",
                warmup_consumer,
            )
            .await;
        }
    });
}

fn shortcut_transition(mode: &str, pressed: bool, active: bool) -> Option<bool> {
    if mode == "toggle" {
        return pressed.then_some(!active);
    }
    match (pressed, active) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

async fn start_session(
    services: &DesktopServices,
    event_bus: &AppEventBus,
    config: &serde_json::Value,
    active: &mut Option<ActiveVoiceTypingSession>,
    is_translate: bool,
) -> Result<(), String> {
    if active.is_some() {
        return Ok(());
    }
    let request =
        crate::platform::agent_control::facade::resolve_voice_typing_asr_request_from_config(
            config,
        )?;
    let app_data_dir = services
        .sqlite
        .current_context()?
        .app_data_dir()
        .to_path_buf();
    let microphone_id = setting_string(services, config, "microphoneId", "default");
    let device_name = (microphone_id != "default").then_some(microphone_id);
    let lease = crate::integrations::audio::start_native_live_capture(
        services.audio.clone(),
        services.asr.clone(),
        Arc::new(event_bus.clone()),
        app_data_dir,
        "microphone",
        device_name,
        CONSUMER_ID.to_string(),
        None,
    )
    .await?;

    let cjk_spacing = setting_bool(services, config, "voiceTypingCjkSpacingEnabled", true);
    let observer = Arc::new(VoiceTypingObserver::new(
        Arc::new(event_bus.clone()),
        cjk_spacing,
    ));
    let source = LiveSourceEpoch::new(lease.source_id.clone(), lease.source_generation);
    let gain = setting_number(services, config, "microphoneBoost", 1.0) as f32;
    if let Err(error) = services
        .asr
        .live_coordinator()
        .acquire(
            CONSUMER_ID,
            source,
            lease.source_cursor,
            LiveInputTransform { gain },
            request,
            observer.clone(),
        )
        .await
    {
        let _ = crate::integrations::audio::stop_native_live_capture(
            &services.audio,
            "microphone",
            CONSUMER_ID.to_string(),
        )
        .await;
        return Err(error.to_string());
    }

    let shortcut = setting_string(services, config, "voiceTypingShortcut", "Alt+V");
    let selection = crate::platform::system::host::get_focused_selection_text()
        .ok()
        .flatten()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    let context = resolve_context(config);
    *active = Some(ActiveVoiceTypingSession {
        observer,
        shortcut,
        is_translate,
        selection,
        context: context.clone(),
    });
    let _ = event_bus.emit(
        "voice-typing:text",
        serde_json::json!({
            "text": "",
            "phase": "listening",
            "hasSelection": active.as_ref().and_then(|session| session.selection.as_ref()).is_some(),
            "contextMode": context.as_ref().map(|value| value.mode.as_str()),
            "contextName": context.as_ref().map(|value| value.name.as_str()),
            "contextIcon": context.as_ref().and_then(|value| value.icon.as_deref()),
        }),
    );
    Ok(())
}

async fn finish_session(
    services: &DesktopServices,
    event_bus: &AppEventBus,
    active: &mut Option<ActiveVoiceTypingSession>,
    cancel: bool,
) {
    let Some(session) = active.take() else {
        return;
    };
    let flush_result = services.asr.live_coordinator().release(CONSUMER_ID).await;
    let stop_result = crate::integrations::audio::stop_native_live_capture(
        &services.audio,
        "microphone",
        CONSUMER_ID.to_string(),
    )
    .await;
    if let Err(error) = flush_result {
        emit_error(event_bus, &error.to_string());
        return;
    }
    if let Err(error) = stop_result {
        emit_error(event_bus, &error);
        return;
    }

    if cancel {
        let _ = event_bus.emit(
            "voice-typing:text",
            serde_json::json!({"text": "", "phase": "done", "isFinal": true, "hasSelection": false}),
        );
        return;
    }

    let text = session.observer.transcript(true).trim().to_string();
    let config = load_config(services);
    let processing_mode = setting_string(services, &config, "voiceTypingProcessingMode", "raw");
    let intermediate_text = if let Some(selection) = session.selection.as_deref()
        && !text.is_empty()
    {
        let _ = event_bus.emit(
            "voice-typing:text",
            serde_json::json!({"text": text, "phase": "polishing", "hasSelection": true}),
        );
        transform_selected_text(
            services,
            &config,
            selection,
            &text,
            session.context.as_ref(),
        )
        .await
        .unwrap_or_else(|error| {
            log::warn!("Voice typing selection rewrite failed; using instruction: {error}");
            text.clone()
        })
    } else if !text.is_empty() && processing_mode == "polish" {
        let _ = event_bus.emit(
            "voice-typing:text",
            serde_json::json!({"text": text, "phase": "polishing"}),
        );
        polish_text(services, &config, &text, session.context.as_ref())
            .await
            .unwrap_or_else(|error| {
                log::warn!("Voice typing polish failed; using raw transcript: {error}");
                text.clone()
            })
    } else {
        text
    };
    let final_text = if session.is_translate && !intermediate_text.is_empty() {
        let _ = event_bus.emit(
            "voice-typing:text",
            serde_json::json!({"text": intermediate_text, "phase": "translating"}),
        );
        translate_text(services, &config, &intermediate_text)
            .await
            .unwrap_or_else(|error| {
                log::warn!("Voice typing translation failed; using source transcript: {error}");
                intermediate_text
            })
    } else {
        intermediate_text
    };
    let final_text = format_final_text(
        services,
        &config,
        &final_text,
        session
            .context
            .as_ref()
            .is_some_and(|context| context.strip_trailing_punctuation),
    );
    if !final_text.is_empty() {
        let modifiers = shortcut_modifiers(&session.shortcut);
        if let Err(error) =
            crate::platform::system::host::inject_text(final_text.clone(), Some(modifiers))
        {
            emit_error(event_bus, &error);
            return;
        }
    }
    let _ = event_bus.emit(
        "voice-typing:text",
        serde_json::json!({"text": final_text, "phase": "done", "isFinal": true, "hasSelection": false}),
    );
}

async fn polish_text(
    services: &DesktopServices,
    config: &serde_json::Value,
    text: &str,
    context: Option<&VoiceTypingContext>,
) -> Result<String, String> {
    let llm_config =
        crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
            config, "polish",
        )
        .ok_or_else(|| "No polish LLM is configured".to_string())?;
    let custom_prompt = setting_string(services, config, "voiceTypingPolishPrompt", "");
    let base_prompt = if custom_prompt.trim().is_empty() {
        DEFAULT_POLISH_PROMPT.to_string()
    } else {
        custom_prompt
    };
    let system_prompt = append_context_directive(base_prompt, context);
    let default_data_dir = crate::platform::storage_location::default_app_local_data_dir();
    let models_dir =
        crate::platform::storage_location::resolve_active_models_dir_for_path(&default_data_dir);
    let adapter = crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);
    let request = sona_core::llm::runtime::LlmCompletionRequest {
        config: llm_config,
        system_prompt: Some(system_prompt),
        input: text.to_string(),
        options: sona_core::llm::runtime::LlmCompletionOptions {
            temperature: Some(0.2),
            max_output_tokens: Some(2048),
            ..Default::default()
        },
        source: None,
    };
    let response = tokio::time::timeout(
        Duration::from_secs(6),
        sona_application::llm::LlmRuntimeService::new(&adapter, adapter.clone()).complete(request),
    )
    .await
    .map_err(|_| "LLM polish timed out after 6 seconds".to_string())?
    .map_err(|error| error.to_string())?;
    let cleaned = clean_polish_output(&response.text);
    if cleaned.is_empty() {
        return Err("LLM polish returned empty text".to_string());
    }
    Ok(cleaned)
}

async fn translate_text(
    _services: &DesktopServices,
    config: &serde_json::Value,
    text: &str,
) -> Result<String, String> {
    let llm_config =
        crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
            config,
            "translation",
        )
        .ok_or_else(|| "No translation LLM is configured".to_string())?;
    let target_language = config
        .get("voiceTypingTargetLanguage")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("en")
        .to_string();
    let default_data_dir = crate::platform::storage_location::default_app_local_data_dir();
    let models_dir =
        crate::platform::storage_location::resolve_active_models_dir_for_path(&default_data_dir);
    let adapter = crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);
    let request = sona_core::ports::llm::LlmTranslationRequest {
        config: llm_config,
        texts: vec![text.to_string()],
        target_language,
        source_language: None,
    };
    let response = tokio::time::timeout(Duration::from_secs(6), adapter.translate_batch(request))
        .await
        .map_err(|_| "LLM translation timed out after 6 seconds".to_string())?
        .map_err(|error| error.to_string())?;
    response
        .into_iter()
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "LLM translation returned empty text".to_string())
}

async fn transform_selected_text(
    services: &DesktopServices,
    config: &serde_json::Value,
    selected_text: &str,
    instruction: &str,
    context: Option<&VoiceTypingContext>,
) -> Result<String, String> {
    let llm_config =
        crate::platform::api_server_config::resolve_feature_llm_config_from_app_config(
            config, "polish",
        )
        .ok_or_else(|| "No polish LLM is configured for selection rewrite".to_string())?;
    let custom_prompt = setting_string(services, config, "voiceTypingPolishPrompt", "");
    let base_prompt = if custom_prompt.trim().is_empty() {
        DEFAULT_TRANSFORM_PROMPT
    } else {
        custom_prompt.as_str()
    };
    let input = format!(
        "【选中文本】：\n{}\n\n【修改指令】：\n{}",
        selected_text.trim(),
        instruction.trim()
    );
    let default_data_dir = crate::platform::storage_location::default_app_local_data_dir();
    let models_dir =
        crate::platform::storage_location::resolve_active_models_dir_for_path(&default_data_dir);
    let adapter = crate::integrations::llm::DesktopLlmAdapter::new(Some(models_dir), None);
    let request = sona_core::llm::runtime::LlmCompletionRequest {
        config: llm_config,
        system_prompt: Some(append_context_directive(base_prompt.to_string(), context)),
        input,
        options: sona_core::llm::runtime::LlmCompletionOptions {
            temperature: Some(0.2),
            max_output_tokens: Some(2048),
            ..Default::default()
        },
        source: None,
    };
    let response = tokio::time::timeout(
        Duration::from_secs(6),
        sona_application::llm::LlmRuntimeService::new(&adapter, adapter.clone()).complete(request),
    )
    .await
    .map_err(|_| "LLM selection rewrite timed out after 6 seconds".to_string())?
    .map_err(|error| error.to_string())?;
    let cleaned = clean_polish_output(&response.text);
    if cleaned.is_empty() {
        return Err("LLM selection rewrite returned empty text".to_string());
    }
    Ok(cleaned)
}

fn clean_polish_output(output: &str) -> String {
    let mut cleaned = output.trim();
    if cleaned.starts_with(&[char::from(96); 3][..]) {
        cleaned = cleaned
            .trim_start_matches(char::from(96))
            .split_once('\n')
            .map_or(cleaned, |(_, body)| body)
            .trim_end_matches(char::from(96))
            .trim();
    }
    let quote = if cleaned.starts_with('"') && cleaned.ends_with('"') {
        Some('"')
    } else if cleaned.starts_with('“') && cleaned.ends_with('”') {
        Some('“')
    } else {
        None
    };
    if let Some(quote) = quote {
        let closing_quote = if quote == '“' { '”' } else { quote };
        cleaned = cleaned
            .strip_prefix(quote)
            .and_then(|value| value.strip_suffix(closing_quote))
            .unwrap_or(cleaned);
    }
    cleaned.trim().to_string()
}

const DEFAULT_POLISH_PROMPT: &str = "你是一个智能语音输入润色助手。你的任务是将用户的语音识别草稿整理为通顺、专业、自然的最终书面文本。\n规则：\n1. 去除所有无意义口头禅与填充语气词（如“那个”、“然后”、“就是说”、“嗯”、“啊”等）。\n2. 修正同音错别字、口语倒装与重复碎句，理顺句子逻辑与表达。\n3. 补全或修正标点符号，保持层级分明。\n4. 严格保留用户的原意与专有名词，严禁删减核心信息。\n5. 严禁生成任何问候、解释、代码块标记或前后缀，仅直接输出最终处理后的纯文本内容。";
const DEFAULT_TRANSFORM_PROMPT: &str = "你是一个智能文本重写与编辑助手。用户选中了一段文本，并口述了修改指令。请严格按照指令编辑选中文本，直接输出最终纯文本；保留关键专业术语，不要解释、问候或添加代码块标记。";

fn append_context_directive(base_prompt: String, context: Option<&VoiceTypingContext>) -> String {
    context.map_or(base_prompt.clone(), |context| {
        if context.directive.is_empty() {
            base_prompt
        } else {
            format!("{base_prompt}\n\n{}", context.directive)
        }
    })
}

fn resolve_context(config: &serde_json::Value) -> Option<VoiceTypingContext> {
    if !config
        .get("voiceTypingContextAwarenessEnabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
    {
        return None;
    }
    let info = crate::platform::system::host::get_foreground_window_info()
        .ok()
        .flatten()?;
    let rules = config
        .get("voiceTypingContextRules")
        .cloned()
        .and_then(|value| {
            serde_json::from_value::<Vec<sona_core::config::VoiceTypingContextRuleRecord>>(value)
                .ok()
        })
        .unwrap_or_default();
    let preset = config
        .get("voiceTypingContextPreset")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("auto");
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "windows"
    };
    let matched = if preset != "auto" && preset != "general" {
        rules.iter().find(|rule| rule.id == preset)
    } else if preset == "general" {
        None
    } else {
        rules.iter().find(|rule| {
            if !rule.enabled {
                return false;
            }
            let app = info.app_name.trim().to_ascii_lowercase();
            let app_match = match platform {
                "windows" => rule.apps_by_platform.windows.iter().any(|target| {
                    target
                        .trim()
                        .trim_end_matches(".exe")
                        .eq_ignore_ascii_case(app.trim_end_matches(".exe"))
                }),
                "macos" => rule
                    .apps_by_platform
                    .macos
                    .iter()
                    .any(|target| target.trim().eq_ignore_ascii_case(&app)),
                _ => rule
                    .apps_by_platform
                    .linux
                    .iter()
                    .any(|target| target.trim().eq_ignore_ascii_case(&app)),
            };
            app_match
                || rule.title_patterns.iter().any(|pattern| {
                    info.window_title
                        .to_ascii_lowercase()
                        .contains(&pattern.trim().to_ascii_lowercase())
                })
        })
    };
    let title = info
        .window_title
        .replace(['\r', '\n', '\t'], " ")
        .trim()
        .chars()
        .take(100)
        .collect::<String>();
    let title_context = if title.is_empty() {
        String::new()
    } else {
        format!(
            "Active window context and topic: \"{title}\". Disambiguate technical terms and domain abbreviations accordingly."
        )
    };
    // React keeps a general context object even when no rule matches, so the
    // active window title still reaches the polish/transform prompt.
    let (mode, name, icon, directive, strip_trailing_punctuation) = match matched {
        Some(rule) => {
            let directive = if title_context.is_empty() {
                rule.prompt_directive.clone()
            } else {
                format!("{title_context}\n{}", rule.prompt_directive)
            };
            (
                rule.id.clone(),
                rule.name.clone(),
                rule.icon.clone(),
                directive,
                rule.strip_trailing_punctuation,
            )
        }
        None => (
            "general".to_string(),
            "General".to_string(),
            None,
            title_context,
            false,
        ),
    };
    Some(VoiceTypingContext {
        mode,
        name,
        icon,
        directive,
        strip_trailing_punctuation,
    })
}

fn format_final_text(
    services: &DesktopServices,
    config: &serde_json::Value,
    text: &str,
    strip_trailing_punctuation: bool,
) -> String {
    let cjk_spacing = setting_bool(services, config, "voiceTypingCjkSpacingEnabled", true);
    let mut formatted = if cjk_spacing {
        format_cjk_typography(text.trim())
    } else {
        text.trim().to_string()
    };
    if strip_trailing_punctuation {
        formatted = formatted.trim_end_matches(['。', '.']).to_string();
    }
    let sets = config
        .get("textReplacementSets")
        .cloned()
        .and_then(|value| {
            serde_json::from_value::<
                Vec<sona_core::transcription::postprocess::TranscriptTextReplacementRuleSet>,
            >(value)
            .ok()
        })
        .unwrap_or_default();
    if !sets.is_empty() {
        let postprocessor = sona_core::transcription::postprocess::TranscriptPostprocessor::compile(
            sona_core::transcription::postprocess::TranscriptPostprocessOptions {
                text_replacement_sets: sets,
                drop_final_dot_segments: false,
            },
        );
        if let Ok(postprocessor) = postprocessor {
            let segment = sona_core::transcription::transcript::TranscriptSegment {
                id: "voice-typing-final".to_string(),
                text: formatted,
                start: 0.0,
                end: 0.0,
                is_final: true,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                draft_translation: None,
                speaker: None,
                speaker_attribution: None,
            };
            formatted = postprocessor
                .process_segments(vec![segment])
                .into_iter()
                .next()
                .map_or_else(String::new, |segment| segment.text);
        }
    }
    formatted
}

fn format_cjk_typography(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let chars = text.chars().collect::<Vec<_>>();
    let mut spaced = Vec::with_capacity(chars.len() + 8);
    for (index, current) in chars.iter().copied().enumerate() {
        if index > 0 {
            let previous = chars[index - 1];
            let needs_space = (is_cjk(previous) && is_latin_or_number(current))
                || (is_latin_or_number(previous) && is_cjk(current));
            if needs_space && !previous.is_whitespace() && !current.is_whitespace() {
                spaced.push(' ');
            }
        }
        let previous = spaced.iter().rev().copied().find(|ch| !ch.is_whitespace());
        let next = chars.get(index + 1).copied();
        let converted = if previous.is_some_and(is_chinese_or_japanese) {
            match current {
                ',' => '，',
                '?' => '？',
                '!' => '！',
                ':' if !next.is_some_and(|ch| ch.is_ascii_digit() || ch == '/') => '：',
                '.' if next.is_none_or(char::is_whitespace) => '。',
                other => other,
            }
        } else {
            current
        };
        spaced.push(converted);
    }

    let mut cleaned = Vec::with_capacity(spaced.len());
    for current in spaced {
        if is_cjk_closing_punctuation(current) {
            while cleaned.last().is_some_and(|ch: &char| ch.is_whitespace()) {
                cleaned.pop();
            }
        }
        if is_cjk_opening_punctuation(cleaned.last().copied().unwrap_or('\0'))
            && current.is_whitespace()
        {
            continue;
        }
        if current.is_whitespace()
            && cleaned
                .last()
                .is_some_and(|ch| is_cjk_closing_punctuation(*ch))
        {
            continue;
        }
        if is_cjk_duplicate_punctuation(current) && cleaned.last() == Some(&current) {
            continue;
        }
        cleaned.push(current);
    }
    cleaned.into_iter().collect::<String>().trim().to_string()
}

fn is_cjk(ch: char) -> bool {
    is_chinese_or_japanese(ch)
        || matches!(ch as u32, 0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F)
}

fn is_chinese_or_japanese(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FA5 | 0xF900..=0xFAFF | 0xFF66..=0xFF9F)
}

fn is_latin_or_number(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

fn is_cjk_closing_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '、'
            | '）'
            | '】'
            | '》'
            | '」'
            | '』'
            | '”'
            | '’'
    )
}

fn is_cjk_opening_punctuation(ch: char) -> bool {
    matches!(ch, '（' | '【' | '《' | '「' | '『' | '“' | '‘')
}

fn is_cjk_duplicate_punctuation(ch: char) -> bool {
    matches!(ch, '，' | '。' | '！' | '？' | '；' | '：' | '、')
}

fn shortcut_modifiers(shortcut: &str) -> Vec<crate::platform::system::host::ShortcutModifier> {
    shortcut
        .split('+')
        .filter_map(|part| match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(crate::platform::system::host::ShortcutModifier::Control),
            "alt" | "option" => Some(crate::platform::system::host::ShortcutModifier::Alt),
            "shift" => Some(crate::platform::system::host::ShortcutModifier::Shift),
            "super" | "meta" | "win" | "cmd" => {
                Some(crate::platform::system::host::ShortcutModifier::Meta)
            }
            _ => None,
        })
        .collect()
}

fn load_config(services: &DesktopServices) -> serde_json::Value {
    services
        .config
        .load()
        .ok()
        .flatten()
        .unwrap_or_else(|| serde_json::json!({}))
}

fn setting_bool(
    services: &DesktopServices,
    config: &serde_json::Value,
    key: &str,
    default: bool,
) -> bool {
    config
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .or_else(|| {
            services
                .config
                .get_setting(key)
                .ok()
                .flatten()
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(default)
}

fn setting_string(
    services: &DesktopServices,
    config: &serde_json::Value,
    key: &str,
    default: &str,
) -> String {
    config
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            services
                .config
                .get_setting(key)
                .ok()
                .flatten()
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
        })
        .unwrap_or_else(|| default.to_string())
}

fn setting_number(
    services: &DesktopServices,
    config: &serde_json::Value,
    key: &str,
    default: f64,
) -> f64 {
    config
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .or_else(|| {
            services
                .config
                .get_setting(key)
                .ok()
                .flatten()
                .and_then(|value| value.as_f64())
        })
        .unwrap_or(default)
}

fn emit_error(event_bus: &AppEventBus, message: &str) {
    let _ = event_bus.emit(
        "voice-typing:text",
        serde_json::json!({"text": message, "phase": "error", "isFinal": true}),
    );
}

#[cfg(test)]
mod tests {
    use super::shortcut_transition;

    #[test]
    fn hold_shortcut_starts_on_press_and_stops_on_release() {
        assert_eq!(shortcut_transition("hold", true, false), Some(true));
        assert_eq!(shortcut_transition("hold", true, true), None);
        assert_eq!(shortcut_transition("hold", false, true), Some(false));
        assert_eq!(shortcut_transition("hold", false, false), None);
    }

    #[test]
    fn toggle_shortcut_changes_state_only_on_press() {
        assert_eq!(shortcut_transition("toggle", true, false), Some(true));
        assert_eq!(shortcut_transition("toggle", false, true), None);
        assert_eq!(shortcut_transition("toggle", true, true), Some(false));
    }

    #[test]
    fn final_transcript_falls_back_to_latest_non_final_segments() {
        use std::sync::Arc;

        use crate::platform::event::MockEventEmitter;
        use sona_core::transcription::transcript::TranscriptSegment;

        let observer = super::VoiceTypingObserver::new(Arc::new(MockEventEmitter::new()), false);
        observer.segments.lock().unwrap().insert(
            "segment-1".to_string(),
            TranscriptSegment {
                id: "segment-1".to_string(),
                start: 0.0,
                end: 1.0,
                text: "captured words".to_string(),
                is_final: false,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                draft_translation: None,
                speaker: None,
                speaker_attribution: None,
            },
        );

        assert_eq!(observer.transcript(true), "captured words");
    }

    #[test]
    fn polish_output_removes_markdown_fences_and_wrapping_quotes() {
        let fence = char::from(96).to_string().repeat(3);
        let fenced = format!("{fence}text\n整理后的文本\n{fence}");
        assert_eq!(super::clean_polish_output(&fenced), "整理后的文本");
        assert_eq!(super::clean_polish_output("“整理后的文本”"), "整理后的文本");
        assert_eq!(super::clean_polish_output("\"plain text\""), "plain text");
    }

    #[test]
    fn cjk_final_text_spacing_and_punctuation_match_frontend_rules() {
        assert_eq!(
            super::format_cjk_typography("识别到PR请求"),
            "识别到 PR 请求"
        );
        assert_eq!(super::format_cjk_typography("第1阶段完成"), "第 1 阶段完成");
        assert_eq!(super::format_cjk_typography("hello世界"), "hello 世界");
        assert_eq!(super::format_cjk_typography("测试 ,完成."), "测试，完成。");
        assert_eq!(super::format_cjk_typography("时间12:30"), "时间 12:30");
    }

    #[test]
    fn context_directive_is_appended_and_developer_punctuation_is_stripped() {
        let context = super::VoiceTypingContext {
            mode: "developer".to_string(),
            name: "Developer".to_string(),
            icon: Some("💻".to_string()),
            directive: "Developer mode".to_string(),
            strip_trailing_punctuation: true,
        };
        assert_eq!(
            super::append_context_directive("Base prompt".to_string(), Some(&context)),
            "Base prompt\n\nDeveloper mode"
        );
        assert_eq!(super::format_cjk_typography("ship it。"), "ship it。");
    }
}
