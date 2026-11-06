//! Persistent bottom options capsule and parameter popover for transcription.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

#[derive(Clone, Debug, PartialEq)]
pub struct TranscriptionOptionsState {
    pub model: String,
    pub language: String,
    pub diarization: bool,
    pub bilingual: bool,
}

impl Default for TranscriptionOptionsState {
    fn default() -> Self {
        Self {
            model: "SenseVoice Small".to_string(),
            language: "Auto Detect".to_string(),
            diarization: true,
            bilingual: false,
        }
    }
}

pub struct OptionsCapsule {
    pub state: TranscriptionOptionsState,
    pub is_popover_open: bool,
}

impl OptionsCapsule {
    pub fn new() -> Self {
        Self {
            state: TranscriptionOptionsState::default(),
            is_popover_open: false,
        }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1p5()
            .rounded(px(20.0))
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_sm()
            .child(
                Button::new("opt-model")
                    .ghost()
                    .small()
                    .label(format!("Model: {}", self.state.model)),
            )
            .child(
                div()
                    .w(px(1.0))
                    .h(px(14.0))
                    .bg(cx.theme().border),
            )
            .child(
                Button::new("opt-lang")
                    .ghost()
                    .small()
                    .label(format!("Lang: {}", self.state.language)),
            )
            .child(
                div()
                    .w(px(1.0))
                    .h(px(14.0))
                    .bg(cx.theme().border),
            )
            .child(
                Button::new("opt-diarize")
                    .ghost()
                    .small()
                    .label(if self.state.diarization {
                        "Diarization: On"
                    } else {
                        "Diarization: Off"
                    }),
            )
    }
}
