use crate::{
    FfiDiagnosticsInputV1, FfiDiagnosticsSnapshotV1, SonaCoreBindingError, SonaCoreBindingResult,
};
use sona_core::runtime::diagnostics::{DiagnosticsCoreInput, DiagnosticsCoreSnapshot};
use sona_runtime_fs::build_diagnostics_snapshot;
use std::path::PathBuf;

pub(crate) async fn load_diagnostics_snapshot_json(
    app_data_dir: String,
    input_json: String,
) -> SonaCoreBindingResult<String> {
    let input: DiagnosticsCoreInput =
        serde_json::from_str(&input_json).map_err(diagnostics_error)?;
    let snapshot = load_snapshot(app_data_dir, input).await?;
    let canonical = serde_json::to_value(snapshot).map_err(diagnostics_error)?;
    serde_json::to_string(&canonical).map_err(diagnostics_error)
}

pub(crate) async fn load_diagnostics_snapshot_v1(
    app_data_dir: String,
    input: FfiDiagnosticsInputV1,
) -> SonaCoreBindingResult<FfiDiagnosticsSnapshotV1> {
    load_snapshot(app_data_dir, input.into())
        .await
        .map(Into::into)
}

async fn load_snapshot(
    app_data_dir: String,
    input: DiagnosticsCoreInput,
) -> SonaCoreBindingResult<DiagnosticsCoreSnapshot> {
    tokio::task::spawn_blocking(move || {
        let app_data_dir =
            std::path::absolute(PathBuf::from(app_data_dir)).map_err(diagnostics_error)?;
        build_diagnostics_snapshot(app_data_dir.join("models"), input).map_err(diagnostics_error)
    })
    .await
    .map_err(diagnostics_error)?
}

