//! Tool: Media Server (Emby & Jellyfin) Sync controller panel.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::Button;
use gpui_kit::*;

pub struct MediaServerToolPanel {
    pub is_connected: bool,
    pub server_url: String,
    pub auto_inject_subtitles: bool,
    pub synced_libraries: usize,
}

impl MediaServerToolPanel {
    pub fn new() -> Self {
        Self {
            is_connected: true,
            server_url: "http://192.168.1.100:8096".to_string(),
            auto_inject_subtitles: true,
            synced_libraries: 3,
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
                    .child("Media Server Sync (Emby / Jellyfin)"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted)
                    .child("Automatically extract audio, transcribe media files, and inject bilingual SRT subtitles."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(460.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Server Connection")
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(self.server_url.clone()),
                                    )
                                    .child(
                                        Button::new("btn-sync-media-now")
                                            .outline()
                                            .small()
                                            .label("Sync Now"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Auto-Inject Subtitles")
                            .child(
                                Button::new("toggle-auto-inject")
                                    .outline()
                                    .small()
                                    .label(if self.auto_inject_subtitles { "Active" } else { "Disabled" }),
                            ),
                    ),
            )
    }
}
