mod hardware;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// State managed by Tauri to track active downloads and allow cancellation.
struct DownloadState {
    /// Map of download IDs to notification triggers.
    downloads: Mutex<HashMap<String, Arc<Notify>>>,
}

/// Cancels an active download by its ID.
///
/// # Arguments
///
/// * `state` - The managed `DownloadState`.
/// * `id` - The unique ID of the download to cancel.
#[tauri::command]
fn cancel_download(state: tauri::State<DownloadState>, id: String) -> Result<(), String> {
    if let Ok(downloads) = state.downloads.lock() {
        if let Some(notify) = downloads.get(&id) {
            notify.notify_one();
        }
    }
    Ok(())
}

/// Simple greeting command for testing Tauri communication.
///
/// # Arguments
///
/// * `name` - The name to greet.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Extracts a `.tar.bz2` archive to a target directory.
/// Runs in a blocking thread to avoid stalling the async runtime.
/// Emits `extract-progress` events with the current filename being extracted.
///
/// # Arguments
///
/// * `app` - The Tauri app handle.
/// * `archive_path` - Path to the source archive.
/// * `target_dir` - Directory to extract into.
#[tauri::command]
async fn extract_tar_bz2<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    use std::path::Path;
    use std::time::Instant;
    use tauri::Emitter;

    // Move heavy lifting to a blocking thread to avoid blocking the async runtime
    tauri::async_runtime::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path).map_err(|e| e.to_string())?;
        let buffered = std::io::BufReader::new(file);
        let tar = bzip2::read::BzDecoder::new(buffered);
        let mut archive = tar::Archive::new(tar);
        let target_path = Path::new(&target_dir);

        // Get list of entries first to count them?
        // Tar streams don't support counting without reading everything.
        // So we just report "Extracting <filename>" without percentage,
        // or we could roughly estimate if we knew total files, but we don't.
        // We will just emit the current file name.

        let mut last_emit = Instant::now();

        for (_i, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
            let mut entry = entry.map_err(|e| e.to_string())?;

            // Throttle events: emit only if 100ms passed since last emit
            if last_emit.elapsed().as_millis() > 100 {
                let path = entry.path().map_err(|e| e.to_string())?;
                let path_str = path.to_string_lossy().to_string();
                let _ = app.emit("extract-progress", &path_str);
                last_emit = Instant::now();
            }

            entry.unpack_in(target_path).map_err(|e| e.to_string())?;
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Helper to process a download stream and write to a file with progress callbacks.
///
/// # Arguments
///
/// * `stream` - The incoming byte stream.
/// * `writer` - The async writer (file).
/// * `total_size` - Total expected size of the download.
/// * `on_progress` - Closure called with progress updates.
pub async fn process_download<S, W, F>(
    mut stream: S,
    mut writer: W,
    total_size: u64,
    mut on_progress: F,
) -> Result<(), String>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, String>> + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
    F: FnMut(u64, u64),
{
    use futures_util::StreamExt;
    use std::time::Instant;
    use tokio::io::AsyncWriteExt;

    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        writer.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        if total_size > 0 {
            if downloaded == total_size || last_emit.elapsed().as_millis() >= 100 {
                on_progress(downloaded, total_size);
                last_emit = Instant::now();
            }
        }
    }
    Ok(())
}

/// Downloads a file from a URL to a specified path.
/// Supports cancellation via the `DownloadState`.
/// Emits `download-progress` events.
///
/// # Arguments
///
/// * `app` - The Tauri app handle.
/// * `state` - The download state manager.
/// * `url` - Source URL.
/// * `output_path` - Destination file path.
/// * `id` - Unique ID for this download (used for cancellation).
#[tauri::command]
async fn download_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    url: String,
    output_path: String,
    id: String,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let notify = Arc::new(Notify::new());
    if let Ok(mut downloads) = state.downloads.lock() {
        downloads.insert(id.clone(), notify.clone());
    }

    let client = reqwest::Client::builder()
        .user_agent("Sona/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let res = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        // cleanup
        if let Ok(mut downloads) = state.downloads.lock() {
            downloads.remove(&id);
        }
        return Err(format!("Download failed with status: {}", res.status()));
    }

    let total_size = res.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(&output_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = res
        .bytes_stream()
        .map(|item| item.map_err(|e| e.to_string()));

    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now(); // Use std::time::Instant directly

    let result = loop {
        tokio::select! {
             _ = notify.notified() => {
                 break Err("Download cancelled".to_string());
             }
             item = stream.next() => {
                 match item {
                     Some(chunk_res) => {
                         let chunk = match chunk_res {
                             Ok(c) => c,
                             Err(e) => break Err(e),
                         };
                         use tokio::io::AsyncWriteExt;
                         if let Err(e) = file.write_all(&chunk).await {
                             break Err(e.to_string());
                         }
                         downloaded += chunk.len() as u64;

                         if total_size > 0 {
                             if downloaded == total_size || last_emit.elapsed().as_millis() >= 100 {
                                 let _ = app.emit("download-progress", (downloaded, total_size));
                                 last_emit = std::time::Instant::now();
                             }
                         }
                     }
                     None => {
                        break Ok(());
                     }
                 }
             }
        }
    };

    // Cleanup
    if let Ok(mut downloads) = state.downloads.lock() {
        downloads.remove(&id);
    }

    // If cancelled, delete the partial file
    if result.is_err() {
        let _ = tokio::fs::remove_file(&output_path).await;
    }

    result
}

/// Initializes and runs the Tauri application.
/// Sets up plugins and invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(DownloadState {
            downloads: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            extract_tar_bz2,
            download_file,
            cancel_download,
            hardware::check_gpu_availability
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
