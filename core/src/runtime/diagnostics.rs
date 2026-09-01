use crate::models::preset_models::ModelCatalogSnapshot;
pub use crate::runtime::environment::{
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus,
};
use crate::transcription::asr_metrics::AsrRuntimeMetricsSnapshot;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
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
    pub live_transcription: LiveTranscriptionDiagnosticsSnapshot,
    #[serde(default)]
    pub onboarding_ready: bool,
    #[serde(default)]
    pub punctuation_required: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LiveTranscriptionDiagnosticsSnapshot {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub active_sources: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub active_pipelines: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub active_consumers: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub shared_pipelines: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub avoided_feed_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsConfigInput {
    pub streaming_model_path: String,
    pub batch_model_path: String,
    #[serde(default)]
    pub vad_model_path: String,
    #[serde(default)]
    pub punctuation_model_path: String,
    #[serde(default = "default_microphone_id")]
    pub microphone_id: String,
    #[serde(default)]
    pub ffmpeg_path: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SelectedModelsInput {
    pub live: Option<ModelSummaryInput>,
    pub batch: Option<ModelSummaryInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ModelSummaryInput {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ModelRulesInput {
    pub live: Option<ModelRuleInput>,
    pub batch: Option<ModelRuleInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ModelRuleInput {
    pub requires_vad: bool,
    pub requires_punctuation: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct PathStatusesInput {
    pub live_model: Option<RuntimePathStatus>,
    pub batch_model: Option<RuntimePathStatus>,
    pub vad: Option<RuntimePathStatus>,
    pub punctuation: Option<RuntimePathStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct DeviceProbeInput {
    pub options: Vec<DeviceOptionInput>,
    pub available: bool,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub struct DeviceOptionInput {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct VoiceTypingReadinessInput {
    pub state: String,
    pub last_error_message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
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
    pub live_transcription: LiveTranscriptionDiagnosticsSnapshot,
    pub onboarding_ready: bool,
    pub punctuation_required: bool,
}

#[derive(Debug)]
pub struct DiagnosticsEnrichmentMeasurements {
    pub model_catalog: ModelCatalogSnapshot,
    pub path_statuses: PathStatusesInput,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DiagnosticsError {
    #[error("Diagnostics repository error: {0}")]
    Repository(String),
}

pub trait DiagnosticsEnrichmentRepository: Send + Sync {
    fn collect_measurements(
        &self,
        config: &DiagnosticsConfigInput,
    ) -> Result<DiagnosticsEnrichmentMeasurements, DiagnosticsError>;
}

pub fn build_diagnostics_core_snapshot_at(
    input: DiagnosticsCoreInput,
    scanned_at: String,
) -> DiagnosticsCoreSnapshot {
    DiagnosticsCoreSnapshot {
        scanned_at,
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
        live_transcription: input.live_transcription,
        onboarding_ready: input.onboarding_ready,
        punctuation_required: input.punctuation_required,
    }
}

fn default_microphone_id() -> String {
    "default".to_string()
}
