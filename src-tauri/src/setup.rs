use tauri::{Manager, Listener};

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle_for_listener = app.handle().clone();
    let controller = app_handle_for_listener.state::<crate::app::server::ApiServerController>();

    let initial_config = crate::app::server::load_online_asr_config(&app_handle_for_listener);
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
            let new_config_map = crate::app::server::load_online_asr_config(&app_handle);
            *config_for_listener.write().await = new_config_map;
        });
    });

    crate::app::tray::setup_tray(app)?;

    crate::app::server::start_from_app_handle(&app.handle().clone());

    Ok(())
}
