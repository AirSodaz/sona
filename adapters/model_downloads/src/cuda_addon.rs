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
use sona_core::runtime::cuda_addon::{
    CudaAddonManifest, EXPECTED_CUDA_ADDON_LLAMA_CPP_REF, EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION,
};

const DEFAULT_CUDA_ADDON_VERSION: &str = "0.3.0";
const DEFAULT_CUDA_REPO: &str = "AirSodaz/sona";
const CUDA_ADDON_RELEASES_API: &str =
    "https://api.github.com/repos/AirSodaz/sona/releases?per_page=100";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibleCudaAddonRelease {
    pub version: String,
    pub cuda_version: String,
    pub download_url: String,
    pub sha256: String,
}

pub async fn find_compatible_cuda_addon_release(
    client: &DownloadClient,
    target_os: &str,
    target_arch: &str,
) -> Result<CompatibleCudaAddonRelease, String> {
    find_compatible_cuda_addon_release_from_api(
        client,
        CUDA_ADDON_RELEASES_API,
        target_os,
        target_arch,
    )
    .await
}

async fn find_compatible_cuda_addon_release_from_api(
    client: &DownloadClient,
    api_url: &str,
    target_os: &str,
    target_arch: &str,
) -> Result<CompatibleCudaAddonRelease, String> {
    let releases_body = client
        .http_client()
        .get(api_url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| format!("Failed to query CUDA addon releases: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CUDA addon release query failed: {error}"))?
        .text()
        .await
        .map_err(|error| format!("Failed to read CUDA addon release response: {error}"))?;
    let releases: Vec<serde_json::Value> = serde_json::from_str(&releases_body)
        .map_err(|error| format!("Invalid CUDA addon release response: {error}"))?;

    for release in releases {
        if release.get("draft").and_then(serde_json::Value::as_bool) == Some(true)
            || release
                .get("prerelease")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            || !release
                .get("tag_name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|tag| tag.starts_with("cuda-addon-v"))
        {
            continue;
        }
        let Some(asset_url) = release
            .get("assets")
            .and_then(serde_json::Value::as_array)
            .and_then(|assets| {
                assets.iter().find_map(|asset| {
                    if asset.get("name")?.as_str()? == "cuda-addon-manifest.json" {
                        asset.get("browser_download_url")?.as_str()
                    } else {
                        None
                    }
                })
            })
        else {
            continue;
        };
        let response = match client
            .http_client()
            .get(asset_url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            Ok(response) => match response.error_for_status() {
                Ok(response) => response,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        let manifest_body = match response.text().await {
            Ok(body) => body,
            Err(_) => continue,
        };
        let manifest: CudaAddonManifest = match serde_json::from_str(&manifest_body) {
            Ok(manifest) => manifest,
            Err(_) => continue,
        };
        if manifest.schema_version != 1
            || release.get("tag_name").and_then(serde_json::Value::as_str)
                != Some(format!("cuda-addon-v{}", manifest.addon_version).as_str())
            || manifest.llama_cpp_ref.as_deref() != Some(EXPECTED_CUDA_ADDON_LLAMA_CPP_REF)
            || manifest.sherpa_onnx_version.as_deref()
                != Some(EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION)
        {
            continue;
        }
        let platform_key = format!("{target_os}-{target_arch}");
        let Some(platform) = manifest.platforms.get(&platform_key) else {
            continue;
        };
        if !platform.url.starts_with("https://")
            || platform.sha256.len() != 64
            || !platform.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            continue;
        }
        return Ok(CompatibleCudaAddonRelease {
            version: manifest.addon_version,
            cuda_version: manifest.cuda_version,
            download_url: platform.url.clone(),
            sha256: platform.sha256.clone(),
        });
    }
    Err(format!(
        "No compatible CUDA addon release found for {target_os}-{target_arch} (llama.cpp {}, sherpa-onnx {})",
        EXPECTED_CUDA_ADDON_LLAMA_CPP_REF, EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path as AxumPath, State};
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::{Value, json};

    async fn releases(State(manifest_url): State<String>) -> Json<Value> {
        Json(json!([
            {
                "tag_name": "cuda-addon-v0.4.0",
                "draft": false,
                "prerelease": false,
                "assets": [{ "name": "cuda-addon-manifest.json", "browser_download_url": format!("{manifest_url}/mismatch") }]
            },
            {
                "tag_name": "cuda-addon-v0.3.0",
                "draft": false,
                "prerelease": false,
                "assets": [{ "name": "cuda-addon-manifest.json", "browser_download_url": format!("{manifest_url}/compatible") }]
            }
        ]))
    }

    async fn manifest(AxumPath(kind): AxumPath<String>) -> Json<Value> {
        let llama_ref = if kind == "compatible" {
            EXPECTED_CUDA_ADDON_LLAMA_CPP_REF
        } else {
            "old-ref"
        };
        Json(json!({
            "schemaVersion": 1,
            "addonVersion": if kind == "compatible" { "0.3.0" } else { "0.4.0" },
            "cudaVersion": "12.4",
            "llamaCppRef": llama_ref,
            "sherpaOnnxVersion": EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION,
            "platforms": {
                "windows-x86_64": {
                    "filename": "sona-cuda-addon.tar.gz",
                    "url": "https://example.org/sona-cuda-addon.tar.gz",
                    "sha256": "a".repeat(64),
                    "sizeBytes": 100
                }
            }
        }))
    }

    #[tokio::test]
    async fn selects_latest_release_matching_client_dependencies_and_platform() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let manifest_url = format!("http://{address}/manifest");
        let app = Router::new()
            .route("/releases", get(releases))
            .route("/manifest/{kind}", get(manifest))
            .with_state(manifest_url);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let release = find_compatible_cuda_addon_release_from_api(
            &DownloadClient::new(),
            &format!("http://{address}/releases"),
            "windows",
            "x86_64",
        )
        .await
        .unwrap();

        assert_eq!(release.version, "0.3.0");
        assert_eq!(release.cuda_version, "12.4");
        assert_eq!(
            release.download_url,
            "https://example.org/sona-cuda-addon.tar.gz"
        );
        assert_eq!(release.sha256, "a".repeat(64));
    }
}

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
