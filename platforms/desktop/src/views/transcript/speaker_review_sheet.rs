//! Speaker review and voice sample enrollment sheet drawer.

use super::super::shell::MainShell;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct SpeakerReviewSheet {
    pub is_open: bool,
    pub speakers: Vec<(String, String)>,
    pub editing_speaker_id: Option<String>,
    pub edit_name_state: Option<Entity<InputState>>,
}

impl SpeakerReviewSheet {
    pub fn new() -> Self {
        Self {
            is_open: false,
            speakers: Vec::new(),
            editing_speaker_id: None,
            edit_name_state: None,
        }
    }

    pub fn start_rename(
        &mut self,
        id: &str,
        name: &str,
        window: &mut Window,
        cx: &mut Context<MainShell>,
    ) {
        self.editing_speaker_id = Some(id.to_string());
        let current_name = name.to_string();
        let state = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Speaker name");
            s.set_value(current_name, window, cx);
            s.focus(window, cx);
            s
        });
        self.edit_name_state = Some(state);
        cx.notify();
    }

    pub fn cancel_rename(&mut self, cx: &mut Context<MainShell>) {
        self.editing_speaker_id = None;
        self.edit_name_state = None;
        cx.notify();
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
                            .children(self.speakers.iter().map(|(id, name)| {
                                let is_editing =
                                    self.editing_speaker_id.as_deref() == Some(id.as_str());
                                let speaker_id = id.clone();
                                let speaker_name = name.clone();

                                div()
                                    .id(ElementId::from(format!("spk-card-{}", id)))
                                    .flex()
                                    .flex_col()
                                    .p_3()
                                    .rounded_md()
                                    .bg(cx.theme().input)
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .child(if is_editing {
                                                if let Some(state) = &self.edit_name_state {
                                                    div()
                                                        .flex_1()
                                                        .child(Input::new(state).small())
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .text_size(px(13.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(name.clone())
                                                        .into_any_element()
                                                }
                                            } else {
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(name.clone())
                                                    .into_any_element()
                                            })
                                            .child(if is_editing {
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        Button::new(format!("btn-save-spk-{}", id))
                                                            .primary()
                                                            .xsmall()
                                                            .label("Save")
                                                            .on_click(cx.listener({
                                                                let spk_id = speaker_id.clone();
                                                                move |this, _, _, cx| {
                                                                    this.workbench
                                                                        .save_speaker_rename(
                                                                            &spk_id, cx,
                                                                        );
                                                                }
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new(format!(
                                                            "btn-cancel-spk-{}",
                                                            id
                                                        ))
                                                        .ghost()
                                                        .xsmall()
                                                        .label("Cancel")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.workbench
                                                                .speaker_sheet
                                                                .cancel_rename(cx);
                                                        })),
                                                    )
                                                    .into_any_element()
                                            } else {
                                                Button::new(format!("btn-rename-{}", id))
                                                    .ghost()
                                                    .xsmall()
                                                    .label("Rename")
                                                    .on_click(cx.listener({
                                                        let spk_id = speaker_id.clone();
                                                        let spk_name = speaker_name.clone();
                                                        move |this, _, window, cx| {
                                                            this.workbench
                                                                .speaker_sheet
                                                                .start_rename(
                                                                    &spk_id, &spk_name, window, cx,
                                                                );
                                                        }
                                                    }))
                                                    .into_any_element()
                                            }),
                                    )
                            }))
                            .into_any_element()
                    }),
            )
        })
    }
}

impl Default for SpeakerReviewSheet {
    fn default() -> Self {
        Self::new()
    }
}
