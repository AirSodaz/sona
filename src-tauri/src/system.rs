use enigo::{Enigo, Keyboard, Mouse, Settings};

#[tauri::command]
pub fn inject_text(text: String) -> Result<(), String> {
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
#[tauri::command]
pub fn get_text_cursor_position() -> Result<Option<(i32, i32)>, String> {
    use std::mem::size_of;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };

    unsafe {
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

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_text_cursor_position() -> Result<Option<(i32, i32)>, String> {
    Ok(None)
}
