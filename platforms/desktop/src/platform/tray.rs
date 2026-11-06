//! System Tray Integration using `tray-icon` and `muda`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct TrayManager {
    _tray_icon: Option<tray_icon::TrayIcon>,
    is_visible: Arc<AtomicBool>,
}

impl TrayManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let is_visible = Arc::new(AtomicBool::new(true));

        // Note: Tray initialization depends on the OS event loop.
        // We construct menu items with muda.
        let tray_menu = muda::Menu::new();
        let show_item = muda::MenuItem::new("Show Sona", true, None);
        let separator = muda::PredefinedMenuItem::separator();
        let quit_item = muda::MenuItem::new("Quit", true, None);

        let _ = tray_menu.append(&show_item);
        let _ = tray_menu.append(&separator);
        let _ = tray_menu.append(&quit_item);

        // Build tray icon if icon data is available
        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Sona - Speech Recognition & Transcription")
            .build()
            .ok();

        Ok(Self {
            _tray_icon: tray_icon,
            is_visible,
        })
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.is_visible.store(visible, Ordering::SeqCst);
    }

    pub fn is_window_visible(&self) -> bool {
        self.is_visible.load(Ordering::SeqCst)
    }
}
