use std::path::{Path, PathBuf};

use crate::downloads::{
    DownloadError, download_file, publish_download_file, sha256_file, temporary_download_path,
};
use sona_core::model_downloads::ResolvedModelDownload;
use sona_runtime_fs::is_preset_model_installed_at;

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

pub fn remove_model_install_path(install_path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to inspect model path {}: {error}",
                install_path.display()
            ));
        }
    };

    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(install_path).map_err(|error| {
            format!(
                "Failed to delete model directory {}: {error}",
                install_path.display()
            )
        })
    } else {
        std::fs::remove_file(install_path).map_err(|error| {
            format!(
                "Failed to delete model file {}: {error}",
                install_path.display()
            )
        })
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
