mod audio;
mod automation_runtime;
pub mod cli;
pub mod export;
mod hardware;
mod history_repository;
mod llm;
pub mod pipeline;
pub mod preset_models;
pub mod sherpa;
pub mod speaker;
pub mod system;
mod text_alignment;
mod webdav;

use serde::Serialize;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::{Mutex, Notify};

const EXTRACT_PROGRESS_EVENT: &str = "extract-progress";
const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";
const TRAY_OPEN_SETTINGS_EVENT: &str = "open-settings";
const TRAY_TOGGLE_CAPTION_EVENT: &str = "toggle-caption";
const TRAY_CHECK_UPDATES_EVENT: &str = "check-updates";
const TRAY_REQUEST_QUIT_EVENT: &str = "request-quit";

/// State managed by Tauri to track active downloads and allow cancellation.
struct DownloadState {
    /// Maps download IDs to notification triggers for cancellation.
    downloads: Mutex<HashMap<String, Arc<Notify>>>,
}

/// App settings state
struct AppSettings {
    minimize_to_tray: std::sync::Mutex<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainWindowCloseAction {
    Ignore,
    HideToTray,
    RequestQuit,
}

struct AuxWindowStateStore {
    states: std::sync::Mutex<HashMap<String, serde_json::Value>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEnvironmentStatus {
    ffmpeg_path: String,
    ffmpeg_exists: bool,
    log_dir_path: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RuntimePathKind {
    File,
    Directory,
    Missing,
    Unknown,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RuntimePathStatus {
    path: String,
    kind: RuntimePathKind,
    error: Option<String>,
}

fn resolve_runtime_path_status(path: &str) -> RuntimePathStatus {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::File,
            error: None,
        },
        Ok(metadata) if metadata.is_dir() => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Directory,
            error: None,
        },
        Ok(_) => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Unknown,
            error: Some("Path exists but is neither a regular file nor directory.".to_string()),
        },
        Err(error) if error.kind() == ErrorKind::NotFound => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Missing,
            error: None,
        },
        Err(error) => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Unknown,
            error: Some(error.to_string()),
        },
    }
}

