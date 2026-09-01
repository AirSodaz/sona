use sona_model_downloads::DownloadClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

pub struct DownloadState {
    downloads: Mutex<HashMap<String, Arc<Notify>>>,
    client: DownloadClient,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadState {
    pub fn new() -> Self {
        Self {
            downloads: Mutex::new(HashMap::new()),
            client: DownloadClient::new(),
        }
    }

    pub(crate) fn client(&self) -> &DownloadClient {
        &self.client
    }

    pub(crate) async fn insert_download(&self, id: String, notify: Arc<Notify>) {
        self.downloads.lock().await.insert(id, notify);
    }

    pub(crate) async fn remove_download(&self, id: &str) -> Option<Arc<Notify>> {
        self.downloads.lock().await.remove(id)
    }

    pub(crate) async fn notify_download(&self, id: &str) {
        if let Some(notify) = self.notify_for_download(id).await {
            notify.notify_one();
        }
    }

    pub(crate) async fn has_active_downloads(&self) -> bool {
        !self.downloads.lock().await.is_empty()
    }

    async fn notify_for_download(&self, id: &str) -> Option<Arc<Notify>> {
        self.downloads.lock().await.get(id).cloned()
    }
}

pub async fn cancel_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    state.notify_download(&id).await;
    Ok(())
}

pub async fn has_active_downloads(state: tauri::State<'_, DownloadState>) -> Result<bool, String> {
    Ok(state.has_active_downloads().await)
}

pub async fn download_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    url: String,
    output_path: String,
    id: String,
    expected_sha256: Option<String>,
) -> Result<(), String> {
    use sona_model_downloads::{complete_download_file, temporary_download_path};
    use tauri::Emitter;

    let final_path = std::path::PathBuf::from(&output_path);
    let temp_path = temporary_download_path(&final_path);

    let notify = Arc::new(Notify::new());
    state.insert_download(id.clone(), notify.clone()).await;

    let app_clone = app.clone();
    let id_clone = id.clone();
    let mut last_emit = std::time::Instant::now();
    let progress_cb = Box::new(move |downloaded: u64, total: u64| {
        if downloaded == total || last_emit.elapsed().as_millis() >= 100 {
            let _ = app_clone.emit(DOWNLOAD_PROGRESS_EVENT, (downloaded, total, &id_clone));
            last_emit = std::time::Instant::now();
        }
    });

    let result = state
        .client()
        .download_file(&url, &temp_path, notify, Some(progress_cb))
        .await;

    state.remove_download(&id).await;

    match result {
        Ok(()) => complete_download_file(&temp_path, &final_path, expected_sha256.as_deref())
            .await
            .map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    }
}

pub async fn download_preset_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    model_id: String,
    download_id: String,
    mirror: Option<String>,
) -> Result<String, String> {
    use sona_core::models::downloads::resolve_model_download;
    use sona_model_downloads::{
        download_model_with_cancel_and_mirror, installed_model_is_valid, parse_download_mirror,
    };
    use tauri::Emitter;

    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(&app)?;
    let resolved =
        resolve_model_download(&model_id, &models_dir).map_err(|error| error.to_string())?;
    if installed_model_is_valid(&resolved)
        .await
        .map_err(|error| error.to_string())?
    {
        return Ok(resolved.install_path.to_string_lossy().into_owned());
    }

    let notify = Arc::new(Notify::new());
    state
        .insert_download(download_id.clone(), notify.clone())
        .await;
    let app_clone = app.clone();
    let event_download_id = download_id.clone();
    let result = download_model_with_cancel_and_mirror(
        &resolved,
        notify,
        parse_download_mirror(mirror.as_deref().unwrap_or("auto")),
        move |progress| {
            if progress.stage == sona_model_downloads::ModelDownloadStage::Downloading {
                let _ = app_clone.emit(
                    DOWNLOAD_PROGRESS_EVENT,
                    (
                        progress.downloaded_bytes,
                        progress.total_bytes,
                        &event_download_id,
                    ),
                );
            }
        },
    )
    .await;
    state.remove_download(&download_id).await;

    result
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| error.to_string())
}

