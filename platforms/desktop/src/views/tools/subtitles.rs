//! Tool: Live Subtitles controller panel.

use gpui_kit::component::button::Button;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct SubtitlesToolPanel {
    pub is_enabled: bool,
    pub is_locked: bool,
    pub opacity: f32,
    pub font_size: f32,
    pub bilingual: bool,
}

impl SubtitlesToolPanel {
    pub fn new() -> Self {
        Self {
            is_enabled: true,
            is_locked: false,
            opacity: 0.85,
            font_size: 18.0,
            bilingual: true,
        }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Live Subtitles"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted)
                    .child("Configure real-time floating subtitle overlay for meetings, videos, and live audio."),
            )
            .child(
                // Settings rows
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(400.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Overlay Enabled")
                            .child(
                                Button::new("toggle-subtitles-enabled")
                                    .outline()
                                    .small()
                                    .label(if self.is_enabled { "Enabled" } else { "Disabled" }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Click-Through Lock")
                            .child(
                                Button::new("toggle-subtitles-lock")
                                    .outline()
                                    .small()
                                    .label(if self.is_locked { "Locked" } else { "Unlocked" }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Bilingual Translation")
                            .child(
                                Button::new("toggle-bilingual")
                                    .outline()
                                    .small()
                                    .label(if self.bilingual { "Active" } else { "Original Only" }),
                            ),
                    ),
            )
    }
}
