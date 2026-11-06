use crate::integrations::asr::AsrState;
use crate::platform::paths::{PathKind, PathPort};
use sona_application::diagnostics::DiagnosticsService;

pub use sona_core::runtime::diagnostics::{
    DeviceOptionInput, DeviceProbeInput, DiagnosticsConfigInput, DiagnosticsCoreInput,
    DiagnosticsCoreSnapshot, LiveTranscriptionDiagnosticsSnapshot, ModelRuleInput, ModelRulesInput,
    ModelSummaryInput, PathStatusesInput, RuntimeEnvironmentStatus, RuntimePathStatus,
    SelectedModelsInput, VoiceTypingReadinessInput,
};
use sona_runtime_fs::FsDiagnosticsEnrichmentRepository;

pub async fn get_diagnostics_core_snapshot(
    provider: &dyn PathPort,
    asr: &AsrState,
    input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?
        .join("models");
    get_diagnostics_core_snapshot_with_models_dir(provider, models_dir, asr, input).await
}

pub async fn get_diagnostics_core_snapshot_with_models_dir(
    provider: &dyn PathPort,
    models_dir: std::path::PathBuf,
    asr: &AsrState,
    mut input: DiagnosticsCoreInput,
) -> Result<DiagnosticsCoreSnapshot, String> {
    let log_dir = provider
        .resolve_path(PathKind::AppLogData)
        .map_err(|error| error.to_string())?;
    input.asr_runtime_metrics = asr.metrics_snapshot().await;
    let live = asr.live_coordinator().metrics().await;
    input.live_transcription = LiveTranscriptionDiagnosticsSnapshot {
        active_sources: live.active_sources,
        active_pipelines: live.active_pipelines,
        active_consumers: live.active_consumers,
        shared_pipelines: live.shared_pipelines,
        avoided_feed_count: live.avoided_feed_count,
    };

    let snapshot = tokio::task::spawn_blocking(move || {
        let custom_ffmpeg = input.config.ffmpeg_path.as_str();
        input.runtime_environment =
            crate::platform::runtime_status::resolve_runtime_environment_status_for_log_dir_and_custom_ffmpeg(
                log_dir,
                Some(custom_ffmpeg),
            )?;
        let repository = FsDiagnosticsEnrichmentRepository::new(models_dir);
        DiagnosticsService::new(std::sync::Arc::new(repository))
            .build_snapshot_at(input, sona_runtime_fs::diagnostics_scanned_at_now())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_snapshot_builds_cleanly() {
        let input: DiagnosticsCoreInput = serde_json::from_value(serde_json::json!({
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

        let snapshot = sona_core::runtime::diagnostics::build_diagnostics_core_snapshot_at(
            input,
            "2026-07-17T00:00:00Z".to_string(),
        );
        assert_eq!(snapshot.scanned_at, "2026-07-17T00:00:00Z");
    }
}