impl Default for AuxWindowStateStore {
    fn default() -> Self {
        Self {
            states: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

#[tauri::command]
fn set_minimize_to_tray(state: tauri::State<'_, AppSettings>, enabled: bool) {
    if let Ok(mut minimize) = state.minimize_to_tray.lock() {
        *minimize = enabled;
    }
}

#[tauri::command]
fn set_aux_window_state(
    state: tauri::State<'_, AuxWindowStateStore>,
    label: String,
    payload: serde_json::Value,
) -> Result<(), String> {
    let mut states = state.states.lock().map_err(|e| e.to_string())?;
    states.insert(label, payload);
    Ok(())
}

#[tauri::command]
fn get_aux_window_state(
    state: tauri::State<'_, AuxWindowStateStore>,
    label: String,
) -> Result<Option<serde_json::Value>, String> {
    let states = state.states.lock().map_err(|e| e.to_string())?;
    Ok(states.get(&label).cloned())
}

#[tauri::command]
fn clear_aux_window_state(
    state: tauri::State<'_, AuxWindowStateStore>,
    label: String,
) -> Result<(), String> {
    let mut states = state.states.lock().map_err(|e| e.to_string())?;
    states.remove(&label);
    Ok(())
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

fn resolve_main_window_close_action(
    window_label: &str,
    minimize_to_tray: bool,
) -> MainWindowCloseAction {
    if window_label != "main" {
        return MainWindowCloseAction::Ignore;
    }

    if minimize_to_tray {
        MainWindowCloseAction::HideToTray
    } else {
        MainWindowCloseAction::RequestQuit
    }
}

#[cfg(target_os = "windows")]
fn set_mute_windows(mute: bool) -> Result<(), String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitialize, CLSCTX_ALL};

    unsafe {
        // We can ignore the result of CoInitialize as it might already be initialized by Tauri
        let _ = CoInitialize(None);

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;

        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| e.to_string())?;

        let volume: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| e.to_string())?;

        volume
            .SetMute(mute, std::ptr::null())
            .map_err(|e: windows::core::Error| e.to_string())?;
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

    let state = if mute { "1" } else { "0" }; // 1 is mute, 0 is unmute
    let pactl_res = Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", state])
        .output();
    if pactl_res.map(|out| out.status.success()).unwrap_or(false) {
        return Ok(());
    }

    let amixer_state = if mute { "mute" } else { "unmute" };
    if Command::new("amixer")
        .args(["-D", "pulse", "set", "Master", amixer_state])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    if Command::new("amixer")
        .args(["set", "Master", amixer_state])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return Ok(());
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
    caption_text: String,
    caption_checked: bool,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri::menu::{CheckMenuItem, Menu, MenuItem};
        let show_i = MenuItem::with_id(&app, "show", &show_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let caption_i = CheckMenuItem::with_id(
            &app,
            "toggle_caption",
            &caption_text,
            true,
            caption_checked,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;
        let settings_i = MenuItem::with_id(&app, "settings", &settings_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let updates_i = MenuItem::with_id(&app, "check_updates", &updates_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let quit_i = MenuItem::with_id(&app, "quit", &quit_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let menu = Menu::with_items(
            &app,
            &[
                &show_i,
                &caption_i,
                &settings_i,
                &updates_i,
                &tauri::menu::PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?,
                &quit_i,
            ],
        )
        .map_err(|e| e.to_string())?;

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

        let mut last_emit = Instant::now();

        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;

            // Throttle events: emit only if 100ms passed since last emit
            if last_emit.elapsed().as_millis() > 100 {
                let path = entry.path().map_err(|e| e.to_string())?;
                let path_str = path.to_string_lossy().to_string();
                let _ = app.emit(EXTRACT_PROGRESS_EVENT, &path_str);
                last_emit = Instant::now();
            }

            entry.unpack_in(target_path).map_err(|e| e.to_string())?;
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn create_tar_bz2(source_dir: String, archive_path: String) -> Result<(), String> {
    use std::fs::{self, File};
    use std::io::BufWriter;
    use std::path::{Path, PathBuf};

    fn append_directory_contents(
        builder: &mut tar::Builder<bzip2::write::BzEncoder<BufWriter<File>>>,
        root: &Path,
        current: &Path,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;

            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                builder
                    .append_dir(relative, &path)
                    .map_err(|e| e.to_string())?;
                append_directory_contents(builder, root, &path)?;
                continue;
            }

            builder
                .append_path_with_name(&path, relative)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    tauri::async_runtime::spawn_blocking(move || {
        let source_path = PathBuf::from(&source_dir);
        if !source_path.is_dir() {
            return Err(format!("Source directory does not exist: {}", source_dir));
        }

        let archive_path = PathBuf::from(&archive_path);
        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let file = File::create(&archive_path).map_err(|e| e.to_string())?;
        let writer = BufWriter::new(file);
        let encoder = bzip2::write::BzEncoder::new(writer, bzip2::Compression::best());
        let mut builder = tar::Builder::new(encoder);

        append_directory_contents(&mut builder, &source_path, &source_path)?;

        let encoder = builder.into_inner().map_err(|e| e.to_string())?;
        encoder.finish().map_err(|e| e.to_string())?;

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

        if total_size > 0 && (downloaded == total_size || last_emit.elapsed().as_millis() >= 100) {
            on_progress(downloaded, total_size);
            last_emit = Instant::now();
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

#[tauri::command]
async fn open_log_folder<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e: tauri::Error| e.to_string())?;

    // Ensure directory exists
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).map_err(|e: std::io::Error| e.to_string())?;
    }

    app.opener()
        .open_path(log_dir.to_string_lossy(), None::<&str>)
        .map_err(|e: tauri_plugin_opener::Error| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_runtime_environment_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<RuntimeEnvironmentStatus, String> {
    let ffmpeg_path = pipeline::resolve_ffmpeg_sidecar_path()?;
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e: tauri::Error| e.to_string())?;

    Ok(RuntimeEnvironmentStatus {
        ffmpeg_path: ffmpeg_path.to_string_lossy().into_owned(),
        ffmpeg_exists: ffmpeg_path.exists(),
        log_dir_path: log_dir.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
async fn get_path_statuses(paths: Vec<String>) -> Result<Vec<RuntimePathStatus>, String> {
    Ok(paths
        .into_iter()
        .map(|path| resolve_runtime_path_status(&path))
        .collect())
}

/// Initializes and runs the Tauri application.
///
/// Sets up the download state, plugins (opener, dialog, fs, shell, http),
/// and registers invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_level = if cfg!(debug_assertions) {
        tauri_plugin_log::log::LevelFilter::Debug
    } else {
        tauri_plugin_log::log::LevelFilter::Info
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log_level)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .max_file_size(10 * 1024 * 1024) // 10MB
                .clear_targets()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("appsona".to_string()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .format(|out, message, record| {
                    out.finish(format_args!(
                        "{}",
                        serde_json::json!({
                            "time": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            "level": record.level().to_string(),
                            "target": record.target(),
                            "file": record.file(),
                            "line": record.line(),
                            "message": message.to_string(),
                        })
                    ));
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(desktop)]
            {
                use tauri::image::Image;
                use tauri::menu::{CheckMenuItem, Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;

                let show_i =
                    MenuItem::with_id(app, "show", "Show Main Window", true, None::<&str>)?;
                let caption_i = CheckMenuItem::with_id(
                    app,
                    "toggle_caption",
                    "Live Caption",
                    true,
                    false,
                    None::<&str>,
                )?;
                let settings_i =
                    MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
                let updates_i = MenuItem::with_id(
                    app,
                    "check_updates",
                    "Check for Updates",
                    true,
                    None::<&str>,
                )?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

                let menu = Menu::with_items(
                    app,
                    &[
                        &show_i,
                        &caption_i,
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
                        "toggle_caption" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.emit(TRAY_TOGGLE_CAPTION_EVENT, ());
                            }
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit(TRAY_OPEN_SETTINGS_EVENT, ());
                            }
                        }
                        "check_updates" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit(TRAY_CHECK_UPDATES_EVENT, ());
                            }
                        }
                        "quit" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit(TRAY_REQUEST_QUIT_EVENT, ());
                            }
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        use tauri::tray::{MouseButton, TrayIconEvent};
                        let should_show = matches!(
                            event,
                            TrayIconEvent::Click {
                                button: MouseButton::Left,
                                ..
                            } | TrayIconEvent::DoubleClick {
                                button: MouseButton::Left,
                                ..
                            }
                        );

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
                let state = app.state::<AppSettings>();
                // Default to true if lock fails (safe fallback)
                let minimize = state.minimize_to_tray.lock().map(|v| *v).unwrap_or(true);

                match resolve_main_window_close_action(window.label(), minimize) {
                    MainWindowCloseAction::Ignore => {}
                    MainWindowCloseAction::HideToTray => {
                        let _ = window.hide();
                        api.prevent_close();
                    }
                    MainWindowCloseAction::RequestQuit => {
                        api.prevent_close();
                        let _ = window.emit(TRAY_REQUEST_QUIT_EVENT, ());
                    }
                }
            }
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(DownloadState {
            downloads: Mutex::new(std::collections::HashMap::new()),
        })
        .manage(AppSettings {
            minimize_to_tray: std::sync::Mutex::new(true),
        })
        .manage(AuxWindowStateStore::default())
        .manage(automation_runtime::AutomationRuntimeState::default())
        .manage(history_repository::HistoryRepositoryState::default())
        .manage(history_repository::PreparedBackupImportState::default())
        .manage(audio::AudioState::new())
        .manage(sherpa::SherpaState::new())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            extract_tar_bz2,
            create_tar_bz2,
            history_repository::commands::history_list_items,
            history_repository::commands::history_create_live_draft,
            history_repository::commands::history_complete_live_draft,
            history_repository::commands::history_save_recording,
            history_repository::commands::history_save_imported_file,
            history_repository::commands::history_delete_items,
            history_repository::commands::history_load_transcript,
            history_repository::commands::history_update_transcript,
            history_repository::commands::history_create_transcript_snapshot,
            history_repository::commands::history_list_transcript_snapshots,
            history_repository::commands::history_load_transcript_snapshot,
            history_repository::commands::history_update_item_meta,
            history_repository::commands::history_update_project_assignments,
            history_repository::commands::history_reassign_project,
            history_repository::commands::history_load_summary,
            history_repository::commands::history_save_summary,
            history_repository::commands::history_delete_summary,
            history_repository::commands::history_resolve_audio_path,
            history_repository::commands::history_open_folder,
            history_repository::commands::export_backup_archive,
            history_repository::commands::prepare_backup_import,
            history_repository::commands::apply_prepared_history_import,
            history_repository::commands::dispose_prepared_backup_import,
            download_file,
            webdav::webdav_test_connection,
            webdav::webdav_list_backups,
            webdav::webdav_upload_backup,
            webdav::webdav_download_backup,
            cancel_download,
            hardware::check_gpu_availability,
            force_exit,
            has_active_downloads,
            update_tray_menu,
            set_minimize_to_tray,
            set_aux_window_state,
            get_aux_window_state,
            clear_aux_window_state,
            set_system_audio_mute,
            open_log_folder,
            get_runtime_environment_status,
            get_path_statuses,
            automation_runtime::replace_automation_runtime_rules,
            automation_runtime::scan_automation_runtime_rule,
            automation_runtime::collect_automation_runtime_rule_paths,
            system::inject_text,
            system::get_mouse_position,
            system::get_text_cursor_position,
            audio::get_system_audio_devices,
            audio::start_system_audio_capture,
            audio::stop_system_audio_capture,
            audio::set_system_audio_capture_paused,
            audio::set_microphone_boost,
            audio::get_microphone_devices,
            audio::start_microphone_capture,
            audio::stop_microphone_capture,
            audio::set_microphone_capture_paused,
            llm::generate_llm_text,
            llm::list_llm_models,
            llm::polish_transcript_segments,
            llm::summarize_transcript,
            llm::translate_transcript_segments,
            sherpa::init_recognizer,
            sherpa::start_recognizer,
            sherpa::stop_recognizer,
            sherpa::flush_recognizer,
            sherpa::feed_audio_chunk,
            sherpa::process_batch_file,
            speaker::annotate_speaker_segments_from_file,
            speaker::import_speaker_profile_sample
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_main_window_close_action, resolve_runtime_path_status, MainWindowCloseAction,
        RuntimePathKind,
    };
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn resolve_runtime_path_status_detects_existing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("sample.txt");
        File::create(&file_path).unwrap();

        let status = resolve_runtime_path_status(file_path.to_string_lossy().as_ref());

        assert_eq!(status.kind, RuntimePathKind::File);
        assert_eq!(status.error, None);
    }

    #[test]
    fn resolve_runtime_path_status_detects_existing_directory() {
        let dir = tempdir().unwrap();

        let status = resolve_runtime_path_status(dir.path().to_string_lossy().as_ref());

        assert_eq!(status.kind, RuntimePathKind::Directory);
        assert_eq!(status.error, None);
    }

    #[test]
    fn resolve_runtime_path_status_detects_missing_path() {
        let dir = tempdir().unwrap();
        let missing_path = dir.path().join("missing.txt");

        let status = resolve_runtime_path_status(missing_path.to_string_lossy().as_ref());

        assert_eq!(status.kind, RuntimePathKind::Missing);
        assert_eq!(status.error, None);
    }

    #[test]
    fn resolve_runtime_path_status_returns_unknown_for_invalid_path() {
        let invalid_path = "C:\\0\0invalid";

        let status = resolve_runtime_path_status(invalid_path);

        assert_eq!(status.kind, RuntimePathKind::Unknown);
        assert!(status.error.is_some());
    }

    #[test]
    fn main_window_close_hides_to_tray_when_enabled() {
        let action = resolve_main_window_close_action("main", true);

        assert_eq!(action, MainWindowCloseAction::HideToTray);
    }

    #[test]
    fn main_window_close_requests_quit_when_tray_minimize_is_disabled() {
        let action = resolve_main_window_close_action("main", false);

        assert_eq!(action, MainWindowCloseAction::RequestQuit);
    }

    #[test]
    fn non_main_windows_are_ignored_by_quit_guard() {
        let action = resolve_main_window_close_action("caption", false);

        assert_eq!(action, MainWindowCloseAction::Ignore);
    }
}
