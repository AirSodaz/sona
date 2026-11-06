//! System Tray Integration using `tray-icon` and `muda`.
//!
//! Milestone 2: Native OS Integrations Parity (Gap G36)
//! Provides a complete 9-item localized tray menu, dynamic state updating,
//! click-to-focus event processing, and event loop integration with `AppEventBus`.

use muda::{CheckMenuItem, ContextMenu, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const TRAY_ID_SHOW: &str = "tray_show";
pub const TRAY_ID_TOGGLE_RECORDING: &str = "tray_toggle_recording";
pub const TRAY_ID_TOGGLE_SUBTITLES: &str = "tray_toggle_subtitles";
pub const TRAY_ID_TOGGLE_VOICE_TYPING: &str = "tray_toggle_voice_typing";
pub const TRAY_ID_TOGGLE_MUTE: &str = "tray_toggle_mute";
pub const TRAY_ID_SYNC_NOW: &str = "tray_sync_now";
pub const TRAY_ID_SETTINGS: &str = "tray_settings";
pub const TRAY_ID_ABOUT: &str = "tray_about";
pub const TRAY_ID_QUIT: &str = "tray_quit";

/// Supported languages for dynamic tray localization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppLocale {
    #[default]
    En,
    Zh,
    ZhTw,
    Ja,
    Ko,
}

impl AppLocale {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().replace('_', "-").as_str() {
            "zh" | "zh-cn" | "zh-hans" => Self::Zh,
            "zh-tw" | "zh-hk" | "zh-hant" => Self::ZhTw,
            "ja" | "ja-jp" => Self::Ja,
            "ko" | "ko-kr" => Self::Ko,
            _ => Self::En,
        }
    }
}

/// Localized labels for tray menu items.
#[derive(Debug, Clone)]
pub struct TrayLabels {
    pub show: &'static str,
    pub start_recording: &'static str,
    pub stop_recording: &'static str,
    pub live_subtitles: &'static str,
    pub voice_typing: &'static str,
    pub mute_mic: &'static str,
    pub unmute_mic: &'static str,
    pub sync_now: &'static str,
    pub settings: &'static str,
    pub about: &'static str,
    pub quit: &'static str,
}

pub fn get_tray_labels(locale: AppLocale) -> TrayLabels {
    match locale {
        AppLocale::Zh => TrayLabels {
            show: "显示主窗口",
            start_recording: "开始录音",
            stop_recording: "停止录音",
            live_subtitles: "实时字幕",
            voice_typing: "语音输入",
            mute_mic: "静音麦克风",
            unmute_mic: "取消静音",
            sync_now: "立即同步",
            settings: "设置",
            about: "关于 Sona",
            quit: "退出 Sona",
        },
        AppLocale::ZhTw => TrayLabels {
            show: "顯示主視窗",
            start_recording: "開始錄音",
            stop_recording: "停止錄音",
            live_subtitles: "即時字幕",
            voice_typing: "語音輸入",
            mute_mic: "靜音麥克風",
            unmute_mic: "取消靜音",
            sync_now: "立即同步",
            settings: "設定",
            about: "關於 Sona",
            quit: "結束 Sona",
        },
        AppLocale::Ja => TrayLabels {
            show: "Sonaを表示",
            start_recording: "録音を開始",
            stop_recording: "録音を停止",
            live_subtitles: "リアルタイム字幕",
            voice_typing: "音声入力",
            mute_mic: "マイクをミュート",
            unmute_mic: "ミュート解除",
            sync_now: "今すぐ同期",
            settings: "設定",
            about: "Sonaについて",
            quit: "Sonaを終了",
        },
        AppLocale::Ko => TrayLabels {
            show: "Sona 표시",
            start_recording: "녹음 시작",
            stop_recording: "녹음 중지",
            live_subtitles: "실시간 자막",
            voice_typing: "음성 입력",
            mute_mic: "마이크 음소거",
            unmute_mic: "음소거 해제",
            sync_now: "지금 동기화",
            settings: "설정",
            about: "Sona 정보",
            quit: "Sona 종료",
        },
        AppLocale::En => TrayLabels {
            show: "Show Sona",
            start_recording: "Start Recording",
            stop_recording: "Stop Recording",
            live_subtitles: "Live Subtitles",
            voice_typing: "Voice Typing",
            mute_mic: "Mute Microphone",
            unmute_mic: "Unmute Microphone",
            sync_now: "Sync Now",
            settings: "Settings",
            about: "About Sona",
            quit: "Quit Sona",
        },
    }
}

