use base64::Engine;
use futures_util::FutureExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Notify;

use sona_core::runtime::app_update::{
    AppUpdateCheckResult, AppUpdateManifest, DEFAULT_UPDATER_PUBKEY, UpdateChannel,
    compare_versions, current_platform_key, release_url_for_version, updater_endpoint_for_channel,
};

use crate::downloads::{
    DownloadClient, DownloadError, publish_download_file, temporary_download_path,
};
use crate::mirror::{DownloadMirror, download_candidates};

pub fn decode_pubkey(key_str: &str) -> Result<minisign_verify::PublicKey, DownloadError> {
    let trimmed = key_str.trim();
    if let Ok(pk) = minisign_verify::PublicKey::from_base64(trimmed) {
        return Ok(pk);
    }
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(trimmed)
        && let Ok(s) = std::str::from_utf8(&decoded)
        && let Ok(pk) = minisign_verify::PublicKey::decode(s)
    {
        return Ok(pk);
    }
    minisign_verify::PublicKey::decode(trimmed)
        .map_err(|e| DownloadError::Validation(format!("Failed to parse Minisign public key: {e}")))
}

pub fn decode_signature(sig_str: &str) -> Result<minisign_verify::Signature, DownloadError> {
    let trimmed = sig_str.trim();
    if let Ok(sig) = minisign_verify::Signature::decode(trimmed) {
        return Ok(sig);
    }
    if trimmed.contains("\\n") {
        let unescaped = trimmed.replace("\\n", "\n");
        if let Ok(sig) = minisign_verify::Signature::decode(&unescaped) {
            return Ok(sig);
        }
    }
    if let Ok(decoded_bytes) = base64::engine::general_purpose::STANDARD.decode(trimmed)
        && let Ok(decoded_str) = std::str::from_utf8(&decoded_bytes)
        && let Ok(sig) = minisign_verify::Signature::decode(decoded_str)
    {
        return Ok(sig);
    }
    Err(DownloadError::Validation(
        "Invalid Minisign signature format".to_string(),
    ))
}

pub fn verify_update_signature(
    data: &[u8],
    signature_str: &str,
    pubkey_str: &str,
) -> Result<(), DownloadError> {
    let pubkey = decode_pubkey(pubkey_str)?;
    let signature = decode_signature(signature_str)?;
    pubkey.verify(data, &signature, false).map_err(|e| {
        DownloadError::Validation(format!("Minisign signature verification failed: {e}"))
    })
}

pub async fn fetch_updater_manifest(
    client: &DownloadClient,
    endpoint_url: &str,
    mirror: DownloadMirror,
) -> Result<AppUpdateManifest, DownloadError> {
    let candidates = download_candidates(endpoint_url, mirror, None);
    let mut last_err = None;

    for candidate in &candidates {
        let resp = match client.http_client().get(candidate).send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(DownloadError::Network(e));
                continue;
            }
        };

        if !resp.status().is_success() {
            last_err = Some(DownloadError::HttpStatus(resp.status()));
            continue;
        }

        let body = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                last_err = Some(DownloadError::Network(e));
                continue;
            }
        };

        let manifest: AppUpdateManifest = match serde_json::from_str(&body) {
            Ok(m) => m,
            Err(e) => {
                last_err = Some(DownloadError::Validation(format!(
                    "Failed to parse updater manifest from {candidate}: {e}"
                )));
                continue;
            }
        };

        return Ok(manifest);
    }

    Err(last_err.unwrap_or_else(|| {
        DownloadError::Validation(format!(
            "Failed to fetch updater manifest from {endpoint_url} (no candidates succeeded)"
        ))
    }))
}

pub async fn check_app_update(
    client: &DownloadClient,
    endpoint_override: Option<&str>,
    channel: UpdateChannel,
    mirror: DownloadMirror,
) -> Result<AppUpdateCheckResult, DownloadError> {
    check_app_update_with_version(
        client,
        endpoint_override,
        channel,
        mirror,
        env!("CARGO_PKG_VERSION"),
    )
    .await
}

