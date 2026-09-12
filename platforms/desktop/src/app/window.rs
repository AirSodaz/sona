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

pub fn create_main_window<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<WebviewWindow<R>, Box<dyn std::error::Error>> {
    let window = WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::default())
        .title(MAIN_WINDOW_TITLE)
        .inner_size(MAIN_WINDOW_WIDTH, MAIN_WINDOW_HEIGHT)
        .min_inner_size(MAIN_WINDOW_MIN_WIDTH, MAIN_WINDOW_MIN_HEIGHT)
        .build()?;

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
