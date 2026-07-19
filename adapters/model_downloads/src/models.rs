use std::path::{Path, PathBuf};

use crate::downloads::{
    DownloadClient, DownloadError, DownloadFileOperation, publish_download_file, sha256_file,
    temporary_download_path,
};
use sona_core::models::downloads::ResolvedModelDownload;
use sona_runtime_fs::is_preset_model_installed_at;

pub async fn installed_model_is_valid(
    resolved: &ResolvedModelDownload,
) -> Result<bool, DownloadError> {
    if !is_preset_model_installed_at(&resolved.model, &resolved.models_dir) {
        return Ok(false);
    }

    if resolved.model.is_archive() {
        return Ok(true);
    }

    match &resolved.model.sha256 {
        Some(expected_sha) => {
            let actual_sha = sha256_file(&resolved.install_path).await?;
            Ok(actual_sha.eq_ignore_ascii_case(expected_sha))
        }
        None => Ok(true),
    }
}

pub fn remove_model_install_path(install_path: &Path) -> Result<(), DownloadError> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(DownloadError::file_system(
                DownloadFileOperation::InspectInstall,
                install_path,
                error.to_string(),
            ));
        }
    };

    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(install_path).map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::RemoveInstallDirectory,
                install_path,
                error.to_string(),
            )
        })
    } else {
        std::fs::remove_file(install_path).map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::RemoveInstallFile,
                install_path,
                error.to_string(),
            )
        })
    }
}

pub async fn download_model<F>(
    resolved: &ResolvedModelDownload,
    on_progress: F,
) -> Result<PathBuf, DownloadError>
where
    F: FnMut(u64, u64) + Send + 'static,
{
    tokio::fs::create_dir_all(&resolved.models_dir)
        .await
        .map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::CreateModelsDirectory,
                &resolved.models_dir,
                error.to_string(),
            )
        })?;

    let temp_download_path = temporary_download_path(&resolved.download_path);
    let notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let notify_clone = notify.clone();
    let ctrl_c_task = tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            notify_clone.notify_one();
        }
    });

    let client = DownloadClient::try_new()?;
    let result = client
        .download_file(
            &resolved.model.url,
            &temp_download_path,
            notify,
            Some(Box::new(on_progress)),
        )
        .await;

    ctrl_c_task.abort();

    result?;

    if let Some(expected_sha) = &resolved.model.sha256 {
        let actual_sha = match sha256_file(&temp_download_path).await {
            Ok(sha) => sha,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp_download_path).await;
                return Err(error);
            }
        };
        if !actual_sha.eq_ignore_ascii_case(expected_sha) {
            let _ = tokio::fs::remove_file(&temp_download_path).await;
            return Err(DownloadError::HashMismatch {
                path: temp_download_path,
                expected: expected_sha.clone(),
                actual: actual_sha,
            });
        }
    }

    publish_download_file(&temp_download_path, &resolved.download_path).await?;

    if resolved.model.is_archive() {
        extract_tar_bz2_archive(&resolved.download_path, &resolved.models_dir).await?;
        tokio::fs::remove_file(&resolved.download_path)
            .await
            .map_err(|error| {
                DownloadError::file_system(
                    DownloadFileOperation::RemoveArchive,
                    &resolved.download_path,
                    error.to_string(),
                )
            })?;
    }

    Ok(resolved.install_path.clone())
}

async fn extract_tar_bz2_archive(
    archive_path: &Path,
    target_dir: &Path,
) -> Result<(), DownloadError> {
    let archive_path = archive_path.to_path_buf();
    let target_dir = target_dir.to_path_buf();
    let join_archive_path = archive_path.clone();
    let join_target_dir = target_dir.clone();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path).map_err(|error| {
            DownloadError::file_system_with_target(
                DownloadFileOperation::OpenArchive,
                &archive_path,
                &target_dir,
                error.to_string(),
            )
        })?;
        let buffered = std::io::BufReader::new(file);
        let tar = bzip2::read::BzDecoder::new(buffered);
        let mut archive = tar::Archive::new(tar);
        archive.set_preserve_permissions(false);
        archive.set_unpack_xattrs(false);
        archive.unpack(&target_dir).map_err(|error| {
            DownloadError::file_system_with_target(
                DownloadFileOperation::ExtractArchive,
                &archive_path,
                &target_dir,
                error.to_string(),
            )
        })
    })
    .await
    .map_err(|error| {
        DownloadError::file_system_with_target(
            DownloadFileOperation::ExtractArchive,
            join_archive_path,
            join_target_dir,
            format!("Failed to join extraction task: {error}"),
        )
    })?
}
