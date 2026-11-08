//! Individual transcript segment row with speaker badge, timestamp seek, and text.

use super::super::shell::MainShell;
use crate::theme::tokens::{
    COLOR_TEXT_PRIMARY_LIGHT, COLOR_TEXT_SECONDARY_LIGHT, RADIUS_FULL, RADIUS_SM,
};
use gpui_kit::assets::IconName as LucideIcon;
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
            .max_w(px(900.0))
            .mx_auto()
            .py_2()
            .px_3()
            .rounded(RADIUS_SM)
            .bg(if is_active {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .border_b_1()
            .border_color(cx.theme().border)
            .when(!data.speaker.trim().is_empty(), |this| {
                this.child(
                    div().pl(px(76.0)).mb(px(4.0)).child(
                        div()
                            .px(px(10.0))
                            .py(px(2.0))
                            .rounded(RADIUS_FULL)
                            .bg(cx.theme().secondary)
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                            .child(data.speaker.clone()),
                    ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(16.0))
                    .w_full()
                    // Gutter: Timestamp button
                    .child(
                        div()
                            .w(px(60.0))
                            .min_w(px(60.0))
                            .flex()
                            .justify_end()
                            .child(
                                Button::new(format!("btn-seek-{}", data.id))
                                    .ghost()
                                    .xsmall()
                                    .label(timestamp_str),
                            ),
                    )
                    // Content: text + translation
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .line_height(relative(1.8))
                                    .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                    .child(data.text.clone()),
                            )
                            .when_some(data.translation.clone(), |content, trans| {
                                content.child(
                                    div()
                                        .mt(px(4.0))
                                        .text_size(px(14.5))
                                        .line_height(relative(1.5))
                                        .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                        .child(trans),
                                )
                            }),
                    ),
            )
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
            .max_w(px(900.0))
            .mx_auto()
            .py_2()
            .px_3()
            .rounded(RADIUS_SM)
            .bg(if is_active {
                cx.theme().accent
            } else {
                gpui_kit::transparent_black()
            })
            .border_b_1()
            .border_color(cx.theme().border)
            .when(!data.speaker.trim().is_empty(), |this| {
                this.child(
                    div().pl(px(76.0)).mb(px(4.0)).child(
                        div()
                            .px(px(10.0))
                            .py(px(2.0))
                            .rounded(RADIUS_FULL)
                            .bg(cx.theme().secondary)
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                            .child(data.speaker.clone()),
                    ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(16.0))
                    .w_full()
                    // Gutter (60px): Timestamp button
                    .child(
                        div()
                            .w(px(60.0))
                            .min_w(px(60.0))
                            .flex()
                            .justify_end()
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
                            ),
                    )
                    // Content (1fr): text and translation
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(if editing {
                                edit_state
                                    .as_ref()
                                    .map(|state| {
                                        Input::new(state).focus_ring(true).into_any_element()
                                    })
                                    .unwrap_or_else(|| {
                                        div().child(data.text.clone()).into_any_element()
                                    })
                            } else {
                                div()
                                    .text_size(px(18.0))
                                    .line_height(relative(1.8))
                                    .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                    .child(data.text.clone())
                                    .into_any_element()
                            })
                            .when_some(data.translation.clone(), |content, translation| {
                                if translation_editing {
                                    if let Some(state) = translation_state.clone() {
                                        content.child(
                                            div()
                                                .mt(px(4.0))
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(Input::new(&state).focus_ring(true))
                                                .child(
                                                    Button::new(format!(
                                                        "btn-save-translation-{}",
                                                        id
                                                    ))
                                                    .primary()
                                                    .xsmall()
                                                    .label("Save")
                                                    .on_click(cx.listener({
                                                        let id = id.clone();
                                                        move |this, _, _, cx| {
                                                            this.workbench
                                                                .editor
                                                                .save_translation(&id, cx);
                                                            this.workbench
                                                                .persist_editor_change(cx);
                                                            cx.notify();
                                                        }
                                                    })),
                                                ),
                                        )
                                    } else {
                                        content.child(div().mt(px(4.0)).child(translation))
                                    }
                                } else {
                                    content.child(
                                        div()
                                            .mt(px(4.0))
                                            .text_size(px(14.5))
                                            .line_height(relative(1.5))
                                            .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                            .child(translation),
                                    )
                                }
                            }),
                    )
                    // Actions: buttons row with icons
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(!editing, |row| {
                                row.child(
                                    Button::new(format!("btn-edit-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::Pencil)
                                        .tooltip("Edit")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, window, cx| {
                                                this.workbench.begin_edit(&id, window, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    Button::new(format!("btn-merge-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::GitMerge)
                                        .tooltip("Merge with next")
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
                                        .icon(LucideIcon::Split)
                                        .tooltip("Split")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                this.workbench.split_segment(&id, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                                .when(data.translation.is_some(), |actions| {
                                    actions.child(
                                        Button::new(format!("btn-edit-translation-{}", id))
                                            .ghost()
                                            .xsmall()
                                            .icon(LucideIcon::Languages)
                                            .tooltip("Edit translation")
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
                                })
                                .child(
                                    Button::new(format!("btn-delete-{}", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(LucideIcon::Trash)
                                        .tooltip("Delete")
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _, cx| {
                                                this.workbench.delete_segment(&id, cx);
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
                                        .icon(LucideIcon::Check)
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
                                        .icon(LucideIcon::X)
                                        .label("Cancel")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.workbench.editor.cancel_edit();
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
    }
}
