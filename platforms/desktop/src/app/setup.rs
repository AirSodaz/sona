use std::sync::Arc;
use tauri::{Listener, Manager};

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle_for_listener = app.handle().clone();

    let (db, app_local_data_dir) =
        crate::platform::database::open_and_migrate_sqlite_for_app(&app_handle_for_listener)?;

    let dashboard_service = crate::platform::dashboard::create_dashboard_service(
        app_local_data_dir.clone(),
        Arc::clone(&db),
    );

    app.manage(dashboard_service);
    app.manage(db);

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
