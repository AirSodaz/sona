use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

pub(crate) const TRAY_OPEN_SETTINGS_EVENT: &str = "open-settings";
pub(crate) const TRAY_TOGGLE_CAPTION_EVENT: &str = "toggle-caption";
pub(crate) const TRAY_CHECK_UPDATES_EVENT: &str = "check-updates";
pub(crate) const TRAY_REQUEST_QUIT_EVENT: &str = "request-quit";

#[derive(Clone)]
pub struct TrayState {
    pub disconnect_item: Arc<std::sync::RwLock<tauri::menu::MenuItem<tauri::Wry>>>,
    pub stop_recording_item: Arc<std::sync::RwLock<tauri::menu::MenuItem<tauri::Wry>>>,
    pub agent_connected: Arc<AtomicBool>,
    pub is_recording: Arc<AtomicBool>,
}

pub fn set_agent_connected(app: &tauri::AppHandle, connected: bool) {
    if let Some(tray_state) = app.try_state::<TrayState>() {
        tray_state
            .agent_connected
            .store(connected, Ordering::SeqCst);
        if let Ok(item) = tray_state.disconnect_item.read() {
            let _ = item.set_enabled(connected);
        }
    }
}

pub fn set_recording_active(app: &tauri::AppHandle, active: bool) {
    if let Some(tray_state) = app.try_state::<TrayState>() {
        tray_state.is_recording.store(active, Ordering::SeqCst);
        if let Ok(item) = tray_state.stop_recording_item.read() {
            let _ = item.set_enabled(active);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn update_tray_menu(
    app: tauri::AppHandle,
    show_text: String,
    settings_text: String,
    updates_text: String,
    quit_text: String,
    caption_text: String,
    caption_checked: bool,
    disconnect_agent_text: Option<String>,
    stop_recording_text: Option<String>,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri::menu::{CheckMenuItem, Menu, MenuItem};

        let tray_state = app.try_state::<TrayState>();
        let is_connected = tray_state
            .as_ref()
            .map(|s| s.agent_connected.load(Ordering::SeqCst))
            .unwrap_or(false);
        let is_rec = tray_state
            .as_ref()
            .map(|s| s.is_recording.load(Ordering::SeqCst))
            .unwrap_or(false);

        let show_i = MenuItem::with_id(&app, "show", &show_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let caption_i = CheckMenuItem::with_id(
            &app,
            "toggle_caption",
            &caption_text,
            true,
            caption_checked,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;
        let settings_i = MenuItem::with_id(&app, "settings", &settings_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let updates_i = MenuItem::with_id(&app, "check_updates", &updates_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let stop_rec_label = stop_recording_text.unwrap_or_else(|| "Stop Recording".to_string());
        let stop_recording_i = MenuItem::with_id(
            &app,
            "stop_recording",
            &stop_rec_label,
            is_rec,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let disconnect_label =
            disconnect_agent_text.unwrap_or_else(|| "Disconnect Agent".to_string());
        let disconnect_i = MenuItem::with_id(
            &app,
            "disconnect_agent",
            &disconnect_label,
            is_connected,
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;

        let quit_i = MenuItem::with_id(&app, "quit", &quit_text, true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let menu = Menu::with_items(
            &app,
            &[
                &show_i,
                &caption_i,
                &settings_i,
                &updates_i,
                &tauri::menu::PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?,
                &stop_recording_i,
                &disconnect_i,
                &tauri::menu::PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?,
                &quit_i,
            ],
        )
        .map_err(|e| e.to_string())?;

        if let Some(state) = tray_state {
            if let Ok(mut disc_lock) = state.disconnect_item.write() {
                let latest_connected = state.agent_connected.load(Ordering::Acquire);
                let _ = disconnect_i.set_enabled(latest_connected);
                *disc_lock = disconnect_i;
            }
            if let Ok(mut stop_lock) = state.stop_recording_item.write() {
                let latest_recording = state.is_recording.load(Ordering::Acquire);
                let _ = stop_recording_i.set_enabled(latest_recording);
                *stop_lock = stop_recording_i;
            }
        }

        if let Some(tray) = app.tray_by_id("main-tray") {
            tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(desktop)]
    {
        use tauri::image::Image;
        use tauri::menu::{CheckMenuItem, Menu, MenuItem};
        use tauri::tray::TrayIconBuilder;

        let show_i = MenuItem::with_id(app, "show", "Show Main Window", true, None::<&str>)?;
        let caption_i = CheckMenuItem::with_id(
            app,
            "toggle_caption",
            "Live Caption",
            true,
            false,
            None::<&str>,
        )?;
        let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
        let updates_i = MenuItem::with_id(
            app,
            "check_updates",
            "Check for Updates",
            true,
            None::<&str>,
        )?;
        let stop_recording_i =
            MenuItem::with_id(app, "stop_recording", "Stop Recording", false, None::<&str>)?;
        let disconnect_i = MenuItem::with_id(
            app,
            "disconnect_agent",
            "Disconnect Agent",
            false,
            None::<&str>,
        )?;
        let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

        let menu = Menu::with_items(
            app,
            &[
                &show_i,
                &caption_i,
                &settings_i,
                &updates_i,
                &tauri::menu::PredefinedMenuItem::separator(app)?,
                &stop_recording_i,
                &disconnect_i,
                &tauri::menu::PredefinedMenuItem::separator(app)?,
                &quit_i,
            ],
        )?;

        let tray_state = TrayState {
            disconnect_item: Arc::new(std::sync::RwLock::new(disconnect_i)),
            stop_recording_item: Arc::new(std::sync::RwLock::new(stop_recording_i)),
            agent_connected: Arc::new(AtomicBool::new(false)),
            is_recording: Arc::new(AtomicBool::new(false)),
        };
        app.manage(tray_state);

        let icon = Image::from_bytes(include_bytes!("../../icons/128x128.png"))?;

        let _tray = TrayIconBuilder::with_id("main-tray")
            .icon(icon)
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(move |app, event| match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "toggle_caption" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit(TRAY_TOGGLE_CAPTION_EVENT, ());
                    }
                }
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit(TRAY_OPEN_SETTINGS_EVENT, ());
                    }
                }
                "check_updates" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit(TRAY_CHECK_UPDATES_EVENT, ());
                    }
                }
                "quit" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit(TRAY_REQUEST_QUIT_EVENT, ());
                    }
                }
                "disconnect_agent" => {
                    crate::platform::agent_control::disconnect_all_agents(app);
                }
                "stop_recording" => {
                    crate::platform::agent_control::stop_recording_from_tray(app);
                }
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                use tauri::tray::{MouseButton, TrayIconEvent};
                let should_show = matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                );

                if should_show {
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            })
            .build(app)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_state_flag_transitions() {
        let agent_connected = Arc::new(AtomicBool::new(false));
        let is_recording = Arc::new(AtomicBool::new(false));

        assert!(!agent_connected.load(Ordering::SeqCst));
        assert!(!is_recording.load(Ordering::SeqCst));

        agent_connected.store(true, Ordering::SeqCst);
        assert!(agent_connected.load(Ordering::SeqCst));

        is_recording.store(true, Ordering::SeqCst);
        assert!(is_recording.load(Ordering::SeqCst));

        agent_connected.store(false, Ordering::SeqCst);
        assert!(!agent_connected.load(Ordering::SeqCst));

        is_recording.store(false, Ordering::SeqCst);
        assert!(!is_recording.load(Ordering::SeqCst));
    }
}
