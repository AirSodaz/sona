use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, State};

// Command wrappers & implementations

#[tauri::command]
pub fn greet(name: &str) -> String {
    crate::platform::system::greet(name)
}

#[tauri::command]
pub fn force_exit(app: AppHandle) {
    crate::platform::system::force_exit(app);
}

#[tauri::command]
pub fn inject_text(
    text: String,
    shortcut_modifiers: Option<Vec<crate::platform::system::ShortcutModifier>>,
) -> Result<(), String> {
    crate::platform::system::inject_text(text, shortcut_modifiers)
}

#[tauri::command]
pub fn get_mouse_position() -> Result<(i32, i32), String> {
    crate::platform::system::get_mouse_position()
}

#[tauri::command]
pub fn get_text_cursor_position() -> Result<Option<(i32, i32)>, String> {
    crate::platform::system::get_text_cursor_position()
}

#[tauri::command]
pub fn get_focused_selection_text() -> Result<Option<String>, String> {
    crate::platform::system::get_focused_selection_text()
}

#[tauri::command]
pub fn get_foreground_window_info()
-> Result<Option<crate::platform::system::ForegroundWindowInfo>, String> {
    crate::platform::system::get_foreground_window_info()
}

#[tauri::command]
pub fn focus_window(app: AppHandle, label: String) -> Result<(), String> {
    crate::platform::system::focus_window(&app, &label)
}
#[tauri::command]
pub async fn get_dashboard_snapshot(
    dashboard_state: State<'_, crate::platform::dashboard::DesktopDashboardState>,
    request: crate::app::dashboard::DashboardSnapshotRequest,
) -> Result<sona_core::dashboard::models::DashboardSnapshotDomainModel, String> {
    let active_service = dashboard_state.current_service()?;
    crate::app::dashboard::get_dashboard_snapshot_with_service(&active_service, request).await
}

#[tauri::command]
pub async fn check_gpu_availability() -> Result<bool, String> {
    crate::platform::hardware::check_gpu_availability().await
}

#[tauri::command]
pub async fn update_tray_menu(
    app: AppHandle,
    show_text: String,
    settings_text: String,
    updates_text: String,
    quit_text: String,
    caption_text: String,
    caption_checked: bool,
) -> Result<(), String> {
    crate::app::tray::update_tray_menu(
        app,
        show_text,
        settings_text,
        updates_text,
        quit_text,
        caption_text,
        caption_checked,
    )
    .await
}

#[tauri::command]
pub fn set_minimize_to_tray(state: State<'_, crate::app::settings::AppSettings>, enabled: bool) {
    crate::app::settings::set_minimize_to_tray(state, enabled);
}
#[tauri::command]
pub fn set_auto_start<R: Runtime>(app: AppHandle<R>, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| e.to_string())
    } else {
        autolaunch.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn is_auto_start_enabled<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_log_level(
    state: State<'_, crate::app::settings::AppSettings>,
    level: String,
) -> Result<(), String> {
    crate::app::settings::set_log_level(state, level)
}

/// Recolors the main window's native frame to match the resolved app theme.
///
/// Called by the frontend whenever the effective theme changes, including
/// `auto` transitions driven by the OS color-scheme. Returns `Ok(())` on
/// platforms where the native frame needs no theming.
#[tauri::command]
pub fn set_window_theme(
    app: AppHandle,
    theme: crate::platform::system::ResolvedTheme,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window(crate::app::window::MAIN_WINDOW_LABEL) else {
        // The window can already be gone during shutdown; nothing to theme.
        return Ok(());
    };

    crate::app::window::apply_main_window_chrome(&window, theme)
}

#[tauri::command]
pub fn set_aux_window_state(
    state: State<'_, crate::app::window_state::AuxWindowStateStore>,
    label: String,
    payload: Value,
) -> Result<(), String> {
    crate::app::window_state::set_aux_window_state(state, label, payload)
}

#[tauri::command]
pub fn get_aux_window_state(
    state: State<'_, crate::app::window_state::AuxWindowStateStore>,
    label: String,
) -> Result<Option<Value>, String> {
    crate::app::window_state::get_aux_window_state(state, label)
}

#[tauri::command]
pub fn clear_aux_window_state(
    state: State<'_, crate::app::window_state::AuxWindowStateStore>,
    label: String,
) -> Result<(), String> {
    crate::app::window_state::clear_aux_window_state(state, label)
}

#[tauri::command]
pub async fn open_log_folder(app: AppHandle) -> Result<(), String> {
    crate::platform::runtime_status::open_log_folder(app).await
}

#[tauri::command]
pub async fn get_runtime_environment_status(
    app: AppHandle,
) -> Result<crate::platform::runtime_status::RuntimeEnvironmentStatus, String> {
    crate::platform::runtime_status::get_runtime_environment_status(app).await
}

#[tauri::command]
pub async fn get_path_statuses(
    paths: Vec<String>,
) -> Result<Vec<crate::platform::runtime_status::RuntimePathStatus>, String> {
    crate::platform::runtime_status::get_path_statuses(paths).await
}

#[tauri::command]
pub async fn get_model_catalog_snapshot(
    app: AppHandle,
) -> Result<crate::platform::preset_models::ModelCatalogSnapshot, String> {
    crate::platform::preset_models::get_model_catalog_snapshot_for_app(&app).await
}

#[tauri::command(rename = "resolve_model_catalog_selected_ids")]
pub async fn resolve_model_catalog_selected_ids_command(
    app: AppHandle,
    paths: crate::platform::preset_models::ModelSelectionPaths,
) -> Result<crate::platform::preset_models::ModelCatalogSelectedIds, String> {
    crate::platform::preset_models::resolve_model_catalog_selected_ids_for_app(&app, paths).await
}

#[tauri::command]
pub async fn get_diagnostics_core_snapshot(
    app: AppHandle,
    state: State<'_, crate::integrations::asr::AsrState>,
    input: crate::platform::diagnostics::DiagnosticsCoreInput,
) -> Result<crate::platform::diagnostics::DiagnosticsCoreSnapshot, String> {
    crate::platform::diagnostics::get_diagnostics_core_snapshot_for_app(&app, state, input).await
}

#[tauri::command]
pub async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    crate::platform::media_detector::check_media_formats(paths).await
}
