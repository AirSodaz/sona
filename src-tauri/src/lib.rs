pub mod app;
pub mod commands;
pub mod core;
pub mod integrations;
pub mod repositories;

pub mod cli;
mod dashboard;
pub mod setup;

use tauri::{Emitter, Manager};

/// Initializes and runs the Tauri application.
///
/// Sets up the download state, plugins (opener, dialog, fs, shell, http),
/// and registers invoke handlers.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_settings = crate::app::settings::AppSettings::new();
    let log_level_filter = app_settings.log_level_filter();

    #[cfg(debug_assertions)]
    {
        tauri_specta::Builder::<tauri::Wry>::new()
            .typ::<crate::core::domain::LlmProvider>()
            .typ::<crate::core::domain::PolishPresetId>()
            .typ::<crate::core::domain::SummaryTemplateId>()
            .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
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
        .setup(crate::setup::init)
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
        .manage(crate::app::server::ApiServerController::default())
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
        .invoke_handler(crate::commands::get_handlers())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
