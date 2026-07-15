use crate::integrations::asr::AsrState;
use crate::platform::paths::{PathKind, PathProvider};
use tauri::State;

pub use sona_core::runtime::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    DiagnosticsCoreSnapshot, ModelRuleInput, ModelRulesInput, ModelSummaryInput, PathStatusesInput,
    RuntimeEnvironmentStatus, RuntimePathStatus, SelectedModelsInput, VoiceTypingReadinessInput,
};
use sona_runtime_fs::build_diagnostics_snapshot;

pub async fn get_diagnostics_core_snapshot(
    provider: &dyn PathProvider,
    state: State<'_, AsrState>,
    mut input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?
        .join("models");
    let log_dir = provider
        .resolve_path(PathKind::AppLogData)
        .map_err(|error| error.to_string())?;
    input.asr_runtime_metrics = state.metrics_snapshot().await;

    tauri::async_runtime::spawn_blocking(move || {
        input.runtime_environment =
            crate::platform::runtime_status::resolve_runtime_environment_status_for_log_dir(
                log_dir,
            )?;
        build_diagnostics_snapshot(models_dir, input).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn get_diagnostics_core_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: State<'_, AsrState>,
    input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    get_diagnostics_core_snapshot(&provider, state, input).await
}