fn diagnostics_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Diagnostics {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{load_diagnostics_snapshot_json, load_diagnostics_snapshot_v1};
    use crate::{
        FfiAsrRuntimeMetricsSnapshotV1, FfiDiagnosticsConfigV1, FfiDiagnosticsDeviceProbeV1,
        FfiDiagnosticsInputV1, FfiDiagnosticsModelRulesV1, FfiDiagnosticsPathStatusesV1,
        FfiDiagnosticsSelectedModelsV1, FfiRuntimeEnvironmentStatusV1, FfiRuntimePathKind,
        FfiVoiceTypingReadinessV1, SonaCoreBindingError,
    };
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use sona_core::models::preset_models::find_preset_model;
    use sona_core::runtime::diagnostics::DiagnosticsCoreSnapshot;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    const LIVE_MODEL_ID: &str = "sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en";
    const BATCH_MODEL_ID: &str = "sherpa-onnx-whisper-turbo";

    fn input_json(models_dir: &Path) -> String {
        let live_path = find_preset_model(LIVE_MODEL_ID)
            .unwrap()
            .resolve_install_path(models_dir);
        let batch_path = find_preset_model(BATCH_MODEL_ID)
            .unwrap()
            .resolve_install_path(models_dir);
        serde_json::to_string(&json!({
            "config": {
                "streamingModelPath": live_path,
                "batchModelPath": batch_path,
                "vadModelPath": "",
                "punctuationModelPath": "",
                "microphoneId": "mobile-default"
            },
            "permissionState": "granted",
            "microphoneProbe": {"options": [], "available": true, "errorMessage": null},
            "systemAudioProbe": {"options": [], "available": false, "errorMessage": "unsupported"},
            "voiceTypingReadiness": {"state": "mobile-ready", "lastErrorMessage": null},
            "runtimeEnvironment": {
                "ffmpegPath": "mobile://ffmpeg",
                "ffmpegExists": false,
                "logDirPath": "mobile://logs"
            }
        }))
        .unwrap()
    }

    /// The typed twin of `input_json`, field for field.
    fn typed_input(models_dir: &Path) -> FfiDiagnosticsInputV1 {
        let live_path = find_preset_model(LIVE_MODEL_ID)
            .unwrap()
            .resolve_install_path(models_dir);
        let batch_path = find_preset_model(BATCH_MODEL_ID)
            .unwrap()
            .resolve_install_path(models_dir);
        FfiDiagnosticsInputV1 {
            config: FfiDiagnosticsConfigV1 {
                streaming_model_path: live_path.to_string_lossy().into_owned(),
                batch_model_path: batch_path.to_string_lossy().into_owned(),
                vad_model_path: String::new(),
                punctuation_model_path: String::new(),
                microphone_id: "mobile-default".to_string(),
            },
            selected_models: FfiDiagnosticsSelectedModelsV1 {
                live: None,
                batch: None,
            },
            model_rules: FfiDiagnosticsModelRulesV1 {
                live: None,
                batch: None,
            },
            path_statuses: FfiDiagnosticsPathStatusesV1 {
                live_model: None,
                batch_model: None,
                vad: None,
                punctuation: None,
            },
            permission_state: "granted".to_string(),
            microphone_probe: FfiDiagnosticsDeviceProbeV1 {
                options: Vec::new(),
                available: true,
                error_message: None,
            },
            system_audio_probe: FfiDiagnosticsDeviceProbeV1 {
                options: Vec::new(),
                available: false,
                error_message: Some("unsupported".to_string()),
            },
            voice_typing_readiness: FfiVoiceTypingReadinessV1 {
                state: "mobile-ready".to_string(),
                last_error_message: None,
            },
            runtime_environment: FfiRuntimeEnvironmentStatusV1 {
                ffmpeg_path: "mobile://ffmpeg".to_string(),
                ffmpeg_exists: false,
                log_dir_path: "mobile://logs".to_string(),
            },
            asr_runtime_metrics: FfiAsrRuntimeMetricsSnapshotV1 {
                model_load: None,
                live_inference: None,
                batch_inference: None,
            },
            onboarding_ready: false,
            punctuation_required: false,
        }
    }

    fn file_hashes(root: &Path) -> BTreeMap<PathBuf, String> {
        fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, String>) {
            let mut entries = fs::read_dir(current)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.insert(
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
                    );
                }
            }
        }

        let mut files = BTreeMap::new();
        visit(root, root, &mut files);
        files
    }

    #[tokio::test]
    async fn invalid_json_uses_diagnostics_error_without_creating_app_data() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");

        let error =
            load_diagnostics_snapshot_json(missing.to_string_lossy().into_owned(), "{".to_string())
                .await
                .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Diagnostics { .. }));
        assert!(!missing.exists());
    }

    #[tokio::test]
    async fn relative_unicode_snapshot_is_canonical_and_preserves_host_facts() {
        let current = std::env::current_dir().unwrap();
        let parent = tempfile::tempdir_in(&current).unwrap();
        let app_data_dir = parent.path().join("诊断-移动端-🌍");
        let models_dir = app_data_dir.join("models");
        fs::create_dir_all(&models_dir).unwrap();
        fs::write(app_data_dir.join("sentinel.txt"), b"unchanged").unwrap();
        let relative = app_data_dir.strip_prefix(&current).unwrap();
        let before = file_hashes(&app_data_dir);

        let output = load_diagnostics_snapshot_json(
            relative.to_string_lossy().into_owned(),
            input_json(&models_dir),
        )
        .await
        .unwrap();
        let typed: DiagnosticsCoreSnapshot = serde_json::from_str(&output).unwrap();
        let snapshot = serde_json::to_value(typed).unwrap();

        assert_eq!(serde_json::to_string(&snapshot).unwrap(), output);
        assert_eq!(snapshot["selectedModels"]["live"]["id"], LIVE_MODEL_ID);
        assert_eq!(snapshot["selectedModels"]["batch"]["id"], BATCH_MODEL_ID);
        assert_eq!(snapshot["modelRules"]["live"]["requiresPunctuation"], true);
        assert_eq!(
            snapshot["runtimeEnvironment"]["ffmpegPath"],
            "mobile://ffmpeg"
        );
        assert_eq!(snapshot["voiceTypingReadiness"]["state"], "mobile-ready");
        assert!(snapshot["scannedAt"].as_str().unwrap().ends_with('Z'));
        assert_eq!(file_hashes(&app_data_dir), before);
    }

    #[tokio::test]
    async fn typed_snapshot_matches_the_legacy_json_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let app_data_dir = root.path().join("app-data");
        let models_dir = app_data_dir.join("models");
        fs::create_dir_all(&models_dir).unwrap();
        let dir_arg = app_data_dir.to_string_lossy().into_owned();

        let typed = load_diagnostics_snapshot_v1(dir_arg.clone(), typed_input(&models_dir))
            .await
            .unwrap();
        let json: DiagnosticsCoreSnapshot = serde_json::from_str(
            &load_diagnostics_snapshot_json(dir_arg, input_json(&models_dir))
                .await
                .unwrap(),
        )
        .unwrap();

        // Core resolves selected models and path statuses from the models dir,
        // so both surfaces must report the same enrichment.
        assert_eq!(
            typed.selected_models.live.as_ref().unwrap().id,
            LIVE_MODEL_ID
        );
        assert_eq!(
            typed.selected_models.batch.as_ref().unwrap().id,
            BATCH_MODEL_ID
        );
        assert_eq!(
            typed.selected_models.live.as_ref().unwrap().id,
            json.selected_models.live.as_ref().unwrap().id
        );
        assert!(
            typed
                .model_rules
                .live
                .as_ref()
                .unwrap()
                .requires_punctuation
        );
        assert_eq!(
            typed
                .model_rules
                .live
                .as_ref()
                .unwrap()
                .requires_punctuation,
            json.model_rules.live.as_ref().unwrap().requires_punctuation
        );
        assert_eq!(
            typed.path_statuses.live_model.as_ref().unwrap().path,
            json.path_statuses.live_model.as_ref().unwrap().path
        );
        // Host-reported facts must survive the round trip untouched.
        assert_eq!(typed.runtime_environment.ffmpeg_path, "mobile://ffmpeg");
        assert_eq!(typed.voice_typing_readiness.state, "mobile-ready");
        assert_eq!(
            typed.system_audio_probe.error_message.as_deref(),
            Some("unsupported")
        );
        assert!(typed.scanned_at.ends_with('Z'));
    }

    #[tokio::test]
    async fn typed_snapshot_ensures_the_models_directory_like_the_json_surface() {
        let root = tempfile::tempdir().unwrap();
        let typed_dir = root.path().join("typed");
        let json_dir = root.path().join("json");

        // Diagnostics enrichment calls `ensure_directory_exists` on the models
        // directory, so scanning an app-data directory that does not exist yet
        // creates it. That is deliberate and must hold identically on both
        // surfaces — the typed contract changes the payload, not the behaviour.
        let typed = load_diagnostics_snapshot_v1(
            typed_dir.to_string_lossy().into_owned(),
            typed_input(&typed_dir.join("models")),
        )
        .await
        .unwrap();
        let json: DiagnosticsCoreSnapshot = serde_json::from_str(
            &load_diagnostics_snapshot_json(
                json_dir.to_string_lossy().into_owned(),
                input_json(&json_dir.join("models")),
            )
            .await
            .unwrap(),
        )
        .unwrap();

        assert!(typed_dir.join("models").is_dir());
        assert!(json_dir.join("models").is_dir());
        // With nothing installed, both report the resolved model paths missing.
        assert_eq!(
            typed.path_statuses.live_model.as_ref().unwrap().kind,
            FfiRuntimePathKind::Missing
        );
        assert_eq!(
            json.path_statuses.live_model.as_ref().unwrap().kind,
            sona_core::runtime::environment::RuntimePathKind::Missing
        );
        assert!(typed.selected_models.live.is_some());
        assert!(typed.scanned_at.ends_with('Z'));
    }
}
