use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

pub(crate) const MAIN_TRAY_ID: &str = "main-tray";
pub(crate) const TRAY_APP_NAME: &str = "Sona";

const DEFAULT_SHOW_LABEL: &str = "Show Main Window";
const DEFAULT_CAPTION_LABEL: &str = "Live Caption";
const DEFAULT_SETTINGS_LABEL: &str = "Settings";
const DEFAULT_UPDATES_LABEL: &str = "Check for Updates";
const DEFAULT_STOP_RECORDING_LABEL: &str = "Stop Recording";
const DEFAULT_DISCONNECT_AGENT_LABEL: &str = "Disconnect Agent";
const DEFAULT_QUIT_LABEL: &str = "Quit";

pub(crate) const TRAY_OPEN_SETTINGS_EVENT: &str = "open-settings";
pub(crate) const TRAY_TOGGLE_CAPTION_EVENT: &str = "toggle-caption";
pub(crate) const TRAY_CHECK_UPDATES_EVENT: &str = "check-updates";
pub(crate) const TRAY_REQUEST_QUIT_EVENT: &str = "request-quit";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TrayTexts {
    pub status_label_template: String,
    pub status_idle: String,
    pub status_recording: String,
    pub status_agent_connected: String,
}

impl Default for TrayTexts {
    fn default() -> Self {
        Self {
            status_label_template: "Status: {{status}}".to_string(),
            status_idle: "Idle".to_string(),
            status_recording: "Recording".to_string(),
            status_agent_connected: "Agent Connected".to_string(),
        }
    }
}

pub(crate) struct TrayMenuUpdate {
    pub show_text: String,
    pub settings_text: String,
    pub updates_text: String,
    pub quit_text: String,
    pub caption_text: String,
    pub caption_checked: bool,
    pub disconnect_agent_text: Option<String>,
    pub stop_recording_text: Option<String>,
    pub status_label_text: Option<String>,
    pub status_idle_text: Option<String>,
    pub status_recording_text: Option<String>,
    pub status_agent_connected_text: Option<String>,
}

struct TrayInner {
    status_item: tauri::menu::MenuItem<tauri::Wry>,
    disconnect_item: tauri::menu::MenuItem<tauri::Wry>,
    stop_recording_item: tauri::menu::MenuItem<tauri::Wry>,
    texts: TrayTexts,
}

#[derive(Clone)]
pub struct TrayState {
    inner: Arc<parking_lot::Mutex<TrayInner>>,
    menu_update_lock: Arc<tokio::sync::Mutex<()>>,
    pub agent_connected: Arc<AtomicBool>,
    pub is_recording: Arc<AtomicBool>,
}

pub(crate) fn merge_tray_texts(current: &TrayTexts, update: &TrayMenuUpdate) -> TrayTexts {
    TrayTexts {
        status_label_template: update
            .status_label_text
            .clone()
            .unwrap_or_else(|| current.status_label_template.clone()),
        status_idle: update
            .status_idle_text
            .clone()
            .unwrap_or_else(|| current.status_idle.clone()),
        status_recording: update
            .status_recording_text
            .clone()
            .unwrap_or_else(|| current.status_recording.clone()),
        status_agent_connected: update
            .status_agent_connected_text
            .clone()
            .unwrap_or_else(|| current.status_agent_connected.clone()),
    }
}

pub(crate) fn format_tray_status_summary(
    texts: &TrayTexts,
    is_recording: bool,
    agent_connected: bool,
) -> String {
    match (is_recording, agent_connected) {
        (true, true) => format!("{} · {}", texts.status_recording, texts.status_agent_connected),
        (true, false) => texts.status_recording.clone(),
        (false, true) => texts.status_agent_connected.clone(),
        (false, false) => texts.status_idle.clone(),
    }
}

