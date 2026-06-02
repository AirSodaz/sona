pub mod app;
pub mod commands;
pub mod core;
pub mod integrations;
pub mod repositories;

pub mod cli;
mod dashboard;

use tauri::{Emitter, Listener, Manager};
use tokio::sync::Mutex as AsyncMutex;

pub struct ApiServerController {
    shutdown_sender: std::sync::Arc<AsyncMutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    pub online_asr_config:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>>,
}

impl Default for ApiServerController {
    fn default() -> Self {
        Self {
            shutdown_sender: std::sync::Arc::new(AsyncMutex::new(None)),
            online_asr_config: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }
}

fn load_online_asr_config(
    app: &tauri::AppHandle,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut online_asr_config = std::collections::HashMap::new();
    if let Ok(data_dir) = app.path().app_data_dir() {
        let config_path = data_dir.join("settings.json");
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => {
                    if let Some(config) = json
                        .get("asr")
                        .and_then(|v| v.get("providers"))
                        .and_then(|v| v.get("online"))
                    {
                        if let Some(map) = config.as_object() {
                            for (k, v) in map {
                                online_asr_config.insert(k.clone(), v.clone());
                            }
                        }
                    }
                }
                Err(e) => log::error!("Failed to parse settings.json: {}", e),
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to read settings.json: {}", e);
                }
            }
        }
    }
    online_asr_config
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn start_api_server(
    app: tauri::AppHandle,
    controller: tauri::State<'_, ApiServerController>,
    host: String,
    port: u16,
    api_key: String,
    max_concurrent: usize,
    max_queue_size: usize,
    max_upload_size_mb: usize,
    job_ttl_minutes: u64,
    max_streaming: usize,
    ip_whitelist: String,
) -> Result<String, String> {
    let parsed_whitelist = crate::app::server::parse_ip_whitelist(&ip_whitelist)?;
    let normalized_whitelist = parsed_whitelist
        .iter()
        .map(|net| net.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let parsed_arc = std::sync::Arc::new(parsed_whitelist);

    let mut sender_lock = controller.shutdown_sender.lock().await;

    // Stop existing server if running
    if let Some(sender) = sender_lock.take() {
        let _ = sender.send(());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    *sender_lock = Some(tx);

    let app_local_data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let temp_dir = app_local_data_dir.join("api_temp");
    let models_dir = app_local_data_dir.join("models");

    let new_config = load_online_asr_config(&app);
    *controller.online_asr_config.write().await = new_config;
    let online_asr_config = controller.online_asr_config.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::app::server::run_server(
            Some(app.clone()),
            &host,
            port,
            &api_key,
            temp_dir,
            models_dir,
            max_concurrent,
            max_queue_size,
            max_upload_size_mb,
            job_ttl_minutes,
            max_streaming,
            parsed_arc,
            online_asr_config,
            rx,
        )
        .await
        {
            log::error!("HTTP API Server failed: {}", e);
        }
    });

    Ok(normalized_whitelist)
}

#[tauri::command]
async fn stop_api_server(controller: tauri::State<'_, ApiServerController>) -> Result<(), String> {
    let mut sender_lock = controller.shutdown_sender.lock().await;
    if let Some(sender) = sender_lock.take() {
        let _ = sender.send(());
        log::info!("Sent shutdown signal to API server.");
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

#[tauri::command]
async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    let mut results = Vec::with_capacity(paths.len());
    for path in paths {
        let is_valid = crate::integrations::media_detector::is_valid_media_file(&path).await;
        results.push(is_valid);
    }
    Ok(results)
}

#[tauri::command]
fn resolve_model_catalog_selected_ids<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    paths: crate::core::preset_models::ModelSelectionPaths,
) -> Result<crate::core::preset_models::ModelCatalogSelectedIds, String> {
    let models_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?
        .join("models");

    std::fs::create_dir_all(&models_dir).map_err(|error| {
        format!(
            "Failed to create models directory {}: {error}",
            models_dir.display()
        )
    })?;

    let snapshot = crate::core::preset_models::build_model_catalog_snapshot(&models_dir);
    Ok(crate::core::preset_models::resolve_model_catalog_selected_ids(&snapshot, &paths))
}