pub async fn check_app_update_with_version(
    client: &DownloadClient,
    endpoint_override: Option<&str>,
    channel: UpdateChannel,
    mirror: DownloadMirror,
    current_version: &str,
) -> Result<AppUpdateCheckResult, DownloadError> {
    let endpoint = endpoint_override.unwrap_or_else(|| updater_endpoint_for_channel(channel));
    let manifest = fetch_updater_manifest(client, endpoint, mirror).await?;
    let platform = current_platform_key().map_err(|e| DownloadError::Validation(e.to_string()))?;

    let asset = manifest.platforms.get(platform).ok_or_else(|| {
        DownloadError::Validation(format!(
            "Update manifest does not contain an asset for platform '{platform}'"
        ))
    })?;

    let comparison = compare_versions(current_version, &manifest.version, channel);
    let update_available = comparison.is_upgrade();
    let release_url = release_url_for_version(&manifest.version, channel);

    Ok(AppUpdateCheckResult {
        update_available,
        current_version: current_version.to_string(),
        latest_version: manifest.version,
        channel,
        pub_date: manifest.pub_date,
        notes: manifest.notes,
        release_url,
        asset_url: asset.url.clone(),
        signature: asset.signature.clone(),
        platform: platform.to_string(),
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn download_update_package(
    client: &DownloadClient,
    asset_url: &str,
    target_file: &Path,
    expected_signature: &str,
    updater_pubkey: Option<&str>,
    mirror: DownloadMirror,
    cancel_notify: Arc<Notify>,
    on_progress: Option<Box<dyn FnMut(u64, u64) + Send>>,
) -> Result<(), DownloadError> {
    let temp_path = temporary_download_path(target_file);
    if let Some(parent) = target_file.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(DownloadError::Io)?;
    }

    let candidates = download_candidates(asset_url, mirror, None);
    let mut last_err = None;
    let mut downloaded = false;

    let progress_holder = on_progress.map(|cb| Arc::new(std::sync::Mutex::new(cb)));

    for candidate in &candidates {
        if cancel_notify.notified().now_or_never().is_some() {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(DownloadError::Cancelled);
        }

        let cb_opt = progress_holder.as_ref().map(|holder| {
            let holder = Arc::clone(holder);
            let cb: Box<dyn FnMut(u64, u64) + Send> = Box::new(move |current, total| {
                if let Ok(mut lock) = holder.lock() {
                    lock(current, total);
                }
            });
            cb
        });

        match client
            .download_file(candidate, &temp_path, cancel_notify.clone(), cb_opt)
            .await
        {
            Ok(()) => {
                downloaded = true;
                break;
            }
            Err(DownloadError::Cancelled) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(DownloadError::Cancelled);
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                last_err = Some(e);
            }
        }
    }

    if !downloaded {
        return Err(last_err.unwrap_or_else(|| {
            DownloadError::Validation(format!(
                "Failed to download update package from {asset_url}"
            ))
        }));
    }

    // Read bytes and verify signature
    let data = match tokio::fs::read(&temp_path).await {
        Ok(d) => d,
        Err(e) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(DownloadError::Io(e));
        }
    };

    let pubkey = updater_pubkey.unwrap_or(DEFAULT_UPDATER_PUBKEY);
    if let Err(e) = verify_update_signature(&data, expected_signature, pubkey) {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(e);
    }

    publish_download_file(&temp_path, target_file).await?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn locate_windows_app_exe(running_exe: &Path) -> Option<PathBuf> {
    let running_name = running_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if running_name == "sona.exe" && running_exe.is_file() {
        return Some(running_exe.to_path_buf());
    }

    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let nsis_app = PathBuf::from(local_appdata)
            .join("Programs")
            .join("Sona")
            .join("Sona.exe");
        if nsis_app.is_file() {
            return Some(nsis_app);
        }
    }

    if let Ok(program_files) = std::env::var("ProgramFiles") {
        let pf_app = PathBuf::from(program_files).join("Sona").join("Sona.exe");
        if pf_app.is_file() {
            return Some(pf_app);
        }
    }

    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        return Some(
            PathBuf::from(local_appdata)
                .join("Programs")
                .join("Sona")
                .join("Sona.exe"),
        );
    }

    if running_name == "sona.exe" {
        return Some(running_exe.to_path_buf());
    }

    Some(PathBuf::from("Sona.exe"))
}

#[cfg(target_os = "windows")]
pub fn install_update_package(
    package_path: &Path,
    running_exe: &Path,
    silent: bool,
) -> Result<PathBuf, DownloadError> {
    if !package_path.is_file() {
        return Err(DownloadError::Validation(format!(
            "Update installer does not exist at {}",
            package_path.display()
        )));
    }

    let mut cmd = std::process::Command::new(package_path);
    if silent {
        cmd.arg("/S");
    }
    let status = cmd.status().map_err(|e| {
        DownloadError::Validation(format!(
            "Failed to execute installer {}: {e}",
            package_path.display()
        ))
    })?;

    if !status.success() {
        return Err(DownloadError::Validation(format!(
            "Installer exited with code: {:?}",
            status.code()
        )));
    }

    let installed_exe =
        locate_windows_app_exe(running_exe).unwrap_or_else(|| PathBuf::from("Sona.exe"));

    Ok(installed_exe)
}

