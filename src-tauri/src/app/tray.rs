use tauri::{Emitter, Manager};

pub(crate) const TRAY_OPEN_SETTINGS_EVENT: &str = "open-settings";
pub(crate) const TRAY_TOGGLE_CAPTION_EVENT: &str = "toggle-caption";
pub(crate) const TRAY_CHECK_UPDATES_EVENT: &str = "check-updates";
pub(crate) const TRAY_REQUEST_QUIT_EVENT: &str = "request-quit";

pub(crate) async fn update_tray_menu<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    show_text: String,
    settings_text: String,
    updates_text: String,
    quit_text: String,
    caption_text: String,
    caption_checked: bool,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri::menu::{CheckMenuItem, Menu, MenuItem};
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
                &quit_i,
            ],
        )
        .map_err(|e| e.to_string())?;

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
        let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

        let menu = Menu::with_items(
            app,
            &[
                &show_i,
                &caption_i,
                &settings_i,
                &updates_i,
                &tauri::menu::PredefinedMenuItem::separator(app)?,
                &quit_i,
            ],
        )?;

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
