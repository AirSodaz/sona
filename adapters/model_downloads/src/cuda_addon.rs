use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;

use crate::downloads::{
    DownloadClient, DownloadError, DownloadFileOperation, sha256_file, temporary_download_path,
};
use crate::mirror::{DownloadMirror, download_candidates};
use crate::models::{ModelDownloadProgress, ModelDownloadStage};
use sona_core::runtime::cuda_addon::{CudaAddonInspection, activate_cuda_addon_directory};

const DEFAULT_CUDA_ADDON_VERSION: &str = "0.1.0";
const DEFAULT_CUDA_REPO: &str = "AirSodaz/sona";

pub fn default_cuda_addon_download_url(
    version: Option<&str>,
    repo: Option<&str>,
    target_os: &str,
) -> Option<String> {
    let ver = version.unwrap_or(DEFAULT_CUDA_ADDON_VERSION);
    let r = repo.unwrap_or(DEFAULT_CUDA_REPO);
    let tag = format!("cuda-addon-v{ver}");

    match target_os {
        "windows" => Some(format!(
            "https://github.com/{r}/releases/download/{tag}/sona-cuda-addon-v{ver}-windows-x64.tar.gz"
        )),
        "linux" => Some(format!(
            "https://github.com/{r}/releases/download/{tag}/sona-cuda-addon-v{ver}-linux-x64.tar.gz"
        )),
        _ => None,
    }
}

pub async fn download_and_install_cuda_addon<F>(
    client: &DownloadClient,
    target_dir: &Path,
    download_url: &str,
    expected_sha256: Option<&str>,
    mirror: DownloadMirror,
    cancel_notify: Arc<Notify>,
    on_progress: F,
) -> Result<CudaAddonInspection, DownloadError>
where
    F: FnMut(ModelDownloadProgress) + Send + 'static,
{
    let candidates = download_candidates(download_url, mirror, None);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_archive_path = target_dir
        .parent()
        .unwrap_or(target_dir)
        .join(format!(".cuda-addon-{timestamp}.tar.gz"));
    let temp_download_path = temporary_download_path(&temp_archive_path);

    std::fs::create_dir_all(target_dir.parent().unwrap_or(target_dir)).map_err(|error| {
        DownloadError::file_system(
            DownloadFileOperation::CreateInstallDirectory,
            target_dir,
            error.to_string(),
        )
    })?;

    let progress = Arc::new(Mutex::new(on_progress));
    let mut last_error = None;
    let mut downloaded = false;

    for candidate_url in candidates {
        let notify = cancel_notify.clone();
        let download_progress = progress.clone();
        let download_result = client
            .download_file(
                &candidate_url,
                &temp_download_path,
                notify,
                Some(Box::new(move |downloaded, total| {
                    report_progress(
                        &download_progress,
                        ModelDownloadProgress {
                            stage: ModelDownloadStage::Downloading,
                            downloaded_bytes: downloaded,
                            total_bytes: total,
                        },
                    );
                })),
            )
            .await;
        match download_result {
            Ok(()) => {
                downloaded = true;
                break;
            }
            Err(error) => {
                last_error = Some(error);
            }
        }
    }

    if !downloaded {
        let _ = tokio::fs::remove_file(&temp_download_path).await;
        return Err(last_error.unwrap_or(DownloadError::Cancelled));
    }

    if let Err(error) = tokio::fs::rename(&temp_download_path, &temp_archive_path).await {
        let _ = tokio::fs::remove_file(&temp_download_path).await;
        return Err(DownloadError::file_system(
            DownloadFileOperation::Publish,
            &temp_archive_path,
            error.to_string(),
        ));
    }

    if let Some(expected_sha) = expected_sha256 {
        report_progress(
            &progress,
            ModelDownloadProgress {
                stage: ModelDownloadStage::Verifying,
                downloaded_bytes: 0,
                total_bytes: 0,
            },
        );
        let actual_sha = match sha256_file(&temp_archive_path).await {
            Ok(sha) => sha,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp_archive_path).await;
                return Err(error);
            }
        };
        if !actual_sha.eq_ignore_ascii_case(expected_sha) {
            let _ = tokio::fs::remove_file(&temp_archive_path).await;
            return Err(DownloadError::HashMismatch {
                path: temp_archive_path,
                expected: expected_sha.to_string(),
                actual: actual_sha,
            });
        }
    }
    report_progress(
        &progress,
        ModelDownloadProgress {
            stage: ModelDownloadStage::Installing,
            downloaded_bytes: 0,
            total_bytes: 0,
        },
    );
    let extract_archive_path = temp_archive_path.clone();
    let extract_target_dir = target_dir.to_path_buf();
    let staging_dir = target_dir
        .parent()
        .unwrap_or(target_dir)
        .join(format!(".cuda-staging-{timestamp}"));
    let staging_dir_clone = staging_dir.clone();

    let extract_res = tokio::task::spawn_blocking(move || {
        extract_and_stage_cuda_addon(
            &extract_archive_path,
            &staging_dir_clone,
            &extract_target_dir,
        )
    })
    .await;

    let _ = tokio::fs::remove_file(&temp_archive_path).await;
    let _ = tokio::fs::remove_dir_all(&staging_dir).await;

    match extract_res {
        Ok(res) => res?,
        Err(error) => {
            return Err(DownloadError::file_system(
                DownloadFileOperation::ExtractArchive,
                target_dir,
                format!("Join error during extraction: {error}"),
            ));
        }
    }
    let inspection = activate_cuda_addon_directory(target_dir).map_err(|error| {
        DownloadError::file_system(
            DownloadFileOperation::Publish,
            target_dir,
            error.to_string(),
        )
    })?;

    Ok(inspection)
}

