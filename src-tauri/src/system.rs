use enigo::{Enigo, Keyboard, Mouse, Settings};
use serde::Deserialize;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::GetLastError;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
    VK_SHIFT,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShortcutModifier {
    Control,
    Alt,
    Shift,
    Meta,
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum InjectionStepKind {
    AsciiVirtualKey,
    UnicodeSendInput,
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct InjectionStep {
    kind: InjectionStepKind,
    text: String,
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct ModifierWaitResult {
    released: bool,
    attempts: usize,
    still_pressed: Vec<ShortcutModifier>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnicodeKeyInputSpec {
    scan_code: u16,
    is_key_up: bool,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct StepInjectionFailure {
    sent_chars: usize,
    error: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct WindowsInjectionFailure {
    injection_mode: &'static str,
    sent_prefix_chars: usize,
    error: String,
}

#[cfg(target_os = "windows")]
const SHORTCUT_MODIFIER_RELEASE_WAIT_MS: u64 = 250;
#[cfg(target_os = "windows")]
const SHORTCUT_MODIFIER_POLL_INTERVAL_MS: u64 = 10;
#[cfg(target_os = "windows")]
const ASCII_SHIFT_STATE_SHIFT: u8 = 0b0000_0001;
#[cfg(target_os = "windows")]
const ASCII_SHIFT_STATE_CTRL: u8 = 0b0000_0010;
#[cfg(target_os = "windows")]
const ASCII_SHIFT_STATE_ALT: u8 = 0b0000_0100;

fn shortcut_modifier_label(modifier: ShortcutModifier) -> &'static str {
    match modifier {
        ShortcutModifier::Control => "control",
        ShortcutModifier::Alt => "alt",
        ShortcutModifier::Shift => "shift",
        ShortcutModifier::Meta => "meta",
    }
}

fn format_shortcut_modifiers(shortcut_modifiers: &[ShortcutModifier]) -> String {
    if shortcut_modifiers.is_empty() {
        return "none".to_string();
    }

    shortcut_modifiers
        .iter()
        .map(|modifier| shortcut_modifier_label(*modifier))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(any(test, target_os = "windows"))]
fn format_modifier_wait_result(result: &ModifierWaitResult) -> String {
    if result.released {
        return format!("released:{}", result.attempts);
    }

    format!(
        "timeout:{}:{}",
        result.attempts,
        format_shortcut_modifiers(&result.still_pressed)
    )
}

#[cfg(any(test, target_os = "windows"))]
fn should_use_ascii_virtual_key_path(ch: char) -> bool {
    ch.is_ascii() && !ch.is_ascii_control()
}

#[cfg(any(test, target_os = "windows"))]
fn build_injection_steps(text: &str) -> Vec<InjectionStep> {
    let mut steps = Vec::new();
    let mut current_kind: Option<InjectionStepKind> = None;
    let mut current_text = String::new();

    for ch in text.chars() {
        let next_kind = if should_use_ascii_virtual_key_path(ch) {
            InjectionStepKind::AsciiVirtualKey
        } else {
            InjectionStepKind::UnicodeSendInput
        };

        if current_kind.as_ref() != Some(&next_kind) {
            if let Some(kind) = current_kind.take() {
                steps.push(InjectionStep {
                    kind,
                    text: std::mem::take(&mut current_text),
                });
            }
            current_kind = Some(next_kind);
        }

        current_text.push(ch);
    }

    if let Some(kind) = current_kind {
        steps.push(InjectionStep {
            kind,
            text: current_text,
        });
    }

    steps
}

#[cfg(any(test, target_os = "windows"))]
fn remaining_text_after_sent_prefix(text: &str, sent_prefix_chars: usize) -> String {
    text.chars().skip(sent_prefix_chars).collect()
}

#[cfg(test)]
fn poll_shortcut_modifiers_release_with_probe<F>(
    shortcut_modifiers: &[ShortcutModifier],
    max_attempts: usize,
    mut is_pressed: F,
) -> ModifierWaitResult
where
    F: FnMut(ShortcutModifier, usize) -> bool,
{
    if shortcut_modifiers.is_empty() {
        return ModifierWaitResult {
            released: true,
            attempts: 0,
            still_pressed: Vec::new(),
        };
    }

    let attempts = max_attempts.max(1);
    for attempt in 0..attempts {
        let still_pressed = shortcut_modifiers
            .iter()
            .copied()
            .filter(|modifier| is_pressed(*modifier, attempt))
            .collect::<Vec<_>>();

        if still_pressed.is_empty() {
            return ModifierWaitResult {
                released: true,
                attempts: attempt + 1,
                still_pressed,
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

    unreachable!("modifier polling should always return within the configured attempts")
}

#[cfg(test)]
fn build_unicode_key_input_specs(text: &str) -> Vec<UnicodeKeyInputSpec> {
    let mut specs = Vec::with_capacity(text.encode_utf16().count() * 2);
    for scan_code in text.encode_utf16() {
        specs.push(UnicodeKeyInputSpec {
            scan_code,
            is_key_up: false,
        });
        specs.push(UnicodeKeyInputSpec {
            scan_code,
            is_key_up: true,
        });
    }

    specs
}

#[tauri::command]
pub fn inject_text(
    text: String,
    shortcut_modifiers: Option<Vec<ShortcutModifier>>,
) -> Result<(), String> {
    let shortcut_modifiers = shortcut_modifiers.unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        let modifier_wait_result = wait_for_shortcut_modifiers_release(&shortcut_modifiers);
        let modifier_wait_result_label = format_modifier_wait_result(&modifier_wait_result);

        match inject_text_with_windows_sendinput(&text) {
            Ok(injection_mode) => {
                log::info!(
                    "[System] windows_injection_success text_len={} shortcut_modifiers={} modifier_wait_result={} injection_mode={} sent_prefix_len={} fallback_remaining_len=0",
                    text.chars().count(),
                    format_shortcut_modifiers(&shortcut_modifiers),
                    modifier_wait_result_label,
                    injection_mode,
                    text.chars().count(),
                );
                return Ok(());
            }
            Err(failure) => {
                let remaining_text =
                    remaining_text_after_sent_prefix(&text, failure.sent_prefix_chars);
                let fallback_remaining_len = remaining_text.chars().count();
                log::warn!(
                    "[System] windows_injection_failed_fallback_enigo text_len={} shortcut_modifiers={} modifier_wait_result={} injection_mode={} sent_prefix_len={} fallback_remaining_len={} error={}",
                    text.chars().count(),
                    format_shortcut_modifiers(&shortcut_modifiers),
                    modifier_wait_result_label,
                    failure.injection_mode,
                    failure.sent_prefix_chars,
                    fallback_remaining_len,
                    failure.error,
                );

                if remaining_text.is_empty() {
                    return Ok(());
                }

                inject_text_with_enigo(&remaining_text)?;
                return Ok(());
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        inject_text_with_enigo(&text)
    }
}

fn inject_text_with_enigo(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_mouse_position() -> Result<(i32, i32), String> {
    let enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.location().map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn injection_step_mode(kind: &InjectionStepKind) -> &'static str {
    match kind {
        InjectionStepKind::AsciiVirtualKey => "ascii_virtual_key",
        InjectionStepKind::UnicodeSendInput => "unicode_sendinput",
    }
}

#[cfg(target_os = "windows")]
fn injection_plan_mode(steps: &[InjectionStep]) -> &'static str {
    let has_ascii = steps
        .iter()
        .any(|step| matches!(step.kind, InjectionStepKind::AsciiVirtualKey));
    let has_unicode = steps
        .iter()
        .any(|step| matches!(step.kind, InjectionStepKind::UnicodeSendInput));

    match (has_ascii, has_unicode) {
        (true, true) => "mixed",
        (true, false) => "ascii_virtual_key",
        (false, true) => "unicode_sendinput",
        (false, false) => "none",
    }
}

#[cfg(target_os = "windows")]
fn wait_for_shortcut_modifiers_release(
    shortcut_modifiers: &[ShortcutModifier],
) -> ModifierWaitResult {
    if shortcut_modifiers.is_empty() {
        return ModifierWaitResult {
            released: true,
            attempts: 0,
            still_pressed: Vec::new(),
        };
    }

    let max_attempts =
        (SHORTCUT_MODIFIER_RELEASE_WAIT_MS / SHORTCUT_MODIFIER_POLL_INTERVAL_MS) as usize + 1;

    for attempt in 0..max_attempts {
        let still_pressed = shortcut_modifiers
            .iter()
            .copied()
            .filter(|modifier| is_shortcut_modifier_pressed(*modifier))
            .collect::<Vec<_>>();

        if still_pressed.is_empty() {
            return ModifierWaitResult {
                released: true,
                attempts: attempt + 1,
                still_pressed,
            };
        }

        if attempt + 1 < max_attempts {
            std::thread::sleep(std::time::Duration::from_millis(
                SHORTCUT_MODIFIER_POLL_INTERVAL_MS,
            ));
        } else {
            return ModifierWaitResult {
                released: false,
                attempts: max_attempts,
                still_pressed,
            };
        }
    }

    unreachable!("modifier waiting should always return within the configured attempts")
}

#[cfg(target_os = "windows")]
fn is_shortcut_modifier_pressed(modifier: ShortcutModifier) -> bool {
    match modifier {
        ShortcutModifier::Control => is_virtual_key_pressed(VK_CONTROL),
        ShortcutModifier::Alt => is_virtual_key_pressed(VK_MENU),
        ShortcutModifier::Shift => is_virtual_key_pressed(VK_SHIFT),
        ShortcutModifier::Meta => {
            is_virtual_key_pressed(VK_LWIN) || is_virtual_key_pressed(VK_RWIN)
        }
    }
}

#[cfg(target_os = "windows")]
fn is_virtual_key_pressed(virtual_key: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(i32::from(virtual_key.0)) as u16 & 0x8000) != 0 }
}

#[cfg(target_os = "windows")]
fn inject_text_with_windows_sendinput(text: &str) -> Result<&'static str, WindowsInjectionFailure> {
    if text.is_empty() {
        return Ok("none");
    }

    let steps = build_injection_steps(text);
    let plan_mode = injection_plan_mode(&steps);
    let mut sent_prefix_chars = 0usize;

    for step in steps {
        let step_mode = injection_step_mode(&step.kind);
        let sent_chars = match step.kind {
            InjectionStepKind::AsciiVirtualKey => inject_ascii_text_with_virtual_key(&step.text)
                .map_err(|failure| WindowsInjectionFailure {
                    injection_mode: step_mode,
                    sent_prefix_chars: sent_prefix_chars + failure.sent_chars,
                    error: failure.error,
                })?,
            InjectionStepKind::UnicodeSendInput => inject_unicode_text_with_sendinput(&step.text)
                .map_err(|failure| WindowsInjectionFailure {
                injection_mode: step_mode,
                sent_prefix_chars: sent_prefix_chars + failure.sent_chars,
                error: failure.error,
            })?,
        };

        sent_prefix_chars += sent_chars;
    }

    Ok(plan_mode)
}

#[cfg(target_os = "windows")]
fn inject_ascii_text_with_virtual_key(text: &str) -> Result<usize, StepInjectionFailure> {
    let mut sent_chars = 0usize;
    for ch in text.chars() {
        inject_ascii_char_with_virtual_key(ch)
            .map_err(|error| StepInjectionFailure { sent_chars, error })?;
        sent_chars += 1;
    }

    Ok(sent_chars)
}

#[cfg(target_os = "windows")]
fn inject_unicode_text_with_sendinput(text: &str) -> Result<usize, StepInjectionFailure> {
    let mut sent_chars = 0usize;
    for ch in text.chars() {
        inject_unicode_char_with_sendinput(ch)
            .map_err(|error| StepInjectionFailure { sent_chars, error })?;
        sent_chars += 1;
    }

    Ok(sent_chars)
}

#[cfg(target_os = "windows")]
fn inject_ascii_char_with_virtual_key(ch: char) -> Result<(), String> {
    if !should_use_ascii_virtual_key_path(ch) {
        return Err(format!(
            "ASCII virtual key injection does not support {:?}",
            ch
        ));
    }

    let vk_key_scan = unsafe { VkKeyScanW(ch as u16) };
    if vk_key_scan == -1 {
        return Err(format!("VkKeyScanW could not resolve {:?}", ch));
    }

    let virtual_key = VIRTUAL_KEY((vk_key_scan as u16) & 0x00FF);
    let shift_state = ((vk_key_scan as u16) >> 8) as u8;

    if shift_state & (ASCII_SHIFT_STATE_CTRL | ASCII_SHIFT_STATE_ALT) != 0 {
        return Err(format!(
            "VkKeyScanW returned unsupported modifier state {} for {:?}",
            shift_state, ch
        ));
    }

    let needs_shift = shift_state & ASCII_SHIFT_STATE_SHIFT != 0;
    let mut inputs = Vec::with_capacity(if needs_shift { 4 } else { 2 });

    if needs_shift {
        inputs.push(build_virtual_key_input(VK_SHIFT, false));
    }
    inputs.push(build_virtual_key_input(virtual_key, false));
    inputs.push(build_virtual_key_input(virtual_key, true));
    if needs_shift {
        inputs.push(build_virtual_key_input(VK_SHIFT, true));
    }

    send_keyboard_inputs(&inputs)
}

#[cfg(target_os = "windows")]
fn inject_unicode_char_with_sendinput(ch: char) -> Result<(), String> {
    let mut utf16_buffer = [0u16; 2];
    let utf16_units = ch.encode_utf16(&mut utf16_buffer);
    let mut inputs = Vec::with_capacity(utf16_units.len() * 2);

    for scan_code in utf16_units {
        inputs.push(build_unicode_input(*scan_code, false));
        inputs.push(build_unicode_input(*scan_code, true));
    }

    send_keyboard_inputs(&inputs)
}

#[cfg(target_os = "windows")]
fn send_keyboard_inputs(inputs: &[INPUT]) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }

    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        let last_error = unsafe { GetLastError() };
        return Err(format!(
            "SendInput delivered {sent}/{} input events, last_error={last_error:?}",
            inputs.len()
        ));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn build_virtual_key_input(virtual_key: VIRTUAL_KEY, is_key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                wScan: 0,
                dwFlags: if is_key_up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(target_os = "windows")]
fn build_unicode_input(scan_code: u16, is_key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan_code,
                dwFlags: if is_key_up {
                    KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                } else {
                    KEYEVENTF_UNICODE
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_text_cursor_position() -> Result<Option<(i32, i32)>, String> {
    use std::mem::size_of;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };

    unsafe {
        // 1. Try UI Automation (Modern apps: Chrome, Edge, VS Code, etc.)
        if let Ok(pos) = get_uia_caret_position() {
            if let Some(p) = pos {
                return Ok(Some(p));
            }
        }

        // 2. Fallback to Win32 GUI Thread Info (Legacy apps: Notepad, etc.)
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return Ok(None);
        }

        let thread_id = GetWindowThreadProcessId(foreground, None);
        if thread_id == 0 {
            return Ok(None);
        }

        let mut info = GUITHREADINFO::default();
        info.cbSize = size_of::<GUITHREADINFO>() as u32;

        if GetGUIThreadInfo(thread_id, &mut info).is_err() {
            return Ok(None);
        }

        // Use hwndCaret if available, otherwise fallback to hwndFocus which many modern apps use
        let hwnd = if !info.hwndCaret.0.is_null() {
            info.hwndCaret
        } else if !info.hwndFocus.0.is_null() {
            info.hwndFocus
        } else {
            return Ok(None);
        };

        // If the caret rectangle is all zeros, it's likely not a valid caret position
        if info.rcCaret.left == 0
            && info.rcCaret.top == 0
            && info.rcCaret.right == 0
            && info.rcCaret.bottom == 0
        {
            return Ok(None);
        }

        let mut caret_point = POINT {
            x: info.rcCaret.left,
            y: info.rcCaret.bottom,
        };

        if !ClientToScreen(hwnd, &mut caret_point).as_bool() {
            return Ok(None);
        }

        Ok(Some((caret_point.x, caret_point.y)))
    }
}

#[cfg(target_os = "windows")]
unsafe fn get_uia_caret_position() -> windows::core::Result<Option<(i32, i32)>> {
    use windows::core::*;
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Accessibility::*;

    // Initialize COM for this thread
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

    let automation: IUIAutomation = CoCreateInstance(&CUIAutomation8, None, CLSCTX_ALL)?;
    let focused_element = automation.GetFocusedElement()?;

    // Try TextPattern2 (Windows 8.1+)
    if let Ok(pattern) = focused_element.GetCurrentPattern(UIA_TextPattern2Id) {
        if let Ok(text_pattern2) = pattern.cast::<IUIAutomationTextPattern2>() {
            let mut is_active = Default::default();
            if let Ok(range) = text_pattern2.GetCaretRange(&mut is_active) {
                if let Ok(rects) = range.GetBoundingRectangles() {
                    let rect_data = safearray_to_f64_vec(rects)?;
                    if rect_data.len() >= 4 {
                        // rects is an array of [left, top, width, height]
                        let x = rect_data[0] as i32;
                        let y = (rect_data[1] + rect_data[3]) as i32;
                        return Ok(Some((x, y)));
                    }
                }
            }
        }
    }

    // Fallback to TextPattern selection
    if let Ok(pattern) = focused_element.GetCurrentPattern(UIA_TextPatternId) {
        if let Ok(text_pattern) = pattern.cast::<IUIAutomationTextPattern>() {
            if let Ok(selection) = text_pattern.GetSelection() {
                if selection.Length()? > 0 {
                    let range = selection.GetElement(0)?;
                    if let Ok(rects) = range.GetBoundingRectangles() {
                        let rect_data = safearray_to_f64_vec(rects)?;
                        if rect_data.len() >= 4 {
                            let x = rect_data[0] as i32;
                            let y = (rect_data[1] + rect_data[3]) as i32;
                            return Ok(Some((x, y)));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

#[cfg(target_os = "windows")]
unsafe fn safearray_to_f64_vec(
    psa: *mut windows::Win32::System::Com::SAFEARRAY,
) -> windows::core::Result<Vec<f64>> {
    use windows::Win32::System::Ole::*;
    if psa.is_null() {
        return Ok(Vec::new());
    }

    let lbound = SafeArrayGetLBound(psa, 1)?;
    let ubound = SafeArrayGetUBound(psa, 1)?;
    let len = (ubound - lbound + 1) as usize;

    let mut data_ptr = std::ptr::null_mut();
    SafeArrayAccessData(psa, &mut data_ptr)?;
    let slice = std::slice::from_raw_parts(data_ptr as *const f64, len);
    let vec = slice.to_vec();
    SafeArrayUnaccessData(psa)?;

    // We should ideally free the SAFEARRAY if we're the owner.
    // In UIA, GetBoundingRectangles returns a new SAFEARRAY that the caller must free.
    SafeArrayDestroy(psa)?;

    Ok(vec)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_text_cursor_position() -> Result<Option<(i32, i32)>, String> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{
        build_injection_steps, build_unicode_key_input_specs, format_modifier_wait_result,
        injection_plan_mode, poll_shortcut_modifiers_release_with_probe,
        remaining_text_after_sent_prefix, InjectionStep, InjectionStepKind, ModifierWaitResult,
        ShortcutModifier, UnicodeKeyInputSpec,
    };

    #[test]
    fn builds_injection_steps_for_ascii_digits_and_full_width_punctuation() {
        assert_eq!(
            build_injection_steps("123。"),
            vec![
                InjectionStep {
                    kind: InjectionStepKind::AsciiVirtualKey,
                    text: "123".to_string(),
                },
                InjectionStep {
                    kind: InjectionStepKind::UnicodeSendInput,
                    text: "。".to_string(),
                },
            ]
        );
    }

    #[test]
    fn builds_injection_steps_for_mixed_text() {
        assert_eq!(
            build_injection_steps("喂，123。"),
            vec![
                InjectionStep {
                    kind: InjectionStepKind::UnicodeSendInput,
                    text: "喂，".to_string(),
                },
                InjectionStep {
                    kind: InjectionStepKind::AsciiVirtualKey,
                    text: "123".to_string(),
                },
                InjectionStep {
                    kind: InjectionStepKind::UnicodeSendInput,
                    text: "。".to_string(),
                },
            ]
        );
    }

    #[test]
    fn reports_modifier_release_when_probe_clears_immediately() {
        let result = poll_shortcut_modifiers_release_with_probe(
            &[ShortcutModifier::Control],
            4,
            |_modifier, _attempt| false,
        );

        assert_eq!(
            result,
            ModifierWaitResult {
                released: true,
                attempts: 1,
                still_pressed: Vec::new(),
            }
        );
        assert_eq!(format_modifier_wait_result(&result), "released:1");
    }

    #[test]
    fn reports_modifier_wait_timeout_when_probe_never_clears() {
        let result = poll_shortcut_modifiers_release_with_probe(
            &[ShortcutModifier::Control, ShortcutModifier::Shift],
            3,
            |_modifier, _attempt| true,
        );

        assert_eq!(
            result,
            ModifierWaitResult {
                released: false,
                attempts: 3,
                still_pressed: vec![ShortcutModifier::Control, ShortcutModifier::Shift],
            }
        );
        assert_eq!(
            format_modifier_wait_result(&result),
            "timeout:3:control,shift"
        );
    }

    #[test]
    fn keeps_only_the_remaining_suffix_for_fallback() {
        assert_eq!(remaining_text_after_sent_prefix("喂，123。", 2), "123。");
        assert_eq!(remaining_text_after_sent_prefix("123。", 3), "。");
    }

    #[test]
    fn reports_the_expected_injection_plan_mode() {
        assert_eq!(
            injection_plan_mode(&build_injection_steps("123")),
            "ascii_virtual_key"
        );
        assert_eq!(
            injection_plan_mode(&build_injection_steps("喂。")),
            "unicode_sendinput"
        );
        assert_eq!(
            injection_plan_mode(&build_injection_steps("喂，123。")),
            "mixed"
        );
    }

    #[test]
    fn builds_unicode_key_input_specs_for_mixed_text() {
        let specs = build_unicode_key_input_specs("喂，123。");
        let utf16_units = "喂，123。".encode_utf16().collect::<Vec<_>>();

        assert_eq!(specs.len(), utf16_units.len() * 2);
        for (index, unit) in utf16_units.iter().enumerate() {
            assert_eq!(
                specs[index * 2],
                UnicodeKeyInputSpec {
                    scan_code: *unit,
                    is_key_up: false,
                }
            );
            assert_eq!(
                specs[index * 2 + 1],
                UnicodeKeyInputSpec {
                    scan_code: *unit,
                    is_key_up: true,
                }
            );
        }
    }

    #[test]
    fn builds_empty_unicode_key_input_specs_for_empty_text() {
        assert!(build_unicode_key_input_specs("").is_empty());
    }
}
