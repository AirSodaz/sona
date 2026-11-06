//! Global shortcuts controller using `global-hotkey`.
//!
//! Milestone 2: Native OS Integrations Parity (Gap G37)
//! Registers all 4 required system hotkeys, processes global keyboard events,
//! and implements Win32 `GetAsyncKeyState` physical modifier key release polling.

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use std::collections::HashMap;
use std::sync::Arc;

pub const DEFAULT_SHORTCUT_VOICE_TYPING: &str = "Alt+V";
pub const DEFAULT_SHORTCUT_SUBTITLES: &str = "Ctrl+Shift+L";
pub const DEFAULT_SHORTCUT_QUICK_RECALL: &str = "Alt+Shift+H";
pub const DEFAULT_SHORTCUT_CAPTION_LOCK: &str = "Alt+C";

#[cfg(windows)]
const SHORTCUT_MODIFIER_RELEASE_WAIT_MS: u64 = 250;
#[cfg(windows)]
const SHORTCUT_MODIFIER_POLL_INTERVAL_MS: u64 = 10;

/// Logical hotkey actions mapped to application commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    VoiceTyping,
    LiveSubtitles,
    QuickRecall,
    CaptionLock,
}

/// Modifier keys monitored for physical release before simulating input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutModifier {
    Control,
    Alt,
    Shift,
    Meta,
}

/// Result of polling hardware modifier release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModifierWaitResult {
    pub released: bool,
    pub attempts: usize,
    pub still_pressed: Vec<ShortcutModifier>,
}

pub struct HotkeyController {
    manager: Option<Arc<GlobalHotKeyManager>>,
    voice_typing_hotkey: Option<HotKey>,
    subtitles_hotkey: Option<HotKey>,
    quick_recall_hotkey: Option<HotKey>,
    caption_lock_hotkey: Option<HotKey>,
    action_map: Arc<parking_lot::RwLock<HashMap<u32, HotkeyAction>>>,
}

impl HotkeyController {
    pub fn new() -> Self {
        let manager = GlobalHotKeyManager::new().ok().map(Arc::new);
        let action_map = Arc::new(parking_lot::RwLock::new(HashMap::new()));
        let mut controller = Self {
            manager,
            voice_typing_hotkey: None,
            subtitles_hotkey: None,
            quick_recall_hotkey: None,
            caption_lock_hotkey: None,
            action_map,
        };

        controller.register_all_defaults();
        controller
    }

    pub fn action_map(&self) -> Arc<parking_lot::RwLock<HashMap<u32, HotkeyAction>>> {
        self.action_map.clone()
    }

    /// Registers the 4 default system hotkeys required by specification.
    pub fn register_all_defaults(&mut self) {
        if let Some(ref mgr) = self.manager {
            let mut map = self.action_map.write();

            // 1. Voice Typing: Alt+V
            let vt = HotKey::new(Some(Modifiers::ALT), Code::KeyV);
            if mgr.register(vt).is_ok() {
                map.insert(vt.id(), HotkeyAction::VoiceTyping);
                self.voice_typing_hotkey = Some(vt);
            }

            // 2. Live Subtitles Overlay: Ctrl+Shift+L
            let sub = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyL);
            if mgr.register(sub).is_ok() {
                map.insert(sub.id(), HotkeyAction::LiveSubtitles);
                self.subtitles_hotkey = Some(sub);
            }

            // 3. Quick Recall History: Alt+Shift+H
            let qr = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyH);
            if mgr.register(qr).is_ok() {
                map.insert(qr.id(), HotkeyAction::QuickRecall);
                self.quick_recall_hotkey = Some(qr);
            }

