//! Speaker review and voice sample enrollment sheet drawer.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct SpeakerReviewSheet {
    pub is_open: bool,
    pub speakers: Vec<(String, String)>,
}

impl SpeakerReviewSheet {
    pub fn new() -> Self {
        Self {
            is_open: false,
            speakers: vec![
                ("spk-0".to_string(), "Alice (Engineering Lead)".to_string()),
                ("spk-1".to_string(), "Bob (Core Developer)".to_string()),
                ("spk-2".to_string(), "Charlie (Product Architect)".to_string()),
            ],
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .when(self.is_open, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(360.0))
                        .h_full()
                        .p_4()
                        .border_l_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(16.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child("Speaker Diarization"),
                                )
                                .child(
                                    Button::new("btn-close-speakers")
                                        .ghost()
                                        .small()
                                        .label("✕")
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .children(self.speakers.iter().map(|(id, name)| {
                                    div()
                                        .id(ElementId::from(format!("spk-card-{}", id)))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().input)
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(name.clone()),
                                        )
                                        .child(
                                            Button::new(format!("btn-rename-{}", id))
                                                .ghost()
                                                .xsmall()
                                                .label("Rename")
                                        )
                                })),
                        ),
                )
            })
    }
}
