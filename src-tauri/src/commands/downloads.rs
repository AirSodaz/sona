use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

pub struct DownloadState {
    pub downloads: Mutex<HashMap<String, Arc<Notify>>>,
    pub client: reqwest::Client,
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
            client: reqwest::Client::builder()
                .user_agent("Sona/1.0")
                .build()
                .unwrap(),
        }
    }
}

#[tauri::command]
pub async fn cancel_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let downloads = state.downloads.lock().await;
    if let Some(notify) = downloads.get(&id) {
        notify.notify_one();
    }
    Ok(())
}

#[tauri::command]
pub async fn has_active_downloads(state: tauri::State<'_, DownloadState>) -> Result<bool, String> {
    let downloads = state.downloads.lock().await;
    Ok(!downloads.is_empty())
}

#[tauri::command]
pub async fn download_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    url: String,
    output_path: String,
    id: String,
    expected_sha256: Option<String>,
) -> Result<(), String> {
    use sona_core::downloads::{
        complete_download_file, download_file as core_download_file, temporary_download_path,
    };
    use tauri::Emitter;

    let final_path = std::path::PathBuf::from(&output_path);
    let temp_path = temporary_download_path(&final_path);

    let notify = Arc::new(Notify::new());
    {
        let mut downloads = state.downloads.lock().await;
        downloads.insert(id.clone(), notify.clone());
    }

    let client = state.client.clone();

    let app_clone = app.clone();
    let id_clone = id.clone();
    let mut last_emit = std::time::Instant::now();
    let progress_cb = Box::new(move |downloaded: u64, total: u64| {
        if downloaded == total || last_emit.elapsed().as_millis() >= 100 {
            let _ = app_clone.emit(DOWNLOAD_PROGRESS_EVENT, (downloaded, total, &id_clone));
            last_emit = std::time::Instant::now();
        }
    });

    let result = core_download_file(&client, &url, &temp_path, notify, Some(progress_cb)).await;

    {
        let mut downloads = state.downloads.lock().await;
        downloads.remove(&id);
    }

    match result {
        Ok(()) => complete_download_file(&temp_path, &final_path, expected_sha256.as_deref())
            .await
            .map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}
