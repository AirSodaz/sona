//! Window decorations and native platform styling via Win32 DWM.
//!
//! Milestone 2: Native OS Integrations Parity (Gap G38)
//! Provides title-bar chrome recoloring matching in-app Light and Dark themes,
//! byte-reversed `COLORREF` calculation, and Windows 11 rounded corner preference.

use serde::Deserialize;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};

/// The resolved application theme.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedTheme {
    Light,
    Dark,
}

/// Caption, text, and border colors for one theme in Win32 `COLORREF` format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChromePalette {
    /// Base caption header color (`0x00BBGGRR`).
    pub caption: u32,
    /// Caption text and minimize/maximize/close button glyph color (`0x00BBGGRR`).
    pub text: u32,
    /// 1px window frame outline color (`0x00BBGGRR`).
    pub border: u32,
}

/// Packs sRGB bytes into the `COLORREF` layout Windows expects (`0x00BBGGRR`).
///
/// Note: Passing standard RGB `0x00RRGGBB` causes a blue/red swap in DWM.
pub const fn colorref(r: u8, g: u8, b: u8) -> u32 {
    ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

/// Light theme chrome palette:
/// - Header: `#f3f3f2`
/// - Text: `#1a1a19`
/// - Border: `#e5e5e5`
pub const LIGHT_PALETTE: ChromePalette = ChromePalette {
    caption: colorref(0xf3, 0xf3, 0xf2),
    text: colorref(0x1a, 0x1a, 0x19),
    border: colorref(0xe5, 0xe5, 0xe5),
};

/// Dark theme chrome palette:
/// - Header: `#1f1f1e`
/// - Text: `#f5f5f4`
/// - Border: `#2c2c2c`
pub const DARK_PALETTE: ChromePalette = ChromePalette {
    caption: colorref(0x1f, 0x1f, 0x1e),
    text: colorref(0xf5, 0xf5, 0xf4),
    border: colorref(0x2c, 0x2c, 0x2c),
};

impl ResolvedTheme {
    pub const fn palette(self) -> ChromePalette {
        match self {
            ResolvedTheme::Light => LIGHT_PALETTE,
            ResolvedTheme::Dark => DARK_PALETTE,
        }
    }
}

/// Applies theme decorations (caption, text, border colors, and rounded corners) to a window HWND.
pub fn apply_window_decorations(hwnd_ptr: isize, theme: ResolvedTheme) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        let hwnd = HWND(hwnd_ptr as *mut _);
        let palette = theme.palette();

        // 1. Dark mode flag (supported on Windows 10 build 17763+)
        let dark_mode: i32 = match theme {
            ResolvedTheme::Dark => 1,
            ResolvedTheme::Light => 0,
        };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        // 2. Caption, text, and border colors (Windows 11 build 22000+)
        for (attr, val) in [
            (DWMWA_CAPTION_COLOR, palette.caption),
            (DWMWA_TEXT_COLOR, palette.text),
            (DWMWA_BORDER_COLOR, palette.border),
        ] {
            let _ = DwmSetWindowAttribute(
                hwnd,
                attr,
                &val as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        // 3. Windows 11 rounded corner preference (DWMWCP_ROUND = 2)
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of_val(&corner) as u32,
        );

        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = (hwnd_ptr, theme);
        Ok(())
    }
}

/// Applies theme decorations to an existing top-level window by its title.
pub fn apply_window_decorations_to_title(
    window_title: &str,
    theme: ResolvedTheme,
) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
        use windows::core::HSTRING;

        let title = HSTRING::from(window_title);
        let hwnd = FindWindowW(None, &title)
            .map_err(|e| format!("Failed to find window '{}': {e}", window_title))?;

        apply_window_decorations(hwnd.0 as isize, theme)
    }

    #[cfg(not(windows))]
    {
        let _ = (window_title, theme);
        Ok(())
    }
}

/// Backward compatibility entry point for existing calls.
pub fn apply_dark_mode_titlebar(hwnd_ptr: isize, dark: bool) {
    let theme = if dark {
        ResolvedTheme::Dark
    } else {
        ResolvedTheme::Light
    };
    let _ = apply_window_decorations(hwnd_ptr, theme);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_colorref_in_byte_reversed_order() {
        // #f3f3f2 -> R=0xf3, G=0xf3, B=0xf2 -> (B << 16) | (G << 8) | R = 0x00F2F3F3
        assert_eq!(colorref(0xf3, 0xf3, 0xf2), 0x00F2_F3F3);
        // #1a1a19 -> R=0x1a, G=0x1a, B=0x19 -> 0x00191A1A
        assert_eq!(colorref(0x1a, 0x1a, 0x19), 0x0019_1A1A);
        // Pure red #ff0000 -> 0x000000FF
        assert_eq!(colorref(0xff, 0x00, 0x00), 0x0000_00FF);
        // Pure blue #0000ff -> 0x00FF0000
        assert_eq!(colorref(0x00, 0x00, 0xff), 0x00FF_0000);
    }

    #[test]
    fn light_and_dark_palettes_are_distinct() {
        assert_ne!(
            ResolvedTheme::Light.palette(),
            ResolvedTheme::Dark.palette()
        );
    }

    #[test]
    fn dark_theme_has_dark_header_and_light_text() {
        let p = ResolvedTheme::Dark.palette();
        assert!(
            relative_luma(p.caption) < relative_luma(p.text),
            "Dark theme caption should be darker than text"
        );
    }

    #[test]
    fn light_theme_has_light_header_and_dark_text() {
        let p = ResolvedTheme::Light.palette();
        assert!(
            relative_luma(p.caption) > relative_luma(p.text),
            "Light theme caption should be lighter than text"
        );
    }

    #[test]
    fn deserializes_theme_strings() {
        assert_eq!(
            serde_json::from_str::<ResolvedTheme>("\"light\"").unwrap(),
            ResolvedTheme::Light
        );
        assert_eq!(
            serde_json::from_str::<ResolvedTheme>("\"dark\"").unwrap(),
            ResolvedTheme::Dark
        );
        assert!(serde_json::from_str::<ResolvedTheme>("\"auto\"").is_err());
    }

    fn relative_luma(c: u32) -> u32 {
        let r = c & 0xFF;
        let g = (c >> 8) & 0xFF;
        let b = (c >> 16) & 0xFF;
        r * 299 + g * 587 + b * 114
    }
}
