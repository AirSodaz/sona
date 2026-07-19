use crate::integrations::asr::AsrState;
use crate::platform::paths::{PathKind, PathProvider};
use tauri::State;

pub use sona_core::runtime::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    DiagnosticsCoreSnapshot, ModelRuleInput, ModelRulesInput, ModelSummaryInput, PathStatusesInput,
    RuntimeEnvironmentStatus, RuntimePathStatus, SelectedModelsInput, VoiceTypingReadinessInput,
};
use sona_runtime_fs::build_diagnostics_snapshot;

fn validate_diagnostics_input(input: &DiagnosticsCoreInput) -> Result<(), String> {
    sona_ts_bind::validate_diagnostics_input_for_typescript(input)
        .map_err(|error| error.to_string())
}

fn validate_diagnostics_snapshot(snapshot: &DiagnosticsCoreSnapshot) -> Result<(), String> {
    sona_ts_bind::validate_diagnostics_snapshot_for_typescript(snapshot)
        .map_err(|error| error.to_string())
}

pub async fn get_diagnostics_core_snapshot(
    provider: &dyn PathProvider,
    state: State<'_, AsrState>,
    mut input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    validate_diagnostics_input(&input)?;
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?
        .join("models");
    let log_dir = provider
        .resolve_path(PathKind::AppLogData)
        .map_err(|error| error.to_string())?;
    input.asr_runtime_metrics = state.metrics_snapshot().await;

    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        input.runtime_environment =
            crate::platform::runtime_status::resolve_runtime_environment_status_for_log_dir(
                log_dir,
            )?;
        build_diagnostics_snapshot(models_dir, input).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    validate_diagnostics_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub async fn get_diagnostics_core_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: State<'_, AsrState>,
    input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    get_diagnostics_core_snapshot(&provider, state, input).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_with_unsafe_metric() -> DiagnosticsCoreInput {
        let mut input: DiagnosticsCoreInput = serde_json::from_value(serde_json::json!({
            "config": {
                "streamingModelPath": "",
                "batchModelPath": ""
            },
            "permissionState": "prompt",
            "microphoneProbe": {
                "options": [],
                "available": false,
                "errorMessage": null
            },
            "systemAudioProbe": {
                "options": [],
                "available": false,
                "errorMessage": null
            },
            "voiceTypingReadiness": {
                "state": "off",
                "lastErrorMessage": null
            }
        }))
        .unwrap();
        input.asr_runtime_metrics.model_load =
            Some(sona_core::transcription::asr_metrics::AsrModelLoadMetric {
                occurred_at_ms: sona_ts_bind::TYPESCRIPT_MAX_SAFE_INTEGER + 1,
                instance_id: "instance-1".to_string(),
                model_path: "model.onnx".to_string(),
                model_type: "streaming".to_string(),
                recognizer_kind: "online".to_string(),
                num_threads: 4,
                reused_from_pool: false,
                load_ms: 1.0,
                rss_before_mb: None,
                rss_after_mb: None,
                rss_delta_mb: None,
                process_rss_mb: None,
            });
        input
    }

    #[test]
    fn diagnostics_tauri_transport_rejects_unsafe_metrics_in_both_directions() {
        let input = input_with_unsafe_metric();
        let input_error = validate_diagnostics_input(&input).unwrap_err();
        assert!(input_error.contains("occurredAtMs"), "{input_error}");

        let snapshot = sona_core::runtime::diagnostics::build_diagnostics_core_snapshot_at(
            input,
            "2026-07-17T00:00:00Z".to_string(),
        );
        let output_error = validate_diagnostics_snapshot(&snapshot).unwrap_err();
        assert!(output_error.contains("occurredAtMs"), "{output_error}");
    }
}
