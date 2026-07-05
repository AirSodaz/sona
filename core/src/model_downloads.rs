use std::path::{Path, PathBuf};

use crate::downloads::{
    DownloadError, download_file, publish_download_file, sha256_file, temporary_download_path,
};
use crate::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, PresetModel, find_preset_model,
    is_preset_model_installed_at,
};

#[derive(Debug, Clone)]
pub struct ResolvedModelDownload {
    pub model: PresetModel,
    pub models_dir: PathBuf,
    pub download_path: PathBuf,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequiredCompanionModels {
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

pub fn resolve_model_download(
    model_id: &str,
    models_dir: &Path,
) -> Result<ResolvedModelDownload, String> {
    let model = find_preset_model(model_id)
        .ok_or_else(|| format!("Unknown model id: {model_id}"))?
        .clone();
    let download_path = model.resolve_download_path(models_dir);
    let install_path = model.resolve_install_path(models_dir);

    Ok(ResolvedModelDownload {
        model,
        models_dir: models_dir.to_path_buf(),
        download_path,
        install_path,
    })
}

pub fn required_companion_models(model: &PresetModel) -> RequiredCompanionModels {
    let rules = model.resolved_rules();
    RequiredCompanionModels {
        vad_model_id: rules
            .requires_vad
            .then(|| DEFAULT_SILERO_VAD_MODEL_ID.to_string()),
        punctuation_model_id: rules
            .requires_punctuation
            .then(|| DEFAULT_PUNCTUATION_MODEL_ID.to_string()),
    }
}

pub async fn installed_model_is_valid(resolved: &ResolvedModelDownload) -> Result<bool, String> {
    if !is_preset_model_installed_at(&resolved.model, &resolved.models_dir) {
        return Ok(false);
    }

    if resolved.model.is_archive() {
        return Ok(true);
    }

    match &resolved.model.sha256 {
        Some(expected_sha) => {
            let actual_sha = sha256_file(&resolved.install_path).await.map_err(|error| {
                format!(
                    "Failed to calculate hash of installed model {}: {error}",
                    resolved.install_path.display()
                )
            })?;
            Ok(actual_sha.eq_ignore_ascii_case(expected_sha))
        }
        None => Ok(true),
    }
}

pub async fn download_model<F>(
    resolved: &ResolvedModelDownload,
    on_progress: F,
) -> Result<PathBuf, String>
where
    F: FnMut(u64, u64) + Send + 'static,
{
    tokio::fs::create_dir_all(&resolved.models_dir)
        .await
        .map_err(|error| {
            format!(
                "Failed to create models directory {}: {error}",
                resolved.models_dir.display()
            )
        })?;

    let client = reqwest::Client::builder()
        .user_agent("Sona/1.0")
        .build()
        .map_err(|error| format!("Failed to create HTTP client: {error}"))?;

    let temp_download_path = temporary_download_path(&resolved.download_path);
    let notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let notify_clone = notify.clone();
    let ctrl_c_task = tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            notify_clone.notify_one();
        }
    });

    let result = download_file(
        &client,
        &resolved.model.url,
        &temp_download_path,
        notify,
        Some(Box::new(on_progress)),
    )
    .await;

    ctrl_c_task.abort();

    result.map_err(map_download_error)?;

    if let Some(expected_sha) = &resolved.model.sha256 {
        let actual_sha = match sha256_file(&temp_download_path).await {
            Ok(sha) => sha,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp_download_path).await;
                return Err(format!(
                    "Failed to calculate hash of downloaded file: {error}"
                ));
            }
        };
        if !actual_sha.eq_ignore_ascii_case(expected_sha) {
            let _ = tokio::fs::remove_file(&temp_download_path).await;
            return Err(format!(
                "Downloaded file hash mismatch for {}. Expected: {}, got: {}",
                temp_download_path.display(),
                expected_sha,
                actual_sha
            ));
        }
    }

    publish_download_file(&temp_download_path, &resolved.download_path)
        .await
        .map_err(|error| {
            format!(
                "Failed to publish download {} to {}: {error}",
                temp_download_path.display(),
                resolved.download_path.display()
            )
        })?;

    if resolved.model.is_archive() {
        extract_tar_bz2_archive(&resolved.download_path, &resolved.models_dir).await?;
        tokio::fs::remove_file(&resolved.download_path)
            .await
            .map_err(|error| {
                format!(
                    "Failed to remove archive {}: {error}",
                    resolved.download_path.display()
                )
            })?;
    }

    Ok(resolved.install_path.clone())
}

fn map_download_error(error: DownloadError) -> String {
    match error {
        DownloadError::Cancelled => "Download cancelled by user".to_string(),
        DownloadError::HttpStatus(status) => format!("Download failed with status: {status}"),
        other => format!("Failed to download model: {other}"),
    }
}

async fn extract_tar_bz2_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let archive_path = archive_path.to_path_buf();
    let target_dir = target_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path).map_err(|error| {
            format!("Failed to open archive {}: {error}", archive_path.display())
        })?;
        let buffered = std::io::BufReader::new(file);
        let tar = bzip2::read::BzDecoder::new(buffered);
        let mut archive = tar::Archive::new(tar);
        archive.set_preserve_permissions(false);
        archive.set_unpack_xattrs(false);
        archive.unpack(&target_dir).map_err(|error| {
            format!(
                "Failed to extract archive into {}: {error}",
                target_dir.display()
            )
        })
    })
    .await
    .map_err(|error| format!("Failed to join extraction task: {error}"))?
}
