use tauri::{AppHandle, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN_WINDOW_LABEL: &str = "main";
pub const MAIN_WINDOW_TITLE: &str = "Sona - Transcript Editor";
pub const MAIN_WINDOW_WIDTH: f64 = 1200.0;
pub const MAIN_WINDOW_HEIGHT: f64 = 800.0;
pub const MAIN_WINDOW_MIN_WIDTH: f64 = 900.0;
pub const MAIN_WINDOW_MIN_HEIGHT: f64 = 600.0;

/// Theme used for the native window frame before the webview reports the
/// user's persisted preference. Light matches the default application theme,
/// so a first launch is never momentarily mismatched.
pub const DEFAULT_WINDOW_CHROME_THEME: crate::platform::system::ResolvedTheme =
    crate::platform::system::ResolvedTheme::Light;

pub fn should_start_silently_from_args_and_config(
    args: impl IntoIterator<Item = impl AsRef<str>>,
    config: Option<&serde_json::Value>,
) -> bool {
    let args: Vec<String> = args
        .into_iter()
        .map(|s| s.as_ref().trim().to_lowercase())
        .collect();

    if args
        .iter()
        .any(|arg| arg == "--silent" || arg == "--minimized" || arg == "--hidden")
    {
        return true;
    }

    if args.iter().any(|arg| arg == "--autostart") {
        return config.is_some_and(|config| {
            let auto_start = config
                .get("autoStart")
                .or_else(|| config.get("auto_start"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let silent_start = config
                .get("silentStart")
                .or_else(|| config.get("silent_start"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let minimize_to_tray = config
                .get("minimizeToTrayOnExit")
                .or_else(|| config.get("minimize_to_tray_on_exit"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            auto_start && silent_start && minimize_to_tray
        });
    }

    false
}

pub fn create_main_window<R: Runtime>(
    app: &AppHandle<R>,
    start_silently: bool,
) -> Result<WebviewWindow<R>, Box<dyn std::error::Error>> {
    let window = WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::default())
        .title(MAIN_WINDOW_TITLE)
        .inner_size(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT)
        .min_inner_size(MAIN_WINDOW_MIN_WIDTH, MAIN_WINDOW_MIN_HEIGHT)
        .visible(!start_silently)
        .build()?;

    if start_silently {
        let _ = window.hide();
    }

    // Paint the frame before the first visible frame is composited so the
    // caption never flashes the wrong color on startup. A failure here is not
    // fatal: the window still works, it just keeps the default frame.
    if let Err(error) = apply_main_window_chrome(&window, DEFAULT_WINDOW_CHROME_THEME) {
        log::warn!("[Window] Failed to apply default window chrome error={error}");
    }

    Ok(window)
}

/// Recolor the native window frame to match the application theme.
///
/// No-op on macOS and Linux, where the frame is themed by the platform.
pub fn apply_main_window_chrome<R: Runtime>(
    window: &WebviewWindow<R>,
    theme: crate::platform::system::ResolvedTheme,
) -> Result<(), String> {
    crate::platform::system::apply_window_chrome(window, theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn starts_silently_with_explicit_flags() {
        assert!(should_start_silently_from_args_and_config(
            ["--silent"],
            None
        ));
        assert!(should_start_silently_from_args_and_config(
            ["--minimized"],
            None
        ));
        assert!(should_start_silently_from_args_and_config(
            ["--hidden"],
            None
        ));
        assert!(should_start_silently_from_args_and_config(
            ["sona.exe", "--SILENT"],
            None
        ));
    }

    #[test]
    fn starts_silently_with_autostart_when_all_conditions_met() {
        let config = json!({
            "autoStart": true,
            "silentStart": true,
            "minimizeToTrayOnExit": true
        });
        assert!(should_start_silently_from_args_and_config(
            ["--autostart"],
            Some(&config)
        ));

        // Works with snake_case and default minimizeToTrayOnExit
        let config_snake = json!({
            "auto_start": true,
            "silent_start": true
        });
        assert!(should_start_silently_from_args_and_config(
            ["--autostart"],
            Some(&config_snake)
        ));
    }

    #[test]
    fn does_not_start_silently_with_autostart_if_silent_start_is_false() {
        let config = json!({
            "autoStart": true,
            "silentStart": false,
            "minimizeToTrayOnExit": true
        });
        assert!(!should_start_silently_from_args_and_config(
            ["--autostart"],
            Some(&config)
        ));
    }

    #[test]
    fn does_not_start_silently_with_autostart_if_minimize_to_tray_is_false() {
        let config = json!({
            "autoStart": true,
            "silentStart": true,
            "minimizeToTrayOnExit": false
        });
        assert!(!should_start_silently_from_args_and_config(
            ["--autostart"],
            Some(&config)
        ));
    }

    #[test]
    fn does_not_start_silently_with_autostart_if_auto_start_is_false() {
        let config = json!({
            "autoStart": false,
            "silentStart": true,
            "minimizeToTrayOnExit": true
        });
        assert!(!should_start_silently_from_args_and_config(
            ["--autostart"],
            Some(&config)
        ));
    }

    #[test]
    fn does_not_start_silently_on_normal_launch() {
        let config = json!({
            "autoStart": true,
            "silentStart": true,
            "minimizeToTrayOnExit": true
        });
        assert!(!should_start_silently_from_args_and_config(
            ["sona.exe"],
            Some(&config)
        ));
        assert!(!should_start_silently_from_args_and_config(
            Vec::<String>::new(),
            Some(&config)
        ));
    }
}