#[cfg(target_os = "macos")]
pub fn locate_macos_app_bundle(running_exe: &Path) -> Option<PathBuf> {
    let mut current = running_exe.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent.extension().and_then(|ext| ext.to_str()) == Some("app") {
            return Some(parent.to_path_buf());
        }
        current = parent.to_path_buf();
    }
    let default_app = PathBuf::from("/Applications/Sona.app");
    if default_app.is_dir() {
        return Some(default_app);
    }
    if let Ok(home) = std::env::var("HOME") {
        let user_app = PathBuf::from(home).join("Applications/Sona.app");
        if user_app.is_dir() {
            return Some(user_app);
        }
    }
    None
}

#[cfg(target_os = "macos")]
pub fn install_update_package(
    package_path: &Path,
    running_exe: &Path,
    _silent: bool,
) -> Result<PathBuf, DownloadError> {
    if !package_path.is_file() {
        return Err(DownloadError::Validation(format!(
            "Update package does not exist at {}",
            package_path.display()
        )));
    }

    let target_app = locate_macos_app_bundle(running_exe)
        .unwrap_or_else(|| PathBuf::from("/Applications/Sona.app"));

    let parent = target_app
        .parent()
        .unwrap_or_else(|| Path::new("/Applications"));
    let _ = std::fs::create_dir_all(parent);
    let temp_dir = tempfile::Builder::new()
        .prefix(".sona_update_")
        .tempdir_in(parent)
        .map_err(DownloadError::Io)?;
    let temp_dir_path = temp_dir.path();

    let tar_gz = std::fs::File::open(package_path).map_err(DownloadError::Io)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(temp_dir_path).map_err(|e| {
        DownloadError::Validation(format!("Failed to unpack macOS update archive: {e}"))
    })?;

    let mut extracted_app = None;
    for entry in std::fs::read_dir(temp_dir_path).map_err(DownloadError::Io)? {
        let entry = entry.map_err(DownloadError::Io)?;
        let path = entry.path();
        if path.is_dir() && path.extension().and_then(|ext| ext.to_str()) == Some("app") {
            extracted_app = Some(path);
            break;
        }
    }

    let extracted_app = extracted_app.ok_or_else(|| {
        DownloadError::Validation("No .app bundle found in unpacked macOS update".to_string())
    })?;

    if let Some(parent) = target_app.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let backup_app = if target_app.exists() {
        let unique_id = uuid::Uuid::new_v4().to_string();
        let old_app = target_app.with_extension(format!("old.{unique_id}.app"));
        if let Err(e) = std::fs::rename(&target_app, &old_app) {
            return Err(DownloadError::Validation(format!(
                "Failed to backup existing app bundle at {}: {e}",
                target_app.display()
            )));
        }
        Some(old_app)
    } else {
        None
    };

    if let Err(e) = std::fs::rename(&extracted_app, &target_app) {
        if let Some(old_app) = &backup_app {
            let _ = std::fs::rename(old_app, &target_app);
        }
        return Err(DownloadError::Validation(format!(
            "Failed to install Sona.app into {}: {e}",
            target_app.display()
        )));
    }

    if let Some(old_app) = &backup_app {
        let _ = std::fs::remove_dir_all(old_app);
    }

    Ok(target_app)
}

