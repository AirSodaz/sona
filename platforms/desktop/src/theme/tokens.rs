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

pub const RADIUS_SM: Pixels = px(4.0);
pub const RADIUS_MD: Pixels = px(8.0);
pub const RADIUS_LG: Pixels = px(12.0);
pub const RADIUS_FULL: Pixels = px(9999.0);

// Header & Theme Colors
pub const COLOR_HEADER_BG_LIGHT: Hsla = hsla(60.0 / 360.0, 0.03, 0.95, 1.0); // #f3f3f2
pub const COLOR_HEADER_TEXT_LIGHT: Hsla = hsla(60.0 / 360.0, 0.02, 0.10, 1.0); // #1a1a19
pub const COLOR_HEADER_BG_DARK: Hsla = hsla(60.0 / 360.0, 0.02, 0.12, 1.0); // #1f1f1e
pub const COLOR_HEADER_TEXT_DARK: Hsla = hsla(60.0 / 360.0, 0.03, 0.96, 1.0); // #f5f5f4

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