pub async fn delete_preset_model<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    model_id: &str,
) -> Result<(), String> {
    use sona_core::models::downloads::resolve_model_download;
    use sona_model_downloads::{remove_model_install_path, temporary_download_path};

    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(app)?;
    let resolved =
        resolve_model_download(model_id, &models_dir).map_err(|error| error.to_string())?;

    remove_model_install_path(&resolved.install_path).map_err(|error| error.to_string())?;

    let mut staging = resolved.install_path.as_os_str().to_os_string();
    staging.push(".installing");
    let staging_path = std::path::PathBuf::from(staging);
    let _ = remove_model_install_path(&staging_path);

    if resolved.download_path != resolved.install_path {
        let _ = remove_model_install_path(&resolved.download_path);
    }
    let temp_download = temporary_download_path(&resolved.download_path);
    let _ = remove_model_install_path(&temp_download);

    Ok(())
}

pub fn get_cuda_addon_status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    use crate::platform::paths::{PathKind, PathPort, TauriPathProvider};
    use sona_core::runtime::cuda_addon::{CUDA_ADDON_SUBPATH, inspect_cuda_addon_directory};

    let base_dir = TauriPathProvider::from_app(app)
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;
    let cuda_dir = base_dir.join(CUDA_ADDON_SUBPATH);
    Ok(inspect_cuda_addon_directory(&cuda_dir))
}

pub fn try_auto_activate_cuda_addon<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use crate::platform::paths::{PathKind, PathPort, TauriPathProvider};
    use sona_core::runtime::cuda_addon::{
        CUDA_ADDON_SUBPATH, activate_cuda_addon_directory, inspect_cuda_addon_directory,
    };

    if let Ok(base_dir) = TauriPathProvider::from_app(app).resolve_path(PathKind::AppLocalData) {
        let cuda_dir = base_dir.join(CUDA_ADDON_SUBPATH);
        let inspection = inspect_cuda_addon_directory(&cuda_dir);
        if inspection.is_installed {
            let _ = activate_cuda_addon_directory(&cuda_dir);
        }
    }
}

pub async fn activate_cuda_addon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    use crate::platform::paths::{PathKind, PathPort, TauriPathProvider};
    use sona_core::runtime::cuda_addon::{CUDA_ADDON_SUBPATH, activate_cuda_addon_directory};

    let base_dir = TauriPathProvider::from_app(&app)
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;
    let cuda_dir = base_dir.join(CUDA_ADDON_SUBPATH);
    activate_cuda_addon_directory(&cuda_dir).map_err(|error| error.to_string())
}

pub async fn download_and_install_cuda_addon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    download_id: String,
    mirror: Option<String>,
    version: Option<String>,
    custom_url: Option<String>,
    expected_sha256: Option<String>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    use crate::platform::paths::{PathKind, PathPort, TauriPathProvider};
    use sona_core::runtime::cuda_addon::CUDA_ADDON_SUBPATH;
    use sona_model_downloads::{
        default_cuda_addon_download_url, download_and_install_cuda_addon as download_addon,
        parse_download_mirror,
    };
    use tauri::Emitter;

    let base_dir = TauriPathProvider::from_app(&app)
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;
    let cuda_dir = base_dir.join(CUDA_ADDON_SUBPATH);

    let download_url = match custom_url {
        Some(url) if !url.trim().is_empty() => url,
        _ => default_cuda_addon_download_url(version.as_deref(), None, std::env::consts::OS)
            .ok_or_else(|| {
                format!(
                    "CUDA addon is not available for platform: {}",
                    std::env::consts::OS
                )
            })?,
    };

    let notify = Arc::new(Notify::new());
    state
        .insert_download(download_id.clone(), notify.clone())
        .await;
    let app_clone = app.clone();
    let event_download_id = download_id.clone();

    let result = download_addon(
        state.client(),
        &cuda_dir,
        &download_url,
        expected_sha256.as_deref(),
        parse_download_mirror(mirror.as_deref().unwrap_or("auto")),
        notify,
        move |progress| {
            if progress.stage == sona_model_downloads::ModelDownloadStage::Downloading {
                let _ = app_clone.emit(
                    DOWNLOAD_PROGRESS_EVENT,
                    (
                        progress.downloaded_bytes,
                        progress.total_bytes,
                        &event_download_id,
                    ),
                );
            }
        },
    )
    .await;

    state.remove_download(&download_id).await;
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn download_state_tracks_active_downloads_by_id() {
        let state = DownloadState::new();
        let notify = Arc::new(Notify::new());

        assert!(!state.has_active_downloads().await);

        state
            .insert_download("model-a".to_string(), notify.clone())
            .await;

        assert!(state.has_active_downloads().await);
        let stored = state
            .notify_for_download("model-a")
            .await
            .expect("download exists");
        assert!(Arc::ptr_eq(&notify, &stored));

        let removed = state.remove_download("model-a").await;
        assert!(removed.is_some());
        assert!(!state.has_active_downloads().await);
    }
}