            // 4. Caption Window Click-Through Lock: Alt+C
            let cl = HotKey::new(Some(Modifiers::ALT), Code::KeyC);
            if mgr.register(cl).is_ok() {
                map.insert(cl.id(), HotkeyAction::CaptionLock);
                self.caption_lock_hotkey = Some(cl);
            }
        }
    }

    /// Dynamically remaps a shortcut from a configuration string (e.g. "Alt+V").
    pub fn rebind_shortcut(
        &mut self,
        action: HotkeyAction,
        shortcut_str: &str,
    ) -> Result<(), String> {
        let mgr = self
            .manager
            .as_ref()
            .ok_or_else(|| "GlobalHotKeyManager unavailable".to_string())?;

        let hotkey = parse_shortcut(shortcut_str)?;
        let mut map = self.action_map.write();

        match action {
            HotkeyAction::VoiceTyping => {
                if let Some(old) = self.voice_typing_hotkey.take() {
                    let _ = mgr.unregister(old);
                    map.remove(&old.id());
                }
                mgr.register(hotkey).map_err(|e| e.to_string())?;
                map.insert(hotkey.id(), action);
                self.voice_typing_hotkey = Some(hotkey);
            }
            HotkeyAction::LiveSubtitles => {
                if let Some(old) = self.subtitles_hotkey.take() {
                    let _ = mgr.unregister(old);
                    map.remove(&old.id());
                }
                mgr.register(hotkey).map_err(|e| e.to_string())?;
                map.insert(hotkey.id(), action);
                self.subtitles_hotkey = Some(hotkey);
            }
            HotkeyAction::QuickRecall => {
                if let Some(old) = self.quick_recall_hotkey.take() {
                    let _ = mgr.unregister(old);
                    map.remove(&old.id());
                }
                mgr.register(hotkey).map_err(|e| e.to_string())?;
                map.insert(hotkey.id(), action);
                self.quick_recall_hotkey = Some(hotkey);
            }
            HotkeyAction::CaptionLock => {
                if let Some(old) = self.caption_lock_hotkey.take() {
                    let _ = mgr.unregister(old);
                    map.remove(&old.id());
                }
                mgr.register(hotkey).map_err(|e| e.to_string())?;
                map.insert(hotkey.id(), action);
                self.caption_lock_hotkey = Some(hotkey);
            }
        }
        Ok(())
    }

    /// Matches an incoming `GlobalHotKeyEvent` against registered hotkeys.
    pub fn match_event(&self, event: &GlobalHotKeyEvent) -> Option<(HotkeyAction, HotKeyState)> {
        let id = event.id;
        let map = self.action_map.read();
        map.get(&id).copied().map(|act| (act, event.state))
    }

    pub fn unregister_all(&mut self) {
        if let Some(ref mgr) = self.manager {
            let mut map = self.action_map.write();
            if let Some(h) = self.voice_typing_hotkey.take() {
                let _ = mgr.unregister(h);
                map.remove(&h.id());
            }
            if let Some(h) = self.subtitles_hotkey.take() {
                let _ = mgr.unregister(h);
                map.remove(&h.id());
            }
            if let Some(h) = self.quick_recall_hotkey.take() {
                let _ = mgr.unregister(h);
                map.remove(&h.id());
            }
            if let Some(h) = self.caption_lock_hotkey.take() {
                let _ = mgr.unregister(h);
                map.remove(&h.id());
            }
        }
    }
}

impl Default for HotkeyController {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HotkeyController {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

/// Parses a human-readable shortcut string into a `HotKey`.
pub fn parse_shortcut(shortcut: &str) -> Result<HotKey, String> {
    let parts: Vec<&str> = shortcut.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return Err("Empty shortcut string".to_string());
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "super" | "meta" | "win" | "cmd" => modifiers |= Modifiers::META,
            key => {
                let code = match key {
                    "space" => Code::Space,
                    "enter" | "return" => Code::Enter,
                    "esc" | "escape" => Code::Escape,
                    "tab" => Code::Tab,
                    "a" => Code::KeyA,
                    "b" => Code::KeyB,
                    "c" => Code::KeyC,
                    "d" => Code::KeyD,
                    "e" => Code::KeyE,
                    "f" => Code::KeyF,
                    "g" => Code::KeyG,
                    "h" => Code::KeyH,
                    "i" => Code::KeyI,
                    "j" => Code::KeyJ,
                    "k" => Code::KeyK,
                    "l" => Code::KeyL,
                    "m" => Code::KeyM,
                    "n" => Code::KeyN,
                    "o" => Code::KeyO,
                    "p" => Code::KeyP,
                    "q" => Code::KeyQ,
                    "r" => Code::KeyR,
                    "s" => Code::KeyS,
                    "t" => Code::KeyT,
                    "u" => Code::KeyU,
                    "v" => Code::KeyV,
                    "w" => Code::KeyW,
                    "x" => Code::KeyX,
                    "y" => Code::KeyY,
                    "z" => Code::KeyZ,
                    "0" => Code::Digit0,
                    "1" => Code::Digit1,
                    "2" => Code::Digit2,
                    "3" => Code::Digit3,
                    "4" => Code::Digit4,
                    "5" => Code::Digit5,
                    "6" => Code::Digit6,
                    "7" => Code::Digit7,
                    "8" => Code::Digit8,
                    "9" => Code::Digit9,
                    "f1" => Code::F1,
                    "f2" => Code::F2,
                    "f3" => Code::F3,
                    "f4" => Code::F4,
                    "f5" => Code::F5,
                    "f6" => Code::F6,
                    "f7" => Code::F7,
                    "f8" => Code::F8,
                    "f9" => Code::F9,
                    "f10" => Code::F10,
                    "f11" => Code::F11,
                    "f12" => Code::F12,
                    other => return Err(format!("Unsupported key: {other}")),
                };
                key_code = Some(code);
            }
        }
    }

