use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::runtime::diagnostics::{DiagnosticsCoreInput, DiagnosticsService};
use sona_runtime_fs::FsDiagnosticsEnrichmentRepository;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) async fn load_diagnostics_snapshot_json(
    app_data_dir: String,
    input_json: String,
) -> SonaCoreBindingResult<String> {
    tokio::task::spawn_blocking(move || build_diagnostics_snapshot_json(app_data_dir, input_json))
        .await
        .map_err(diagnostics_error)?
}

fn build_diagnostics_snapshot_json(
    app_data_dir: String,
    input_json: String,
) -> SonaCoreBindingResult<String> {
    let input: DiagnosticsCoreInput =
        serde_json::from_str(&input_json).map_err(diagnostics_error)?;
    let app_data_dir =
        std::path::absolute(PathBuf::from(app_data_dir)).map_err(diagnostics_error)?;
    let repository = FsDiagnosticsEnrichmentRepository::new(app_data_dir.join("models"));
    let snapshot = DiagnosticsService::new(Arc::new(repository))
        .build_snapshot_at(input, sona_runtime_fs::diagnostics_scanned_at_now())
        .map_err(diagnostics_error)?;
    let canonical = serde_json::to_value(snapshot).map_err(diagnostics_error)?;
    serde_json::to_string(&canonical).map_err(diagnostics_error)
}

fn diagnostics_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Diagnostics {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::load_diagnostics_snapshot_json;
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use sona_core::models::preset_models::find_preset_model;
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
        let snapshot: Value = serde_json::from_str(&output).unwrap();

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
}
