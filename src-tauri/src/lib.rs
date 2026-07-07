pub mod app;
pub mod commands;
pub mod integrations;
pub mod platform;

#[cfg(all(test, target_os = "windows", target_env = "msvc"))]
#[link(name = "windows-test-manifest")]
unsafe extern "C" {}

use tauri::{Emitter, Manager};

/// Initializes and runs the Tauri application.
///
/// Sets up the download state, plugins (opener, dialog, fs, shell, http),
/// and registers invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = run_app() {
        panic!("error while running tauri application: {:?}", e);
    }
}

#[cfg(target_os = "windows")]
pub fn init_dll_directory() {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::LibraryLoader::SetDllDirectoryW;
    use windows::core::PCWSTR;

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    {
        let candidates = [
            exe_dir.join("resources").join("shared_libs"),
            exe_dir
                .join("..")
                .join("..")
                .join("resources")
                .join("shared_libs"),
            exe_dir
                .join("..")
                .join("..")
                .join("..")
                .join("resources")
                .join("shared_libs"),
        ];

        for path in &candidates {
            if path.exists() {
                let mut path_u16: Vec<u16> = path.as_os_str().encode_wide().collect();
                path_u16.push(0);
                unsafe {
                    let _ = SetDllDirectoryW(PCWSTR(path_u16.as_ptr()));
                }
                break;
            }
        }
    }
}

pub fn run_app() -> Result<(), tauri::Error> {
    #[cfg(target_os = "windows")]
    init_dll_directory();

    let app_settings = crate::app::settings::AppSettings::new();
    let log_level_filter = app_settings.log_level_filter();

    // Enable browser reload, devtools, and right-click context menu in debug builds;
    // block them in production builds for a native desktop application experience.
    #[cfg(debug_assertions)]
    let prevent_default = tauri_plugin_prevent_default::debug();
    #[cfg(not(debug_assertions))]
    let prevent_default = tauri_plugin_prevent_default::Builder::new().build();

    #[cfg(debug_assertions)]
    {
        tauri_specta::Builder::<tauri::Wry>::new()
            .typ::<sona_core::domain::LlmProvider>()
            .typ::<sona_core::domain::PolishPresetId>()
            .typ::<sona_core::domain::SummaryTemplateId>()
            .export(
                specta_typescript::Typescript::default(),
                "../src/bindings.ts",
            )
            .expect("Failed to export typescript bindings");
    }

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
        .setup(crate::app::setup::init)
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
        .manage(crate::commands::downloads::DownloadState::new())
        .manage(crate::app::server::ApiServerController::default())
        .manage(app_settings)
        .manage(crate::app::window_state::AuxWindowStateStore::default())
        .manage(crate::platform::automation_runtime::AutomationRuntimeState::default())
        .manage(crate::platform::history_repository::HistoryRepositoryState::default())
        .manage(crate::platform::history_repository::PreparedBackupImportState::default())
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
        .plugin(prevent_default)
        .invoke_handler(crate::commands::get_handlers())
        .run(tauri::generate_context!())
}
