//! Global shortcuts controller using `global-hotkey`.

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};
use std::sync::Arc;

pub struct HotkeyController {
    manager: Option<Arc<GlobalHotKeyManager>>,
    voice_typing_hotkey: Option<HotKey>,
    caption_toggle_hotkey: Option<HotKey>,
}

impl HotkeyController {
    pub fn new() -> Self {
        let manager = GlobalHotKeyManager::new().ok().map(Arc::new);
        let mut controller = Self {
            manager,
            voice_typing_hotkey: None,
            caption_toggle_hotkey: None,
        };

        controller.register_defaults();
        controller
    }

    pub fn register_defaults(&mut self) {
        if let Some(ref mgr) = self.manager {
            // Default: Ctrl+Shift+Space for Voice Typing
            let vt = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
            let _ = mgr.register(vt);
            self.voice_typing_hotkey = Some(vt);

            // Default: Ctrl+Shift+L for Caption Lock/Toggle
            let cap = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyL);
            let _ = mgr.register(cap);
            self.caption_toggle_hotkey = Some(cap);
        }
    }

    pub fn unregister_all(&mut self) {
        if let Some(ref mgr) = self.manager {
            if let Some(vt) = self.voice_typing_hotkey.take() {
                let _ = mgr.unregister(vt);
            }
            if let Some(cap) = self.caption_toggle_hotkey.take() {
                let _ = mgr.unregister(cap);
            }
        }
    }
}

impl Drop for HotkeyController {
    fn drop(&mut self) {
        self.unregister_all();
    }
}
