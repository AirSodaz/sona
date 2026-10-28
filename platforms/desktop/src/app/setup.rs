use std::sync::Arc;
use tauri::{Listener, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    log::info!(
        "Sona Desktop starting | version={} | channel={} | commit={} | dirty={} | build_time={}",
        env!("SONA_DISPLAY_VERSION"),
        env!("SONA_BUILD_CHANNEL"),
        env!("SONA_COMMIT_SHA"),
        env!("SONA_IS_DIRTY"),
        env!("SONA_BUILD_TIMESTAMP")
    );
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
    let connection_tracker = crate::platform::agent_control::ActiveConnectionTracker::new();
    let mcp_controller = crate::platform::agent_control::AgentControlServerController::new(
        connection_tracker.clone(),
    );
    app.manage(agent_facade.clone());
    app.manage(connection_tracker);
    app.manage(mcp_controller.clone());

    let mcp_config = agent_facade.load_app_config();
    if mcp_config.mcp_enabled.unwrap_or(true) {
        mcp_controller.start(Some(app.handle().clone()), agent_facade);
    }
    let emby_state = crate::integrations::emby::EmbyState::default();
    if let Some(config_val) = initial_config.as_ref() {
        let server_url = config_val
            .get("embyServerUrl")
            .or_else(|| config_val.get("emby_server_url"))
            .and_then(|v| v.as_str());
        let api_key = config_val
            .get("embyApiKey")
            .or_else(|| config_val.get("emby_api_key"))
            .and_then(|v| v.as_str());
        let allow_invalid_certs = config_val
            .get("embyAllowInvalidCerts")
            .or_else(|| config_val.get("emby_allow_invalid_certs"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if let (Some(url), Some(key)) = (server_url, api_key)
            && !url.trim().is_empty()
            && !key.trim().is_empty()
            && let Ok(client) = crate::integrations::emby::EmbyClient::new(
                crate::integrations::emby::EmbyConnectionConfig {
                    server_url: url.to_string(),
                    api_key: key.to_string(),
                    allow_invalid_certs,
                },
            )
        {
            let emby_state_clone = emby_state.clone();
            tauri::async_runtime::spawn(async move {
                emby_state_clone.set_client(client).await;
            });
        }
    }
    app.manage(emby_state);
    app.manage(desktop_services);
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
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        if let Err(e) = app.deep_link().register_all() {
            log::warn!("[DeepLink] Failed to register deep link scheme: {e}");
        }
    }

    let deep_link_app_handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        let urls: Vec<String> = event.urls().into_iter().map(|u| u.to_string()).collect();
        let should_stay_silent = matches!(
            crate::app::window::parse_launch_visibility_from_args(&urls),
            Some(crate::app::window::LaunchVisibility::Silent)
        );
        if !should_stay_silent && let Some(window) = deep_link_app_handle.get_webview_window("main")
        {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    });

    crate::app::server::start_from_app_handle(&app.handle().clone());

    Ok(())
}
