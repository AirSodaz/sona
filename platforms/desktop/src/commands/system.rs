use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, State};

// Command wrappers & implementations

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
#[allow(clippy::too_many_arguments)]
pub async fn update_tray_menu(
    app: AppHandle,
    show_text: String,
    settings_text: String,
    updates_text: String,
    quit_text: String,
    caption_text: String,
    caption_checked: bool,
    disconnect_agent_text: Option<String>,
    stop_recording_text: Option<String>,
) -> Result<(), String> {
    crate::app::tray::update_tray_menu(
        app,
        show_text,
        settings_text,
        updates_text,
        quit_text,
        caption_text,
        caption_checked,
        disconnect_agent_text,
        stop_recording_text,
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
    services: State<'_, crate::services::DesktopServices>,
    input: crate::platform::diagnostics::DiagnosticsCoreInput,
) -> Result<crate::platform::diagnostics::DiagnosticsCoreSnapshot, String> {
    crate::platform::diagnostics::get_diagnostics_core_snapshot_for_app(&app, &services.asr, input)
        .await
}

#[tauri::command]
pub async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    crate::platform::media_detector::check_media_formats(paths).await
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopShimStatus {
    pub shim_dir: String,
    pub in_current_path: bool,
    pub in_user_path: bool,
    pub is_configured: bool,
    pub installed_shims: Vec<String>,
    pub default_channel: String,
    pub current_channel: String,
    pub stable_installed: bool,
    pub nightly_installed: bool,
}

#[tauri::command]
pub async fn get_shim_status(app: AppHandle) -> Result<DesktopShimStatus, String> {
    let status = sona_shim::installer::check_path_status();
    let default_ch = sona_shim::registry::get_default_channel();
    let stable_installed =
        sona_shim::registry::is_channel_installed(sona_shim::registry::Channel::Stable);
    let nightly_installed =
        sona_shim::registry::is_channel_installed(sona_shim::registry::Channel::Nightly);

    let is_nightly = {
        let identifier = app.config().identifier.to_lowercase();
        let product_name = app
            .config()
            .product_name
            .as_deref()
            .unwrap_or("")
            .to_lowercase();
        let version = app.config().version.as_deref().unwrap_or("");
        let env_channel = std::env::var("SONA_CHANNEL")
            .unwrap_or_default()
            .to_lowercase();

        env_channel == "nightly"
            || identifier.ends_with(".nightly")
            || product_name.contains("nightly")
            || version.contains('-')
    };

    let current_channel = if is_nightly { "nightly" } else { "stable" };
    let is_configured =
        (status.in_current_path || status.in_user_path) && !status.installed_shims.is_empty();

    Ok(DesktopShimStatus {
        shim_dir: status.shim_dir.to_string_lossy().to_string(),
        in_current_path: status.in_current_path,
        in_user_path: status.in_user_path,
        is_configured,
        installed_shims: status.installed_shims,
        default_channel: default_ch.as_str().to_string(),
        current_channel: current_channel.to_string(),
        stable_installed,
        nightly_installed,
    })
}

#[tauri::command]
pub async fn install_shim(app: AppHandle) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let is_nightly = {
        let identifier = app.config().identifier.to_lowercase();
        let product_name = app
            .config()
            .product_name
            .as_deref()
            .unwrap_or("")
            .to_lowercase();
        let version = app.config().version.as_deref().unwrap_or("");
        let env_channel = std::env::var("SONA_CHANNEL")
            .unwrap_or_default()
            .to_lowercase();

        env_channel == "nightly"
            || identifier.ends_with(".nightly")
            || product_name.contains("nightly")
            || version.contains('-')
    };
    let channel = if is_nightly {
        sona_shim::registry::Channel::Nightly
    } else {
        sona_shim::registry::Channel::Stable
    };

    let parent = current_exe
        .parent()
        .ok_or_else(|| "Failed to find parent directory".to_string())?;
    let shim_binary_name = if cfg!(windows) {
        "sona-shim.exe"
    } else {
        "sona-shim"
    };
    let mut source_shim = parent.join(shim_binary_name);

    if !source_shim.exists() {
        for rel in &[
            "../debug",
            "../release",
            "../../target/debug",
            "../../target/release",
        ] {
            let candidate = parent.join(rel).join(shim_binary_name);
            if candidate.exists() {
                source_shim = candidate;
                break;
            }
        }
    }

    if !source_shim.exists() {
        return Err(format!("Could not find {} to install", shim_binary_name));
    }

    let version_str = app.config().version.as_deref();
    let exe_name = current_exe.file_name().and_then(|n| n.to_str());
    let reg_tuple = Some((channel, parent, version_str, exe_name));

    sona_shim::installer::install_shims(&source_shim, reg_tuple).map(|_| ())
}

#[tauri::command]
pub async fn uninstall_shim() -> Result<(), String> {
    // From Settings UI, remove shims and PATH without unregistering the application channel
    sona_shim::installer::uninstall_shims(None, true).map(|_| ())
}
#[tauri::command]
pub async fn set_default_channel(channel: String) -> Result<(), String> {
    let ch = sona_shim::registry::Channel::parse(&channel).ok_or_else(|| {
        format!(
            "Invalid channel '{}'. Expected 'stable' or 'nightly'.",
            channel
        )
    })?;
    sona_shim::registry::set_default_channel(ch)
}