pub(crate) fn format_tray_status_menu_text(
    texts: &TrayTexts,
    is_recording: bool,
    agent_connected: bool,
) -> String {
    let summary = format_tray_status_summary(texts, is_recording, agent_connected);
    if texts.status_label_template.contains("{{status}}") {
        texts.status_label_template.replace("{{status}}", &summary)
    } else if texts.status_label_template.trim().is_empty() {
        summary
    } else {
        format!("{}: {summary}", texts.status_label_template.trim())
    }
}

pub(crate) fn format_tray_tooltip(
    texts: &TrayTexts,
    is_recording: bool,
    agent_connected: bool,
) -> String {
    if !is_recording && !agent_connected {
        return TRAY_APP_NAME.to_string();
    }
    let summary = format_tray_status_summary(texts, is_recording, agent_connected);
    format!("{TRAY_APP_NAME} · {summary}")
}

fn load_platform_tray_icon(
    #[allow(unused_variables)] is_recording: bool,
) -> Result<tauri::image::Image<'static>, tauri::Error> {
    #[cfg(target_os = "macos")]
    {
        let bytes: &[u8] = if is_recording {
            include_bytes!("../../icons/tray-macos-recording.png")
        } else {
            include_bytes!("../../icons/tray-macos-template.png")
        };
        tauri::image::Image::from_bytes(bytes)
    }

    #[cfg(not(target_os = "macos"))]
    {
        tauri::image::Image::from_bytes(include_bytes!("../../icons/128x128.png"))
    }
}

fn sync_tray_macos_icon(#[allow(unused_variables)] app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let app_handle = app.clone();
        if let Err(err) = app.run_on_main_thread(move || {
            let is_recording = app_handle
                .try_state::<TrayState>()
                .map(|s| s.is_recording.load(Ordering::SeqCst))
                .unwrap_or(false);
            if let Some(tray) = app_handle.tray_by_id(MAIN_TRAY_ID) {
                match load_platform_tray_icon(is_recording) {
                    Ok(icon) => {
                        if let Err(err) = tray.set_icon_with_as_template(Some(icon), !is_recording) {
                            log::warn!("[Tray] Failed to update macOS tray icon: {err}");
                        }
                    }
                    Err(err) => {
                        log::warn!("[Tray] Failed to decode macOS tray icon: {err}");
                    }
                }
            }
        }) {
            log::warn!("[Tray] Failed to schedule macOS tray icon update on main thread: {err}");
        }
    }
}

fn sync_tray_status_and_tooltip(app: &tauri::AppHandle, tray_state: &TrayState) {
    // Snapshot handles and formatted strings under the mutex, then release it before
    // invoking Tauri UI methods that synchronously dispatch to the main thread.
    let (status_item, disconnect_item, stop_recording_item, status_text, tooltip, is_recording, is_connected) = {
        let guard = tray_state.inner.lock();
        let is_connected = tray_state.agent_connected.load(Ordering::SeqCst);
        let is_recording = tray_state.is_recording.load(Ordering::SeqCst);
        let status_text = format_tray_status_menu_text(&guard.texts, is_recording, is_connected);
        let tooltip = format_tray_tooltip(&guard.texts, is_recording, is_connected);
        (
            guard.status_item.clone(),
            guard.disconnect_item.clone(),
            guard.stop_recording_item.clone(),
            status_text,
            tooltip,
            is_recording,
            is_connected,
        )
    };

    if let Err(err) = status_item.set_text(&status_text) {
        log::warn!("[Tray] Failed to update status menu item text: {err}");
    }
    if let Err(err) = disconnect_item.set_enabled(is_connected) {
        log::warn!("[Tray] Failed to update disconnect menu item state: {err}");
    }
    if let Err(err) = stop_recording_item.set_enabled(is_recording) {
        log::warn!("[Tray] Failed to update stop recording menu item state: {err}");
    }
    if let Some(tray) = app.tray_by_id(MAIN_TRAY_ID)
        && let Err(err) = tray.set_tooltip(Some(&tooltip))
    {
        log::warn!("[Tray] Failed to update tray tooltip: {err}");
    }
}

