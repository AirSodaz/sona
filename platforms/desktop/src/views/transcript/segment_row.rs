//! Individual transcript segment row with speaker badge, timestamp seek, and text.

use super::super::shell::MainShell;
use gpui_kit::base::FocusableExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[derive(Clone, Debug)]
pub struct SegmentData {
    pub id: String,
    pub start_time: f64,
    pub end_time: f64,
    pub speaker: String,
    pub text: String,
    pub translation: Option<String>,
}

pub struct SegmentRow;

impl SegmentRow {
    fn format_timestamp(seconds: f64) -> String {
        let mins = (seconds / 60.0).floor() as u64;
        let secs = (seconds % 60.0).floor() as u64;
        let millis = ((seconds.fract()) * 10.0).floor() as u64;
        format!("{:02}:{:02}.{}", mins, secs, millis)
    }

    pub fn render(data: &SegmentData, is_active: bool, cx: &App) -> impl IntoElement {
        let timestamp_str = Self::format_timestamp(data.start_time);

        div()
            .id(ElementId::from(format!("seg-row-{}", data.id)))
            .flex()
            .flex_col()
            .w_full()
            .p_3()
            .rounded_md()
            .bg(if is_active {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .border_b_1()
            .border_color(cx.theme().border)
            .gap_1p5()
            .child(
                // Metadata row: timestamp seek button + speaker badge
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new(format!("btn-seek-{}", data.id))
                            .ghost()
                            .xsmall()
                            .label(timestamp_str),
                    )
                    .when(!data.speaker.trim().is_empty(), |row| {
                        row.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(cx.theme().secondary)
                                .text_size(px(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child(data.speaker.clone()),
                        )
                    }),
            )
            .child(
                // Segment transcript text
                div()
                    .text_size(px(14.0))
                    .line_height(relative(1.5))
                    .text_color(cx.theme().foreground)
                    .child(data.text.clone()),
            )
            .when_some(data.translation.clone(), |this, trans| {
                this.child(
                    div()
                        .text_size(px(13.0))
                        .line_height(relative(1.4))
                        .text_color(cx.theme().muted)
                        .child(trans),
                )
            })
    }

    pub fn render_interactive(
        data: SegmentData,
        is_active: bool,
        editing: bool,
        edit_state: Option<Entity<InputState>>,
        translation_editing: bool,
        translation_state: Option<Entity<InputState>>,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement + use<> {
        let id = data.id.clone();
        let timestamp = Self::format_timestamp(data.start_time);
        div()
            .id(ElementId::from(format!("seg-row-{}", data.id)))
            .flex()
            .flex_col()
            .w_full()
            .p_3()
            .rounded_md()
            .bg(if is_active {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .border_b_1()
            .border_color(cx.theme().border)
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(format!("btn-seek-{}", data.id))
                            .ghost()
                            .xsmall()
                            .label(timestamp)
                            .on_click(cx.listener({
                                let time = data.start_time;
                                move |this, _, _, cx| {
                                    this.workbench.player.current_time = time;
                                    cx.notify();
                                }
                            })),
                    )
                    .when(!data.speaker.trim().is_empty(), |row| {
                        row.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(cx.theme().secondary)
                                .text_size(px(11.0))
                                .text_color(cx.theme().foreground)
                                .child(data.speaker.clone()),
                        )
                    })
                    .child(div().flex_1())
                    .when(!editing, |row| {
                        row.child(
                            Button::new(format!("btn-edit-{}", id))
                                .ghost()
                                .xsmall()
                                .label("Edit")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, window, cx| {
                                        this.workbench.begin_edit(&id, window, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new(format!("btn-delete-{}", id))
                                .ghost()
                                .xsmall()
                                .label("Delete")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, _, cx| {
                                        this.workbench.delete_segment(&id, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new(format!("btn-merge-{}", id))
                                .ghost()
                                .xsmall()
                                .label("Merge")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, _, cx| {
                                        this.workbench.merge_with_next(&id, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new(format!("btn-split-{}", id))
                                .ghost()
                                .xsmall()
                                .label("Split")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, _, cx| {
                                        this.workbench.split_segment(&id, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                    })
                    .when(editing, |row| {
                        row.child(
                            Button::new(format!("btn-save-{}", id))
                                .primary()
                                .xsmall()
                                .label("Save")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, _, cx| {
                                        this.workbench.save_edit(&id, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            Button::new(format!("btn-cancel-{}", id))
                                .ghost()
                                .xsmall()
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.workbench.editor.cancel_edit();
                                    cx.notify();
                                })),
                        )
                    })
                    .when(!editing && data.translation.is_some(), |row| {
                        row.child(
                            Button::new(format!("btn-edit-translation-{}", id))
                                .ghost()
                                .xsmall()
                                .label("Edit translation")
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, window, cx| {
                                        this.workbench
                                            .editor
                                            .begin_translation_edit(&id, window, cx);
                                        cx.notify();
                                    }
                                })),
                        )
                    }),
            )
            .child(if editing {
                edit_state
                    .as_ref()
                    .map(|state| Input::new(state).focus_ring(true).into_any_element())
                    .unwrap_or_else(|| div().child(data.text.clone()).into_any_element())
            } else {
                div()
                    .text_size(px(14.0))
                    .line_height(relative(1.5))
                    .text_color(cx.theme().foreground)
                    .child(data.text.clone())
                    .into_any_element()
            })
            .when_some(data.translation.clone(), |this, translation| {
                if translation_editing {
                    if let Some(state) = translation_state.clone() {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(Input::new(&state).focus_ring(true))
                                .child(
                                    Button::new(format!("btn-save-translation-{}", id))
                                        .primary()
                                        .xsmall()
                                        .label("Save")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                this.workbench.editor.save_translation(&id, cx);
                                                this.workbench.persist_editor_change(cx);
                                                cx.notify();
                                            }
                                        })),
                                ),
                        )
                    } else {
                        this.child(div().child(translation))
                    }
                } else {
                    this.child(
                        div()
                            .text_size(px(13.0))
                            .line_height(relative(1.4))
                            .text_color(cx.theme().muted)
                            .child(translation),
                    )
                }
            })
    }
}
