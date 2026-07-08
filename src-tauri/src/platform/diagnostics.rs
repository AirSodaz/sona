use crate::integrations::asr::AsrState;
use crate::platform::paths::{PathKind, PathProvider};
use crate::platform::preset_models::{
    ModelCatalogModel, ModelRules as PresetModelRules, ModelSelectionPaths,
    build_model_catalog_snapshot, resolve_model_catalog_selected_ids,
};

use tauri::State;

pub use sona_core::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    DiagnosticsCoreSnapshot, ModelRuleInput, ModelRulesInput, ModelSummaryInput, PathStatusesInput,
    RuntimeEnvironmentStatus, RuntimePathStatus, SelectedModelsInput, VoiceTypingReadinessInput,
    build_diagnostics_core_snapshot_at,
};

pub async fn get_diagnostics_core_snapshot(
    provider: &dyn PathProvider,
    state: State<'_, AsrState>,
    input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let input = enrich_diagnostics_core_input(provider, state.inner(), input).await?;
    let scanned_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    Ok(build_diagnostics_core_snapshot_at(input, scanned_at))
}

async fn enrich_diagnostics_core_input(
    provider: &dyn PathProvider,
    state: &AsrState,
    mut input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreInput, String> {
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
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
            batch_model_path: input.config.batch_model_path.clone(),
            speaker_segmentation_model_path: String::new(),
            speaker_embedding_model_path: String::new(),
        },
    );

    let live_model = selected
        .streaming
        .as_deref()
        .and_then(|model_id| catalog.models.iter().find(|model| model.id == model_id));
    let batch_model = selected
        .batch
        .as_deref()
        .and_then(|model_id| catalog.models.iter().find(|model| model.id == model_id));

    input.selected_models = SelectedModelsInput {
        live: live_model.map(model_summary_input),
        batch: batch_model.map(model_summary_input),
    };
    input.model_rules = ModelRulesInput {
        live: live_model.map(|model| model_rule_input(model.rules)),
        batch: batch_model.map(|model| model_rule_input(model.rules)),
    };
    input.path_statuses = PathStatusesInput {
        live_model: resolve_core_path_status(input.config.streaming_model_path.trim()),
        batch_model: resolve_core_path_status(input.config.batch_model_path.trim()),
        vad: resolve_core_path_status(input.config.vad_model_path.trim()),
        punctuation: resolve_core_path_status(input.config.punctuation_model_path.trim()),
    };
    input.runtime_environment = runtime_environment_input(
        crate::app::runtime_status::resolve_runtime_environment_status(provider)?,
    );
    input.asr_runtime_metrics = state.metrics_snapshot().await;
    input.onboarding_ready = !input.config.streaming_model_path.trim().is_empty()
        && !input.config.batch_model_path.trim().is_empty();
    input.punctuation_required = [
        input.model_rules.live.as_ref(),
        input.model_rules.batch.as_ref(),
    ]
    .iter()
    .any(|rules| rules.map(|item| item.requires_punctuation).unwrap_or(false));

    Ok(input)
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
    status
}

fn runtime_environment_input(
    status: crate::app::runtime_status::RuntimeEnvironmentStatus,
) -> RuntimeEnvironmentStatus {
    status
}