pub fn set_agent_connected(app: &tauri::AppHandle, connected: bool) {
    if let Some(tray_state) = app.try_state::<TrayState>() {
        tray_state
            .agent_connected
            .store(connected, Ordering::SeqCst);
        sync_tray_status_and_tooltip(app, &tray_state);
    }
}

pub fn set_recording_active(app: &tauri::AppHandle, active: bool) {
    if let Some(tray_state) = app.try_state::<TrayState>() {
        let previous = tray_state.is_recording.swap(active, Ordering::SeqCst);
        sync_tray_status_and_tooltip(app, &tray_state);
        if previous != active {
            sync_tray_macos_icon(app);
        }
    }
}

struct BuiltTrayMenu {
    menu: tauri::menu::Menu<tauri::Wry>,
    status_item: tauri::menu::MenuItem<tauri::Wry>,
    disconnect_item: tauri::menu::MenuItem<tauri::Wry>,
    stop_recording_item: tauri::menu::MenuItem<tauri::Wry>,
}

struct BuildTrayMenuArgs<'a> {
    status_text: &'a str,
    show_text: &'a str,
    caption_text: &'a str,
    caption_checked: bool,
    settings_text: &'a str,
    updates_text: &'a str,
    stop_recording_text: &'a str,
    stop_recording_enabled: bool,
    disconnect_agent_text: &'a str,
    disconnect_agent_enabled: bool,
    quit_text: &'a str,
}

fn settings_accelerator() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("Cmd+,")
    } else {
        None
    }
}

fn quit_accelerator() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("Cmd+Q")
    } else {
        None
    }
}

