use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::mapper::{
    self, FfiInstalledLocalAsrModel, FfiModelCatalogSelectedIds, FfiModelCatalogSnapshot,
    FfiModelSelectionPaths, FfiPresetModel, FfiResolvedModelDownload,
};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::models::downloads::{
    required_companion_models, resolve_model_download as core_resolve_model_download,
};
use sona_core::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID,
    build_model_catalog_snapshot_with_installed_ids, find_preset_model,
    preset_models as core_preset_models, resolve_model_catalog_selected_ids,
};
use sona_core::runtime::gpu::resolve_gpu_acceleration as core_resolve_gpu_acceleration;
use sona_model_downloads::{
    DownloadError, ModelDownloadStage, download_model_with_cancel, installed_model_is_complete,
    installed_model_is_valid, remove_model_install_path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiModelDownloadStage {
    Downloading,
    Verifying,
    Installing,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiModelDownloadProgress {
    pub model_id: String,
    pub component_model_id: String,
    pub stage: FfiModelDownloadStage,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[uniffi::export(foreign)]
pub trait FfiModelDownloadObserver: Send + Sync {
    fn on_progress(&self, event: FfiModelDownloadProgress);
}

pub(crate) fn default_vad_model_id() -> String {
    DEFAULT_SILERO_VAD_MODEL_ID.to_string()
}

pub(crate) fn default_punctuation_model_id() -> String {
    DEFAULT_PUNCTUATION_MODEL_ID.to_string()
}

pub(crate) fn preset_model_name(model_id: String) -> Option<String> {
    find_preset_model(&model_id).map(|model| model.name.clone())
}

pub(crate) fn preset_models() -> Vec<FfiPresetModel> {
    core_preset_models()
        .iter()
        .map(mapper::preset_model_to_ffi)
        .collect()
}

pub(crate) fn model_catalog_snapshot(
    models_dir: String,
    installed_model_ids: Vec<String>,
) -> FfiModelCatalogSnapshot {
    let installed_model_ids = installed_model_ids.into_iter().collect::<HashSet<_>>();
    mapper::model_catalog_snapshot_to_ffi(build_model_catalog_snapshot_with_installed_ids(
        Path::new(&models_dir),
        &installed_model_ids,
    ))
}

pub(crate) fn model_catalog_selected_ids(
    models_dir: String,
    installed_model_ids: Vec<String>,
    paths: FfiModelSelectionPaths,
) -> FfiModelCatalogSelectedIds {
    let installed_model_ids = installed_model_ids.into_iter().collect::<HashSet<_>>();
    let snapshot = build_model_catalog_snapshot_with_installed_ids(
        Path::new(&models_dir),
        &installed_model_ids,
    );

    mapper::model_catalog_selected_ids_to_ffi(resolve_model_catalog_selected_ids(
        &snapshot,
        &mapper::model_selection_paths_from_ffi(paths),
    ))
}

pub(crate) fn resolve_model_download(
    model_id: String,
    models_dir: String,
) -> SonaCoreBindingResult<FfiResolvedModelDownload> {
    let resolved =
        core_resolve_model_download(&model_id, Path::new(&models_dir)).map_err(|error| {
            SonaCoreBindingError::InvalidInput {
                reason: error.to_string(),
            }
        })?;
    let required_companions = required_companion_models(&resolved.model);

    Ok(mapper::resolved_model_download_to_ffi(
        resolved,
        required_companions,
    ))
}

pub(crate) fn list_installed_local_asr_models(
    models_dir: String,
    num_threads: u32,
) -> SonaCoreBindingResult<Vec<FfiInstalledLocalAsrModel>> {
    validate_model_arguments(&models_dir, num_threads)?;
    let models_dir = Path::new(&models_dir);

    core_preset_models()
        .iter()
        .filter(|model| {
            model.engine.as_deref().unwrap_or("sherpa-onnx") == "sherpa-onnx"
                && model.modes.as_ref().is_some_and(|modes| !modes.is_empty())
        })
        .filter_map(|model| {
            let resolved = core_resolve_model_download(&model.id, models_dir).ok()?;
            model_bundle_is_complete(&resolved)
                .then(|| installed_model_to_ffi(&resolved, num_threads))
        })
        .collect()
}

pub(crate) async fn download_local_asr_model(
    model_id: String,
    models_dir: String,
    num_threads: u32,
    observer: Arc<dyn FfiModelDownloadObserver>,
) -> SonaCoreBindingResult<FfiInstalledLocalAsrModel> {
    validate_model_arguments(&models_dir, num_threads)?;
    let resolved = resolve_download(&model_id, Path::new(&models_dir))?;
    require_recognition_model(&resolved)?;

    ensure_downloaded(&model_id, &resolved, observer.clone()).await?;
    for companion_id in companion_model_ids(&resolved) {
        let companion = resolve_download(&companion_id, Path::new(&models_dir))?;
        ensure_downloaded(&model_id, &companion, observer.clone()).await?;
    }

    if !model_bundle_is_complete(&resolved) {
        return Err(SonaCoreBindingError::ModelDownload {
            code: "invalid_install".to_string(),
            reason: format!("Model installation is incomplete: {model_id}"),
        });
    }

    installed_model_to_ffi(&resolved, num_threads)
}

pub(crate) async fn validate_local_asr_model(
    model_id: String,
    models_dir: String,
) -> SonaCoreBindingResult<bool> {
    if models_dir.trim().is_empty() {
        return Err(invalid_model_input("Models directory must not be blank"));
    }
    let resolved = resolve_download(&model_id, Path::new(&models_dir))?;
    require_recognition_model(&resolved)?;
    if !model_install_is_valid(&resolved)
        .await
        .map_err(map_download_error)?
    {
        return Ok(false);
    }
    for companion_id in companion_model_ids(&resolved) {
        let companion = resolve_download(&companion_id, Path::new(&models_dir))?;
        if !model_install_is_valid(&companion)
            .await
            .map_err(map_download_error)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn delete_local_asr_model(
    model_id: String,
    models_dir: String,
) -> SonaCoreBindingResult<()> {
    if models_dir.trim().is_empty() {
        return Err(invalid_model_input("Models directory must not be blank"));
    }
    let resolved = resolve_download(&model_id, Path::new(&models_dir))?;
    require_recognition_model(&resolved)?;
    remove_model_install_path(&resolved.install_path).map_err(map_download_error)
}

fn validate_model_arguments(models_dir: &str, num_threads: u32) -> SonaCoreBindingResult<()> {
    if models_dir.trim().is_empty() {
        return Err(invalid_model_input("Models directory must not be blank"));
    }
    if !(1..=8).contains(&num_threads) {
        return Err(invalid_model_input(
            "Model thread count must be between 1 and 8",
        ));
    }
    Ok(())
}

fn resolve_download(
    model_id: &str,
    models_dir: &Path,
) -> SonaCoreBindingResult<sona_core::models::downloads::ResolvedModelDownload> {
    core_resolve_model_download(model_id, models_dir)
        .map_err(|error| invalid_model_input(&error.to_string()))
}

fn require_recognition_model(
    resolved: &sona_core::models::downloads::ResolvedModelDownload,
) -> SonaCoreBindingResult<()> {
    if resolved
        .model
        .modes
        .as_ref()
        .is_some_and(|modes| !modes.is_empty())
        && resolved.model.file_config.is_some()
    {
        Ok(())
    } else {
        Err(invalid_model_input(
            "Model is not a local ASR recognition model",
        ))
    }
}

fn companion_model_ids(
    resolved: &sona_core::models::downloads::ResolvedModelDownload,
) -> Vec<String> {
    let companions = required_companion_models(&resolved.model);
    [companions.vad_model_id, companions.punctuation_model_id]
        .into_iter()
        .flatten()
        .collect()
}

fn model_bundle_is_complete(
    resolved: &sona_core::models::downloads::ResolvedModelDownload,
) -> bool {
    model_install_is_complete(resolved)
        && companion_model_ids(resolved)
            .into_iter()
            .all(|companion_id| {
                resolve_download(&companion_id, &resolved.models_dir)
                    .is_ok_and(|companion| model_install_is_complete(&companion))
            })
}

fn model_install_is_complete(
    resolved: &sona_core::models::downloads::ResolvedModelDownload,
) -> bool {
    if !installed_model_is_complete(resolved) {
        return false;
    }
    let Some(files) = &resolved.model.file_config else {
        return true;
    };
    [
        files.encoder.as_deref(),
        files.decoder.as_deref(),
        files.model.as_deref(),
        files.joiner.as_deref(),
        files.tokens.as_deref(),
        files.conv_frontend.as_deref(),
        files.encoder_adaptor.as_deref(),
        files.llm.as_deref(),
        files.embedding.as_deref(),
        files.tokenizer.as_deref(),
    ]
    .into_iter()
    .flatten()
    .all(|relative_path| installed_component_is_complete(&resolved.install_path, relative_path))
}

fn installed_component_is_complete(root: &Path, relative_path: &str) -> bool {
    let path = root.join(relative_path);
    let Ok(metadata) = std::fs::symlink_metadata(&path) else {
        return false;
    };
    if metadata.file_type().is_symlink() {
        return false;
    }
    if metadata.is_file() {
        return metadata.len() > 0;
    }
    metadata.is_dir()
        && std::fs::read_dir(path).is_ok_and(|entries| entries.flatten().next().is_some())
}

async fn model_install_is_valid(
    resolved: &sona_core::models::downloads::ResolvedModelDownload,
) -> Result<bool, DownloadError> {
    if !model_install_is_complete(resolved) {
        return Ok(false);
    }
    installed_model_is_valid(resolved).await
}

async fn ensure_downloaded(
    requested_model_id: &str,
    resolved: &sona_core::models::downloads::ResolvedModelDownload,
    observer: Arc<dyn FfiModelDownloadObserver>,
) -> SonaCoreBindingResult<()> {
    if model_install_is_valid(resolved)
        .await
        .map_err(map_download_error)?
    {
        return Ok(());
    }

    let requested_model_id = requested_model_id.to_string();
    let component_model_id = resolved.model.id.clone();
    download_model_with_cancel(
        resolved,
        Arc::new(tokio::sync::Notify::new()),
        move |progress| {
            notify_progress(
                observer.as_ref(),
                FfiModelDownloadProgress {
                    model_id: requested_model_id.clone(),
                    component_model_id: component_model_id.clone(),
                    stage: match progress.stage {
                        ModelDownloadStage::Downloading => FfiModelDownloadStage::Downloading,
                        ModelDownloadStage::Verifying => FfiModelDownloadStage::Verifying,
                        ModelDownloadStage::Installing => FfiModelDownloadStage::Installing,
                    },
                    downloaded_bytes: progress.downloaded_bytes,
                    total_bytes: progress.total_bytes,
                },
            );
        },
    )
    .await
    .map_err(map_download_error)?;
    Ok(())
}

fn notify_progress(observer: &dyn FfiModelDownloadObserver, event: FfiModelDownloadProgress) {
    let _ = catch_unwind(AssertUnwindSafe(|| observer.on_progress(event)));
}

fn installed_model_to_ffi(
    resolved: &sona_core::models::downloads::ResolvedModelDownload,
    num_threads: u32,
) -> SonaCoreBindingResult<FfiInstalledLocalAsrModel> {
    let files =
        resolved.model.file_config.clone().ok_or_else(|| {
            invalid_model_input("Local ASR model is missing its file configuration")
        })?;
    let companions = required_companion_models(&resolved.model);
    let companion_path = |id: Option<String>| -> Option<String> {
        id.and_then(|id| core_resolve_model_download(&id, &resolved.models_dir).ok())
            .map(|download| download.install_path.to_string_lossy().into_owned())
    };
    let display_name = match resolved.model.version_label.as_deref() {
        Some(version) if version != resolved.model.name => {
            format!("{} {version}", resolved.model.name)
        }
        _ => resolved.model.name.clone(),
    };

    Ok(FfiInstalledLocalAsrModel {
        id: resolved.model.id.clone(),
        display_name,
        model_path: resolved.install_path.to_string_lossy().into_owned(),
        model_type: resolved.model.model_type.clone(),
        modes: resolved.model.modes.clone().unwrap_or_default(),
        size_bytes: installed_size_bytes(&resolved.install_path),
        num_threads,
        vad_model_path: companion_path(companions.vad_model_id),
        punctuation_model_path: companion_path(companions.punctuation_model_id),
        files: mapper::model_file_config_to_ffi(files),
    })
}

fn installed_size_bytes(path: &Path) -> u64 {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() {
        return 0;
    }

    let mut total = 0_u64;
    let mut pending = vec![PathBuf::from(path)];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.path().symlink_metadata() else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    total
}

fn invalid_model_input(reason: &str) -> SonaCoreBindingError {
    SonaCoreBindingError::InvalidInput {
        reason: reason.to_string(),
    }
}

fn map_download_error(error: DownloadError) -> SonaCoreBindingError {
    let code = match &error {
        DownloadError::Cancelled => "cancelled",
        DownloadError::Network(_)
        | DownloadError::HttpStatus(_)
        | DownloadError::HttpClient { .. }
        | DownloadError::RangeNotSatisfiable => "network",
        DownloadError::HashMismatch { .. } => "hash_mismatch",
        DownloadError::AlreadyInProgress => "already_in_progress",
        DownloadError::Io(_) | DownloadError::FileSystem(_) => "filesystem",
    };
    SonaCoreBindingError::ModelDownload {
        code: code.to_string(),
        reason: error.to_string(),
    }
}

pub(crate) fn resolve_gpu_acceleration(
    value: Option<String>,
) -> SonaCoreBindingResult<Option<String>> {
    core_resolve_gpu_acceleration(value).map_err(|error| SonaCoreBindingError::InvalidInput {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENSEVOICE_ID: &str = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17";

    #[test]
    fn installed_listing_requires_preset_files_and_delete_retains_shared_companions() {
        let temp = tempfile::tempdir().unwrap();
        let models_dir = temp.path().join("models");
        let primary = core_resolve_model_download(SENSEVOICE_ID, &models_dir).unwrap();
        let vad = core_resolve_model_download(DEFAULT_SILERO_VAD_MODEL_ID, &models_dir).unwrap();
        std::fs::create_dir_all(&primary.install_path).unwrap();
        std::fs::write(primary.install_path.join("model.int8.onnx"), b"model").unwrap();
        std::fs::write(&vad.install_path, b"vad").unwrap();

        assert!(
            list_installed_local_asr_models(models_dir.to_string_lossy().into_owned(), 2)
                .unwrap()
                .is_empty()
        );

        std::fs::write(primary.install_path.join("tokens.txt"), b"tokens").unwrap();
        let installed =
            list_installed_local_asr_models(models_dir.to_string_lossy().into_owned(), 2).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, SENSEVOICE_ID);

        delete_local_asr_model(
            SENSEVOICE_ID.to_string(),
            models_dir.to_string_lossy().into_owned(),
        )
        .unwrap();
        assert!(!primary.install_path.exists());
        assert!(vad.install_path.exists());
    }
}
