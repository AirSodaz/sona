//! Semantic design tokens and constants.

use gpui_kit::*;

pub const SIDEBAR_WIDTH: Pixels = px(260.0);
pub const TOOLS_SIDEBAR_WIDTH: Pixels = px(220.0);
pub const HEADER_HEIGHT: Pixels = px(56.0);
pub const BOTTOM_BAR_HEIGHT: Pixels = px(64.0);
pub const SETTINGS_MODAL_WIDTH: Pixels = px(1120.0);
pub const SETTINGS_MODAL_HEIGHT: Pixels = px(780.0);

pub const PANEL_LEFT_WIDTH: Pixels = px(350.0);
pub const PANEL_RIGHT_MIN_WIDTH: Pixels = px(500.0);

pub const RADIUS_XS: Pixels = px(6.0);
pub const RADIUS_SM: Pixels = px(10.0);
pub const RADIUS_MD: Pixels = px(12.0);
pub const RADIUS_LG: Pixels = px(16.0);
pub const RADIUS_XL: Pixels = px(20.0);
pub const RADIUS_FULL: Pixels = px(9999.0);

pub const SPACING_XS: Pixels = px(4.0);
pub const SPACING_SM: Pixels = px(8.0);
pub const SPACING_MD: Pixels = px(16.0);
pub const SPACING_LG: Pixels = px(24.0);
pub const SPACING_XL: Pixels = px(32.0);
pub const SPACING_2XL: Pixels = px(48.0);

// Header & Theme Colors
pub const COLOR_HEADER_BG_LIGHT: Hsla = hsla(60.0 / 360.0, 0.03, 0.95, 1.0); // #f3f3f2
pub const COLOR_HEADER_TEXT_LIGHT: Hsla = hsla(60.0 / 360.0, 0.02, 0.10, 1.0); // #1a1a19
pub const COLOR_HEADER_BG_DARK: Hsla = hsla(60.0 / 360.0, 0.02, 0.12, 1.0); // #1f1f1e
pub const COLOR_HEADER_TEXT_DARK: Hsla = hsla(60.0 / 360.0, 0.03, 0.96, 1.0); // #f5f5f4

// Canonical Sona Palette Tokens (Light mode)
pub const COLOR_BG_PRIMARY_LIGHT: Hsla = hsla(40.0 / 360.0, 0.20, 0.98, 1.0); // #faf9f7
pub const COLOR_BG_SECONDARY_LIGHT: Hsla = hsla(60.0 / 360.0, 0.03, 0.95, 1.0); // #f3f3f2
pub const COLOR_BG_TERTIARY_LIGHT: Hsla = hsla(0.0, 0.0, 0.90, 1.0); // #e5e5e5
pub const COLOR_BG_ELEVATED_LIGHT: Hsla = hsla(0.0, 0.0, 1.0, 1.0); // #ffffff
pub const COLOR_BORDER_LIGHT: Hsla = hsla(40.0 / 360.0, 0.08, 0.20, 0.09); // rgba(55, 53, 47, 0.09)
pub const COLOR_BORDER_HOVER_LIGHT: Hsla = hsla(40.0 / 360.0, 0.08, 0.20, 0.16); // rgba(55, 53, 47, 0.16)
pub const COLOR_TEXT_PRIMARY_LIGHT: Hsla = hsla(40.0 / 360.0, 0.08, 0.20, 1.0); // #37352f
pub const COLOR_TEXT_SECONDARY_LIGHT: Hsla = hsla(40.0 / 360.0, 0.02, 0.46, 1.0); // #787774
pub const COLOR_TEXT_MUTED_LIGHT: Hsla = hsla(40.0 / 360.0, 0.02, 0.60, 1.0); // #9b9a97
pub const COLOR_RECORDING_RED: Hsla = hsla(0.0, 0.73, 0.56, 1.0); // #e03e3e
pub const COLOR_TRANSCRIPT_HIGHLIGHT: Hsla = hsla(46.0 / 360.0, 0.99, 0.44, 1.0); // #dfab01

// Colors for project tags and badges
pub const COLOR_PROJECT_BLUE: Hsla = hsla(217.0 / 360.0, 0.91, 0.60, 1.0);
pub const COLOR_PROJECT_EMERALD: Hsla = hsla(160.0 / 360.0, 0.84, 0.39, 1.0);
pub const COLOR_PROJECT_AMBER: Hsla = hsla(38.0 / 360.0, 0.92, 0.50, 1.0);
pub const COLOR_PROJECT_ROSE: Hsla = hsla(347.0 / 360.0, 0.77, 0.50, 1.0);
pub const COLOR_PROJECT_PURPLE: Hsla = hsla(271.0 / 360.0, 0.81, 0.56, 1.0);

pub fn project_color_from_index(index: usize) -> Hsla {
    match index % 5 {
        0 => COLOR_PROJECT_BLUE,
        1 => COLOR_PROJECT_EMERALD,
        2 => COLOR_PROJECT_AMBER,
        3 => COLOR_PROJECT_ROSE,
        _ => COLOR_PROJECT_PURPLE,
    }
}

/// Parses a 6-digit hex color string (e.g., "#3b82f6" or "3b82f6") into `Hsla`.
pub fn hex_to_hsla(hex: &str) -> Option<Hsla> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < 1e-5 {
        return Some(hsla(0.0, 0.0, l, 1.0));
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < 1e-5 {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < 1e-5 {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };

    Some(hsla(h, s, l, 1.0))
}
