//! Native Windows title-bar chrome that follows the in-app theme.
//!
//! Sona keeps the operating system window frame (so that Aero Snap, Snap
//! Layouts, Win+Arrow, double-click-to-maximize and the system menu keep
//! working), but recolors it to match the application's own light/dark
//! palette. The application theme is authoritative here: the caption does
//! *not* follow the Windows system theme.
//!
//! `DWMWA_CAPTION_COLOR` and friends require Windows 11 (build 22000). On
//! Windows 10 the calls fail harmlessly and the default frame is kept, so
//! every attribute write is deliberately best-effort.

use serde::Deserialize;

/// The resolved application theme, after `auto` has been matched against the
/// OS color-scheme preference. Only the two concrete values are accepted so
/// that the frontend cannot accidentally hand us an unresolved preference.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedTheme {
    Light,
    Dark,
}

/// Caption, text, and border colors for one theme.
///
/// These mirror the `--color-bg-secondary`, `--color-text-primary` and
/// `--color-border` custom properties in `frontend/src/styles/base.css`.
/// Keeping them here (rather than reading CSS) is intentional: the window
/// frame is drawn by DWM in the native coordinate space and must not block
/// on the webview.
#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChromePalette {
    /// Base caption color. Must be opaque.
    caption: u32,
    /// Caption text *and* the glyph color of the minimize/maximize/close
    /// buttons.
    text: u32,
    /// The 1px frame outline drawn around the whole window.
    border: u32,
}

/// Pack sRGB bytes into the `COLORREF` layout Windows expects.
///
/// `COLORREF` is `0x00BBGGRR`, i.e. *byte-reversed* relative to the familiar
/// `0xRRGGBB` hex notation. Passing an RGB literal straight through produces
/// a blue/red swap, which is the classic symptom of getting this wrong.
#[cfg(any(target_os = "windows", test))]
const fn colorref(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | r as u32
}

#[cfg(any(target_os = "windows", test))]
const LIGHT_PALETTE: ChromePalette = ChromePalette {
    // #f3f3f2 - --color-bg-secondary
    caption: colorref(0xf3, 0xf3, 0xf2),
    // #37352f - --color-text-primary
    text: colorref(0x37, 0x35, 0x2f),
    // #e5e5e5 - --color-bg-tertiary, used as a readable frame outline
    border: colorref(0xe5, 0xe5, 0xe5),
};

#[cfg(any(target_os = "windows", test))]
const DARK_PALETTE: ChromePalette = ChromePalette {
    // #202020 - --color-bg-secondary
    caption: colorref(0x20, 0x20, 0x20),
    // #d4d4d4 - --color-text-primary
    text: colorref(0xd4, 0xd4, 0xd4),
    // #2c2c2c - --color-bg-tertiary
    border: colorref(0x2c, 0x2c, 0x2c),
};

#[cfg(any(target_os = "windows", test))]
impl ResolvedTheme {
    const fn palette(self) -> ChromePalette {
        match self {
            ResolvedTheme::Light => LIGHT_PALETTE,
            ResolvedTheme::Dark => DARK_PALETTE,
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{ChromePalette, ResolvedTheme};
    use tauri::{Runtime, WebviewWindow};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_ROUND, DwmSetWindowAttribute,
    };

    /// Apply `palette` to the window's native frame.
    ///
    /// Returns `Ok(())` even when individual attributes are rejected, because
    /// a partially themed frame is still strictly better than a hard failure
    /// and pre-Windows-11 hosts reject most of these attributes by design.
    pub fn apply<R: Runtime>(
        window: &WebviewWindow<R>,
        theme: ResolvedTheme,
    ) -> Result<(), String> {
        let raw = window.hwnd().map_err(|e| e.to_string())?;
        let hwnd = HWND(raw.0 as *mut _);
        let palette: ChromePalette = theme.palette();

        unsafe {
            for (attribute, value) in [
                (DWMWA_CAPTION_COLOR, palette.caption),
                (DWMWA_TEXT_COLOR, palette.text),
                (DWMWA_BORDER_COLOR, palette.border),
            ] {
                // A rejected write (Windows 10, or a frame DWM declines to
                // recolor) is logged and skipped rather than propagated.
                if let Err(error) = write_color(hwnd, attribute, value) {
                    log::debug!(
                        "[WindowsChrome] Skipped frame color attribute {attribute:?} theme={theme:?} error={error}"
                    );
                }
            }

            // Windows 11 rounded corners; ignored on Windows 10.
            let corner = DWMWCP_ROUND;
            let result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                std::ptr::from_ref(&corner).cast(),
                std::mem::size_of_val(&corner) as u32,
            );
            if let Err(error) = result {
                log::debug!(
                    "[WindowsChrome] Skipped corner preference theme={theme:?} error={error}"
                );
            }
        }

        Ok(())
    }

    /// # Safety
    ///
    /// `hwnd` must be a valid window handle owned by this process. `value` is
    /// borrowed for the duration of the call only, which matches the DWM
    /// contract for color attributes.
    unsafe fn write_color(
        hwnd: HWND,
        attribute: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE,
        value: u32,
    ) -> windows::core::Result<()> {
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                attribute,
                std::ptr::from_ref(&value).cast(),
                std::mem::size_of_val(&value) as u32,
            )
        }
    }
}

/// Apply the native window chrome for `theme` on the given window.
///
/// On non-Windows targets this is a no-op: macOS and Linux keep their own
/// frame conventions and are themed by the platform, not by DWM.
pub fn apply_window_chrome<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    theme: ResolvedTheme,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        imp::apply(window, theme)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, theme);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_colorref_in_byte_reversed_order() {
        // #f3f3f2 -> 0x00F2F3F3, proving blue/red are not swapped.
        assert_eq!(colorref(0xf3, 0xf3, 0xf2), 0x00F2_F3F3);
        assert_eq!(colorref(0x37, 0x35, 0x2f), 0x002F_3537);
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
    fn dark_theme_uses_dark_caption_with_light_text() {
        let palette = ResolvedTheme::Dark.palette();

        // Caption must be the darker surface and text the lighter ink.
        let caption_luma = luminance(palette.caption);
        let text_luma = luminance(palette.text);

        assert!(
            caption_luma < text_luma,
            "dark theme should pair a dark caption with light text"
        );
    }

    #[test]
    fn light_theme_uses_light_caption_with_dark_text() {
        let palette = ResolvedTheme::Light.palette();

        let caption_luma = luminance(palette.caption);
        let text_luma = luminance(palette.text);

        assert!(
            caption_luma > text_luma,
            "light theme should pair a light caption with dark text"
        );
    }

    #[test]
    fn deserializes_only_concrete_themes() {
        assert_eq!(
            serde_json::from_str::<ResolvedTheme>("\"light\"").unwrap(),
            ResolvedTheme::Light
        );
        assert_eq!(
            serde_json::from_str::<ResolvedTheme>("\"dark\"").unwrap(),
            ResolvedTheme::Dark
        );
        // `auto` is resolved by the frontend and must not reach this layer.
        assert!(serde_json::from_str::<ResolvedTheme>("\"auto\"").is_err());
    }

    /// Rec. 601 relative luminance, good enough to assert light/dark ordering.
    fn luminance(colorref: u32) -> u32 {
        let r = colorref & 0xFF;
        let g = (colorref >> 8) & 0xFF;
        let b = (colorref >> 16) & 0xFF;
        r * 299 + g * 587 + b * 114
    }
}
