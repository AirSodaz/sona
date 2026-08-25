use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::downloads::{
    DownloadClient, DownloadError, DownloadFileOperation, publish_download_file, sha256_file,
    temporary_download_path,
};
use sona_core::models::downloads::ResolvedModelDownload;

const MAX_ARCHIVE_ENTRIES: usize = 20_000;
const MAX_ARCHIVE_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelDownloadStage {
    Downloading,
    Verifying,
    Installing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelDownloadProgress {
    pub stage: ModelDownloadStage,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

pub fn installed_model_is_complete(resolved: &ResolvedModelDownload) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(&resolved.install_path) else {
        return false;
    };
    if metadata.file_type().is_symlink() {
        return false;
    }
    if resolved.model.is_multi_file() {
        return metadata.is_dir()
            && resolved.artifacts.iter().all(|artifact| {
                std::fs::symlink_metadata(&artifact.install_path).is_ok_and(|metadata| {
                    metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() > 0
                })
            });
    }
    if resolved.model.is_archive() {
        return metadata.is_dir()
            && std::fs::read_dir(&resolved.install_path)
                .is_ok_and(|entries| entries.flatten().next().is_some());
    }
    resolved
        .model
        .install_path_is_complete(true, metadata.is_file(), metadata.len())
}

pub async fn installed_model_is_valid(
    resolved: &ResolvedModelDownload,
) -> Result<bool, DownloadError> {
    if !installed_model_is_complete(resolved) {
        return Ok(false);
    }

    if resolved.model.is_archive() {
        return Ok(true);
    }

    if resolved.model.is_multi_file() {
        for artifact in &resolved.artifacts {
            let Some(expected_sha) = artifact.sha256.as_deref() else {
                continue;
            };
            let actual_sha = sha256_file(&artifact.install_path).await?;
            if !actual_sha.eq_ignore_ascii_case(expected_sha) {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    let Some(expected_sha) = resolved
        .artifacts
        .first()
        .and_then(|artifact| artifact.sha256.as_deref())
    else {
        return Ok(true);
    };
    let actual_sha = sha256_file(&resolved.install_path).await?;
    Ok(actual_sha.eq_ignore_ascii_case(expected_sha))
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
    mut on_progress: F,
) -> Result<PathBuf, DownloadError>
where
    F: FnMut(u64, u64) + Send + 'static,
{
    let notify = Arc::new(tokio::sync::Notify::new());
    let notify_clone = notify.clone();
    let ctrl_c_task = tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            notify_clone.notify_one();
        }
    });

    let result = download_model_with_cancel(resolved, notify, move |progress| {
        if progress.stage == ModelDownloadStage::Downloading {
            on_progress(progress.downloaded_bytes, progress.total_bytes);
        }
    })
    .await;

    ctrl_c_task.abort();
    result
}

pub async fn download_model_with_cancel<F>(
    resolved: &ResolvedModelDownload,
    cancel: Arc<tokio::sync::Notify>,
    on_progress: F,
) -> Result<PathBuf, DownloadError>
where
    F: FnMut(ModelDownloadProgress) + Send + 'static,
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
    let install_lock = InstallLock::acquire(&resolved.install_path)?;

    let progress = Arc::new(Mutex::new(on_progress));
    if resolved.model.is_multi_file() {
        return download_multi_file_model(resolved, cancel, progress, install_lock).await;
    }
    let Some(primary_artifact) = resolved.artifacts.first() else {
        drop(install_lock);
        return Err(DownloadError::file_system(
            DownloadFileOperation::InspectInstall,
            &resolved.install_path,
            format!("Model '{}' has no download artifacts", resolved.model.id),
        ));
    };
    let primary_url = primary_artifact.url.clone();
    let expected_sha = primary_artifact.sha256.clone();
    let download_progress = progress.clone();
    let temp_download_path = temporary_download_path(&resolved.download_path);

    let client = DownloadClient::try_new()?;
    let result = client
        .download_file(
            &primary_url,
            &temp_download_path,
            cancel,
            Some(Box::new(move |downloaded_bytes, total_bytes| {
                report_progress(
                    &download_progress,
                    ModelDownloadProgress {
                        stage: ModelDownloadStage::Downloading,
                        downloaded_bytes,
                        total_bytes,
                    },
                );
            })),
        )
        .await;
    result?;

    report_progress(
        &progress,
        ModelDownloadProgress {
            stage: ModelDownloadStage::Verifying,
            downloaded_bytes: 0,
            total_bytes: 0,
        },
    );

    if let Some(expected_sha) = expected_sha {
        let actual_sha = match sha256_file(&temp_download_path).await {
            Ok(sha) => sha,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp_download_path).await;
                return Err(error);
            }
        };
        if !actual_sha.eq_ignore_ascii_case(&expected_sha) {
            let _ = tokio::fs::remove_file(&temp_download_path).await;
            return Err(DownloadError::HashMismatch {
                path: temp_download_path,
                expected: expected_sha.clone(),
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

    publish_download_file(&temp_download_path, &resolved.download_path).await?;

    if resolved.model.is_archive() {
        let install_result = install_tar_bz2_archive(resolved, install_lock).await;
        let remove_result = tokio::fs::remove_file(&resolved.download_path).await;
        install_result?;
        remove_result.map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::RemoveArchive,
                &resolved.download_path,
                error.to_string(),
            )
        })?;
    }

    Ok(resolved.install_path.clone())
}