/// Active dynamic state reflected in tray menu items.
#[derive(Debug, Clone)]
pub struct TrayDynamicState {
    pub is_recording: bool,
    pub is_subtitles_active: bool,
    pub is_voice_typing_active: bool,
    pub is_muted: bool,
    pub is_syncing: bool,
    pub locale: AppLocale,
}

impl Default for TrayDynamicState {
    fn default() -> Self {
        Self {
            is_recording: false,
            is_subtitles_active: false,
            is_voice_typing_active: false,
            is_muted: false,
            is_syncing: false,
            locale: AppLocale::En,
        }
    }
}

pub struct TrayManager {
    _tray_icon: Option<TrayIcon>,
    show_item: MenuItem,
    recording_item: MenuItem,
    subtitles_item: CheckMenuItem,
    voice_typing_item: CheckMenuItem,
    mute_item: MenuItem,
    sync_item: MenuItem,
    settings_item: MenuItem,
    about_item: MenuItem,
    quit_item: MenuItem,
    state: parking_lot::RwLock<TrayDynamicState>,
    is_window_visible: Arc<AtomicBool>,
}

impl TrayManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let is_window_visible = Arc::new(AtomicBool::new(true));
        let state = TrayDynamicState::default();
        let labels = get_tray_labels(state.locale);

        // 1. Open / Show Sona (Item 1, bold default)
        let show_item = MenuItem::with_id(MenuId::new(TRAY_ID_SHOW), labels.show, true, None);
        let sep1 = PredefinedMenuItem::separator();

        // 2. Control items (Items 3-7)
        let recording_item = MenuItem::with_id(
            MenuId::new(TRAY_ID_TOGGLE_RECORDING),
            labels.start_recording,
            true,
            None,
        );
        let subtitles_item = CheckMenuItem::with_id(
            MenuId::new(TRAY_ID_TOGGLE_SUBTITLES),
            labels.live_subtitles,
            true,
            false,
            None,
        );
        let voice_typing_item = CheckMenuItem::with_id(
            MenuId::new(TRAY_ID_TOGGLE_VOICE_TYPING),
            labels.voice_typing,
            true,
            false,
            None,
        );
        let mute_item = MenuItem::with_id(
            MenuId::new(TRAY_ID_TOGGLE_MUTE),
            labels.mute_mic,
            true,
            None,
        );
        let sync_item =
            MenuItem::with_id(MenuId::new(TRAY_ID_SYNC_NOW), labels.sync_now, true, None);

        let sep2 = PredefinedMenuItem::separator();

        // 3. Navigation & lifecycle items (Items 9-11)
        let settings_item =
            MenuItem::with_id(MenuId::new(TRAY_ID_SETTINGS), labels.settings, true, None);
        let about_item = MenuItem::with_id(MenuId::new(TRAY_ID_ABOUT), labels.about, true, None);
        let quit_item = MenuItem::with_id(MenuId::new(TRAY_ID_QUIT), labels.quit, true, None);

        // Assemble 9-item menu
        let tray_menu = Menu::new();
        let _ = tray_menu.append(&show_item);
        let _ = tray_menu.append(&sep1);
        let _ = tray_menu.append(&recording_item);
        let _ = tray_menu.append(&subtitles_item);
        let _ = tray_menu.append(&voice_typing_item);
        let _ = tray_menu.append(&mute_item);
        let _ = tray_menu.append(&sync_item);
        let _ = tray_menu.append(&sep2);
        let _ = tray_menu.append(&settings_item);
        let _ = tray_menu.append(&about_item);
        let _ = tray_menu.append(&quit_item);

        // Win32 bold default item enforcement
        #[cfg(windows)]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{HMENU, SetMenuDefaultItem};
            let hmenu = HMENU(tray_menu.hpopupmenu() as *mut _);
            if !hmenu.is_invalid() {
                let _ = SetMenuDefaultItem(hmenu, 0, 1);
            }
        }

        // Build TrayIcon with tooltip
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Sona - Speech Recognition & Transcription")
            .build()
            .ok();

        Ok(Self {
            _tray_icon: tray_icon,
            show_item,
            recording_item,
            subtitles_item,
            voice_typing_item,
            mute_item,
            sync_item,
            settings_item,
            about_item,
            quit_item,
            state: parking_lot::RwLock::new(state),
            is_window_visible,
        })
    }

    /// Dynamically updates the menu titles to reflect the new active application locale.
    pub fn update_locale(&self, locale: AppLocale) {
        let mut state = self.state.write();
        state.locale = locale;
        let labels = get_tray_labels(locale);

        self.show_item.set_text(labels.show);
        self.recording_item.set_text(if state.is_recording {
            labels.stop_recording
        } else {
            labels.start_recording
        });
        self.subtitles_item.set_text(labels.live_subtitles);
        self.voice_typing_item.set_text(labels.voice_typing);
        self.mute_item.set_text(if state.is_muted {
            labels.unmute_mic
        } else {
            labels.mute_mic
        });
        self.sync_item.set_text(labels.sync_now);
        self.settings_item.set_text(labels.settings);
        self.about_item.set_text(labels.about);
        self.quit_item.set_text(labels.quit);
    }

    /// Dynamically updates recording active state.
    pub fn set_recording_active(&self, active: bool) {
        let mut state = self.state.write();
        state.is_recording = active;
        let labels = get_tray_labels(state.locale);
        self.recording_item.set_text(if active {
            labels.stop_recording
        } else {
            labels.start_recording
        });
    }

    /// Dynamically updates subtitle overlay active state.
    pub fn set_subtitles_active(&self, active: bool) {
        let mut state = self.state.write();
        state.is_subtitles_active = active;
        self.subtitles_item.set_checked(active);
    }

    /// Dynamically updates voice typing active state.
    pub fn set_voice_typing_active(&self, active: bool) {
        let mut state = self.state.write();
        state.is_voice_typing_active = active;
        self.voice_typing_item.set_checked(active);
    }

    /// Dynamically updates microphone mute state.
    pub fn set_mic_muted(&self, muted: bool) {
        let mut state = self.state.write();
        state.is_muted = muted;
        let labels = get_tray_labels(state.locale);
        self.mute_item.set_text(if muted {
            labels.unmute_mic
        } else {
            labels.mute_mic
        });
    }

    /// Dynamically updates sync status (disables sync item while running).
    pub fn set_syncing(&self, syncing: bool) {
        let mut state = self.state.write();
        state.is_syncing = syncing;
        self.sync_item.set_enabled(!syncing);
    }

    pub fn set_window_visible(&self, visible: bool) {
        self.is_window_visible.store(visible, Ordering::SeqCst);
    }

    pub fn is_window_visible(&self) -> bool {
        self.is_window_visible.load(Ordering::SeqCst)
    }

    /// Synchronously drains and processes queued muda and tray-icon events.
    pub fn poll_events<E: Fn(&str, serde_json::Value)>(&self, emit_fn: &E, window_title: &str) {
        poll_tray_events(emit_fn, window_title);
    }
}