#[cfg(target_os = "windows")]
fn set_mute_windows(mute: bool) -> Result<(), String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoInitialize};

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

/// Initializes and runs the Tauri application.
///
/// Sets up the download state, plugins (opener, dialog, fs, shell, http),
/// and registers invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_settings = crate::app::settings::AppSettings::new();
    let log_level_filter = app_settings.log_level_filter();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Trace)
                .filter(move |metadata| log_level_filter.should_log(metadata))
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
            let app_handle_for_listener = app.handle().clone();
            let controller = app_handle_for_listener.state::<ApiServerController>();

            let initial_config = load_online_asr_config(&app_handle_for_listener);
            let config_for_init = controller.online_asr_config.clone();
            tauri::async_runtime::spawn(async move {
                *config_for_init.write().await = initial_config;
            });

            let config_for_listener = controller.online_asr_config.clone();
            let listener_app_handle = app_handle_for_listener.clone();
            app.listen_any("asr-config-updated", move |_event| {
                let config_for_listener = config_for_listener.clone();
                let app_handle = listener_app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let new_config_map = load_online_asr_config(&app_handle);
                    *config_for_listener.write().await = new_config_map;
                });
            });

            crate::app::tray::setup_tray(app)?;

            // Start HTTP API Server if enabled
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let app_data_dir = match app_handle.path().app_data_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        log::error!("Failed to get app_data_dir: {}", e);
                        return;
                    }
                };
                let config_path = app_data_dir.join("settings.json");
                let mut http_server_enabled = false;
                let mut host = "127.0.0.1".to_string();
                let mut port = 14200;
                let mut api_key = "".to_string();
                let mut max_concurrent = 2;
                let mut max_queue_size = 100;
                let mut max_upload_size_mb = 50;
                let mut job_ttl_minutes = 60;
                let mut max_streaming = 2;
                let mut ip_whitelist = "localhost".to_string();

                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(config) = json.get("sona-config") {
                            if let Some(enabled) =
                                config.get("httpServerEnabled").and_then(|v| v.as_bool())
                            {
                                http_server_enabled = enabled;
                            }
                            if let Some(h) = config.get("httpServerHost").and_then(|v| v.as_str()) {
                                host = h.to_string();
                            }
                            if let Some(p) = config.get("httpServerPort").and_then(|v| v.as_u64()) {
                                port = p as u16;
                            }
                            if let Some(key) =
                                config.get("httpServerApiKey").and_then(|v| v.as_str())
                            {
                                api_key = key.to_string();
                            }
                            if let Some(mc) = config
                                .get("httpServerMaxConcurrent")
                                .and_then(|v| v.as_u64())
                            {
                                max_concurrent = mc as usize;
                            }
                            if let Some(mq) = config
                                .get("httpServerMaxQueueSize")
                                .and_then(|v| v.as_u64())
                            {
                                max_queue_size = mq as usize;
                            }
                            if let Some(ms) = config
                                .get("httpServerMaxUploadSizeMB")
                                .and_then(|v| v.as_u64())
                            {
                                max_upload_size_mb = ms as usize;
                            }
                            if let Some(ttl) = config
                                .get("httpServerJobTtlMinutes")
                                .and_then(|v| v.as_u64())
                            {
                                job_ttl_minutes = ttl;
                            }
                            if let Some(ms) = config
                                .get("httpServerMaxStreaming")
                                .and_then(|v| v.as_u64())
                            {
                                max_streaming = ms as usize;
                            }
                            if let Some(ip_list) =
                                config.get("httpServerIpWhitelist").and_then(|v| v.as_str())
                            {
                                ip_whitelist = ip_list.to_string();
                            }
                        }
                    }
                }

                if http_server_enabled {
                    let app_local_data_dir = match app_handle.path().app_local_data_dir() {
                        Ok(dir) => dir,
                        Err(e) => {
                            log::error!("Failed to get app_local_data_dir: {}", e);
                            return;
                        }
                    };
                    let temp_dir = app_local_data_dir.join("api_temp");
                    let models_dir = app_local_data_dir.join("models");

                    let parsed_whitelist = match crate::app::server::parse_ip_whitelist(
                        &ip_whitelist,
                    ) {
                        Ok(nets) => nets,
                        Err(e) => {
                            log::error!(
                                "HTTP API Server failed to start due to invalid IP whitelist: {}",
                                e
                            );
                            return;
                        }
                    };
                    let parsed_arc = std::sync::Arc::new(parsed_whitelist);

                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let controller = app_handle.state::<ApiServerController>();
                    *controller.shutdown_sender.lock().await = Some(tx);

                    let online_asr_config = controller.online_asr_config.clone();

                    if let Err(e) = crate::app::server::run_server(
                        Some(app_handle.clone()),
                        &host,
                        port,
                        &api_key,
                        temp_dir,
                        models_dir,
                        max_concurrent,
                        max_queue_size,
                        max_upload_size_mb,
                        job_ttl_minutes,
                        max_streaming,
                        parsed_arc,
                        online_asr_config,
                        rx,
                    )
                    .await
                    {
                        log::error!("HTTP API Server failed: {}", e);
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<crate::app::settings::AppSettings>();
                // Default to true if lock fails (safe fallback)
                let minimize = state.minimize_to_tray.lock().map(|v| *v).unwrap_or(true);

                match crate::app::settings::resolve_main_window_close_action(
                    window.label(),
                    minimize,
                ) {
                    crate::app::settings::MainWindowCloseAction::Ignore => {}
                    crate::app::settings::MainWindowCloseAction::HideToTray => {
                        let _ = window.hide();
                        api.prevent_close();
                    }
                    crate::app::settings::MainWindowCloseAction::RequestQuit => {
                        api.prevent_close();
                        let _ = window.emit(crate::app::tray::TRAY_REQUEST_QUIT_EVENT, ());
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
        .manage(crate::repositories::downloads::DownloadState::new())
        .manage(ApiServerController::default())
        .manage(app_settings)
        .manage(crate::app::window_state::AuxWindowStateStore::default())
        .manage(crate::core::automation::AutomationRuntimeState::default())
        .manage(crate::repositories::history::HistoryRepositoryState::default())
        .manage(crate::repositories::history::PreparedBackupImportState::default())
        .manage(crate::integrations::audio::AudioState::new())
        .manage(crate::integrations::asr::AsrState::new())
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
            crate::repositories::archive::extract_tar_bz2,
            crate::repositories::archive::create_tar_bz2,
            crate::dashboard::get_dashboard_snapshot,
            crate::repositories::project::project_list,
            crate::repositories::project::project_save_all,
            crate::repositories::project::project_create,
            crate::repositories::project::project_update,
            crate::repositories::project::project_delete,
            crate::repositories::project::project_reorder,
            crate::repositories::project::project_get_active_id,
            crate::repositories::project::project_set_active_id,
            crate::repositories::automation::automation_load_repository_state,
            crate::repositories::automation::automation_persist_rules,
            crate::repositories::automation::automation_persist_processed_entries,
            crate::repositories::automation::automation_persist_repository_state,
            crate::repositories::automation::automation_validate_rule_activation,
            crate::repositories::history::commands::history_list_items,
            crate::repositories::history::commands::history_query_workspace,
            crate::repositories::history::commands::history_create_live_draft,
            crate::repositories::history::commands::history_complete_live_draft,
            crate::repositories::history::commands::history_save_recording,
            crate::repositories::history::commands::history_save_imported_file,
            crate::repositories::history::commands::history_delete_items,
            crate::repositories::history::commands::history_load_transcript,
            crate::repositories::history::commands::history_update_transcript,
            crate::repositories::history::commands::history_create_transcript_snapshot,
            crate::repositories::history::commands::history_list_transcript_snapshots,
            crate::repositories::history::commands::history_load_transcript_snapshot,
            crate::repositories::history::commands::history_build_transcript_diff,
            crate::repositories::history::commands::history_restore_transcript_diff_rows,
            crate::repositories::history::commands::history_update_item_meta,
            crate::repositories::history::commands::history_update_project_assignments,
            crate::repositories::history::commands::history_reassign_project,
            crate::repositories::history::commands::history_load_summary,
            crate::repositories::history::commands::history_save_summary,
            crate::repositories::history::commands::history_delete_summary,
            crate::repositories::history::commands::history_resolve_audio_path,
            crate::repositories::history::commands::history_open_folder,
            crate::repositories::history::commands::export_backup_archive,
            crate::repositories::history::commands::prepare_backup_import,
            crate::repositories::history::commands::apply_prepared_history_import,
            crate::repositories::history::commands::dispose_prepared_backup_import,
            crate::repositories::downloads::download_file,
            crate::integrations::webdav::webdav_test_connection,
            crate::integrations::webdav::webdav_list_backups,
            crate::integrations::webdav::webdav_upload_backup,
            crate::integrations::webdav::webdav_download_backup,
            crate::repositories::downloads::cancel_download,
            crate::core::preset_models::get_model_catalog_snapshot,
            resolve_model_catalog_selected_ids,
            crate::core::diagnostics::get_diagnostics_core_snapshot,
            crate::app::hardware::check_gpu_availability,
            force_exit,
            crate::repositories::downloads::has_active_downloads,
            crate::app::tray::update_tray_menu,
            crate::app::settings::set_minimize_to_tray,
            crate::app::settings::set_log_level,
            crate::app::window_state::set_aux_window_state,
            crate::app::window_state::get_aux_window_state,
            crate::app::window_state::clear_aux_window_state,
            set_system_audio_mute,
            crate::app::runtime_status::open_log_folder,
            crate::app::runtime_status::get_runtime_environment_status,
            crate::app::runtime_status::get_path_statuses,
            crate::core::task_ledger::commands::task_ledger_load_snapshot,
            crate::core::task_ledger::commands::task_ledger_upsert_task,
            crate::core::task_ledger::commands::task_ledger_patch_task,
            crate::core::task_ledger::commands::task_ledger_remove_task,
            crate::core::task_ledger::commands::task_ledger_clear_resolved,
            crate::core::recovery::commands::recovery_load_snapshot,
            crate::core::recovery::commands::recovery_save_snapshot,
            crate::core::recovery::commands::recovery_persist_queue_snapshot,
            crate::core::automation::replace_automation_runtime_rules,
            crate::core::automation::scan_automation_runtime_rule,
            crate::core::automation::collect_automation_runtime_rule_paths,
            crate::core::config::migrate_app_config,
            crate::core::config::resolve_effective_config,
            crate::app::system::inject_text,
            crate::app::system::get_mouse_position,
            crate::app::system::get_text_cursor_position,
            crate::integrations::audio::get_system_audio_devices,
            crate::integrations::audio::start_system_audio_capture,
            crate::integrations::audio::stop_system_audio_capture,
            crate::integrations::audio::set_system_audio_capture_paused,
            crate::integrations::audio::set_microphone_boost,
            crate::integrations::audio::get_microphone_devices,
            crate::integrations::audio::start_microphone_capture,
            crate::integrations::audio::stop_microphone_capture,
            crate::integrations::audio::set_microphone_capture_paused,
            crate::integrations::llm::generate_llm_text,
            crate::integrations::llm::list_llm_models,
            crate::integrations::llm::llm_usage::llm_usage_ensure_storage,
            crate::integrations::llm::llm_usage::llm_usage_read_raw,
            crate::integrations::llm::llm_usage::llm_usage_replace_raw,
            crate::integrations::llm::polish_transcript_segments,
            crate::integrations::llm::run_transcript_llm_job,
            crate::integrations::llm::summarize_transcript,
            crate::integrations::llm::translate_transcript_segments,
            crate::integrations::asr::init_recognizer,
            crate::integrations::asr::start_recognizer,
            crate::integrations::asr::stop_recognizer,
            crate::integrations::asr::flush_recognizer,
            crate::integrations::asr::feed_audio_chunk,
            crate::integrations::asr::process_batch_file,
            crate::integrations::asr::get_asr_runtime_metrics,
            crate::repositories::export::export_transcript_file,
            crate::integrations::speaker::annotate_speaker_segments_from_file,
            crate::integrations::speaker::import_speaker_profile_sample,
            crate::core::speaker_review::build_speaker_review_snapshot,
            crate::core::speaker_correction::apply_speaker_profile_to_group,
            crate::core::speaker_correction::reset_speaker_group_to_anonymous,
            crate::core::speaker_correction::confirm_speaker_group_review,
            start_api_server,
            stop_api_server,
            check_media_formats
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
