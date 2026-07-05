use crate::core::paths::{PathKind, PathProvider};
pub use sona_core::preset_models::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Returns a settings-page-ready catalog snapshot for the app-local models dir.
pub async fn get_model_catalog_snapshot(
    provider: &dyn PathProvider,
) -> Result<ModelCatalogSnapshot, String> {
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?
        .join("models");

    build_model_catalog_snapshot_for_models_dir(models_dir).await
}

/// Builds model metadata and settings groups, injecting install status from the filesystem.
pub fn build_model_catalog_snapshot(models_dir: &Path) -> ModelCatalogSnapshot {
    let installed_model_ids = installed_model_ids_for_models_dir(models_dir);
    build_model_catalog_snapshot_with_installed_ids(models_dir, &installed_model_ids)
}

pub async fn resolve_model_catalog_selected_ids_command(
    provider: &dyn PathProvider,
    paths: ModelSelectionPaths,
) -> Result<ModelCatalogSelectedIds, String> {
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?
        .join("models");

    let snapshot = build_model_catalog_snapshot_for_models_dir(models_dir).await?;
    Ok(resolve_model_catalog_selected_ids(&snapshot, &paths))
}

async fn build_model_catalog_snapshot_for_models_dir(
    models_dir: PathBuf,
) -> Result<ModelCatalogSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&models_dir).map_err(|error| {
            format!(
                "Failed to create models directory {}: {error}",
                models_dir.display()
            )
        })?;

        Ok(build_model_catalog_snapshot(&models_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn installed_model_ids_for_models_dir(models_dir: &Path) -> HashSet<String> {
    preset_models()
        .iter()
        .filter(|model| is_preset_model_installed_at(model, models_dir))
        .map(|model| model.id.clone())
        .collect()
}

/// Returns true when the preset's install path contains a complete installed model.
pub fn is_preset_model_installed_at(model: &PresetModel, models_dir: &Path) -> bool {
    is_preset_model_install_path_complete(model, &model.resolve_install_path(models_dir))
}

fn is_preset_model_install_path_complete(model: &PresetModel, install_path: &Path) -> bool {
    let Ok(metadata) = install_path.metadata() else {
        return false;
    };

    if model.is_archive() {
        return install_path.exists();
    }

    if !metadata.is_file() {
        return false;
    }

    metadata.len() > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn build_model_catalog_snapshot_uses_filesystem_install_status() {
        let dir = tempfile::tempdir().unwrap();
        let models_dir = dir.path().join("models");
        fs::create_dir_all(
            models_dir.join("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"),
        )
        .unwrap();
        fs::write(
            models_dir.join("silero_vad.onnx"),
            b"not the expected model",
        )
        .unwrap();

        let snapshot = build_model_catalog_snapshot(&models_dir);
        let int8_path = snapshot
            .model_path_by_id
            .get("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
            .unwrap()
            .clone();
        let silero_path = snapshot.model_path_by_id.get("silero-vad").unwrap().clone();

        assert_eq!(
            snapshot.models_dir,
            models_dir.to_string_lossy().to_string()
        );
        assert!(snapshot.models.iter().any(|model| {
            model.id == "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"
                && model.is_installed
        }));
        assert!(
            snapshot
                .models
                .iter()
                .any(|model| model.id == "silero-vad" && model.is_installed)
        );
        assert_eq!(
            snapshot.restore_defaults.streaming_model_path,
            Some(int8_path.clone())
        );
        assert_eq!(
            snapshot.restore_defaults.offline_model_path,
            Some(int8_path)
        );
        assert_eq!(snapshot.restore_defaults.vad_model_path, Some(silero_path));
    }

    #[test]
    fn non_archive_install_status_requires_non_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let empty_path = dir.path().join("empty.onnx");
        let non_empty_path = dir.path().join("non-empty.onnx");
        let directory_path = dir.path().join("directory.onnx");
        fs::write(&empty_path, []).unwrap();
        fs::write(&non_empty_path, b"model bytes").unwrap();
        fs::create_dir_all(&directory_path).unwrap();

        let mut model = find_preset_model("silero-vad").unwrap().clone();
        model.sha256 = None;

        assert!(!is_preset_model_install_path_complete(&model, &empty_path));
        assert!(is_preset_model_install_path_complete(
            &model,
            &non_empty_path
        ));
        assert!(!is_preset_model_install_path_complete(
            &model,
            &directory_path
        ));
    }

    #[test]
    fn archive_install_status_accepts_existing_install_path() {
        let dir = tempfile::tempdir().unwrap();
        let models_dir = dir.path().join("models");
        let model = find_preset_model("sherpa-onnx-whisper-turbo").unwrap();
        fs::create_dir_all(model.resolve_install_path(&models_dir)).unwrap();

        assert!(is_preset_model_installed_at(model, &models_dir));
    }
}
