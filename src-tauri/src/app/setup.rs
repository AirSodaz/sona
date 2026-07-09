use std::sync::Arc;
use tauri::{Listener, Manager};

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle_for_listener = app.handle().clone();
    let controller = app_handle_for_listener.state::<crate::app::server::ApiServerController>();

    let (db, app_local_data_dir) =
        crate::platform::database::open_and_migrate_sqlite_for_app(&app_handle_for_listener)?;

    let dashboard_service = crate::platform::dashboard::create_dashboard_service(
        app_local_data_dir.clone(),
        Arc::clone(&db),
    );

    app.manage(dashboard_service);
    app.manage(db);

    let config_for_listener = controller.online_asr_config.clone();
    let listener_app_handle = app_handle_for_listener.clone();
    app.listen_any("asr-config-updated", move |_event| {
        let config_for_listener = config_for_listener.clone();
        let app_handle = listener_app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let path_provider = crate::platform::paths::TauriPathProvider::from_app(&app_handle);
            let new_config_map = crate::app::server::load_online_asr_config(&path_provider);
            *config_for_listener.write().await = new_config_map;
        });
    });

    crate::app::tray::setup_tray(app)?;

    crate::app::server::start_from_app_handle(&app.handle().clone());

    Ok(())
}