async fn download_multi_file_model<F>(
    resolved: &ResolvedModelDownload,
    cancel: Arc<tokio::sync::Notify>,
    progress: Arc<Mutex<F>>,
    install_lock: InstallLock,
) -> Result<PathBuf, DownloadError>
where
    F: FnMut(ModelDownloadProgress) + Send + 'static,
{
    let staging_path = staging_install_path(&resolved.install_path);
    if std::fs::symlink_metadata(&staging_path)
        .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_dir())
    {
        remove_model_install_path(&staging_path)?;
    }
    tokio::fs::create_dir_all(&staging_path)
        .await
        .map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::CreateInstallDirectory,
                &staging_path,
                error.to_string(),
            )
        })?;

    let expected_total = resolved
        .artifacts
        .iter()
        .map(|artifact| artifact.size_bytes.unwrap_or(0))
        .sum::<u64>();
    let client = DownloadClient::try_new()?;
    let mut completed_bytes = 0_u64;

    for artifact in &resolved.artifacts {
        let staged_path = staging_path.join(&artifact.filename);
        let artifact_size = artifact.size_bytes.unwrap_or(0);
        if staged_artifact_is_valid(&staged_path, artifact.sha256.as_deref()).await? {
            completed_bytes = completed_bytes.saturating_add(artifact_size);
            report_progress(
                &progress,
                ModelDownloadProgress {
                    stage: ModelDownloadStage::Downloading,
                    downloaded_bytes: completed_bytes,
                    total_bytes: expected_total,
                },
            );
            continue;
        }

        let temp_path = temporary_download_path(&staged_path);
        let artifact_progress = progress.clone();
        let completed_before_artifact = completed_bytes;
        client
            .download_file(
                &artifact.url,
                &temp_path,
                cancel.clone(),
                Some(Box::new(move |downloaded_bytes, _| {
                    report_progress(
                        &artifact_progress,
                        ModelDownloadProgress {
                            stage: ModelDownloadStage::Downloading,
                            downloaded_bytes: completed_before_artifact
                                .saturating_add(downloaded_bytes.min(artifact_size)),
                            total_bytes: expected_total,
                        },
                    );
                })),
            )
            .await?;

        report_progress(
            &progress,
            ModelDownloadProgress {
                stage: ModelDownloadStage::Verifying,
                downloaded_bytes: completed_bytes,
                total_bytes: expected_total,
            },
        );
        crate::downloads::complete_download_file(&temp_path, &staged_path, artifact.sha256.as_deref())
            .await?;
        completed_bytes = completed_bytes.saturating_add(artifact_size);
    }

    report_progress(
        &progress,
        ModelDownloadProgress {
            stage: ModelDownloadStage::Installing,
            downloaded_bytes: expected_total,
            total_bytes: expected_total,
        },
    );

    remove_model_install_path(&resolved.install_path)?;
    tokio::fs::rename(&staging_path, &resolved.install_path)
        .await
        .map_err(|error| {
            DownloadError::file_system_with_target(
                DownloadFileOperation::Publish,
                &staging_path,
                &resolved.install_path,
                error.to_string(),
            )
        })?;
    drop(install_lock);

    Ok(resolved.install_path.clone())
}

