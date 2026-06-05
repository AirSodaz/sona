use crate::core::preset_models::{
    ModelCatalogModel, ModelRules as PresetModelRules, ModelSelectionPaths,
    build_model_catalog_snapshot, resolve_model_catalog_selected_ids,
};
use crate::integrations::asr::{AsrRuntimeMetricsSnapshot, AsrState};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, State};

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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsCoreSnapshot {
    pub scanned_at: String,
    pub config: DiagnosticsConfigInput,
    pub selected_models: SelectedModelsInput,
    pub model_rules: ModelRulesInput,
    pub path_statuses: PathStatusesInput,
    pub permission_state: String,
    pub microphone_probe: DeviceProbeInput,
    pub system_audio_probe: DeviceProbeInput,
    pub voice_typing_readiness: VoiceTypingReadinessInput,
    pub runtime_environment: RuntimeEnvironmentStatus,
    pub asr_runtime_metrics: AsrRuntimeMetricsSnapshot,
    pub onboarding_ready: bool,
    pub punctuation_required: bool,
}

pub async fn get_diagnostics_core_snapshot<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AsrState>,
    input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let input = enrich_diagnostics_core_input(&app, state.inner(), input).await?;
    Ok(build_diagnostics_core_snapshot(input))
}

async fn enrich_diagnostics_core_input<R: Runtime>(
    app: &AppHandle<R>,
    state: &AsrState,
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
    input.runtime_environment = runtime_environment_input(
        crate::app::runtime_status::resolve_runtime_environment_status(app)?,
    );
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
    DiagnosticsCoreSnapshot {
        scanned_at: now_iso_like(),
        config: input.config,
        selected_models: input.selected_models,
        model_rules: input.model_rules,
        path_statuses: input.path_statuses,
        permission_state: input.permission_state,
        microphone_probe: input.microphone_probe,
        system_audio_probe: input.system_audio_probe,
        voice_typing_readiness: input.voice_typing_readiness,
        runtime_environment: input.runtime_environment,
        asr_runtime_metrics: input.asr_runtime_metrics,
        onboarding_ready: input.onboarding_ready,
        punctuation_required: input.punctuation_required,
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
        crate::app::runtime_status::resolve_runtime_path_status(path),
    ))
}

fn runtime_path_status_input(
    status: crate::app::runtime_status::RuntimePathStatus,
) -> RuntimePathStatus {
    RuntimePathStatus {
        path: status.path,
        kind: match status.kind {
            crate::app::runtime_status::RuntimePathKind::File => "file",
            crate::app::runtime_status::RuntimePathKind::Directory => "directory",
            crate::app::runtime_status::RuntimePathKind::Missing => "missing",
            crate::app::runtime_status::RuntimePathKind::Unknown => "unknown",
        }
        .to_string(),
        error: status.error,
    }
}

fn runtime_environment_input(
    status: crate::app::runtime_status::RuntimeEnvironmentStatus,
) -> RuntimeEnvironmentStatus {
    RuntimeEnvironmentStatus {
        ffmpeg_path: status.ffmpeg_path,
        ffmpeg_exists: status.ffmpeg_exists,
        log_dir_path: status.log_dir_path,
    }
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
    use crate::integrations::asr::{AsrInferenceMetric, AsrModelLoadMetric};

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

    #[test]
    fn core_snapshot_serializes_fact_fields_without_ui_spec() {
        let value = serde_json::to_value(build_diagnostics_core_snapshot(base_input())).unwrap();

        assert!(value.get("overview").is_none());
        assert!(value.get("sections").is_none());
        assert!(value.get("config").is_some());
        assert!(value.get("selectedModels").is_some());
        assert!(value.get("pathStatuses").is_some());
        assert!(value.to_string().find("settingsTab").is_none());
        assert!(
            value
                .to_string()
                .find("settings.diagnostics.open_model_settings")
                .is_none()
        );
    }

    #[test]
    fn core_snapshot_preserves_fact_fields_for_frontend_ui_builder() {
        let mut input = base_input();
        input.permission_state = "prompt".to_string();
        input.runtime_environment.ffmpeg_exists = false;
        input.path_statuses.live_model = Some(path_status("C:\\models\\live", "unknown"));
        input.path_statuses.punctuation = Some(path_status("C:\\models\\punct.onnx", "unknown"));
        input.config.punctuation_model_path = "C:\\models\\punct.onnx".to_string();
        input.punctuation_required = true;

        let snapshot = build_diagnostics_core_snapshot(input);

        assert_eq!(snapshot.permission_state, "prompt");
        assert!(!snapshot.runtime_environment.ffmpeg_exists);
        assert!(snapshot.punctuation_required);
        assert_eq!(
            snapshot.path_statuses.live_model.as_ref().unwrap().kind,
            "unknown"
        );
        assert_eq!(
            snapshot.path_statuses.punctuation.as_ref().unwrap().path,
            "C:\\models\\punct.onnx"
        );
        assert_eq!(
            snapshot.selected_models.live.as_ref().unwrap().name,
            "Live Model"
        );
    }

    #[test]
    fn core_snapshot_carries_asr_metrics_without_formatting() {
        let mut input = base_input();
        let metrics = AsrRuntimeMetricsSnapshot {
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
        input.asr_runtime_metrics = metrics.clone();

        let snapshot = build_diagnostics_core_snapshot(input);

        assert_eq!(snapshot.asr_runtime_metrics, metrics);
        assert_eq!(
            snapshot
                .asr_runtime_metrics
                .model_load
                .as_ref()
                .map(|metric| metric.process_rss_mb)
                .unwrap(),
            Some(512.5)
        );
        assert_eq!(
            snapshot
                .asr_runtime_metrics
                .live_inference
                .as_ref()
                .and_then(|metric| metric.emit_latency_ms),
            Some(60.1)
        );
    }
}
