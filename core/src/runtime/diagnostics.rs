use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::sync::Arc;

use crate::models::preset_models::{
    ModelCatalogModel, ModelCatalogSnapshot, ModelRules, ModelSelectionPaths,
    resolve_model_catalog_selected_ids,
};
pub use crate::runtime::environment::{
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus,
};
use crate::transcription::asr_metrics::AsrRuntimeMetricsSnapshot;

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
    pub onboarding_ready: bool,
    #[serde(default)]
    pub punctuation_required: bool,
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

pub struct DiagnosticsService<R>
where
    R: DiagnosticsEnrichmentRepository,
{
    repository: Arc<R>,
}

impl<R> DiagnosticsService<R>
where
    R: DiagnosticsEnrichmentRepository,
{
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }

    pub fn build_snapshot_at(
        &self,
        mut input: DiagnosticsCoreInput,
        scanned_at: String,
    ) -> Result<DiagnosticsCoreSnapshot, DiagnosticsError> {
        let measurements = self.repository.collect_measurements(&input.config)?;
        let selected = resolve_model_catalog_selected_ids(
            &measurements.model_catalog,
            &ModelSelectionPaths {
                streaming_model_path: input.config.streaming_model_path.clone(),
                batch_model_path: input.config.batch_model_path.clone(),
                speaker_segmentation_model_path: String::new(),
                speaker_embedding_model_path: String::new(),
            },
        );
        let live_model = selected
            .streaming
            .as_deref()
            .and_then(|model_id| find_catalog_model(&measurements.model_catalog, model_id));
        let batch_model = selected
            .batch
            .as_deref()
            .and_then(|model_id| find_catalog_model(&measurements.model_catalog, model_id));

        input.selected_models = SelectedModelsInput {
            live: live_model.map(model_summary_input),
            batch: batch_model.map(model_summary_input),
        };
        input.model_rules = ModelRulesInput {
            live: live_model.map(|model| model_rule_input(model.rules)),
            batch: batch_model.map(|model| model_rule_input(model.rules)),
        };
        input.path_statuses = measurements.path_statuses;
        input.onboarding_ready = !input.config.streaming_model_path.trim().is_empty()
            && !input.config.batch_model_path.trim().is_empty();
        input.punctuation_required = [
            input.model_rules.live.as_ref(),
            input.model_rules.batch.as_ref(),
        ]
        .into_iter()
        .any(|rules| rules.is_some_and(|item| item.requires_punctuation));

        Ok(build_diagnostics_core_snapshot_at(input, scanned_at))
    }
}

fn find_catalog_model<'a>(
    catalog: &'a ModelCatalogSnapshot,
    model_id: &str,
) -> Option<&'a ModelCatalogModel> {
    catalog.models.iter().find(|model| model.id == model_id)
}

fn model_summary_input(model: &ModelCatalogModel) -> ModelSummaryInput {
    ModelSummaryInput {
        id: model.id.clone(),
        name: model.name.clone(),
    }
}

fn model_rule_input(rules: ModelRules) -> ModelRuleInput {
    ModelRuleInput {
        requires_vad: rules.requires_vad,
        requires_punctuation: rules.requires_punctuation,
    }
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
        onboarding_ready: input.onboarding_ready,
        punctuation_required: input.punctuation_required,
    }
}

fn default_microphone_id() -> String {
    "default".to_string()
}
