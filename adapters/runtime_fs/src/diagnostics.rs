use sona_core::runtime::diagnostics::{
    DiagnosticsConfigInput, DiagnosticsCoreInput, DiagnosticsCoreSnapshot,
    DiagnosticsEnrichmentMeasurements, DiagnosticsEnrichmentRepository, DiagnosticsError,
    DiagnosticsService, PathStatusesInput, RuntimePathStatus,
};
use std::path::PathBuf;
use std::sync::Arc;

pub struct FsDiagnosticsEnrichmentRepository {
    models_dir: PathBuf,
}

impl FsDiagnosticsEnrichmentRepository {
    pub fn new(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }
}

impl DiagnosticsEnrichmentRepository for FsDiagnosticsEnrichmentRepository {
    fn collect_measurements(
        &self,
        config: &DiagnosticsConfigInput,
    ) -> Result<DiagnosticsEnrichmentMeasurements, DiagnosticsError> {
        crate::ensure_directory_exists(&self.models_dir).map_err(DiagnosticsError::Repository)?;

        Ok(DiagnosticsEnrichmentMeasurements {
            model_catalog: crate::build_model_catalog_snapshot(&self.models_dir),
            path_statuses: PathStatusesInput {
                live_model: resolve_optional_path(&config.streaming_model_path),
                batch_model: resolve_optional_path(&config.batch_model_path),
                vad: resolve_optional_path(&config.vad_model_path),
                punctuation: resolve_optional_path(&config.punctuation_model_path),
            },
        })
    }
}

pub fn build_diagnostics_snapshot(
    models_dir: PathBuf,
    input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, DiagnosticsError> {
    let repository = FsDiagnosticsEnrichmentRepository::new(models_dir);
    DiagnosticsService::new(Arc::new(repository))
        .build_snapshot_at(input, crate::diagnostics_scanned_at_now())
}

fn resolve_optional_path(path: &str) -> Option<RuntimePathStatus> {
    let path = path.trim();
    (!path.is_empty()).then(|| crate::resolve_runtime_path_status(path))
}
