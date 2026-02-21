mod hardware;

use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::{Mutex, Notify};

/// State managed by Tauri to track active downloads and allow cancellation.
struct DownloadState {
    /// Maps download IDs to notification triggers for cancellation.
    downloads: Mutex<HashMap<String, Arc<Notify>>>,
}

/// App settings state
struct AppSettings {
    minimize_to_tray: std::sync::Mutex<bool>,
}

#[tauri::command]
fn set_minimize_to_tray(state: tauri::State<'_, AppSettings>, enabled: bool) {
    if let Ok(mut minimize) = state.minimize_to_tray.lock() {
        *minimize = enabled;
    }
}

/// Cancels an active download by its ID.
///
/// # Arguments
///
/// * `state` - The managed `DownloadState`.
/// * `id` - The unique ID of the download to cancel.
///
/// # Returns
///
/// Returns `Ok(())` if the cancellation signal was sent or if the download was not found.
#[tauri::command]
async fn cancel_download(state: tauri::State<'_, DownloadState>, id: String) -> Result<(), String> {
    let downloads = state.downloads.lock().await;
    if let Some(notify) = downloads.get(&id) {
        notify.notify_one();
    }
    Ok(())
}

/// Returns a greeting message.
///
/// # Arguments
///
/// * `name` - The name to greet.
///
/// # Returns
///
/// Returns a formatted greeting string.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn force_exit<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    app.exit(0);
}

#[cfg(target_os = "windows")]
fn set_mute_windows(mute: bool) -> Result<(), String> {
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eRender, eConsole, IAudioEndpointVolume,
    };
    use windows::Win32::System::Com::{CoInitialize, CoCreateInstance, CLSCTX_ALL};

    unsafe {
        // We can ignore the result of CoInitialize as it might already be initialized by Tauri
        let _ = CoInitialize(None);

        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| e.to_string())?;

        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| e.to_string())?;

        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)
            .map_err(|e| e.to_string())?;

        volume.SetMute(mute, std::ptr::null()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_mute_macos(mute: bool) -> Result<(), String> {
    use std::process::Command;
    let state = if mute { "true" } else { "false" };
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!("set volume output muted {}", state))
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_mute_linux(mute: bool) -> Result<(), String> {
    use std::process::Command;

    // Try pactl first (PulseAudio/PipeWire)
    let state = if mute { "1" } else { "0" }; // 1 is mute, 0 is unmute

    let pactl_res = Command::new("pactl")
        .args(&["set-sink-mute", "@DEFAULT_SINK@", state])
        .output();

    if let Ok(out) = pactl_res {
        if out.status.success() {
            return Ok(());
        }
    }

    // Fallback to amixer
    let amixer_state = if mute { "mute" } else { "unmute" };
    let amixer_res = Command::new("amixer")
        .args(&["-D", "pulse", "set", "Master", amixer_state])
        .output();

    if let Ok(out) = amixer_res {
        if out.status.success() {
            return Ok(());
        }
    }

    // Fallback to amixer without -D pulse
    let amixer_res2 = Command::new("amixer")
        .args(&["set", "Master", amixer_state])
        .output();

    if let Ok(out) = amixer_res2 {
        if out.status.success() {
            return Ok(());
        }
    }

    Err("Failed to set mute state on Linux".to_string())
}

#[tauri::command]
async fn set_system_audio_mute(mute: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return set_mute_windows(mute);

    #[cfg(target_os = "macos")]
    return set_mute_macos(mute);

    #[cfg(target_os = "linux")]
    return set_mute_linux(mute);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Err("Unsupported platform".to_string())
}

#[tauri::command]
async fn has_active_downloads(state: tauri::State<'_, DownloadState>) -> Result<bool, String> {
    let downloads = state.downloads.lock().await;
    Ok(!downloads.is_empty())
}

#[tauri::command]
async fn update_tray_menu<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    show_text: String,
    settings_text: String,
    updates_text: String,
    quit_text: String,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri::menu::{Menu, MenuItem};
        let show_i = MenuItem::with_id(&app, "show", &show_text, true, None::<&str>).map_err(|e| e.to_string())?;
        let settings_i = MenuItem::with_id(&app, "settings", &settings_text, true, None::<&str>).map_err(|e| e.to_string())?;
        let updates_i = MenuItem::with_id(&app, "check_updates", &updates_text, true, None::<&str>).map_err(|e| e.to_string())?;
        let quit_i = MenuItem::with_id(&app, "quit", &quit_text, true, None::<&str>).map_err(|e| e.to_string())?;

        let menu = Menu::with_items(
            &app,
            &[
                &show_i,
                &settings_i,
                &updates_i,
                &tauri::menu::PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?,
                &quit_i,
            ],
        ).map_err(|e| e.to_string())?;

        if let Some(tray) = app.tray_by_id("main-tray") {
            tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Extracts a `.tar.bz2` archive to a target directory.
///
/// Runs in a blocking thread to avoid stalling the async runtime.
/// Emits `extract-progress` events with the current filename being extracted.
///
/// # Arguments
///
/// * `app` - The Tauri app handle.
/// * `archive_path` - The path to the source archive.
/// * `target_dir` - The directory to extract the archive into.
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `Err` containing an error message on failure.
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

/// Processes a download stream and writes it to a file with progress callbacks.
///
/// # Arguments
///
/// * `stream` - The incoming byte stream.
/// * `writer` - The async writer (e.g., a file).
/// * `total_size` - The total expected size of the download in bytes.
/// * `on_progress` - A closure called with progress updates (downloaded bytes, total bytes).
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `Err` containing an error message on failure.
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
///
/// Supports cancellation via the `DownloadState` and emits `download-progress` events.
///
/// # Arguments
///
/// * `app` - The Tauri app handle.
/// * `state` - The download state manager.
/// * `url` - The source URL.
/// * `output_path` - The destination file path.
/// * `id` - A unique ID for this download (used for cancellation).
///
/// # Returns
///
/// Returns `Ok(())` on success, or an `Err` containing an error message on failure.
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
        // cleanup
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
    let mut last_emit = std::time::Instant::now(); // Use std::time::Instant directly

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

                if total_size > 0 {
                    if downloaded == total_size || last_emit.elapsed().as_millis() >= 100 {
                        let _ = app.emit("download-progress", (downloaded, total_size, &id));
                        last_emit = std::time::Instant::now();
                    }
                }
            }
            writer.flush().await.map_err(|e| e.to_string())?;
            Ok(())
        } => res
    };

    // Cleanup
    {
        let mut downloads = state.downloads.lock().await;
        downloads.remove(&id);
    }

    // If cancelled, delete the partial file
    if result.is_err() {
        drop(writer);
        let _ = tokio::fs::remove_file(&output_path).await;
    }

    result
}

