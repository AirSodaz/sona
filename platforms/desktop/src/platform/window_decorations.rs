//! Window decorations and native platform styling.

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};

pub fn apply_dark_mode_titlebar(_hwnd_ptr: isize, _dark: bool) {
    #[cfg(windows)]
    unsafe {
        let hwnd = HWND(_hwnd_ptr as _);
        let dark_mode: i32 = if _dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }
}
