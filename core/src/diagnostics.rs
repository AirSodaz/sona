use serde::{Deserialize, Serialize};

use crate::asr_metrics::AsrRuntimeMetricsSnapshot;
pub use crate::runtime::{RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus};

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

fn now_iso_like() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn default_microphone_id() -> String {
    "default".to_string()
}
