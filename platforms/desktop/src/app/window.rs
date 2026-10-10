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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchVisibility {
    Silent,
    Focus,
}

pub fn parse_launch_visibility_from_args(
    args: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<LaunchVisibility> {
    let mut silent_flag = false;

    for arg in args {
        let raw = arg.as_ref().trim();
        let lower = raw.to_lowercase();

        if lower == "--focus"
            || lower == "--focus=true"
            || lower == "--focus=1"
            || lower == "--silent=false"
        {
            return Some(LaunchVisibility::Focus);
        }

        if lower == "--silent"
            || lower == "--minimized"
            || lower == "--hidden"
            || lower == "--silent=true"
            || lower == "--silent=1"
        {
            silent_flag = true;
        }

        let is_sona_url = lower.starts_with("sona://")
            || (lower.contains("://")
                && lower
                    .split("://")
                    .next()
                    .is_some_and(|scheme| scheme == "sona"));

        if is_sona_url && let Some((_, query_part)) = raw.split_once('?') {
            let query_str = query_part.split('#').next().unwrap_or(query_part);
            for param in query_str.split('&') {
                if param.is_empty() {
                    continue;
                }
                let (k, v) = match param.split_once('=') {
                    Some((k, v)) => (k.trim().to_lowercase(), v.trim().to_lowercase()),
                    None => (param.trim().to_lowercase(), String::new()),
                };

                if (k == "focus" && (v == "true" || v == "1"))
                    || (k == "silent" && (v == "false" || v == "0"))
                {
                    return Some(LaunchVisibility::Focus);
                }
                if k == "silent" && (v == "true" || v == "1") {
                    silent_flag = true;
                }
            }
        }
    }

    if silent_flag {
        Some(LaunchVisibility::Silent)
    } else {
        None
    }
}

pub fn should_start_silently_from_args_and_config(
    args: impl IntoIterator<Item = impl AsRef<str>>,
    config: Option<&serde_json::Value>,
) -> bool {
    let args: Vec<String> = args
        .into_iter()
        .map(|s| s.as_ref().trim().to_string())
        .collect();

    if let Some(visibility) = parse_launch_visibility_from_args(&args) {
        return match visibility {
            LaunchVisibility::Silent => true,
            LaunchVisibility::Focus => false,
        };
    }

    if args.iter().any(|arg| arg.to_lowercase() == "--autostart") {
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

    #[test]
    fn parses_launch_visibility_flags() {
        assert_eq!(
            parse_launch_visibility_from_args(["--silent"]),
            Some(LaunchVisibility::Silent)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["--minimized"]),
            Some(LaunchVisibility::Silent)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["--hidden"]),
            Some(LaunchVisibility::Silent)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["--focus"]),
            Some(LaunchVisibility::Focus)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["sona.exe", "--silent"]),
            Some(LaunchVisibility::Silent)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["sona.exe", "--focus"]),
            Some(LaunchVisibility::Focus)
        );
        assert_eq!(parse_launch_visibility_from_args(["sona.exe"]), None);
        assert_eq!(
            parse_launch_visibility_from_args(Vec::<String>::new()),
            None
        );
    }

    #[test]
    fn parses_launch_visibility_deep_links() {
        assert_eq!(
            parse_launch_visibility_from_args(["sona://agent/launch?silent=true"]),
            Some(LaunchVisibility::Silent)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["sona://agent/launch?silent=1"]),
            Some(LaunchVisibility::Silent)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["sona://agent/launch?foo=1&silent=true"]),
            Some(LaunchVisibility::Silent)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["sona://agent/launch?silent=false"]),
            Some(LaunchVisibility::Focus)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["sona://agent/launch?focus=true"]),
            Some(LaunchVisibility::Focus)
        );
        assert_eq!(
            parse_launch_visibility_from_args(["sona://agent/launch?silent=true&focus=true"]),
            Some(LaunchVisibility::Focus)
        );
    }
}