async fn staged_artifact_is_valid(
    path: &Path,
    expected_sha256: Option<&str>,
) -> Result<bool, DownloadError> {
    let Ok(metadata) = tokio::fs::symlink_metadata(path).await else {
        return Ok(false);
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() == 0 {
        return Ok(false);
    }
    let Some(expected_sha256) = expected_sha256 else {
        return Ok(true);
    };
    Ok(sha256_file(path)
        .await?
        .eq_ignore_ascii_case(expected_sha256))
}

struct InstallLock {
    file: Option<std::fs::File>,
    path: PathBuf,
}

impl InstallLock {
    fn acquire(install_path: &Path) -> Result<Self, DownloadError> {
        let mut path = install_path.as_os_str().to_os_string();
        path.push(".install.lock");
        let path = PathBuf::from(path);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| {
                DownloadError::file_system(
                    DownloadFileOperation::AcquireInstallLock,
                    &path,
                    error.to_string(),
                )
            })?;
        use fs3::FileExt;
        file.try_lock_exclusive()
            .map_err(|_| DownloadError::AlreadyInProgress)?;
        Ok(Self {
            file: Some(file),
            path,
        })
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

fn report_progress<F>(progress: &Arc<Mutex<F>>, event: ModelDownloadProgress)
where
    F: FnMut(ModelDownloadProgress),
{
    if let Ok(mut callback) = progress.lock() {
        callback(event);
    }
}

async fn install_tar_bz2_archive(
    resolved: &ResolvedModelDownload,
    install_lock: InstallLock,
) -> Result<(), DownloadError> {
    let archive_path = resolved.download_path.clone();
    let install_path = resolved.install_path.clone();
    let staging_path = staging_install_path(&install_path);
    remove_model_install_path(&staging_path)?;
    tokio::fs::create_dir_all(&staging_path)
        .await
        .map_err(|error| {
            DownloadError::file_system(
                DownloadFileOperation::ExtractArchive,
                &staging_path,
                error.to_string(),
            )
        })?;

    let expected_root = install_path
        .file_name()
        .ok_or_else(|| archive_error(&archive_path, &install_path, "Invalid install path"))?
        .to_owned();
    let extraction_root = expected_root.clone();
    let extraction_install_path = install_path.clone();
    let join_archive_path = archive_path.clone();
    let join_staging_path = staging_path.clone();

    tokio::task::spawn_blocking(move || {
        let _install_lock = install_lock;
        let result = (|| -> Result<(), DownloadError> {
            let file = std::fs::File::open(&archive_path).map_err(|error| {
                DownloadError::file_system_with_target(
                    DownloadFileOperation::OpenArchive,
                    &archive_path,
                    &staging_path,
                    error.to_string(),
                )
            })?;
            let buffered = std::io::BufReader::new(file);
            let tar = bzip2::read::BzDecoder::new(buffered);
            let mut archive = tar::Archive::new(tar);
            archive.set_preserve_permissions(false);
            archive.set_unpack_xattrs(false);
            archive.set_preserve_mtime(false);

            let mut entry_count = 0_usize;
            let mut extracted_bytes = 0_u64;
            let entries = archive
                .entries()
                .map_err(|error| archive_error(&archive_path, &staging_path, error))?;
            for entry in entries {
                let mut entry =
                    entry.map_err(|error| archive_error(&archive_path, &staging_path, error))?;
                entry_count += 1;
                if entry_count > MAX_ARCHIVE_ENTRIES {
                    return Err(archive_error(
                        &archive_path,
                        &staging_path,
                        "Model archive has too many entries",
                    ));
                }

                let entry_type = entry.header().entry_type();
                if !entry_type.is_file() && !entry_type.is_dir() {
                    return Err(archive_error(
                        &archive_path,
                        &staging_path,
                        "Model archive contains an unsupported entry",
                    ));
                }
                let entry_path = entry
                    .path()
                    .map_err(|error| archive_error(&archive_path, &staging_path, error))?;
                validate_archive_path(&entry_path, &extraction_root)
                    .map_err(|error| archive_error(&archive_path, &staging_path, error))?;

                if entry_type.is_file() {
                    let size = entry
                        .header()
                        .size()
                        .map_err(|error| archive_error(&archive_path, &staging_path, error))?;
                    if size > MAX_ARCHIVE_FILE_BYTES {
                        return Err(archive_error(
                            &archive_path,
                            &staging_path,
                            "Model archive contains an oversized file",
                        ));
                    }
                    extracted_bytes = extracted_bytes.checked_add(size).ok_or_else(|| {
                        archive_error(&archive_path, &staging_path, "Model archive is too large")
                    })?;
                    if extracted_bytes > MAX_ARCHIVE_TOTAL_BYTES {
                        return Err(archive_error(
                            &archive_path,
                            &staging_path,
                            "Model archive is too large",
                        ));
                    }
                }

                if !entry
                    .unpack_in(&staging_path)
                    .map_err(|error| archive_error(&archive_path, &staging_path, error))?
                {
                    return Err(archive_error(
                        &archive_path,
                        &staging_path,
                        "Model archive contains an invalid path",
                    ));
                }
            }

            let staged_install = staging_path.join(&extraction_root);
            if !std::fs::symlink_metadata(&staged_install).is_ok_and(|metadata| metadata.is_dir()) {
                return Err(archive_error(
                    &archive_path,
                    &extraction_install_path,
                    "Model archive did not contain the expected install directory",
                ));
            }

            remove_model_install_path(&extraction_install_path)?;
            std::fs::rename(&staged_install, &extraction_install_path)
                .map_err(|error| archive_error(&archive_path, &extraction_install_path, error))?;
            Ok(())
        })();
        let _ = remove_model_install_path(&staging_path);
        result
    })
    .await
    .map_err(|error| {
        DownloadError::file_system_with_target(
            DownloadFileOperation::ExtractArchive,
            join_archive_path,
            &join_staging_path,
            format!("Failed to join extraction task: {error}"),
        )
    })?
}

fn staging_install_path(install_path: &Path) -> PathBuf {
    let mut path = install_path.as_os_str().to_os_string();
    path.push(".installing");
    PathBuf::from(path)
}

fn validate_archive_path(path: &Path, expected_root: &std::ffi::OsStr) -> io::Result<()> {
    if path.as_os_str().is_empty() || path.to_string_lossy().contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid archive path",
        ));
    }
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(root)) if root == expected_root => {}
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Archive entry is outside the expected model directory",
            ));
        }
    }
    if components.any(|component| !matches!(component, Component::Normal(_))) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid archive path",
        ));
    }
    Ok(())
}