fn build_tray_menu<M: Manager<tauri::Wry>>(
    manager: &M,
    args: BuildTrayMenuArgs<'_>,
) -> Result<BuiltTrayMenu, String> {
    use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};

    let status_item = MenuItem::with_id(manager, "status", args.status_text, false, None::<&str>)
        .map_err(|e| e.to_string())?;
    let show_item = MenuItem::with_id(manager, "show", args.show_text, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let caption_item = CheckMenuItem::with_id(
        manager,
        "toggle_caption",
        args.caption_text,
        true,
        args.caption_checked,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let settings_item = MenuItem::with_id(
        manager,
        "settings",
        args.settings_text,
        true,
        settings_accelerator(),
    )
    .map_err(|e| e.to_string())?;
    let updates_item = MenuItem::with_id(
        manager,
        "check_updates",
        args.updates_text,
        true,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let stop_recording_item = MenuItem::with_id(
        manager,
        "stop_recording",
        args.stop_recording_text,
        args.stop_recording_enabled,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let disconnect_item = MenuItem::with_id(
        manager,
        "disconnect_agent",
        args.disconnect_agent_text,
        args.disconnect_agent_enabled,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    let quit_item = MenuItem::with_id(manager, "quit", args.quit_text, true, quit_accelerator())
        .map_err(|e| e.to_string())?;

    let sep_top = PredefinedMenuItem::separator(manager).map_err(|e| e.to_string())?;
    let sep_mid = PredefinedMenuItem::separator(manager).map_err(|e| e.to_string())?;
    let sep_bottom = PredefinedMenuItem::separator(manager).map_err(|e| e.to_string())?;

    let menu = Menu::with_items(
        manager,
        &[
            &status_item,
            &sep_top,
            &show_item,
            &caption_item,
            &settings_item,
            &updates_item,
            &sep_mid,
            &stop_recording_item,
            &disconnect_item,
            &sep_bottom,
            &quit_item,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(BuiltTrayMenu {
        menu,
        status_item,
        disconnect_item,
        stop_recording_item,
    })
}

/// Installs a custom macOS application menu that keeps `Settings...` (`Cmd+,`) and
/// `Quit` (`Cmd+Q`) routed through Sona's own menu handlers (and `quitGuard`) instead of
/// `PredefinedMenuItem::quit` which calls `NSApplication.terminate:` directly.
#[cfg(target_os = "macos")]
fn setup_macos_app_menu(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{
        AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu,
        WINDOW_SUBMENU_ID,
    };

    let app_handle = app.handle();
    let pkg_info = app_handle.package_info();
    let config = app_handle.config();
    let about_metadata = AboutMetadata {
        name: Some(pkg_info.name.clone()),
        version: Some(pkg_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config.bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };

    let app_settings = MenuItem::with_id(
        app_handle,
        "settings",
        "Settings...",
        true,
        Some("Cmd+,"),
    )?;
    let app_quit = MenuItem::with_id(
        app_handle,
        "quit",
        format!("Quit {}", pkg_info.name),
        true,
        Some("Cmd+Q"),
    )?;

    let app_submenu = Submenu::with_items(
        app_handle,
        pkg_info.name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
            &PredefinedMenuItem::separator(app_handle)?,
            &app_settings,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::services(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::hide(app_handle, None)?,
            &PredefinedMenuItem::hide_others(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &app_quit,
        ],
    )?;

    let file_submenu = Submenu::with_items(
        app_handle,
        "File",
        true,
        &[&PredefinedMenuItem::close_window(app_handle, None)?],
    )?;

    let edit_submenu = Submenu::with_items(
        app_handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app_handle, None)?,
            &PredefinedMenuItem::redo(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::cut(app_handle, None)?,
            &PredefinedMenuItem::copy(app_handle, None)?,
            &PredefinedMenuItem::paste(app_handle, None)?,
            &PredefinedMenuItem::select_all(app_handle, None)?,
        ],
    )?;

    let view_submenu = Submenu::with_items(
        app_handle,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
    )?;

    let window_submenu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;

    let help_submenu = Submenu::with_id_and_items(app_handle, HELP_SUBMENU_ID, "Help", true, &[])?;

    let menu = Menu::with_items(
        app_handle,
        &[
            &app_submenu,
            &file_submenu,
            &edit_submenu,
            &view_submenu,
            &window_submenu,
            &help_submenu,
        ],
    )?;

    app.set_menu(menu)?;
    Ok(())
}

pub(crate) async fn update_tray_menu(
    app: tauri::AppHandle,
    update: TrayMenuUpdate,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        let Some(tray_state) = app.try_state::<TrayState>() else {
            return Ok(());
        };

        // Serialize menu rebuilds so concurrent frontend calls cannot install out-of-order
        // menus or leave TrayState holding orphaned MenuItem handles.
        let _update_guard = tray_state.menu_update_lock.lock().await;

        let (updated_texts, is_connected, is_rec) = {
            let mut inner = tray_state.inner.lock();
            inner.texts = merge_tray_texts(&inner.texts, &update);
            (
                inner.texts.clone(),
                tray_state.agent_connected.load(Ordering::SeqCst),
                tray_state.is_recording.load(Ordering::SeqCst),
            )
        };

        let status_text = format_tray_status_menu_text(&updated_texts, is_rec, is_connected);
        let stop_rec_label = update
            .stop_recording_text
            .unwrap_or_else(|| DEFAULT_STOP_RECORDING_LABEL.to_string());
        let disconnect_label = update
            .disconnect_agent_text
            .unwrap_or_else(|| DEFAULT_DISCONNECT_AGENT_LABEL.to_string());

        let built = build_tray_menu(
            &app,
            BuildTrayMenuArgs {
                status_text: &status_text,
                show_text: &update.show_text,
                caption_text: &update.caption_text,
                caption_checked: update.caption_checked,
                settings_text: &update.settings_text,
                updates_text: &update.updates_text,
                stop_recording_text: &stop_rec_label,
                stop_recording_enabled: is_rec,
                disconnect_agent_text: &disconnect_label,
                disconnect_agent_enabled: is_connected,
                quit_text: &update.quit_text,
            },
        )?;

        {
            let mut inner = tray_state.inner.lock();
            inner.status_item = built.status_item;
            inner.disconnect_item = built.disconnect_item;
            inner.stop_recording_item = built.stop_recording_item;
        }

        if let Some(tray) = app.tray_by_id(MAIN_TRAY_ID) {
            tray.set_menu(Some(built.menu))
                .map_err(|e| e.to_string())?;
        }

        sync_tray_status_and_tooltip(&app, &tray_state);
    }
    Ok(())
}

pub(crate) fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(desktop)]
    {
        use tauri::tray::TrayIconBuilder;

        #[cfg(target_os = "macos")]
        setup_macos_app_menu(app)?;

        let default_texts = TrayTexts::default();
        let initial_status_text = format_tray_status_menu_text(&default_texts, false, false);
        let initial_tooltip = format_tray_tooltip(&default_texts, false, false);

        let built = build_tray_menu(
            app,
            BuildTrayMenuArgs {
                status_text: &initial_status_text,
                show_text: DEFAULT_SHOW_LABEL,
                caption_text: DEFAULT_CAPTION_LABEL,
                caption_checked: false,
                settings_text: DEFAULT_SETTINGS_LABEL,
                updates_text: DEFAULT_UPDATES_LABEL,
                stop_recording_text: DEFAULT_STOP_RECORDING_LABEL,
                stop_recording_enabled: false,
                disconnect_agent_text: DEFAULT_DISCONNECT_AGENT_LABEL,
                disconnect_agent_enabled: false,
                quit_text: DEFAULT_QUIT_LABEL,
            },
        )
        .map_err(std::io::Error::other)?;

        let tray_state = TrayState {
            inner: Arc::new(parking_lot::Mutex::new(TrayInner {
                status_item: built.status_item,
                disconnect_item: built.disconnect_item,
                stop_recording_item: built.stop_recording_item,
                texts: default_texts,
            })),
            menu_update_lock: Arc::new(tokio::sync::Mutex::new(())),
            agent_connected: Arc::new(AtomicBool::new(false)),
            is_recording: Arc::new(AtomicBool::new(false)),
        };
        app.manage(tray_state);

        let icon = load_platform_tray_icon(false)?;

        let mut builder = TrayIconBuilder::with_id(MAIN_TRAY_ID)
            .icon(icon)
            .tooltip(initial_tooltip)
            .menu(&built.menu)
            // On macOS, status bar extras open their menu on left-click;
            // on Windows/Linux, left-click restores the main window and right-click opens the menu.
            .show_menu_on_left_click(cfg!(target_os = "macos"))
            .on_menu_event(move |app, event| match event.id.as_ref() {
                "show" => {
                    crate::app::window::show_main_window(app);
                }
                "toggle_caption" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit(TRAY_TOGGLE_CAPTION_EVENT, ());
                    }
                }
                "settings" => {
                    crate::app::window::show_main_window(app);
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit(TRAY_OPEN_SETTINGS_EVENT, ());
                    }
                }
                "check_updates" => {
                    crate::app::window::show_main_window(app);
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit(TRAY_CHECK_UPDATES_EVENT, ());
                    }
                }
                "quit" => {
                    if let Some(window) = app.get_webview_window("main") {
                        // Do not pop open a hidden window before quitGuard confirms active tasks.
                        let _ = window.emit(TRAY_REQUEST_QUIT_EVENT, ());
                    } else {
                        app.exit(0);
                    }
                }
                "disconnect_agent" => {
                    crate::platform::agent_control::disconnect_all_agents(app);
                }
                "stop_recording" => {
                    crate::platform::agent_control::stop_recording_from_tray(app);
                }
                _ => {}
            });

        #[cfg(target_os = "macos")]
        {
            builder = builder.icon_as_template(true);
        }

        #[cfg(not(target_os = "macos"))]
        {
            builder = builder.on_tray_icon_event(|tray, event| {
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
                let should_show = matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                );

                if should_show {
                    crate::app::window::show_main_window(tray.app_handle());
                }
            });
        }

        let _tray = builder.build(app)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_update() -> TrayMenuUpdate {
        TrayMenuUpdate {
            show_text: "Show".to_string(),
            settings_text: "Settings".to_string(),
            updates_text: "Updates".to_string(),
            quit_text: "Quit".to_string(),
            caption_text: "Caption".to_string(),
            caption_checked: false,
            disconnect_agent_text: None,
            stop_recording_text: None,
            status_label_text: None,
            status_idle_text: None,
            status_recording_text: None,
            status_agent_connected_text: None,
        }
    }

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

    #[test]
    fn merge_tray_texts_updates_provided_fields_and_preserves_omitted_ones() {
        let initial = TrayTexts::default();
        let partial = TrayMenuUpdate {
            status_recording_text: Some("正在录音".to_string()),
            status_idle_text: Some("空闲".to_string()),
            ..empty_update()
        };

        let merged = merge_tray_texts(&initial, &partial);
        assert_eq!(merged.status_label_template, "Status: {{status}}");
        assert_eq!(merged.status_idle, "空闲");
        assert_eq!(merged.status_recording, "正在录音");
        assert_eq!(merged.status_agent_connected, "Agent Connected");
    }

    #[test]
    fn formats_status_and_tooltip_for_idle_and_active_states() {
        let texts = TrayTexts::default();

        assert_eq!(format_tray_tooltip(&texts, false, false), "Sona");
        assert_eq!(
            format_tray_status_menu_text(&texts, false, false),
            "Status: Idle"
        );

        assert_eq!(
            format_tray_tooltip(&texts, true, false),
            "Sona · Recording"
        );
        assert_eq!(
            format_tray_status_menu_text(&texts, true, false),
            "Status: Recording"
        );

        assert_eq!(
            format_tray_tooltip(&texts, false, true),
            "Sona · Agent Connected"
        );
        assert_eq!(
            format_tray_status_menu_text(&texts, false, true),
            "Status: Agent Connected"
        );

        assert_eq!(
            format_tray_tooltip(&texts, true, true),
            "Sona · Recording · Agent Connected"
        );
        assert_eq!(
            format_tray_status_menu_text(&texts, true, true),
            "Status: Recording · Agent Connected"
        );
    }

    #[test]
    fn formats_localized_status_without_placeholder_fallback() {
        let texts = TrayTexts {
            status_label_template: "状态：{{status}}".to_string(),
            status_idle: "空闲".to_string(),
            status_recording: "正在录音".to_string(),
            status_agent_connected: "Agent 已连接".to_string(),
        };

        assert_eq!(
            format_tray_status_menu_text(&texts, false, false),
            "状态：空闲"
        );
        assert_eq!(
            format_tray_tooltip(&texts, true, false),
            "Sona · 正在录音"
        );

        let fallback_texts = TrayTexts {
            status_label_template: "Status".to_string(),
            ..texts
        };
        assert_eq!(
            format_tray_status_menu_text(&fallback_texts, false, true),
            "Status: Agent 已连接"
        );
    }

    #[test]
    fn embedded_macos_tray_icons_decode_cleanly() {
        let template_icon = tauri::image::Image::from_bytes(include_bytes!(
            "../../icons/tray-macos-template.png"
        ))
        .expect("template icon should decode");
        assert_eq!((template_icon.width(), template_icon.height()), (44, 44));
        // AppKit template images expect pure black RGB channels with alpha mask.
        let mut has_opaque_pixel = false;
        for px in template_icon.rgba().as_chunks::<4>().0 {
            if px[3] > 0 {
                has_opaque_pixel = true;
                assert_eq!((px[0], px[1], px[2]), (0, 0, 0));
            }
        }
        assert!(has_opaque_pixel);

        let recording_icon = tauri::image::Image::from_bytes(include_bytes!(
            "../../icons/tray-macos-recording.png"
        ))
        .expect("recording icon should decode");
        assert_eq!((recording_icon.width(), recording_icon.height()), (44, 44));
    }
}
