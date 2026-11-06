//! Floating Caption Overlay Window for live bilingual subtitles.
//! Borderless, transparent, and always-on-top.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct CaptionOverlayWindow {
    text_primary: String,
    text_secondary: String,
    locked: bool,
    opacity: f32,
    font_size: f32,
}

impl CaptionOverlayWindow {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            text_primary: "Waiting for speech...".to_string(),
            text_secondary: String::new(),
            locked: false,
            opacity: 0.85,
            font_size: 18.0,
        }
    }

    pub fn set_text(&mut self, primary: impl Into<String>, secondary: Option<String>, cx: &mut Context<Self>) {
        self.text_primary = primary.into();
        self.text_secondary = secondary.unwrap_or_default();
        cx.notify();
    }

    pub fn toggle_lock(&mut self, cx: &mut Context<Self>) {
        self.locked = !self.locked;
        cx.notify();
    }
}

impl Render for CaptionOverlayWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg_color = hsla(0.0, 0.0, 0.0, self.opacity);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .rounded(px(12.0))
            .bg(bg_color)
            .border_1()
            .border_color(hsla(0.0, 0.0, 1.0, 0.15))
            .text_color(cx.theme().foreground)
            .child(
                // Subtitle primary text
                div()
                    .text_size(px(self.font_size))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(gpui_kit::white())
                    .child(self.text_primary.clone()),
            )
            .when(!self.text_secondary.is_empty(), |this| {
                this.child(
                    div()
                        .mt_1()
                        .text_size(px(self.font_size * 0.85))
                        .text_color(hsla(0.0, 0.0, 0.9, 0.85))
                        .child(self.text_secondary.clone()),
                )
            })
    }
}
