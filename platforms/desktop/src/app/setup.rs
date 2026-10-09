use std::sync::Arc;
use tauri::{Listener, Manager};

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle_for_listener = app.handle().clone();
    crate::platform::storage_location::cleanup_pending_storage_locations_for_app(
        &app_handle_for_listener,
    );

    let (db, app_local_data_dir) =
        crate::platform::database::open_and_migrate_sqlite_for_app(&app_handle_for_listener)?;

    let dashboard_service = crate::platform::dashboard::create_dashboard_service(
        app_local_data_dir.clone(),
        Arc::clone(&db),
    );
    let history_dir = app_local_data_dir.join("history");
    if let Err(e) = app
        .asset_protocol_scope()
        .allow_directory(&history_dir, true)
    {
        log::warn!("Failed to allow history directory in asset scope: {e}");
    }
    if let Ok(default_dir) = app.path().app_local_data_dir() {
        let default_history = default_dir.join("history");
        if default_history != history_dir
            && let Err(e) = app
                .asset_protocol_scope()
                .allow_directory(&default_history, true)
        {
            log::warn!("Failed to allow default history directory in asset scope: {e}");
        }
    }
    let sqlite_context = Arc::new(sona_sqlite::SqliteApplicationContext::from_database(
        app_local_data_dir.clone(),
        db.clone(),
    )?);

    let initial_config = {
        let adapter = sqlite_context.app_config_adapter(Arc::new(sona_runtime_fs::SystemClock));
        adapter.load_config().ok().flatten()
    };

    if let Some(minimize) = initial_config.as_ref().and_then(|config_val| {
        config_val
            .get("minimizeToTrayOnExit")
            .or_else(|| config_val.get("minimize_to_tray_on_exit"))
            .and_then(|v| v.as_bool())
    }) {
        let settings = app.state::<crate::app::settings::AppSettings>();
        settings.set_minimize_to_tray_enabled(minimize);
    }

    let start_silently = crate::app::window::should_start_silently_from_args_and_config(
        std::env::args(),
        initial_config.as_ref(),
    );

    let dashboard_state = crate::platform::dashboard::DesktopDashboardState::new(dashboard_service);
    let sqlite_state = crate::platform::database::DesktopSqliteState::new(sqlite_context);
    app.manage(dashboard_state);
    let sqlite_for_services = sqlite_state.clone();
    app.manage(sqlite_state);
    let sync_config_path = app_local_data_dir.join(crate::platform::sync::SYNC_CONFIG_FILE);

    let audio_state = Arc::new(crate::integrations::audio::AudioState::new());
    let asr_state = Arc::new(crate::integrations::asr::AsrState::new());
    let download_state = Arc::new(crate::platform::model_downloads::DownloadState::new());
    let history_state = crate::platform::history_repository::HistoryRepositoryState::default();
    let backup_state = crate::platform::history_repository::PreparedBackupImportState::default();
    let event_emitter = Arc::new(crate::platform::event::TauriEventEmitter(
        app_handle_for_listener.clone(),
    )) as Arc<dyn crate::platform::event::EventEmitterPort>;

    let desktop_services = crate::services::DesktopServices::builder()
        .sqlite(sqlite_for_services)
        .event_emitter(event_emitter)
        .sync_config_path(sync_config_path)
        .history_state(history_state.clone())
        .backup_state(backup_state.clone())
        .audio(Arc::clone(&audio_state))
        .asr(Arc::clone(&asr_state))
        .downloads(Arc::clone(&download_state))
        .build()
        .map_err(|e| format!("Failed to build DesktopServices: {e}"))?;

    let agent_facade = crate::platform::agent_control::AgentControlFacade::new(
        desktop_services.clone(),
        Some(app.handle().clone()),
    );
    crate::platform::agent_control::start_agent_control_ipc_server(agent_facade);
    app.manage(desktop_services);
    app.manage((*audio_state).clone());
    app.manage((*asr_state).clone());
    app.manage((*download_state).clone());
    app.manage(history_state);
    app.manage(backup_state);
    crate::app::window::create_main_window(app.handle(), start_silently)?;
    let listener_app_handle = app_handle_for_listener.clone();
    app.listen_any("asr-config-updated", move |_event| {
        let app_handle = listener_app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let new_config_map =
                crate::platform::api_server_config::load_online_asr_config_for_app(&app_handle);
            let controller = app_handle.state::<crate::app::server::ApiServerController>();
            crate::app::server::refresh_online_asr_config(&controller, new_config_map).await;
        });
    });

    crate::app::tray::setup_tray(app)?;

    crate::app::server::start_from_app_handle(&app.handle().clone());

    Ok(())
}
