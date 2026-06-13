use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

pub struct DownloadState {
    pub downloads: Mutex<HashMap<String, Arc<Notify>>>,
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
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let final_path = PathBuf::from(&output_path);
    let temp_path = temporary_download_path(&final_path, &id);

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
    let file = tokio::fs::File::create(&temp_path)
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

    if result.is_ok() {
        drop(writer);
        publish_download_file(&temp_path, &final_path).await?;
    } else {
        drop(writer);
        cleanup_failed_download(&temp_path).await;
    }

    result
}

fn temporary_download_path(path: &Path, id: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".");
    s.push(id);
    s.push(".download");
    PathBuf::from(s)
}

async fn cleanup_failed_download(temp_path: &Path) {
    let _ = tokio::fs::remove_file(temp_path).await;
}

async fn publish_download_file(temp_path: &Path, final_path: &Path) -> Result<(), String> {
    if tokio::fs::try_exists(final_path)
        .await
        .map_err(|e| e.to_string())?
    {
        tokio::fs::remove_file(final_path)
            .await
            .map_err(|e| e.to_string())?;
    }

    tokio::fs::rename(temp_path, final_path)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn temporary_download_path_uses_sibling_file() {
        let path = temporary_download_path(Path::new("C:/models/silero_vad.onnx"), "abc123");

        assert_eq!(path, Path::new("C:/models/silero_vad.onnx.abc123.download"));
    }

    #[tokio::test]
    async fn cleanup_failed_download_removes_temp_without_touching_final() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("silero_vad.onnx");
        let temp_path = dir.path().join("silero_vad.onnx.abc123.download");
        tokio::fs::write(&final_path, b"old-good").await.unwrap();
        tokio::fs::write(&temp_path, b"partial").await.unwrap();

        cleanup_failed_download(&temp_path).await;

        assert_eq!(tokio::fs::read(&final_path).await.unwrap(), b"old-good");
        assert!(!temp_path.exists());
    }

    #[tokio::test]
    async fn publish_download_file_replaces_final_only_after_temp_exists() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("silero_vad.onnx");
        let temp_path = dir.path().join("silero_vad.onnx.abc123.download");
        tokio::fs::write(&final_path, b"old").await.unwrap();
        tokio::fs::write(&temp_path, b"complete").await.unwrap();

        publish_download_file(&temp_path, &final_path)
            .await
            .unwrap();

        assert_eq!(tokio::fs::read(&final_path).await.unwrap(), b"complete");
        assert!(!temp_path.exists());
    }
}
