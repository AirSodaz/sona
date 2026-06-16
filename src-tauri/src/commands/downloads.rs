use sha2::{Digest, Sha256};
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
    expected_sha256: Option<String>,
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

    match result {
        Ok(()) => {
            drop(writer);
            complete_download_file(&temp_path, &final_path, expected_sha256.as_deref()).await
        }
        Err(error) => {
            drop(writer);
            cleanup_failed_download(&temp_path).await;
            Err(error)
        }
    }
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

async fn complete_download_file(
    temp_path: &Path,
    final_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    let result = async {
        verify_download_file(temp_path, expected_sha256).await?;
        publish_download_file(temp_path, final_path).await
    }
    .await;

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            cleanup_failed_download(temp_path).await;
            Err(error)
        }
    }
}

async fn verify_download_file(
    temp_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    if let Some(expected_hash) = expected_sha256 {
        let actual_hash = sha256_file(temp_path).await?;
        if !actual_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(format!(
                "Downloaded file hash mismatch for {}",
                temp_path.display()
            ));
        }
    }

    Ok(())
}

async fn sha256_file(path: &Path) -> Result<String, String> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).await.map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
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

    #[tokio::test]
    async fn complete_download_file_keeps_final_and_removes_temp_when_hash_mismatches() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("silero_vad.onnx");
        let temp_path = dir.path().join("silero_vad.onnx.abc123.download");
        tokio::fs::write(&final_path, b"old-good").await.unwrap();
        tokio::fs::write(&temp_path, b"wrong").await.unwrap();

        let result = complete_download_file(
            &temp_path,
            &final_path,
            Some("eebbf6457e46a7f63acdf9b97390f790ba443d60cfa44b607da7e5c40aa1cc1d"),
        )
        .await;

        assert!(
            result
                .unwrap_err()
                .contains("Downloaded file hash mismatch")
        );
        assert_eq!(tokio::fs::read(&final_path).await.unwrap(), b"old-good");
        assert!(!temp_path.exists());
    }

    #[tokio::test]
    async fn complete_download_file_accepts_matching_hash_before_replacing_final() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("silero_vad.onnx");
        let temp_path = dir.path().join("silero_vad.onnx.abc123.download");
        tokio::fs::write(&final_path, b"old").await.unwrap();
        tokio::fs::write(&temp_path, b"complete").await.unwrap();

        complete_download_file(
            &temp_path,
            &final_path,
            Some("eebbf6457e46a7f63acdf9b97390f790ba443d60cfa44b607da7e5c40aa1cc1d"),
        )
        .await
        .unwrap();

        assert_eq!(tokio::fs::read(&final_path).await.unwrap(), b"complete");
        assert!(!temp_path.exists());
    }

    #[tokio::test]
    async fn complete_download_file_removes_temp_when_publish_fails() {
        let dir = tempfile::tempdir().unwrap();
        let temp_path = dir.path().join("silero_vad.onnx.abc123.download");
        let final_path = dir.path().join("missing-parent").join("silero_vad.onnx");
        tokio::fs::write(&temp_path, b"complete").await.unwrap();

        let result = complete_download_file(&temp_path, &final_path, None).await;

        assert!(result.is_err());
        assert!(!temp_path.exists());
    }
}
