use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

pub struct DownloadState {
    pub downloads: Mutex<HashMap<String, Arc<Notify>>>,
}

impl DownloadState {
    pub fn new() -> Self {
        Self {
            downloads: Mutex::new(HashMap::new()),
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
pub async fn has_active_downloads(
    state: tauri::State<'_, DownloadState>,
) -> Result<bool, String> {
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
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let notify = Arc::new(Notify::new());
    {
        let mut downloads = state.downloads.lock().await;
        downloads.insert(id.clone(), notify.clone());
    }

    let client = reqwest::Client::builder()
        .user_agent("Sona/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let res = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let mut downloads = state.downloads.lock().await;
        downloads.remove(&id);
        return Err(format!("Download failed with status: {}", res.status()));
    }

    let total_size = res.content_length().unwrap_or(0);
    let file = tokio::fs::File::create(&output_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut writer = tokio::io::BufWriter::new(file);
    let mut stream = res
        .bytes_stream()
        .map(|item| item.map_err(|e| e.to_string()));

    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    let result = tokio::select! {
        _ = notify.notified() => {
            Err("Download cancelled".to_string())
        }
        res = async {
            use tokio::io::AsyncWriteExt;
            while let Some(item) = stream.next().await {
                let chunk = item?;
                writer.write_all(&chunk).await.map_err(|e| e.to_string())?;
                downloaded += chunk.len() as u64;

                if total_size > 0
                    && (downloaded == total_size || last_emit.elapsed().as_millis() >= 100)
                {
                    let _ = app.emit(DOWNLOAD_PROGRESS_EVENT, (downloaded, total_size, &id));
                    last_emit = std::time::Instant::now();
                }
            }
            writer.flush().await.map_err(|e| e.to_string())?;
            Ok(())
        } => res
    };

    {
        let mut downloads = state.downloads.lock().await;
        downloads.remove(&id);
    }

    if result.is_err() {
        drop(writer);
        let _ = tokio::fs::remove_file(&output_path).await;
    }

    result
}
