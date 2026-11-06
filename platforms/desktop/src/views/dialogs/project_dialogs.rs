//! Project creation, editing, and deletion modals.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct ProjectDialogs {
    pub is_create_open: bool,
    pub is_delete_open: bool,
    pub project_name: String,
}

impl ProjectDialogs {
    pub fn new() -> Self {
        Self {
            is_create_open: false,
            is_delete_open: false,
            project_name: String::new(),
        }
    }

    pub fn open_create(&mut self) {
        self.is_create_open = true;
        self.project_name.clear();
    }

    pub fn close(&mut self) {
        self.is_create_open = false;
        self.is_delete_open = false;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .when(self.is_create_open, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(400.0))
                                .p_6()
                                .rounded_xl()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .gap_4()
                                .child(
                                    div()
                                        .text_size(px(16.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child("Create New Workspace"),
                                )
                                .child(
                                    div()
                                        .p_2p5()
                                        .rounded_md()
                                        .bg(cx.theme().input)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .text_size(px(13.0))
                                        .child("Project name..."),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("btn-cancel-proj")
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                        )
                                        .child(
                                            Button::new("btn-submit-proj")
                                                .primary()
                                                .small()
                                                .label("Create"),
                                        ),
                                ),
                        ),
                )
            })
    }
}