    let code = key_code.ok_or_else(|| "Missing key code in shortcut".to_string())?;
    let mods = if modifiers.is_empty() {
        None
    } else {
        Some(modifiers)
    };
    Ok(HotKey::new(mods, code))
}

/// Spawns a background task running the global hotkey event receiver loop.
pub fn spawn_hotkey_event_loop<E>(
    action_map: Arc<parking_lot::RwLock<HashMap<u32, HotkeyAction>>>,
    emit_fn: E,
) -> tokio::task::JoinHandle<()>
where
    E: Fn(&str, serde_json::Value) + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            while let Ok(event) = receiver.try_recv() {
                let matched = {
                    let map = action_map.read();
                    map.get(&event.id).copied().map(|act| (act, event.state))
                };

                if let Some((action, state)) = matched {
                    let is_pressed = matches!(state, HotKeyState::Pressed);
                    match action {
                        HotkeyAction::VoiceTyping => {
                            emit_fn(
                                "hotkey:voice_typing",
                                serde_json::json!({ "pressed": is_pressed }),
                            );
                        }
                        HotkeyAction::LiveSubtitles => {
                            if is_pressed {
                                emit_fn("hotkey:toggle_subtitles", serde_json::json!({}));
                            }
                        }
                        HotkeyAction::QuickRecall => {
                            if is_pressed {
                                emit_fn("hotkey:toggle_quick_recall", serde_json::json!({}));
                            }
                        }
                        HotkeyAction::CaptionLock => {
                            if is_pressed {
                                emit_fn("hotkey:toggle_caption_lock", serde_json::json!({}));
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    })
}

// -----------------------------------------------------------------------------
// Win32 Physical Key Release Polling (GetAsyncKeyState)
// -----------------------------------------------------------------------------

#[cfg(windows)]
fn is_shortcut_modifier_pressed(modifier: ShortcutModifier) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    let is_pressed = |vk: VIRTUAL_KEY| -> bool {
        unsafe { (GetAsyncKeyState(i32::from(vk.0)) as u16 & 0x8000) != 0 }
    };

    match modifier {
        ShortcutModifier::Control => is_pressed(VK_CONTROL),
        ShortcutModifier::Alt => is_pressed(VK_MENU),
        ShortcutModifier::Shift => is_pressed(VK_SHIFT),
        ShortcutModifier::Meta => is_pressed(VK_LWIN) || is_pressed(VK_RWIN),
    }
}

/// Awaits hardware release of modifier keys before simulating text injection.
pub fn wait_for_shortcut_modifiers_release(modifiers: &[ShortcutModifier]) -> ModifierWaitResult {
    #[cfg(windows)]
    {
        if modifiers.is_empty() {
            return ModifierWaitResult {
                released: true,
                attempts: 0,
                still_pressed: Vec::new(),
            };
        }

        let max_attempts =
            (SHORTCUT_MODIFIER_RELEASE_WAIT_MS / SHORTCUT_MODIFIER_POLL_INTERVAL_MS) as usize + 1;

        for attempt in 0..max_attempts {
            let still_pressed: Vec<ShortcutModifier> = modifiers
                .iter()
                .copied()
                .filter(|m| is_shortcut_modifier_pressed(*m))
                .collect();

            if still_pressed.is_empty() {
                return ModifierWaitResult {
                    released: true,
                    attempts: attempt + 1,
                    still_pressed: Vec::new(),
                };
            }

            std::thread::sleep(std::time::Duration::from_millis(
                SHORTCUT_MODIFIER_POLL_INTERVAL_MS,
            ));
        }

        let final_pressed: Vec<ShortcutModifier> = modifiers
            .iter()
            .copied()
            .filter(|m| is_shortcut_modifier_pressed(*m))
            .collect();

        ModifierWaitResult {
            released: final_pressed.is_empty(),
            attempts: max_attempts,
            still_pressed: final_pressed,
        }
    }

    #[cfg(not(windows))]
    {
        let _ = modifiers;
        ModifierWaitResult {
            released: true,
            attempts: 1,
            still_pressed: Vec::new(),
        }
    }
}

/// Testable probing helper for verifying release polling logic under simulation.
pub fn poll_shortcut_modifiers_release_with_probe<F>(
    modifiers: &[ShortcutModifier],
    max_attempts: usize,
    mut is_pressed: F,
) -> ModifierWaitResult
where
    F: FnMut(ShortcutModifier, usize) -> bool,
{
    if modifiers.is_empty() {
        return ModifierWaitResult {
            released: true,
            attempts: 0,
            still_pressed: Vec::new(),
        };
    }

    let attempts = max_attempts.max(1);
    for attempt in 0..attempts {
        let still_pressed: Vec<ShortcutModifier> = modifiers
            .iter()
            .copied()
            .filter(|m| is_pressed(*m, attempt))
            .collect();

        if still_pressed.is_empty() {
            return ModifierWaitResult {
                released: true,
                attempts: attempt + 1,
                still_pressed: Vec::new(),
            };
        }

        if attempt + 1 == attempts {
            return ModifierWaitResult {
                released: false,
                attempts,
                still_pressed,
            };
        }
    }

    ModifierWaitResult {
        released: false,
        attempts,
        still_pressed: modifiers.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_shortcuts() {
        let vt = parse_shortcut("Alt+V").expect("should parse Alt+V");
        assert_eq!(vt.key, Code::KeyV);
        assert_eq!(vt.mods, Modifiers::ALT);

        let sub = parse_shortcut("Ctrl+Shift+L").expect("should parse Ctrl+Shift+L");
        assert_eq!(sub.key, Code::KeyL);
        assert_eq!(sub.mods, Modifiers::CONTROL | Modifiers::SHIFT);

        let qr = parse_shortcut("Alt+Shift+H").expect("should parse Alt+Shift+H");
        assert_eq!(qr.key, Code::KeyH);
        assert_eq!(qr.mods, Modifiers::ALT | Modifiers::SHIFT);

        let cl = parse_shortcut("Alt+C").expect("should parse Alt+C");
        assert_eq!(cl.key, Code::KeyC);
        assert_eq!(cl.mods, Modifiers::ALT);
    }

    #[test]
    fn fails_on_invalid_shortcuts() {
        assert!(parse_shortcut("").is_err());
        assert!(parse_shortcut("Alt+UnknownKey").is_err());
        assert!(parse_shortcut("Control").is_err()); // missing key code
    }

    #[test]
    fn modifier_release_probe_succeeds_immediately_when_unpressed() {
        let res = poll_shortcut_modifiers_release_with_probe(
            &[ShortcutModifier::Alt, ShortcutModifier::Shift],
            5,
            |_mod, _attempt| false,
        );
        assert!(res.released);
        assert_eq!(res.attempts, 1);
        assert!(res.still_pressed.is_empty());
    }

    #[test]
    fn modifier_release_probe_succeeds_after_key_released() {
        let res = poll_shortcut_modifiers_release_with_probe(
            &[ShortcutModifier::Alt],
            5,
            |_mod, attempt| attempt < 2, // pressed on attempts 0 and 1, released on attempt 2
        );
        assert!(res.released);
        assert_eq!(res.attempts, 3);
        assert!(res.still_pressed.is_empty());
    }

    #[test]
    fn modifier_release_probe_times_out_when_keys_stuck() {
        let res = poll_shortcut_modifiers_release_with_probe(
            &[ShortcutModifier::Alt, ShortcutModifier::Control],
            4,
            |_mod, _attempt| true, // always pressed
        );
        assert!(!res.released);
        assert_eq!(res.attempts, 4);
        assert_eq!(
            res.still_pressed,
            vec![ShortcutModifier::Alt, ShortcutModifier::Control]
        );
    }
}