fn extract_and_stage_cuda_addon(
    archive_path: &Path,
    staging_dir: &Path,
    target_dir: &Path,
) -> Result<(), DownloadError> {
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(staging_dir);
    }
    std::fs::create_dir_all(staging_dir).map_err(|error| {
        DownloadError::file_system(
            DownloadFileOperation::CreateInstallDirectory,
            staging_dir,
            error.to_string(),
        )
    })?;

    let file = File::open(archive_path).map_err(|error| {
        DownloadError::file_system(
            DownloadFileOperation::OpenArchive,
            archive_path,
            error.to_string(),
        )
    })?;

    let mut magic = [0u8; 3];
    let is_gzip = {
        use std::io::Read;
        let mut f = File::open(archive_path).map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::OpenArchive,
                archive_path,
                error.to_string(),
            )
        })?;
        f.read_exact(&mut magic).is_ok() && magic[0] == 0x1f && magic[1] == 0x8b
    };

    let file_name = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    if is_gzip || file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(staging_dir).map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::ExtractArchive,
                staging_dir,
                error.to_string(),
            )
        })?;
    } else if magic == *b"BZh" || file_name.ends_with(".tar.bz2") {
        let tar = bzip2::read::BzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(staging_dir).map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::ExtractArchive,
                staging_dir,
                error.to_string(),
            )
        })?;
    } else {
        return Err(DownloadError::file_system(
            DownloadFileOperation::ExtractArchive,
            archive_path,
            format!("Unsupported archive format: {file_name}"),
        ));
    }

    std::fs::create_dir_all(target_dir).map_err(|error| {
        DownloadError::file_system(
            DownloadFileOperation::CreateInstallDirectory,
            target_dir,
            error.to_string(),
        )
    })?;

    copy_dir_all(staging_dir, target_dir).map_err(|error| {
        DownloadError::file_system(
            DownloadFileOperation::Publish,
            target_dir,
            error.to_string(),
        )
    })?;

    let _ = std::fs::remove_dir_all(staging_dir);
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn report_progress<F>(progress: &Arc<Mutex<F>>, value: ModelDownloadProgress)
where
    F: FnMut(ModelDownloadProgress) + Send + 'static,
{
    if let Ok(mut lock) = progress.lock() {
        (*lock)(value);
    }
}
