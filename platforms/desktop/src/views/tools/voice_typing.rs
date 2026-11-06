//! Tool: Voice Typing dictation controller panel.

use gpui_kit::component::button::Button;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct VoiceTypingToolPanel {
    pub push_to_talk: bool,
    pub auto_polish: bool,
    pub sound_effects: bool,
    pub hotkey: String,
}

impl VoiceTypingToolPanel {
    pub fn new() -> Self {
        Self {
            push_to_talk: false,
            auto_polish: true,
            sound_effects: true,
            hotkey: "Ctrl + Shift + Space".to_string(),
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
                    .child("Voice Typing"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(cx.theme().muted)
                    .child("Dictate anywhere across Windows apps with automatic punctuation and LLM polishing."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(420.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Global Hotkey")
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .text_size(px(12.0))
                                    .child(self.hotkey.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Auto-Polish with LLM")
                            .child(
                                Button::new("toggle-auto-polish")
                                    .outline()
                                    .small()
                                    .label(if self.auto_polish { "Enabled" } else { "Raw Transcript" }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child("Activation Sound Effects")
                            .child(
                                Button::new("toggle-sound-effects")
                                    .outline()
                                    .small()
                                    .label(if self.sound_effects { "On" } else { "Silent" }),
                            ),
                    ),
            )
    }
}
