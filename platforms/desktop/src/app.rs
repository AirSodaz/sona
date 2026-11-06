//! GPUI Application bootstrap and global state registration.
//!
//! Milestone 1: Decoupled Core Direct Linking (Feature 4)
//! Milestone 2: Native OS Integrations & Floating Overlays (Gaps G14, G15, G36–G38)

use gpui_kit::*;
use std::sync::Arc;

use crate::event_bus::AppEventBus;
use crate::platform::event::EventEmitterPort;
use crate::platform::hotkey::HotkeyController;
use crate::platform::tray::TrayManager;
use crate::services::DesktopServices;
use crate::views::shell::MainShell;

/// Global wrapper holding an `Arc<DesktopServices>` in GPUI app globals.
#[derive(Clone)]
pub struct GlobalDesktopServices(pub Arc<DesktopServices>);

impl Global for GlobalDesktopServices {}

impl std::ops::Deref for GlobalDesktopServices {
    type Target = DesktopServices;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Global wrapper holding an `Arc<AppEventBus>` in GPUI app globals.
#[derive(Clone)]
pub struct GlobalEventBus(pub Arc<AppEventBus>);

impl Global for GlobalEventBus {}

impl std::ops::Deref for GlobalEventBus {
    type Target = AppEventBus;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Global wrapper holding an `Arc<TrayManager>` in GPUI app globals.
#[derive(Clone)]
pub struct GlobalTrayManager(pub Arc<TrayManager>);

impl Global for GlobalTrayManager {}

/// Global wrapper holding an `Arc<parking_lot::RwLock<HotkeyController>>` in GPUI app globals.
#[derive(Clone)]
pub struct GlobalHotkeyController(pub Arc<parking_lot::RwLock<HotkeyController>>);

impl Global for GlobalHotkeyController {}

// Direct Global implementation for `DesktopServices`
impl Global for DesktopServices {}

/// Main window dimensions as specified in PROJECT.md Feature 9
pub const MAIN_WINDOW_WIDTH: f32 = 1200.0;
pub const MAIN_WINDOW_HEIGHT: f32 = 800.0;
pub const MAIN_WINDOW_MIN_WIDTH: f32 = 900.0;
pub const MAIN_WINDOW_MIN_HEIGHT: f32 = 600.0;

/// Bootstraps the GPUI application lifecycle:
/// 1. Initializes GPUI Kit framework subsystems (theming, overlays, assets).
/// 2. Registers global accessors for backend domain services and event streaming.
/// 3. Registers system tray and global hotkeys event loops.
/// 4. Opens the primary application window hosting `MainShell`.
pub fn bootstrap(cx: &mut App, services: Arc<DesktopServices>, event_bus: Arc<AppEventBus>) {
    // 1. Initialize GPUI Kit (theming, overlay managers, assets, default bindings)
    gpui_kit::init(cx);

    // 2. Register backend services and event bus into GPUI globals
    cx.set_global((*services).clone());
    cx.set_global(GlobalDesktopServices(Arc::clone(&services)));
    cx.set_global(GlobalEventBus(Arc::clone(&event_bus)));

    // 3. Register System Tray Manager and start background event poller
    if let Ok(tray) = TrayManager::new() {
        let tray_arc = Arc::new(tray);
        let bus_clone = event_bus.clone();
        crate::platform::tray::spawn_tray_event_loop(
            move |event, payload| {
                let _ = bus_clone.emit(event, payload);
            },
            "Sona".to_string(),
        );
        cx.set_global(GlobalTrayManager(tray_arc));
    }

    // 4. Register Global Hotkeys Controller and start background listener
    let hotkey_controller = Arc::new(parking_lot::RwLock::new(HotkeyController::new()));
    let bus_clone2 = event_bus.clone();
    let action_map = hotkey_controller.read().action_map();
    crate::platform::hotkey::spawn_hotkey_event_loop(action_map, move |event, payload| {
        let _ = bus_clone2.emit(event, payload);
    });
    cx.set_global(GlobalHotkeyController(hotkey_controller));

    // 5. Configure primary application window options
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            Default::default(),
            size(px(MAIN_WINDOW_WIDTH), px(MAIN_WINDOW_HEIGHT)),
        ))),
        window_min_size: Some(size(px(MAIN_WINDOW_MIN_WIDTH), px(MAIN_WINDOW_MIN_HEIGHT))),
        titlebar: Some(TitlebarOptions {
            title: Some("Sona".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    };

    // 6. Open primary application window
    gpui_kit::open_window(window_options, cx, |window, cx| {
        cx.new(|cx| MainShell::new(window, cx))
    })
    .expect("Failed to open main application window");
}

/// Opens the floating transparent bilingual caption overlay window (Feature 14 / Gap G14).
pub fn open_caption_window(cx: &mut App) {
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            Default::default(),
            size(px(800.0), px(160.0)),
        ))),
        window_min_size: Some(size(px(320.0), px(60.0))),
        window_background: WindowBackgroundAppearance::Transparent,
        titlebar: Some(TitlebarOptions {
            title: Some(crate::views::overlays::caption_window::CAPTION_WINDOW_TITLE.into()),
            appears_transparent: true,
            traffic_light_position: None,
        }),
        is_movable: true,
        ..Default::default()
    };
    let _ = gpui_kit::open_window(window_options, cx, |window, cx| {
        cx.new(|cx| crate::views::overlays::CaptionOverlayWindow::new(window, cx))
    });
}

/// Opens the floating transparent voice typing dynamic island widget (Feature 15 / Gap G15).
pub fn open_voice_typing_island(cx: &mut App) {
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            Default::default(),
            size(px(400.0), px(80.0)),
        ))),
        window_min_size: Some(size(px(240.0), px(48.0))),
        window_background: WindowBackgroundAppearance::Transparent,
        titlebar: None,
        is_movable: true,
        ..Default::default()
    };
    let _ = gpui_kit::open_window(window_options, cx, |window, cx| {
        cx.new(|cx| crate::views::overlays::VoiceTypingIsland::new(window, cx))
    });
}
