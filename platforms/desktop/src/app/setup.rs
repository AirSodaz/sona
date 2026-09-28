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
    let sqlite_context = Arc::new(sona_sqlite::SqliteApplicationContext::from_database(
        app_local_data_dir,
        db,
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

    app.manage(dashboard_service);
    app.manage(sqlite_context);
    crate::app::window::create_main_window(app.handle(), start_silently)?;
    crate::platform::model_downloads::try_auto_activate_cuda_addon(app.handle());

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
