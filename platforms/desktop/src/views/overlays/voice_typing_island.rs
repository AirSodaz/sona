//! Floating Voice Typing Dynamic Island widget.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct VoiceTypingIsland {
    interim_text: String,
    is_active: bool,
    audio_level: f32, // 0.0 to 1.0
}

impl VoiceTypingIsland {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            interim_text: "Listening...".to_string(),
            is_active: true,
            audio_level: 0.4,
        }
    }

    pub fn update_level(&mut self, level: f32, cx: &mut Context<Self>) {
        self.audio_level = level.clamp(0.0, 1.0);
        cx.notify();
    }

    pub fn set_interim(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.interim_text = text.into();
        cx.notify();
    }
}

impl Render for VoiceTypingIsland {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pill_bg = hsla(0.0, 0.0, 0.08, 0.92);

        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .rounded(px(24.0))
            .bg(pill_bg)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_lg()
            .child(
                // Mic pulse indicator
                div()
                    .w(px(10.0))
                    .h(px(10.0))
                    .rounded_full()
                    .bg(if self.is_active {
                        hsla(142.0 / 360.0, 0.71, 0.45, 1.0)
                    } else {
                        cx.theme().muted
                    }),
            )
            .child(
                // Dictation text
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(gpui_kit::white())
                    .child(self.interim_text.clone()),
            )
    }
}