/// Initializes and runs the Tauri application.
///
/// Sets up the download state, plugins (opener, dialog, fs, shell, http),
/// and registers invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(desktop)]
            {
                use tauri::image::Image;
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;

                let show_i =
                    MenuItem::with_id(app, "show", "Show Main Window", true, None::<&str>)?;
                let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
                let updates_i =
                    MenuItem::with_id(app, "check_updates", "Check for Updates", true, None::<&str>)?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

                let menu = Menu::with_items(
                    app,
                    &[
                        &show_i,
                        &settings_i,
                        &updates_i,
                        &tauri::menu::PredefinedMenuItem::separator(app)?,
                        &quit_i,
                    ],
                )?;

                let icon = Image::from_bytes(include_bytes!("../icons/128x128.png"))?;

                let _tray = TrayIconBuilder::with_id("main-tray")
                    .icon(icon)
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(move |app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit("open-settings", ());
                            }
                        }
                        "check_updates" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit("check-updates", ());
                            }
                        }
                        "quit" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit("request-quit", ());
                            }
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        use tauri::tray::{MouseButton, TrayIconEvent};
                        let should_show = match event {
                            TrayIconEvent::Click {
                                button: MouseButton::Left,
                                ..
                            } => true,
                            TrayIconEvent::DoubleClick {
                                button: MouseButton::Left,
                                ..
                            } => true,
                            _ => false,
                        };

                        if should_show {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                // Check if this is the main window - we only want to intercept close for the main window
                if window.label() == "main" {
                    let state = app.state::<AppSettings>();
                    // Default to true if lock fails (safe fallback)
                    let minimize = state.minimize_to_tray.lock().map(|v| *v).unwrap_or(true);

                    if minimize {
                        let _ = window.hide();
                        api.prevent_close();
                    } else {
                        app.exit(0);
                    }
                }
            }
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .manage(DownloadState {
            downloads: Mutex::new(std::collections::HashMap::new()),
        })
        .manage(AppSettings {
            minimize_to_tray: std::sync::Mutex::new(true),
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
            hardware::check_gpu_availability,
            force_exit,
            has_active_downloads,
            update_tray_menu,
            set_minimize_to_tray,
            set_system_audio_mute
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
