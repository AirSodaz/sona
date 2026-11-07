//! Speaker review and voice sample enrollment sheet drawer.

use super::super::shell::MainShell;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct SpeakerReviewSheet {
    pub is_open: bool,
    pub speakers: Vec<(String, String)>,
}

impl SpeakerReviewSheet {
    pub fn new() -> Self {
        Self {
            is_open: false,
            speakers: Vec::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        div().when(self.is_open, |this| {
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
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.workbench.speaker_sheet.is_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(if self.speakers.is_empty() {
                        div()
                            .text_size(px(13.0))
                            .text_color(cx.theme().muted)
                            .child("No speakers to review")
                            .into_any_element()
                    } else {
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
                                            .label("Rename"),
                                    )
                            }))
                            .into_any_element()
                    }),
            )
        })
    }
}
