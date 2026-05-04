use crate::preset_models::{
    build_model_catalog_snapshot, resolve_model_catalog_selected_ids, ModelCatalogModel,
    ModelRules as PresetModelRules, ModelSelectionPaths,
};
use crate::runtime_status;
use crate::sherpa::{AsrInferenceMetric, AsrRuntimeMetricsSnapshot, SherpaState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Manager, Runtime, State};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Ready,
    Warning,
    Missing,
    Failed,
    Info,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DiagnosticActionSpec {
    #[serde(rename = "open_settings")]
    OpenSettings {
        label: TextSpec,
        #[serde(rename = "settingsTab")]
        settings_tab: String,
    },
    #[serde(rename = "request_microphone_permission")]
    RequestMicrophonePermission { label: TextSpec },
    #[serde(rename = "retry_voice_typing_warmup")]
    RetryVoiceTypingWarmup { label: TextSpec },
    #[serde(rename = "run_first_run_setup")]
    RunFirstRunSetup { label: TextSpec },
    #[serde(rename = "open_log_folder")]
    OpenLogFolder { label: TextSpec },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TextSpec {
    pub key: String,
    pub default_value: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
}

impl TextSpec {
    fn new(key: &str, default_value: &str) -> Self {
        Self {
            key: key.to_string(),
            default_value: default_value.to_string(),
            params: HashMap::new(),
        }
    }

    fn with_param(mut self, key: &str, value: impl Into<String>) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsCoreInput {
    pub config: DiagnosticsConfigInput,
    #[serde(default)]
    pub selected_models: SelectedModelsInput,
    #[serde(default)]
    pub model_rules: ModelRulesInput,
    #[serde(default)]
    pub path_statuses: PathStatusesInput,
    pub permission_state: String,
    pub microphone_probe: DeviceProbeInput,
    pub system_audio_probe: DeviceProbeInput,
    pub voice_typing_readiness: VoiceTypingReadinessInput,
    #[serde(default)]
    pub runtime_environment: RuntimeEnvironmentStatus,
    #[serde(default)]
    pub asr_runtime_metrics: AsrRuntimeMetricsSnapshot,
    #[serde(default)]
    pub onboarding_ready: bool,
    #[serde(default)]
    pub punctuation_required: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsConfigInput {
    pub streaming_model_path: String,
    pub offline_model_path: String,
    #[serde(default)]
    pub vad_model_path: String,
    #[serde(default)]
    pub punctuation_model_path: String,
    #[serde(default = "default_microphone_id")]
    pub microphone_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedModelsInput {
    pub live: Option<ModelSummaryInput>,
    pub offline: Option<ModelSummaryInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummaryInput {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRulesInput {
    pub live: Option<ModelRuleInput>,
    pub offline: Option<ModelRuleInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRuleInput {
    pub requires_vad: bool,
    pub requires_punctuation: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathStatusesInput {
    pub live_model: Option<RuntimePathStatus>,
    pub offline_model: Option<RuntimePathStatus>,
    pub vad: Option<RuntimePathStatus>,
    pub punctuation: Option<RuntimePathStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePathStatus {
    pub path: String,
    pub kind: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProbeInput {
    pub options: Vec<DeviceOptionInput>,
    pub available: bool,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DeviceOptionInput {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTypingReadinessInput {
    pub state: String,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStatus {
    pub ffmpeg_path: String,
    pub ffmpeg_exists: bool,
    pub log_dir_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsCoreSnapshot {
    pub scanned_at: String,
    pub overview: Vec<DiagnosticOverviewCardSpec>,
    pub sections: Vec<DiagnosticSectionSpec>,
    pub runtime_environment: RuntimeEnvironmentStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticOverviewCardSpec {
    pub id: String,
    pub title: TextSpec,
    pub description: TextSpec,
    pub status: DiagnosticStatus,
    pub action: Option<DiagnosticActionSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSectionSpec {
    pub id: String,
    pub title: TextSpec,
    pub description: Option<TextSpec>,
    pub checks: Vec<DiagnosticCheckSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheckSpec {
    pub id: String,
    pub title: TextSpec,
    pub description: TextSpec,
    pub status: DiagnosticStatus,
    pub action: Option<DiagnosticActionSpec>,
    pub meta: Option<TextSpec>,
}

struct BuiltChecks {
    model: ModelChecks,
    input: InputChecks,
    runtime: RuntimeChecks,
    asr: AsrPerformanceChecks,
}

struct ModelChecks {
    live_model_check: DiagnosticCheckSpec,
    offline_model_check: DiagnosticCheckSpec,
    vad_check: DiagnosticCheckSpec,
    punctuation_check: DiagnosticCheckSpec,
}

struct InputChecks {
    permission_check: DiagnosticCheckSpec,
    microphone_check: DiagnosticCheckSpec,
    system_audio_check: DiagnosticCheckSpec,
}

struct RuntimeChecks {
    voice_typing_check: DiagnosticCheckSpec,
    ffmpeg_check: DiagnosticCheckSpec,
    log_dir_check: DiagnosticCheckSpec,
}

struct AsrPerformanceChecks {
    model_memory_check: DiagnosticCheckSpec,
    live_latency_check: DiagnosticCheckSpec,
    batch_latency_check: DiagnosticCheckSpec,
}

#[tauri::command]
pub async fn get_diagnostics_core_snapshot<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SherpaState>,
    input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let input = enrich_diagnostics_core_input(&app, state.inner(), input).await?;
    Ok(build_diagnostics_core_snapshot(input))
}

async fn enrich_diagnostics_core_input<R: Runtime>(
    app: &AppHandle<R>,
    state: &SherpaState,
    mut input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreInput, String> {
    let models_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("models");
    std::fs::create_dir_all(&models_dir).map_err(|error| {
        format!(
            "Failed to create models directory {}: {error}",
            models_dir.display()
        )
    })?;

    let catalog = build_model_catalog_snapshot(&models_dir);
    let selected = resolve_model_catalog_selected_ids(
        &catalog,
        &ModelSelectionPaths {
            streaming_model_path: input.config.streaming_model_path.clone(),
            offline_model_path: input.config.offline_model_path.clone(),
            speaker_segmentation_model_path: String::new(),
            speaker_embedding_model_path: String::new(),
        },
    );

    let live_model = selected
        .streaming
        .as_deref()
        .and_then(|model_id| catalog.models.iter().find(|model| model.id == model_id));
    let offline_model = selected
        .offline
        .as_deref()
        .and_then(|model_id| catalog.models.iter().find(|model| model.id == model_id));

    input.selected_models = SelectedModelsInput {
        live: live_model.map(model_summary_input),
        offline: offline_model.map(model_summary_input),
    };
    input.model_rules = ModelRulesInput {
        live: live_model.map(|model| model_rule_input(model.rules)),
        offline: offline_model.map(|model| model_rule_input(model.rules)),
    };
    input.path_statuses = PathStatusesInput {
        live_model: resolve_core_path_status(input.config.streaming_model_path.trim()),
        offline_model: resolve_core_path_status(input.config.offline_model_path.trim()),
        vad: resolve_core_path_status(input.config.vad_model_path.trim()),
        punctuation: resolve_core_path_status(input.config.punctuation_model_path.trim()),
    };
    input.runtime_environment =
        runtime_environment_input(runtime_status::resolve_runtime_environment_status(app)?);
    input.asr_runtime_metrics = state.metrics_snapshot().await;
    input.onboarding_ready = !input.config.streaming_model_path.trim().is_empty()
        && !input.config.offline_model_path.trim().is_empty();
    input.punctuation_required = [
        input.model_rules.live.as_ref(),
        input.model_rules.offline.as_ref(),
    ]
    .iter()
    .any(|rules| rules.map(|item| item.requires_punctuation).unwrap_or(false));

    Ok(input)
}

pub fn build_diagnostics_core_snapshot(input: DiagnosticsCoreInput) -> DiagnosticsCoreSnapshot {
    let checks = BuiltChecks {
        model: build_model_checks(&input),
        input: build_input_checks(&input),
        runtime: build_runtime_checks(&input),
        asr: build_asr_performance_checks(&input),
    };
    let sections = vec![
        DiagnosticSectionSpec {
            id: "models".to_string(),
            title: text("settings.diagnostics.models_section", "Models"),
            description: Some(text(
                "settings.diagnostics.models_section_description",
                "Check that local transcription models and required dependencies are present.",
            )),
            checks: vec![
                checks.model.live_model_check.clone(),
                checks.model.offline_model_check.clone(),
                checks.model.vad_check.clone(),
                checks.model.punctuation_check.clone(),
            ],
        },
        DiagnosticSectionSpec {
            id: "input-capture".to_string(),
            title: text("settings.diagnostics.input_section", "Input & Capture"),
            description: Some(text(
                "settings.diagnostics.input_section_description",
                "Check permissions and the availability of input or capture devices.",
            )),
            checks: vec![
                checks.input.permission_check.clone(),
                checks.input.microphone_check.clone(),
                checks.input.system_audio_check.clone(),
            ],
        },
        DiagnosticSectionSpec {
            id: "runtime-environment".to_string(),
            title: text(
                "settings.diagnostics.runtime_section",
                "Runtime & Environment",
            ),
            description: Some(text(
                "settings.diagnostics.runtime_section_description",
                "Check background runtime readiness and packaged environment dependencies.",
            )),
            checks: vec![
                checks.runtime.voice_typing_check.clone(),
                checks.runtime.ffmpeg_check.clone(),
                checks.runtime.log_dir_check.clone(),
            ],
        },
        DiagnosticSectionSpec {
            id: "asr-performance".to_string(),
            title: text(
                "settings.diagnostics.asr_performance_section",
                "ASR Performance",
            ),
            description: Some(text(
                "settings.diagnostics.asr_performance_section_description",
                "Review recent local ASR model memory and transcription latency samples.",
            )),
            checks: vec![
                checks.asr.model_memory_check.clone(),
                checks.asr.live_latency_check.clone(),
                checks.asr.batch_latency_check.clone(),
            ],
        },
    ];
    let runtime_environment = input.runtime_environment.clone();

    DiagnosticsCoreSnapshot {
        scanned_at: now_iso_like(),
        overview: build_overview_cards(&input, &checks),
        sections,
        runtime_environment,
    }
}

fn model_summary_input(model: &ModelCatalogModel) -> ModelSummaryInput {
    ModelSummaryInput {
        id: model.id.clone(),
        name: model.name.clone(),
    }
}

fn model_rule_input(rules: PresetModelRules) -> ModelRuleInput {
    ModelRuleInput {
        requires_vad: rules.requires_vad,
        requires_punctuation: rules.requires_punctuation,
    }
}

fn resolve_core_path_status(path: &str) -> Option<RuntimePathStatus> {
    if path.is_empty() {
        return None;
    }

    Some(runtime_path_status_input(
        runtime_status::resolve_runtime_path_status(path),
    ))
}

fn runtime_path_status_input(status: runtime_status::RuntimePathStatus) -> RuntimePathStatus {
    RuntimePathStatus {
        path: status.path,
        kind: match status.kind {
            runtime_status::RuntimePathKind::File => "file",
            runtime_status::RuntimePathKind::Directory => "directory",
            runtime_status::RuntimePathKind::Missing => "missing",
            runtime_status::RuntimePathKind::Unknown => "unknown",
        }
        .to_string(),
        error: status.error,
    }
}

fn runtime_environment_input(
    status: runtime_status::RuntimeEnvironmentStatus,
) -> RuntimeEnvironmentStatus {
    RuntimeEnvironmentStatus {
        ffmpeg_path: status.ffmpeg_path,
        ffmpeg_exists: status.ffmpeg_exists,
        log_dir_path: status.log_dir_path,
    }
}

fn build_model_checks(input: &DiagnosticsCoreInput) -> ModelChecks {
    let config = &input.config;
    let streaming_path = config.streaming_model_path.trim();
    let offline_path = config.offline_model_path.trim();
    let vad_path = config.vad_model_path.trim();
    let punctuation_path = config.punctuation_model_path.trim();

    let live_model_check = build_model_path_policy_check(PathPolicyCheckArgs {
        id: "live-model",
        title: text("settings.diagnostics.live_model_title", "Live Record Model"),
        selected_path: streaming_path,
        path_status: input.path_statuses.live_model.as_ref(),
        missing_selection_status: DiagnosticStatus::Missing,
        missing_selection_description: text(
            "settings.diagnostics.live_model_missing",
            "No Live Record Model is selected yet.",
        ),
        missing_path_status: DiagnosticStatus::Failed,
        unknown_path_status: DiagnosticStatus::Info,
        ready_description: text(
            "settings.diagnostics.model_ready",
            "The selected model is configured and reachable.",
        ),
        action: Some(open_model_settings_action()),
        missing_path_meta: Some(literal_meta(config.streaming_model_path.clone())),
        unknown_path_meta: Some(literal_meta(streaming_path)),
        ready_meta: Some(literal_meta(
            input
                .selected_models
                .live
                .as_ref()
                .map(|model| model.name.clone())
                .unwrap_or_else(|| config.streaming_model_path.clone()),
        )),
    });

    let offline_model_check = build_model_path_policy_check(PathPolicyCheckArgs {
        id: "offline-model",
        title: text(
            "settings.diagnostics.offline_model_title",
            "Batch Import Model",
        ),
        selected_path: offline_path,
        path_status: input.path_statuses.offline_model.as_ref(),
        missing_selection_status: DiagnosticStatus::Missing,
        missing_selection_description: text(
            "settings.diagnostics.offline_model_missing",
            "No Batch Import Model is selected yet.",
        ),
        missing_path_status: DiagnosticStatus::Failed,
        unknown_path_status: DiagnosticStatus::Info,
        ready_description: text(
            "settings.diagnostics.model_ready",
            "The selected model is configured and reachable.",
        ),
        action: Some(open_model_settings_action()),
        missing_path_meta: Some(literal_meta(config.offline_model_path.clone())),
        unknown_path_meta: Some(literal_meta(offline_path)),
        ready_meta: Some(literal_meta(
            input
                .selected_models
                .offline
                .as_ref()
                .map(|model| model.name.clone())
                .unwrap_or_else(|| config.offline_model_path.clone()),
        )),
    });

    let vad_check = if input.selected_models.live.is_none() {
        check(
            "vad",
            text("settings.diagnostics.vad_title", "VAD Dependency"),
            DiagnosticStatus::Info,
            text(
                "settings.diagnostics.vad_unknown",
                "Pick a Live Record Model first to evaluate whether a VAD model is required.",
            ),
            Some(open_model_settings_action()),
            None,
        )
    } else if !input
        .model_rules
        .live
        .as_ref()
        .map(|rules| rules.requires_vad)
        .unwrap_or(false)
    {
        check(
            "vad",
            text("settings.diagnostics.vad_title", "VAD Dependency"),
            DiagnosticStatus::Ready,
            text(
                "settings.diagnostics.vad_not_required",
                "The selected Live Record Model does not require a separate VAD model.",
            ),
            None,
            None,
        )
    } else {
        build_model_path_policy_check(PathPolicyCheckArgs {
            id: "vad",
            title: text("settings.diagnostics.vad_title", "VAD Dependency"),
            selected_path: vad_path,
            path_status: input.path_statuses.vad.as_ref(),
            missing_selection_status: DiagnosticStatus::Missing,
            missing_selection_description: text(
                "settings.diagnostics.vad_missing",
                "The selected Live Record Model still needs a VAD model.",
            ),
            missing_path_status: DiagnosticStatus::Failed,
            unknown_path_status: DiagnosticStatus::Info,
            ready_description: text(
                "settings.diagnostics.vad_ready",
                "The required VAD model is configured and reachable.",
            ),
            action: Some(open_model_settings_action()),
            missing_path_meta: Some(literal_meta(config.vad_model_path.clone())),
            unknown_path_meta: Some(literal_meta(vad_path)),
            ready_meta: None,
        })
    };

    let punctuation_check = if !input.punctuation_required {
        check(
            "punctuation",
            text(
                "settings.diagnostics.punctuation_title",
                "Punctuation Dependency",
            ),
            DiagnosticStatus::Ready,
            text(
                "settings.diagnostics.punctuation_not_required",
                "The current recognition models do not require a separate punctuation model.",
            ),
            None,
            None,
        )
    } else {
        build_model_path_policy_check(PathPolicyCheckArgs {
            id: "punctuation",
            title: text(
                "settings.diagnostics.punctuation_title",
                "Punctuation Dependency",
            ),
            selected_path: punctuation_path,
            path_status: input.path_statuses.punctuation.as_ref(),
            missing_selection_status: DiagnosticStatus::Warning,
            missing_selection_description: text(
                "settings.diagnostics.punctuation_warning",
                "A selected recognition model expects a punctuation model, but none is available yet.",
            ),
            missing_path_status: DiagnosticStatus::Warning,
            unknown_path_status: DiagnosticStatus::Warning,
            ready_description: text(
                "settings.diagnostics.punctuation_ready",
                "The required punctuation model is configured and reachable.",
            ),
            action: Some(open_model_settings_action()),
            missing_path_meta: Some(literal_meta(punctuation_path)),
            unknown_path_meta: Some(literal_meta(punctuation_path)),
            ready_meta: None,
        })
    };

    ModelChecks {
        live_model_check,
        offline_model_check,
        vad_check,
        punctuation_check,
    }
}

struct PathPolicyCheckArgs<'a> {
    id: &'a str,
    title: TextSpec,
    selected_path: &'a str,
    path_status: Option<&'a RuntimePathStatus>,
    missing_selection_status: DiagnosticStatus,
    missing_selection_description: TextSpec,
    missing_path_status: DiagnosticStatus,
    unknown_path_status: DiagnosticStatus,
    ready_description: TextSpec,
    action: Option<DiagnosticActionSpec>,
    missing_path_meta: Option<TextSpec>,
    unknown_path_meta: Option<TextSpec>,
    ready_meta: Option<TextSpec>,
}

fn build_model_path_policy_check(args: PathPolicyCheckArgs<'_>) -> DiagnosticCheckSpec {
    if args.selected_path.is_empty() {
        return check(
            args.id,
            args.title,
            args.missing_selection_status,
            args.missing_selection_description,
            args.action,
            None,
        );
    }

    if matches!(
        args.path_status.map(|status| status.kind.as_str()),
        Some("missing")
    ) {
        return check(
            args.id,
            args.title,
            args.missing_path_status,
            text(
                "settings.diagnostics.model_path_missing",
                "The selected model path no longer exists on disk.",
            ),
            args.action,
            args.missing_path_meta,
        );
    }

    if matches!(
        args.path_status.map(|status| status.kind.as_str()),
        Some("unknown")
    ) {
        return check(
            args.id,
            args.title,
            args.unknown_path_status,
            path_unverified_text(),
            args.action,
            args.unknown_path_meta,
        );
    }

    check(
        args.id,
        args.title,
        DiagnosticStatus::Ready,
        args.ready_description,
        None,
        args.ready_meta,
    )
}

fn build_input_checks(input: &DiagnosticsCoreInput) -> InputChecks {
    let permission_check = {
        let (status, description, action) = match input.permission_state.as_str() {
            "granted" => (
                DiagnosticStatus::Ready,
                text(
                    "settings.diagnostics.permission_granted",
                    "Microphone access is already granted.",
                ),
                None,
            ),
            "denied" => (
                DiagnosticStatus::Failed,
                text(
                    "settings.diagnostics.permission_denied",
                    "Microphone access is denied, so the first live recording path cannot start.",
                ),
                Some(request_microphone_permission_action()),
            ),
            "unsupported" => (
                DiagnosticStatus::Failed,
                text(
                    "settings.diagnostics.permission_unsupported",
                    "This environment does not expose browser microphone permission controls.",
                ),
                None,
            ),
            _ => (
                DiagnosticStatus::Warning,
                text(
                    "settings.diagnostics.permission_prompt",
                    "Microphone access has not been granted yet.",
                ),
                Some(request_microphone_permission_action()),
            ),
        };
        check(
            "microphone-permission",
            text(
                "settings.diagnostics.permission_title",
                "Microphone Permission",
            ),
            status,
            description,
            action,
            None,
        )
    };

    let microphone_id = if input.config.microphone_id.trim().is_empty() {
        "default"
    } else {
        input.config.microphone_id.as_str()
    };
    let microphone_check = if !input.microphone_probe.available {
        check(
            "microphone-device",
            text("settings.diagnostics.microphone_title", "Input Device"),
            DiagnosticStatus::Failed,
            input
                .microphone_probe
                .error_message
                .as_ref()
                .map(|message| text("diagnostics.runtime_message", message))
                .unwrap_or_else(|| {
                    text(
                        "settings.diagnostics.microphone_unavailable",
                        "No microphone devices are currently available.",
                    )
                }),
            Some(open_input_device_action()),
            None,
        )
    } else if microphone_id == "default"
        || input
            .microphone_probe
            .options
            .iter()
            .any(|option| option.value == microphone_id)
    {
        check(
            "microphone-device",
            text("settings.diagnostics.microphone_title", "Input Device"),
            DiagnosticStatus::Ready,
            text(
                "settings.diagnostics.microphone_ready",
                "The current input-device selection is still available.",
            ),
            None,
            Some(if microphone_id == "default" {
                translated_meta("settings.mic_auto", "Auto")
            } else {
                literal_meta(microphone_id)
            }),
        )
    } else {
        check(
            "microphone-device",
            text("settings.diagnostics.microphone_title", "Input Device"),
            DiagnosticStatus::Failed,
            text(
                "settings.diagnostics.microphone_missing_selection",
                "The saved microphone selection is no longer available.",
            ),
            Some(open_input_device_action()),
            Some(literal_meta(microphone_id)),
        )
    };

    let system_audio_check = if input.system_audio_probe.available {
        check(
            "system-audio-capture",
            text(
                "settings.diagnostics.system_audio_title",
                "System Audio Capture",
            ),
            DiagnosticStatus::Ready,
            text(
                "settings.diagnostics.system_audio_ready",
                "System audio capture devices are available.",
            ),
            None,
            None,
        )
    } else {
        check(
            "system-audio-capture",
            text(
                "settings.diagnostics.system_audio_title",
                "System Audio Capture",
            ),
            DiagnosticStatus::Warning,
            input
                .system_audio_probe
                .error_message
                .as_ref()
                .map(|message| text("diagnostics.runtime_message", message))
                .unwrap_or_else(|| {
                    text(
                        "settings.diagnostics.system_audio_warning",
                        "Sona could not enumerate system audio capture devices right now.",
                    )
                }),
            Some(open_input_device_action()),
            None,
        )
    };

    InputChecks {
        permission_check,
        microphone_check,
        system_audio_check,
    }
}

fn build_runtime_checks(input: &DiagnosticsCoreInput) -> RuntimeChecks {
    let voice_typing_check = build_voice_typing_check(input);
    let ffmpeg_check = if input.runtime_environment.ffmpeg_exists {
        check(
            "ffmpeg",
            text("settings.diagnostics.ffmpeg_title", "FFmpeg Sidecar"),
            DiagnosticStatus::Ready,
            text(
                "settings.diagnostics.ffmpeg_ready",
                "The bundled FFmpeg sidecar is present.",
            ),
            None,
            Some(literal_meta(input.runtime_environment.ffmpeg_path.clone())),
        )
    } else {
        check(
            "ffmpeg",
            text("settings.diagnostics.ffmpeg_title", "FFmpeg Sidecar"),
            DiagnosticStatus::Failed,
            text(
                "settings.diagnostics.ffmpeg_missing",
                "The bundled FFmpeg sidecar could not be found. Batch imports and media decoding may fail until the app is reinstalled.",
            ),
            Some(open_log_folder_action()),
            Some(literal_meta(input.runtime_environment.ffmpeg_path.clone())),
        )
    };
    let log_dir_check = if input.runtime_environment.log_dir_path.trim().is_empty() {
        check(
            "log-dir",
            text("settings.diagnostics.log_dir_title", "Log Directory"),
            DiagnosticStatus::Failed,
            text(
                "settings.diagnostics.log_dir_missing",
                "Sona could not resolve the runtime log directory.",
            ),
            None,
            None,
        )
    } else {
        check(
            "log-dir",
            text("settings.diagnostics.log_dir_title", "Log Directory"),
            DiagnosticStatus::Ready,
            text(
                "settings.diagnostics.log_dir_ready",
                "Runtime logs can be resolved for troubleshooting.",
            ),
            Some(open_log_folder_action()),
            Some(literal_meta(input.runtime_environment.log_dir_path.clone())),
        )
    };

    RuntimeChecks {
        voice_typing_check,
        ffmpeg_check,
        log_dir_check,
    }
}

fn build_voice_typing_check(input: &DiagnosticsCoreInput) -> DiagnosticCheckSpec {
    let title = text(
        "settings.diagnostics.voice_typing_title",
        "Voice Typing Runtime",
    );
    match input.voice_typing_readiness.state.as_str() {
        "off" => check(
            "voice-typing",
            title,
            DiagnosticStatus::Info,
            text(
                "settings.diagnostics.voice_typing_off",
                "Voice Typing is currently turned off.",
            ),
            Some(open_voice_typing_action()),
            None,
        ),
        "needs_shortcut" | "needs_live_model" | "needs_vad" => check(
            "voice-typing",
            title,
            DiagnosticStatus::Missing,
            input
                .voice_typing_readiness
                .last_error_message
                .as_ref()
                .map(|message| text("diagnostics.runtime_message", message))
                .unwrap_or_else(|| {
                    let key = match input.voice_typing_readiness.state.as_str() {
                        "needs_shortcut" => "settings.voice_typing_status_summary_missing_shortcut",
                        "needs_vad" => "settings.voice_typing_status_summary_missing_vad",
                        _ => "settings.voice_typing_status_summary_missing_model",
                    };
                    text(key, "Voice Typing still needs setup before it can run.")
                }),
            Some(open_voice_typing_action()),
            None,
        ),
        "failed" => check(
            "voice-typing",
            title,
            DiagnosticStatus::Failed,
            input
                .voice_typing_readiness
                .last_error_message
                .as_ref()
                .map(|message| text("diagnostics.runtime_message", message))
                .unwrap_or_else(|| {
                    text(
                        "settings.voice_typing_status_summary_failed",
                        "Voice Typing hit a runtime problem.",
                    )
                }),
            Some(retry_voice_typing_warmup_action()),
            None,
        ),
        "preparing" => check(
            "voice-typing",
            title,
            DiagnosticStatus::Info,
            text(
                "settings.voice_typing_status_summary_preparing",
                "Voice Typing is getting ready in the background.",
            ),
            None,
            None,
        ),
        _ => check(
            "voice-typing",
            title,
            DiagnosticStatus::Ready,
            text(
                "settings.voice_typing_status_summary_ready",
                "Voice Typing is ready to dictate into other apps.",
            ),
            None,
            None,
        ),
    }
}

fn build_asr_performance_checks(input: &DiagnosticsCoreInput) -> AsrPerformanceChecks {
    let model_memory_check = if let Some(model_load) = input.asr_runtime_metrics.model_load.as_ref()
    {
        check(
            "asr-model-memory",
            text(
                "settings.diagnostics.asr_model_memory_title",
                "Model memory",
            ),
            DiagnosticStatus::Ready,
            text(
                "settings.diagnostics.asr_model_memory_ready",
                &format!(
                    "Last model load: {} ({}).",
                    model_load.model_type, model_load.recognizer_kind
                ),
            )
            .with_param("modelType", &model_load.model_type)
            .with_param("recognizerKind", &model_load.recognizer_kind),
            None,
            Some(literal_meta(
                [
                    format!("{} {}", model_load.model_type, model_load.recognizer_kind),
                    format!(
                        "RSS {}",
                        format_metric_mb(model_load.process_rss_mb.or(model_load.rss_after_mb))
                    ),
                    format!("delta {}", format_signed_metric_mb(model_load.rss_delta_mb)),
                    format!("load {}", format_metric_ms(Some(model_load.load_ms))),
                    if model_load.reused_from_pool {
                        "reused recognizer".to_string()
                    } else {
                        "new recognizer".to_string()
                    },
                    model_load.instance_id.clone(),
                ]
                .join(" · "),
            )),
        )
    } else {
        check(
            "asr-model-memory",
            text(
                "settings.diagnostics.asr_model_memory_title",
                "Model memory",
            ),
            DiagnosticStatus::Info,
            text(
                "settings.diagnostics.asr_model_memory_empty",
                "No ASR runtime metrics have been captured yet.",
            ),
            None,
            None,
        )
    };

    let live_latency_check =
        if let Some(live_inference) = input.asr_runtime_metrics.live_inference.as_ref() {
            let instance_id = live_inference
                .instance_id
                .as_deref()
                .unwrap_or("unknown instance");
            check(
                "asr-live-latency",
                text(
                    "settings.diagnostics.asr_live_latency_title",
                    "Live transcription latency",
                ),
                DiagnosticStatus::Ready,
                text(
                    "settings.diagnostics.asr_live_latency_ready",
                    &format!("Last live inference from {}.", instance_id),
                )
                .with_param("instanceId", instance_id),
                None,
                Some(literal_meta(describe_inference_metric(live_inference))),
            )
        } else {
            check(
                "asr-live-latency",
                text(
                    "settings.diagnostics.asr_live_latency_title",
                    "Live transcription latency",
                ),
                DiagnosticStatus::Info,
                text(
                    "settings.diagnostics.asr_live_latency_empty",
                    "No live transcription latency has been captured yet.",
                ),
                None,
                None,
            )
        };

    let batch_latency_check =
        if let Some(batch_inference) = input.asr_runtime_metrics.batch_inference.as_ref() {
            check(
                "asr-batch-latency",
                text(
                    "settings.diagnostics.asr_batch_latency_title",
                    "Batch transcription latency",
                ),
                DiagnosticStatus::Ready,
                text(
                    "settings.diagnostics.asr_batch_latency_ready",
                    "Last batch transcription run completed.",
                ),
                None,
                Some(literal_meta(describe_inference_metric(batch_inference))),
            )
        } else {
            check(
                "asr-batch-latency",
                text(
                    "settings.diagnostics.asr_batch_latency_title",
                    "Batch transcription latency",
                ),
                DiagnosticStatus::Info,
                text(
                    "settings.diagnostics.asr_batch_latency_empty",
                    "No batch transcription latency has been captured yet.",
                ),
                None,
                None,
            )
        };

    AsrPerformanceChecks {
        model_memory_check,
        live_latency_check,
        batch_latency_check,
    }
}

fn build_overview_cards(
    input: &DiagnosticsCoreInput,
    checks: &BuiltChecks,
) -> Vec<DiagnosticOverviewCardSpec> {
    vec![
        overview_card(
            "first-run",
            text("settings.diagnostics.first_run_card", "First Run Setup"),
            if input.onboarding_ready {
                text(
                    "settings.diagnostics.first_run_ready",
                    "Recommended local models are configured.",
                )
            } else {
                text(
                    "settings.diagnostics.first_run_missing",
                    "The recommended offline setup is still incomplete.",
                )
            },
            &[
                if input.onboarding_ready {
                    DiagnosticStatus::Ready
                } else {
                    DiagnosticStatus::Missing
                },
                if checks.input.permission_check.status == DiagnosticStatus::Failed {
                    DiagnosticStatus::Warning
                } else {
                    checks.input.permission_check.status.clone()
                },
            ],
            if input.onboarding_ready {
                None
            } else {
                Some(run_first_setup_action())
            },
        ),
        overview_card(
            "live-record",
            text("settings.diagnostics.live_record_card", "Live Record"),
            text(
                "settings.diagnostics.live_record_card_description",
                "Model, VAD, permission, and microphone selection for real-time capture.",
            ),
            &[
                checks.model.live_model_check.status.clone(),
                checks.model.vad_check.status.clone(),
                checks.input.permission_check.status.clone(),
                checks.input.microphone_check.status.clone(),
            ],
            live_record_overview_action(checks),
        ),
        overview_card(
            "batch-import",
            text("settings.diagnostics.batch_import_card", "Batch Import"),
            text(
                "settings.diagnostics.batch_import_card_description",
                "Offline model and bundled media decoding support for file processing.",
            ),
            &[
                checks.model.offline_model_check.status.clone(),
                checks.model.punctuation_check.status.clone(),
                checks.runtime.ffmpeg_check.status.clone(),
            ],
            batch_import_overview_action(input, checks),
        ),
        overview_card(
            "voice-typing",
            text("settings.diagnostics.voice_typing_card", "Voice Typing"),
            text(
                "settings.diagnostics.voice_typing_card_description",
                "Shortcut, live model reuse, and runtime warm-up for dictation.",
            ),
            &[checks.runtime.voice_typing_check.status.clone()],
            checks.runtime.voice_typing_check.action.clone(),
        ),
    ]
}

fn live_record_overview_action(checks: &BuiltChecks) -> Option<DiagnosticActionSpec> {
    if checks.model.live_model_check.status != DiagnosticStatus::Ready
        || checks.model.vad_check.status != DiagnosticStatus::Ready
    {
        return Some(open_model_settings_action());
    }
    if checks.input.permission_check.action.is_some() {
        return checks.input.permission_check.action.clone();
    }
    if checks.input.microphone_check.action.is_some() {
        return Some(open_input_device_action());
    }
    None
}

fn batch_import_overview_action(
    input: &DiagnosticsCoreInput,
    checks: &BuiltChecks,
) -> Option<DiagnosticActionSpec> {
    if checks.model.offline_model_check.status != DiagnosticStatus::Ready
        || checks.model.punctuation_check.status == DiagnosticStatus::Warning
    {
        return Some(open_model_settings_action());
    }
    if !input.runtime_environment.ffmpeg_exists {
        return Some(open_log_folder_action());
    }
    None
}

fn overview_card(
    id: &str,
    title: TextSpec,
    description: TextSpec,
    statuses: &[DiagnosticStatus],
    action: Option<DiagnosticActionSpec>,
) -> DiagnosticOverviewCardSpec {
    DiagnosticOverviewCardSpec {
        id: id.to_string(),
        title,
        description,
        status: pick_worse_status(statuses),
        action,
    }
}

fn pick_worse_status(statuses: &[DiagnosticStatus]) -> DiagnosticStatus {
    statuses
        .iter()
        .max_by_key(|status| status_priority(status))
        .cloned()
        .unwrap_or(DiagnosticStatus::Ready)
}

fn status_priority(status: &DiagnosticStatus) -> u8 {
    match status {
        DiagnosticStatus::Failed => 4,
        DiagnosticStatus::Missing => 3,
        DiagnosticStatus::Warning => 2,
        DiagnosticStatus::Info => 1,
        DiagnosticStatus::Ready => 0,
    }
}

fn check(
    id: &str,
    title: TextSpec,
    status: DiagnosticStatus,
    description: TextSpec,
    action: Option<DiagnosticActionSpec>,
    meta: Option<TextSpec>,
) -> DiagnosticCheckSpec {
    DiagnosticCheckSpec {
        id: id.to_string(),
        title,
        description,
        status,
        action,
        meta,
    }
}

fn text(key: &str, default_value: &str) -> TextSpec {
    TextSpec::new(key, default_value)
}

fn literal_meta(value: impl Into<String>) -> TextSpec {
    TextSpec {
        key: "diagnostics.literal_meta".to_string(),
        default_value: value.into(),
        params: HashMap::new(),
    }
}

fn translated_meta(key: &str, default_value: &str) -> TextSpec {
    text(key, default_value)
}

fn path_unverified_text() -> TextSpec {
    text(
        "settings.diagnostics.model_path_unverified",
        "Sona could not verify the selected path from the current runtime. The current configuration is being kept as-is.",
    )
}

fn open_settings_action(label: TextSpec, settings_tab: &str) -> DiagnosticActionSpec {
    DiagnosticActionSpec::OpenSettings {
        label,
        settings_tab: settings_tab.to_string(),
    }
}

fn open_model_settings_action() -> DiagnosticActionSpec {
    open_settings_action(
        text(
            "settings.diagnostics.open_model_settings",
            "Open Model Settings",
        ),
        "models",
    )
}

fn open_input_device_action() -> DiagnosticActionSpec {
    open_settings_action(
        text(
            "settings.diagnostics.open_input_device",
            "Open Input Device",
        ),
        "microphone",
    )
}

fn open_voice_typing_action() -> DiagnosticActionSpec {
    open_settings_action(
        text(
            "settings.diagnostics.open_voice_typing",
            "Open Voice Typing",
        ),
        "voice_typing",
    )
}

fn request_microphone_permission_action() -> DiagnosticActionSpec {
    DiagnosticActionSpec::RequestMicrophonePermission {
        label: text(
            "settings.diagnostics.request_permission",
            "Request Permission",
        ),
    }
}

fn retry_voice_typing_warmup_action() -> DiagnosticActionSpec {
    DiagnosticActionSpec::RetryVoiceTypingWarmup {
        label: text("settings.diagnostics.retry_warmup", "Retry Warm-up"),
    }
}

fn run_first_setup_action() -> DiagnosticActionSpec {
    DiagnosticActionSpec::RunFirstRunSetup {
        label: text(
            "settings.diagnostics.run_first_setup",
            "Run First Run Setup",
        ),
    }
}

fn open_log_folder_action() -> DiagnosticActionSpec {
    DiagnosticActionSpec::OpenLogFolder {
        label: text("settings.about_open_logs", "Open Log Folder"),
    }
}

fn format_metric_ms(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{} ms", value.round() as i64))
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_metric_mb(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{:.1} MB", value))
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_signed_metric_mb(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| {
            if value >= 0.0 {
                format!("+{:.1} MB", value)
            } else {
                format!("{:.1} MB", value)
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_rtf(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{:.2}", value))
        .unwrap_or_else(|| "unknown".to_string())
}

fn describe_inference_metric(metric: &AsrInferenceMetric) -> String {
    let mut parts = vec![
        format!("stage {}", metric.stage),
        format!("decode {}", format_metric_ms(Some(metric.decode_ms))),
    ];

    if metric.audio_extract_ms.is_some() {
        parts.push(format!(
            "extract {}",
            format_metric_ms(metric.audio_extract_ms)
        ));
    }
    if metric.emit_latency_ms.is_some() {
        parts.push(format!(
            "latency {}",
            format_metric_ms(metric.emit_latency_ms)
        ));
    }
    if metric.total_ms.is_some() {
        parts.push(format!("total {}", format_metric_ms(metric.total_ms)));
    }
    parts.push(format!("RTF {}", format_rtf(metric.rtf)));
    parts.push(format!("RSS {}", format_metric_mb(metric.process_rss_mb)));
    if let Some(segment_count) = metric.segment_count {
        parts.push(format!("segments {}", segment_count));
    }

    parts.join(" · ")
}

fn now_iso_like() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn default_microphone_id() -> String {
    "default".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sherpa::AsrModelLoadMetric;

    fn base_input() -> DiagnosticsCoreInput {
        DiagnosticsCoreInput {
            config: DiagnosticsConfigInput {
                streaming_model_path: "C:\\models\\live".to_string(),
                offline_model_path: "C:\\models\\offline".to_string(),
                vad_model_path: "C:\\models\\vad.onnx".to_string(),
                punctuation_model_path: "".to_string(),
                microphone_id: "default".to_string(),
            },
            selected_models: SelectedModelsInput {
                live: Some(ModelSummaryInput {
                    id: "live".to_string(),
                    name: "Live Model".to_string(),
                }),
                offline: Some(ModelSummaryInput {
                    id: "offline".to_string(),
                    name: "Offline Model".to_string(),
                }),
            },
            model_rules: ModelRulesInput {
                live: Some(ModelRuleInput {
                    requires_vad: true,
                    requires_punctuation: false,
                }),
                offline: Some(ModelRuleInput {
                    requires_vad: false,
                    requires_punctuation: false,
                }),
            },
            path_statuses: PathStatusesInput {
                live_model: Some(path_status("C:\\models\\live", "directory")),
                offline_model: Some(path_status("C:\\models\\offline", "directory")),
                vad: Some(path_status("C:\\models\\vad.onnx", "file")),
                punctuation: None,
            },
            permission_state: "granted".to_string(),
            microphone_probe: DeviceProbeInput {
                options: vec![DeviceOptionInput {
                    label: "Auto".to_string(),
                    value: "default".to_string(),
                }],
                available: true,
                error_message: None,
            },
            system_audio_probe: DeviceProbeInput {
                options: vec![],
                available: true,
                error_message: None,
            },
            voice_typing_readiness: VoiceTypingReadinessInput {
                state: "ready".to_string(),
                last_error_message: None,
            },
            runtime_environment: RuntimeEnvironmentStatus {
                ffmpeg_path: "C:\\app\\ffmpeg.exe".to_string(),
                ffmpeg_exists: true,
                log_dir_path: "C:\\app\\logs".to_string(),
            },
            asr_runtime_metrics: AsrRuntimeMetricsSnapshot::default(),
            onboarding_ready: true,
            punctuation_required: false,
        }
    }

    fn path_status(path: &str, kind: &str) -> RuntimePathStatus {
        RuntimePathStatus {
            path: path.to_string(),
            kind: kind.to_string(),
            error: None,
        }
    }

    fn find_check<'a>(
        snapshot: &'a DiagnosticsCoreSnapshot,
        section_id: &str,
        check_id: &str,
    ) -> &'a DiagnosticCheckSpec {
        snapshot
            .sections
            .iter()
            .find(|section| section.id == section_id)
            .and_then(|section| section.checks.iter().find(|check| check.id == check_id))
            .expect("diagnostic check should exist")
    }

    fn assert_meta_text(check: &DiagnosticCheckSpec, expected_key: &str, expected_default: &str) {
        let meta = check.meta.as_ref().expect("check should include meta text");
        assert_eq!(meta.key, expected_key);
        assert_eq!(meta.default_value, expected_default);
    }

    #[test]
    fn check_meta_uses_text_specs_for_translated_and_literal_values() {
        let snapshot = build_diagnostics_core_snapshot(base_input());
        let microphone = find_check(&snapshot, "input-capture", "microphone-device");
        let ffmpeg = find_check(&snapshot, "runtime-environment", "ffmpeg");

        assert_meta_text(microphone, "settings.mic_auto", "Auto");
        assert_meta_text(ffmpeg, "diagnostics.literal_meta", "C:\\app\\ffmpeg.exe");
    }

    #[test]
    fn path_policy_keeps_unknown_models_non_blocking_and_punctuation_warning() {
        let mut input = base_input();
        input.path_statuses.live_model = Some(path_status("C:\\models\\live", "unknown"));
        input.path_statuses.punctuation = Some(path_status("C:\\models\\punct.onnx", "unknown"));
        input.config.punctuation_model_path = "C:\\models\\punct.onnx".to_string();
        input.punctuation_required = true;

        let snapshot = build_diagnostics_core_snapshot(input);
        let live = find_check(&snapshot, "models", "live-model");
        let punctuation = find_check(&snapshot, "models", "punctuation");

        assert_eq!(live.status, DiagnosticStatus::Info);
        assert_eq!(
            live.description.key,
            "settings.diagnostics.model_path_unverified"
        );
        assert_eq!(punctuation.status, DiagnosticStatus::Warning);
        assert_eq!(
            punctuation.description.key,
            "settings.diagnostics.model_path_unverified"
        );
    }

    #[test]
    fn overview_action_prefers_model_repairs_before_permission_or_ffmpeg() {
        let mut input = base_input();
        input.config.streaming_model_path = "".to_string();
        input.permission_state = "denied".to_string();
        input.runtime_environment.ffmpeg_exists = false;
        input.punctuation_required = true;

        let snapshot = build_diagnostics_core_snapshot(input);
        let live = snapshot
            .overview
            .iter()
            .find(|card| card.id == "live-record")
            .expect("live overview should exist");
        let batch = snapshot
            .overview
            .iter()
            .find(|card| card.id == "batch-import")
            .expect("batch overview should exist");

        assert_eq!(live.action, Some(open_model_settings_action()));
        assert_eq!(batch.action, Some(open_model_settings_action()));
    }

    #[test]
    fn asr_metrics_are_info_when_empty_and_ready_when_samples_exist() {
        let empty_snapshot = build_diagnostics_core_snapshot(base_input());
        assert_eq!(
            find_check(&empty_snapshot, "asr-performance", "asr-model-memory").status,
            DiagnosticStatus::Info
        );

        let mut input = base_input();
        input.asr_runtime_metrics = AsrRuntimeMetricsSnapshot {
            model_load: Some(AsrModelLoadMetric {
                occurred_at_ms: 1,
                instance_id: "record".to_string(),
                model_path: "C:\\models\\live".to_string(),
                model_type: "sensevoice".to_string(),
                recognizer_kind: "offline".to_string(),
                num_threads: 4,
                reused_from_pool: false,
                load_ms: 123.4,
                rss_before_mb: Some(416.25),
                rss_after_mb: Some(512.5),
                rss_delta_mb: Some(96.25),
                process_rss_mb: Some(512.5),
            }),
            live_inference: Some(AsrInferenceMetric {
                occurred_at_ms: 2,
                source: "live".to_string(),
                instance_id: Some("record".to_string()),
                stage: "final".to_string(),
                is_final: true,
                audio_duration_ms: 1600.0,
                buffered_samples: 25_600,
                audio_extract_ms: None,
                decode_ms: 42.2,
                emit_latency_ms: Some(60.1),
                total_ms: None,
                rtf: Some(0.026),
                segment_count: None,
                process_rss_mb: Some(520.1),
            }),
            batch_inference: None,
        };

        let snapshot = build_diagnostics_core_snapshot(input);
        let model = find_check(&snapshot, "asr-performance", "asr-model-memory");
        let live = find_check(&snapshot, "asr-performance", "asr-live-latency");

        assert_eq!(model.status, DiagnosticStatus::Ready);
        assert!(model
            .meta
            .as_ref()
            .map(|meta| meta.default_value.as_str())
            .unwrap_or_default()
            .contains("RSS 512.5 MB"));
        assert_eq!(live.status, DiagnosticStatus::Ready);
        assert!(live
            .meta
            .as_ref()
            .map(|meta| meta.default_value.as_str())
            .unwrap_or_default()
            .contains("latency 60 ms"));
    }
}
