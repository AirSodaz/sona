//! Transcript editor formatting toolbar.

use super::super::shell::MainShell;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Redo, Undo};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct EditorToolbar {
    pub is_saving: bool,
}

impl EditorToolbar {
    pub fn new() -> Self {
        Self { is_saving: false }
    }

    pub fn render(
        &mut self,
        is_editing: bool,
        _window: &mut Window,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let is_editing = self.is_saving || is_editing;
        div()
            .when(!is_editing, |this| this.h(px(1.0)))
            .when(is_editing, |this| {
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
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("btn-undo")
                                    .ghost()
                                    .xsmall()
                                    .label("Undo")
                                    .on_click(cx.listener(|_, _, window, cx| {
                                        window.dispatch_action(Box::new(Undo), cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-redo")
                                    .ghost()
                                    .xsmall()
                                    .label("Redo")
                                    .on_click(cx.listener(|_, _, window, cx| {
                                        window.dispatch_action(Box::new(Redo), cx);
                                    })),
                            )
                            .child(div().w(px(1.0)).h(px(14.0)).bg(cx.theme().border))
                            .child(Button::new("btn-bold").ghost().xsmall().label("B"))
                            .child(Button::new("btn-italic").ghost().xsmall().label("I"))
                            .child(div().w(px(1.0)).h(px(14.0)).bg(cx.theme().border))
                            .child(
                                Button::new("btn-split")
                                    .outline()
                                    .xsmall()
                                    .label("Split Segment")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(id) = this.workbench.editor.editing_id.clone() {
                                            this.workbench.split_segment(&id, cx);
                                        }
                                    })),
                            ),
                    )
            })
            .when(is_editing, |this| {
                this.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted)
                        .child(if self.is_saving { "Saving..." } else { "Saved" }),
                )
            })
    }
}
