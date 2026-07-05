use reqwest::header::RANGE;
use sha2::{Digest, Sha256};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
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
    #[error("Download already in progress by another process")]
    AlreadyInProgress,
}

pub fn temporary_download_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
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
    // Acquire an exclusive lock on the download file BEFORE establishing any
    // network connection. This lets us fail fast with AlreadyInProgress
    // instead of wasting a TCP connection and downloading bytes we cannot use.
    let mut file = open_and_lock_download_file(temp_path).await?;

    let max_retries = 3;
    let mut attempt = 0;

    loop {
        // Read the current on-disk size from the already-open handle so we
        // know whether to request a byte range for resumption.
        let current_size = file.metadata().await.map(|m| m.len()).unwrap_or(0);

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
            // The server does not recognise our byte range; truncate the
            // partial file in-place and restart from the beginning.
            file.set_len(0).await?;
            file.seek(SeekFrom::Start(0)).await?;
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

        // Position the file cursor before streaming begins.
        if is_partial {
            // Resume: append after the bytes already on disk.
            file.seek(SeekFrom::End(0)).await?;
        } else {
            // Full response: overwrite from the beginning.
            file.set_len(0).await?;
            file.seek(SeekFrom::Start(0)).await?;
        }

        let mut writer = tokio::io::BufWriter::new(&mut file);
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
        // Drop the writer to release the &mut borrow before calling sync_all.
        drop(writer);
        file.sync_all().await?;

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

/// Opens `temp_path` for reading and writing (creating it if absent) and
/// acquires an exclusive byte-range lock on the file handle.
///
/// Failing early here — before any network activity — means a competing
/// download (e.g. the GUI) is detected without wasting a TCP connection.
async fn open_and_lock_download_file(temp_path: &Path) -> Result<tokio::fs::File, DownloadError> {
    if let Some(parent) = temp_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Open for both reading and writing so that a single handle supports
    // seek, truncation, and streaming writes across all retry attempts.
    // `.truncate(false)` is intentional: any existing partial download bytes
    // are preserved for resumption; truncation is performed explicitly inside
    // the download loop when the server returns a full (non-partial) response.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(temp_path)
        .await?;

    use fs3::FileExt;
    let std_file = file.into_std().await;
    if std_file.try_lock_exclusive().is_err() {
        return Err(DownloadError::AlreadyInProgress);
    }
    Ok(tokio::fs::File::from_std(std_file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn temporary_download_path_uses_sibling_file() {
        let path = temporary_download_path(Path::new("C:/models/silero_vad.onnx"));

        assert_eq!(path, Path::new("C:/models/silero_vad.onnx.download"));
    }

    #[tokio::test]
    async fn remove_download_file_removes_temp_without_touching_final() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("silero_vad.onnx");
        let temp_path = dir.path().join("silero_vad.onnx.download");
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
        let temp_path = dir.path().join("silero_vad.onnx.download");
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
        let temp_path = dir.path().join("silero_vad.onnx.download");
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
        let temp_path = dir.path().join("silero_vad.onnx.download");
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
        let temp_path = dir.path().join("silero_vad.onnx.download");
        let final_path = dir.path().join("missing-parent").join("silero_vad.onnx");
        tokio::fs::write(&temp_path, b"complete").await.unwrap();

        let result = complete_download_file(&temp_path, &final_path, None).await;

        assert!(matches!(result, Err(DownloadError::Io(_))));
        assert!(!temp_path.exists());
    }

    #[tokio::test]
    async fn concurrent_calls_return_already_in_progress() {
        use axum::{Router, routing::get};
        use fs3::FileExt;
        use std::sync::Arc;
        use tokio::fs::File;
        use tokio::net::TcpListener;
        use tokio::sync::Notify;

        let dir = tempfile::tempdir().unwrap();
        let temp_path = dir.path().join("test.onnx.download");

        // Start a dummy axum server to satisfy the network request
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{}/test", addr);

        let app = Router::new().route("/test", get(|| async { "dummy content" }));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Pre-create and lock the file
        let file = File::create(&temp_path).await.unwrap();
        let std_file = file.try_clone().await.unwrap().into_std().await;
        std_file.try_lock_exclusive().unwrap();

        let client = reqwest::Client::new();
        let notify = Arc::new(Notify::new());

        // This should fail with AlreadyInProgress
        let result = download_file(&client, &url, &temp_path, notify, None).await;

        assert!(matches!(result, Err(DownloadError::AlreadyInProgress)));
    }
}