/// Drains and handles tray events without capturing TrayManager.
pub fn poll_tray_events<E: Fn(&str, serde_json::Value)>(emit_fn: &E, window_title: &str) {
    let menu_channel = MenuEvent::receiver();
    while let Ok(event) = menu_channel.try_recv() {
        handle_menu_event(&event, emit_fn, window_title);
    }

    let tray_channel = TrayIconEvent::receiver();
    while let Ok(event) = tray_channel.try_recv() {
        handle_tray_icon_event(&event, window_title);
    }
}

/// Dispatches tray menu item clicks to application events.
pub fn handle_menu_event<E: Fn(&str, serde_json::Value)>(
    event: &MenuEvent,
    emit_fn: &E,
    window_title: &str,
) {
    let id_str = event.id.0.as_str();
    log::info!("[SystemTray] MenuEvent received: id={}", id_str);
    match id_str {
        TRAY_ID_SHOW => {
            focus_app_window(window_title);
        }
        TRAY_ID_TOGGLE_RECORDING => {
            emit_fn("tray:toggle_recording", serde_json::json!({}));
        }
        TRAY_ID_TOGGLE_SUBTITLES => {
            emit_fn("tray:toggle_subtitles", serde_json::json!({}));
        }
        TRAY_ID_TOGGLE_VOICE_TYPING => {
            emit_fn("tray:toggle_voice_typing", serde_json::json!({}));
        }
        TRAY_ID_TOGGLE_MUTE => {
            emit_fn("tray:toggle_mute", serde_json::json!({}));
        }
        TRAY_ID_SYNC_NOW => {
            emit_fn("tray:sync_now", serde_json::json!({}));
        }
        TRAY_ID_SETTINGS => {
            focus_app_window(window_title);
            emit_fn("tray:open_settings", serde_json::json!({}));
        }
        TRAY_ID_ABOUT => {
            focus_app_window(window_title);
            emit_fn("tray:open_about", serde_json::json!({}));
        }
        TRAY_ID_QUIT => {
            focus_app_window(window_title);
            emit_fn("tray:request_quit", serde_json::json!({}));
        }
        _ => {}
    }
}

