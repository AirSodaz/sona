use std::sync::Arc;

use sona_core::models::preset_models::{
    ModelCatalogModel, ModelRules, ModelSelectionPaths, resolve_model_catalog_selected_ids,
};
use sona_core::runtime::diagnostics::{
    DiagnosticsCoreInput, DiagnosticsCoreSnapshot, DiagnosticsEnrichmentRepository,
    DiagnosticsError, ModelRuleInput, ModelRulesInput, ModelSummaryInput, SelectedModelsInput,
    build_diagnostics_core_snapshot_at,
};

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
    catalog: &'a sona_core::models::preset_models::ModelCatalogSnapshot,
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
