//! Transcript editor formatting toolbar.
//! Strict 1:1 parity with React `EditorToolbar.tsx`.

use super::super::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Redo, Undo};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub enum AutoSaveStatus {
    Saving,
    Saved,
    Error(String),
}

pub struct EditorToolbar {
    pub is_saving: bool,
    pub save_status: Option<AutoSaveStatus>,
}

impl EditorToolbar {
    pub fn new() -> Self {
        Self {
            is_saving: false,
            save_status: None,
        }
    }

    pub fn set_saved(&mut self) {
        self.is_saving = false;
        self.save_status = Some(AutoSaveStatus::Saved);
    }

    pub fn set_saving(&mut self) {
        self.is_saving = true;
        self.save_status = Some(AutoSaveStatus::Saving);
    }

    pub fn set_error(&mut self, err: impl Into<String>) {
        self.is_saving = false;
        self.save_status = Some(AutoSaveStatus::Error(err.into()));
    }

    pub fn render(
        &mut self,
        is_editing: bool,
        _window: &mut Window,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let is_visible = is_editing || self.is_saving || self.save_status.is_some();
        div()
            .when(!is_visible, |this| this.h(px(1.0)))
            .when(is_visible, |this| {
                this.flex()
                    .items_center()
                    .justify_between()
                    .h(px(40.0))
                    .w_full()
                    .px_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .child(
                        // Left: Formatting tools (Undo, Redo, Divider, Bold, Italic, Underline, Strikethrough, Code, Divider, Split)
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("btn-undo")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::Undo2)
                                    .tooltip("Undo")
                                    .on_click(cx.listener(|_, _, window, cx| {
                                        window.dispatch_action(Box::new(Undo), cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-redo")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::Redo2)
                                    .tooltip("Redo")
                                    .on_click(cx.listener(|_, _, window, cx| {
                                        window.dispatch_action(Box::new(Redo), cx);
                                    })),
                            )
                            .child(div().w(px(1.0)).h(px(14.0)).bg(cx.theme().border).mx_1())
                            .child(
                                Button::new("btn-bold")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::Bold)
                                    .tooltip("Bold"),
                            )
                            .child(
                                Button::new("btn-italic")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::Italic)
                                    .tooltip("Italic"),
                            )
                            .child(
                                Button::new("btn-underline")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::Underline)
                                    .tooltip("Underline"),
                            )
                            .child(
                                Button::new("btn-strikethrough")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::Strikethrough)
                                    .tooltip("Strikethrough"),
                            )
                            .child(
                                Button::new("btn-code")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::Code)
                                    .tooltip("Code"),
                            )
                            .child(div().w(px(1.0)).h(px(14.0)).bg(cx.theme().border).mx_1())
                            .child(
                                Button::new("btn-split")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::CornerDownLeft)
                                    .label("Split")
                                    .tooltip("Split segment at cursor")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(id) = this.workbench.editor.editing_id.clone() {
                                            this.workbench.split_segment(&id, cx);
                                        }
                                    })),
                            ),
                    )
                    // Right: Autosave status indicator
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_size(px(12.0))
                            .child(match &self.save_status {
                                Some(AutoSaveStatus::Saving) => div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_color(cx.theme().muted)
                                    .child(Icon::new(LucideIcon::Loader).size(px(12.0)))
                                    .child("Saving..."),
                                Some(AutoSaveStatus::Saved) => div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_color(cx.theme().muted)
                                    .child(Icon::new(LucideIcon::Check).size(px(12.0)))
                                    .child("Saved"),
                                Some(AutoSaveStatus::Error(msg)) => div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_color(hsla(0.0, 0.84, 0.60, 1.0))
                                    .child(Icon::new(LucideIcon::CircleAlert).size(px(12.0)))
                                    .child(format!("Save failed: {msg}")),
                                None => {
                                    if self.is_saving {
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .text_color(cx.theme().muted)
                                            .child(Icon::new(LucideIcon::Loader).size(px(12.0)))
                                            .child("Saving...")
                                    } else {
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .text_color(cx.theme().muted)
                                            .child(Icon::new(LucideIcon::Check).size(px(12.0)))
                                            .child("Saved")
                                    }
                                }
                            }),
                    )
            })
    }
}
