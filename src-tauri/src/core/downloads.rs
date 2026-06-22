use reqwest::header::RANGE;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Download cancelled")]
    Cancelled,
    #[error("Range not satisfiable: server reset download")]
    RangeNotSatisfiable,
    #[error("Download failed with status: {0}")]
    HttpStatus(reqwest::StatusCode),
    #[error("Downloaded file hash mismatch for {0}")]
    HashMismatch(String),
}

pub fn temporary_download_path(path: &Path, id: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".");
    s.push(id);
    s.push(".download");
    PathBuf::from(s)
}

pub async fn remove_download_file(temp_path: &Path) {
    let _ = tokio::fs::remove_file(temp_path).await;
}

pub async fn complete_download_file(
    temp_path: &Path,
    final_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), DownloadError> {
    let result = async {
        verify_download_file(temp_path, expected_sha256).await?;
        publish_download_file(temp_path, final_path).await
    }
    .await;

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            remove_download_file(temp_path).await;
            Err(error)
        }
    }
}

pub async fn verify_download_file(
    temp_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), DownloadError> {
    if let Some(expected_hash) = expected_sha256 {
        let actual_hash = sha256_file(temp_path).await?;
        if !actual_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(DownloadError::HashMismatch(temp_path.display().to_string()));
        }
    }
    Ok(())
}

pub async fn sha256_file(path: &Path) -> Result<String, DownloadError> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 16 * 1024];

    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

pub async fn publish_download_file(
    temp_path: &Path,
    final_path: &Path,
) -> Result<(), DownloadError> {
    if tokio::fs::try_exists(final_path).await? {
        tokio::fs::remove_file(final_path).await?;
    }
    tokio::fs::rename(temp_path, final_path).await?;
    Ok(())
}

pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    temp_path: &Path,
    notify: Arc<Notify>,
    mut on_progress: Option<Box<dyn FnMut(u64, u64) + Send>>,
) -> Result<(), DownloadError> {
    let max_retries = 3;
    let mut attempt = 0;

    loop {
        let current_size = tokio::fs::metadata(&temp_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let mut request = client.get(url);

        if current_size > 0 {
            request = request.header(RANGE, format!("bytes={}-", current_size));
        }

        let res_result = request.send().await;

        let res = match res_result {
            Ok(r) => r,
            Err(e) => {
                if attempt < max_retries {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(1 << (attempt - 1))).await;
                    continue;
                }
                return Err(DownloadError::Network(e));
            }
        };

        if res.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            let _ = tokio::fs::remove_file(&temp_path).await;
            continue;
        }

        if !res.status().is_success() {
            return Err(DownloadError::HttpStatus(res.status()));
        }

        let is_partial = res.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let content_length = res.content_length().unwrap_or(0);
        let total_size = if is_partial {
            current_size + content_length
        } else {
            content_length
        };

        if let Some(parent) = temp_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let file = if is_partial {
            OpenOptions::new().append(true).open(&temp_path).await?
        } else {
            tokio::fs::File::create(&temp_path).await?
        };

        let mut writer = tokio::io::BufWriter::new(file);
        use futures_util::StreamExt;
        let mut stream = res.bytes_stream();
        let mut downloaded: u64 = if is_partial { current_size } else { 0 };

        let mut stream_error = None;
        let mut cancelled = false;

        tokio::select! {
            _ = notify.notified() => {
                cancelled = true;
            }
            res = async {
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(chunk) => {
                            if let Err(e) = writer.write_all(&chunk).await {
                                return Err(DownloadError::Io(e));
                            }
                            downloaded += chunk.len() as u64;
                            if let Some(cb) = on_progress.as_mut() {
                                cb(downloaded, total_size);
                            }
                        }
                        Err(e) => {
                            return Err(DownloadError::Network(e));
                        }
                    }
                }
                Ok(())
            } => {
                if let Err(e) = res {
                    stream_error = Some(e);
                }
            }
        };

        writer.flush().await?;
        writer.into_inner().sync_all().await?;

        if cancelled {
            return Err(DownloadError::Cancelled);
        }

        if let Some(e) = stream_error {
            if downloaded > current_size {
                attempt = 0;
            }
            if attempt < max_retries && matches!(e, DownloadError::Network(_)) {
                attempt += 1;
                tokio::time::sleep(std::time::Duration::from_secs(1 << (attempt - 1))).await;
                continue;
            }
            return Err(e);
        }

        return Ok(());
    }
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
    async fn remove_download_file_removes_temp_without_touching_final() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("silero_vad.onnx");
        let temp_path = dir.path().join("silero_vad.onnx.abc123.download");
        tokio::fs::write(&final_path, b"old-good").await.unwrap();
        tokio::fs::write(&temp_path, b"partial").await.unwrap();

        remove_download_file(&temp_path).await;

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

        assert!(matches!(result, Err(DownloadError::HashMismatch(_))));
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

        assert!(matches!(result, Err(DownloadError::Io(_))));
        assert!(!temp_path.exists());
    }
}
