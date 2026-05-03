mod app_settings;
mod archive;
mod audio;
mod automation_runtime;
mod aux_window_state;
pub mod cli;
mod dashboard;
mod downloads;
pub mod export;
mod hardware;
mod history_repository;
mod llm;
pub mod pipeline;
pub mod preset_models;
mod runtime_status;
pub mod sherpa;
pub mod speaker;
pub mod system;
mod text_alignment;
mod tray;
mod webdav;

use tauri::{Emitter, Manager};

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

/// Initializes and runs the Tauri application.
///
/// Sets up the download state, plugins (opener, dialog, fs, shell, http),
/// and registers invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_settings = app_settings::AppSettings::new();
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
        .setup(|app| tray::setup_tray(app))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<app_settings::AppSettings>();
                // Default to true if lock fails (safe fallback)
                let minimize = state.minimize_to_tray.lock().map(|v| *v).unwrap_or(true);

                match app_settings::resolve_main_window_close_action(window.label(), minimize) {
                    app_settings::MainWindowCloseAction::Ignore => {}
                    app_settings::MainWindowCloseAction::HideToTray => {
                        let _ = window.hide();
                        api.prevent_close();
                    }
                    app_settings::MainWindowCloseAction::RequestQuit => {
                        api.prevent_close();
                        let _ = window.emit(tray::TRAY_REQUEST_QUIT_EVENT, ());
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
        .manage(downloads::DownloadState::new())
        .manage(app_settings)
        .manage(aux_window_state::AuxWindowStateStore::default())
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
            archive::extract_tar_bz2,
            archive::create_tar_bz2,
            dashboard::get_dashboard_snapshot,
            history_repository::commands::history_list_items,
            history_repository::commands::history_query_workspace,
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
            downloads::download_file,
            webdav::webdav_test_connection,
            webdav::webdav_list_backups,
            webdav::webdav_upload_backup,
            webdav::webdav_download_backup,
            downloads::cancel_download,
            hardware::check_gpu_availability,
            force_exit,
            downloads::has_active_downloads,
            tray::update_tray_menu,
            app_settings::set_minimize_to_tray,
            app_settings::set_log_level,
            aux_window_state::set_aux_window_state,
            aux_window_state::get_aux_window_state,
            aux_window_state::clear_aux_window_state,
            set_system_audio_mute,
            runtime_status::open_log_folder,
            runtime_status::get_runtime_environment_status,
            runtime_status::get_path_statuses,
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
            sherpa::get_asr_runtime_metrics,
            export::export_transcript_file,
            speaker::annotate_speaker_segments_from_file,
            speaker::import_speaker_profile_sample
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