fn archive_error(
    archive_path: &Path,
    target_path: &Path,
    error: impl std::fmt::Display,
) -> DownloadError {
    DownloadError::file_system_with_target(
        DownloadFileOperation::ExtractArchive,
        archive_path,
        target_path,
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::models::downloads::resolve_model_download;
    use std::io::Cursor;

    fn write_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let encoder = bzip2::write::BzEncoder::new(file, bzip2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        for (entry_path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::file());
            header.set_mode(0o644);
            header.set_size(contents.len() as u64);
            header.set_cksum();
            archive
                .append_data(&mut header, entry_path, Cursor::new(*contents))
                .unwrap();
        }
        let encoder = archive.into_inner().unwrap();
        encoder.finish().unwrap();
    }

    fn archive_download(models_dir: &Path) -> ResolvedModelDownload {
        resolve_model_download(
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
            models_dir,
        )
        .unwrap()
    }

    #[test]
    fn install_lock_rejects_concurrent_model_operations_and_cleans_up() {
        let temp = tempfile::tempdir().unwrap();
        let install_path = temp.path().join("model");
        let lock_path = temp.path().join("model.install.lock");

        let first = InstallLock::acquire(&install_path).unwrap();
        assert!(matches!(
            InstallLock::acquire(&install_path),
            Err(DownloadError::AlreadyInProgress)
        ));
        drop(first);

        assert!(!lock_path.exists());
        InstallLock::acquire(&install_path).unwrap();
    }

    #[tokio::test]
    async fn archive_install_publishes_only_the_expected_model_directory() {
        let temp = tempfile::tempdir().unwrap();
        let models_dir = temp.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();
        let resolved = archive_download(&models_dir);
        write_archive(
            &resolved.download_path,
            &[(
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/model.int8.onnx",
                b"model",
            )],
        );

        let install_lock = InstallLock::acquire(&resolved.install_path).unwrap();
        install_tar_bz2_archive(&resolved, install_lock)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read(resolved.install_path.join("model.int8.onnx")).unwrap(),
            b"model"
        );
        assert!(!staging_install_path(&resolved.install_path).exists());
    }

    #[tokio::test]
    async fn archive_install_rejects_entries_outside_the_expected_model_directory() {
        let temp = tempfile::tempdir().unwrap();
        let models_dir = temp.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();
        let resolved = archive_download(&models_dir);
        std::fs::create_dir_all(&resolved.install_path).unwrap();
        std::fs::write(resolved.install_path.join("existing.onnx"), b"existing").unwrap();
        write_archive(
            &resolved.download_path,
            &[("other-model/model.onnx", b"bad")],
        );

        let install_lock = InstallLock::acquire(&resolved.install_path).unwrap();
        let error = install_tar_bz2_archive(&resolved, install_lock)
            .await
            .unwrap_err();

        assert!(matches!(error, DownloadError::FileSystem(_)));
        assert_eq!(
            std::fs::read(resolved.install_path.join("existing.onnx")).unwrap(),
            b"existing"
        );
        assert!(!models_dir.join("other-model").exists());
        assert!(!staging_install_path(&resolved.install_path).exists());
    }

    #[tokio::test]
    async fn archive_install_rejects_links_without_replacing_an_existing_install() {
        let temp = tempfile::tempdir().unwrap();
        let models_dir = temp.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();
        let resolved = archive_download(&models_dir);
        std::fs::create_dir_all(&resolved.install_path).unwrap();
        std::fs::write(resolved.install_path.join("existing.onnx"), b"existing").unwrap();

        let file = std::fs::File::create(&resolved.download_path).unwrap();
        let encoder = bzip2::write::BzEncoder::new(file, bzip2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::symlink());
        header.set_mode(0o777);
        header.set_size(0);
        header.set_link_name("../outside").unwrap();
        header.set_cksum();
        archive
            .append_data(
                &mut header,
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/link",
                Cursor::new([]),
            )
            .unwrap();
        let encoder = archive.into_inner().unwrap();
        encoder.finish().unwrap();

        let install_lock = InstallLock::acquire(&resolved.install_path).unwrap();
        let error = install_tar_bz2_archive(&resolved, install_lock)
            .await
            .unwrap_err();

        assert!(matches!(error, DownloadError::FileSystem(_)));
        assert_eq!(
            std::fs::read(resolved.install_path.join("existing.onnx")).unwrap(),
            b"existing"
        );
        assert!(!staging_install_path(&resolved.install_path).exists());
    }
}