/// Handles tray icon single-click and double-click to unminimize and focus the main window.
pub fn handle_tray_icon_event(event: &TrayIconEvent, window_title: &str) {
    use tray_icon::MouseButton;
    let should_activate = matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            ..
        } | TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        }
    );

    if should_activate {
        log::info!(
            "[SystemTray] Activating window '{}' via tray click",
            window_title
        );
        focus_app_window(window_title);
    }
}

/// Restores and brings the application window to foreground across platforms.
pub fn focus_app_window(window_title: &str) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow,
        };
        use windows::core::HSTRING;

        let title = HSTRING::from(window_title);
        if let Ok(hwnd) = FindWindowW(None, &title) {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
            true
        } else {
            false
        }
    }

    #[cfg(not(windows))]
    {
        let _ = window_title;
        false
    }
}

/// Spawns a background task running the tray event loop listening to muda and tray-icon receivers.
pub fn spawn_tray_event_loop<E>(emit_fn: E, window_title: String) -> tokio::task::JoinHandle<()>
where
    E: Fn(&str, serde_json::Value) + Send + Sync + 'static,
{
    tokio::spawn(async move {
        loop {
            poll_tray_events(&emit_fn, &window_title);
            tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_locales() {
        assert_eq!(AppLocale::from_str_loose("en"), AppLocale::En);
        assert_eq!(AppLocale::from_str_loose("zh"), AppLocale::Zh);
        assert_eq!(AppLocale::from_str_loose("zh-CN"), AppLocale::Zh);
        assert_eq!(AppLocale::from_str_loose("zh-TW"), AppLocale::ZhTw);
        assert_eq!(AppLocale::from_str_loose("ja"), AppLocale::Ja);
        assert_eq!(AppLocale::from_str_loose("ko"), AppLocale::Ko);
        assert_eq!(AppLocale::from_str_loose("fr"), AppLocale::En); // fallback
    }

    #[test]
    fn provides_localized_labels_for_all_locales() {
        for locale in [
            AppLocale::En,
            AppLocale::Zh,
            AppLocale::ZhTw,
            AppLocale::Ja,
            AppLocale::Ko,
        ] {
            let labels = get_tray_labels(locale);
            assert!(!labels.show.is_empty());
            assert!(!labels.start_recording.is_empty());
            assert!(!labels.stop_recording.is_empty());
            assert!(!labels.live_subtitles.is_empty());
            assert!(!labels.voice_typing.is_empty());
            assert!(!labels.mute_mic.is_empty());
            assert!(!labels.unmute_mic.is_empty());
            assert!(!labels.sync_now.is_empty());
            assert!(!labels.settings.is_empty());
            assert!(!labels.about.is_empty());
            assert!(!labels.quit.is_empty());
        }
    }

    #[test]
    fn emits_expected_event_names_on_menu_clicks() {
        let emitted = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let emit_clone = Arc::clone(&emitted);
        let emit_fn = move |event: &str, _payload: serde_json::Value| {
            emit_clone.lock().push(event.to_string());
        };

        let event = MenuEvent {
            id: MenuId::new(TRAY_ID_TOGGLE_RECORDING),
        };
        handle_menu_event(&event, &emit_fn, "Sona");

        let event2 = MenuEvent {
            id: MenuId::new(TRAY_ID_SYNC_NOW),
        };
        handle_menu_event(&event2, &emit_fn, "Sona");

        let log = emitted.lock();
        assert_eq!(log.as_slice(), &["tray:toggle_recording", "tray:sync_now"]);
    }
}