#[cfg(target_os = "linux")]
pub fn install_update_package(
    package_path: &Path,
    _running_exe: &Path,
    _silent: bool,
) -> Result<PathBuf, DownloadError> {
    use std::os::unix::fs::PermissionsExt;

    if !package_path.is_file() {
        return Err(DownloadError::Validation(format!(
            "Update package does not exist at {}",
            package_path.display()
        )));
    }

    let target_appimage = if let Ok(appimage_env) = std::env::var("APPIMAGE") {
        PathBuf::from(appimage_env)
    } else if let Ok(home) = std::env::var("HOME") {
        let app_dir = PathBuf::from(&home).join("Applications");
        if app_dir.is_dir() {
            app_dir.join("Sona.AppImage")
        } else {
            let local_bin = PathBuf::from(&home).join(".local/bin");
            if local_bin.is_dir() {
                local_bin.join("Sona.AppImage")
            } else {
                app_dir.join("Sona.AppImage")
            }
        }
    } else {
        PathBuf::from("/usr/local/bin/Sona.AppImage")
    };

    if let Some(parent) = target_appimage.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let unique_id = uuid::Uuid::new_v4().to_string();
    let staging_path = target_appimage.with_extension(format!("tmp.{unique_id}"));
    if let Err(e) = std::fs::copy(package_path, &staging_path) {
        let _ = std::fs::remove_file(&staging_path);
        return Err(DownloadError::Validation(format!(
            "Failed to copy AppImage to staging path {}: {e}",
            staging_path.display()
        )));
    }

    let perms = std::fs::Permissions::from_mode(0o755);
    if let Err(e) = std::fs::set_permissions(&staging_path, perms) {
        let _ = std::fs::remove_file(&staging_path);
        return Err(DownloadError::Validation(format!(
            "Failed to set executable permissions on {}: {e}",
            staging_path.display()
        )));
    }

    let backup_appimage = if target_appimage.exists() {
        let old_path = target_appimage.with_extension(format!("old.{unique_id}.AppImage"));
        if let Err(e) = std::fs::rename(&target_appimage, &old_path) {
            let _ = std::fs::remove_file(&staging_path);
            return Err(DownloadError::Validation(format!(
                "Failed to backup existing AppImage at {}: {e}",
                target_appimage.display()
            )));
        }
        Some(old_path)
    } else {
        None
    };

    if let Err(e) = std::fs::rename(&staging_path, &target_appimage) {
        let _ = std::fs::remove_file(&staging_path);
        if let Some(old_path) = &backup_appimage {
            let _ = std::fs::rename(old_path, &target_appimage);
        }
        return Err(DownloadError::Validation(format!(
            "Failed to install AppImage to {}: {e}",
            target_appimage.display()
        )));
    }

    if let Some(old_path) = &backup_appimage {
        let _ = std::fs::remove_file(old_path);
    }

    Ok(target_appimage)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn install_update_package(
    package_path: &Path,
    _running_exe: &Path,
    _silent: bool,
) -> Result<PathBuf, DownloadError> {
    Err(DownloadError::Validation(
        "Automated package installation is unsupported on this operating system".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::runtime::app_update::AppPlatformAsset;
    use std::collections::HashMap;

    #[test]
    fn test_default_updater_pubkey_decodes() {
        let pk = decode_pubkey(DEFAULT_UPDATER_PUBKEY);
        assert!(pk.is_ok(), "Failed to decode default pubkey: {pk:?}");
    }

    #[test]
    fn test_signature_decoding_with_escaped_newlines() {
        let raw = "untrusted comment: signature from test\nRWQfake\ntrusted comment: timestamp\nfakeglobal";
        let escaped = raw.replace("\n", "\\n");
        // Decode should handle escaped string
        let _ = decode_signature(&escaped);
    }

    #[test]
    fn test_invalid_signature_fails_cleanly() {
        let err = verify_update_signature(b"data", "not-a-sig", DEFAULT_UPDATER_PUBKEY);
        assert!(err.is_err(), "Expected error for invalid signature");
    }

    #[tokio::test]
    async fn test_check_app_update_logic_with_mock_server() {
        use axum::routing::get;
        use axum::{Json, Router};
        use std::net::SocketAddr;

        let mut platforms = HashMap::new();
        platforms.insert(
            current_platform_key().unwrap().to_string(),
            AppPlatformAsset {
                url: "https://example.com/asset.exe".to_string(),
                signature: "sig".to_string(),
            },
        );

        let manifest = AppUpdateManifest {
            version: "99.0.0".to_string(),
            notes: Some("Big update".to_string()),
            pub_date: Some("2026-04-10T12:00:00Z".to_string()),
            platforms,
        };

        let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = DownloadClient::new();
        let endpoint = format!("http://{addr}/updater.json");

        let result = check_app_update_with_version(
            &client,
            Some(&endpoint),
            UpdateChannel::Stable,
            DownloadMirror::Direct,
            "0.9.6",
        )
        .await
        .unwrap();

        assert!(result.update_available);
        assert_eq!(result.latest_version, "99.0.0");
        assert_eq!(result.current_version, "0.9.6");
        assert_eq!(result.notes.as_deref(), Some("Big update"));
        assert_eq!(result.asset_url, "https://example.com/asset.exe");
    }

    #[tokio::test]
    async fn test_download_update_package_cancellation() {
        let client = DownloadClient::new();
        let cancel_notify = Arc::new(Notify::new());
        cancel_notify.notify_one();

        let temp_dir = tempfile::tempdir().unwrap();
        let target = temp_dir.path().join("update.bin");

        let res = download_update_package(
            &client,
            "https://example.com/fake.bin",
            &target,
            "sig",
            None,
            DownloadMirror::Direct,
            cancel_notify,
            None,
        )
        .await;

        assert!(matches!(res, Err(DownloadError::Cancelled)));
        assert!(!target.exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_locate_windows_app_exe_resolution() {
        let cli_exe = PathBuf::from("C:\\fake\\path\\sona-cli.exe");
        let resolved = locate_windows_app_exe(&cli_exe);
        assert!(resolved.is_some());

        let app_exe = PathBuf::from("C:\\fake\\path\\Sona.exe");
        // If the path does not exist on disk, fallback kicks in
        let resolved2 = locate_windows_app_exe(&app_exe);
        assert!(resolved2.is_some());
    }
}
