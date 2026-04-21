use enigo::{Enigo, Keyboard, Mouse, Settings};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::GetLastError;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY,
};

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnicodeKeyInputSpec {
    scan_code: u16,
    is_key_up: bool,
}

#[cfg(any(test, target_os = "windows"))]
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
pub fn inject_text(text: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if let Err(error) = inject_text_with_unicode_sendinput(&text) {
            log::warn!(
                "[System] unicode_sendinput_failed_fallback_enigo text_len={} error={}",
                text.chars().count(),
                error,
            );
        } else {
            log::info!(
                "[System] unicode_sendinput_success text_len={}",
                text.chars().count()
            );
            return Ok(());
        }
    }

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(&text).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_mouse_position() -> Result<(i32, i32), String> {
    let enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.location().map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn inject_text_with_unicode_sendinput(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    let inputs = build_unicode_key_input_specs(text)
        .into_iter()
        .map(|spec| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: spec.scan_code,
                    dwFlags: if spec.is_key_up {
                        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                    } else {
                        KEYEVENTF_UNICODE
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect::<Vec<_>>();

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        let last_error = unsafe { GetLastError() };
        return Err(format!(
            "SendInput delivered {sent}/{} input events, last_error={last_error:?}",
            inputs.len()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_unicode_key_input_specs, UnicodeKeyInputSpec};

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
