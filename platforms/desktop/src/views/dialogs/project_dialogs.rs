//! Project creation, editing, and deletion modals.

use gpui_kit::base::Disableable as _;
use gpui_kit::base::FocusableExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::GlobalDesktopServices;
use sona_core::project::ProjectCreateInput;

pub struct ProjectDialogs {
    pub is_create_open: bool,
    pub is_delete_open: bool,
    pub project_name_state: Entity<InputState>,
    pub is_submitting: bool,
}

impl ProjectDialogs {
    pub fn new(window: &mut Window, cx: &mut Context<super::super::shell::MainShell>) -> Self {
        Self {
            is_create_open: false,
            is_delete_open: false,
            project_name_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Project name")),
            is_submitting: false,
        }
    }

    pub fn open_create(
        &mut self,
        window: &mut Window,
        cx: &mut Context<super::super::shell::MainShell>,
    ) {
        self.is_create_open = true;
        self.project_name_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
    }

    pub fn close(&mut self) {
        self.is_create_open = false;
        self.is_delete_open = false;
    }

    pub fn render(&mut self, cx: &mut Context<super::super::shell::MainShell>) -> impl IntoElement {
        div().when(self.is_create_open, |this| {
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
                            .child(Input::new(&self.project_name_state).focus_ring(true))
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
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.project_dialogs.close();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("btn-submit-proj")
                                            .primary()
                                            .small()
                                            .label(if self.is_submitting {
                                                "Creating…"
                                            } else {
                                                "Create"
                                            })
                                            .disabled(self.is_submitting)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let name = this
                                                    .project_dialogs
                                                    .project_name_state
                                                    .read(cx)
                                                    .value()
                                                    .trim()
                                                    .to_string();
                                                if !name.is_empty() {
                                                    let projects = cx
                                                        .global::<GlobalDesktopServices>()
                                                        .0
                                                        .projects
                                                        .clone();
                                                    this.project_dialogs.is_submitting = true;
                                                    cx.spawn(async move |shell, cx| {
                                                        let result = projects
                                                            .create(ProjectCreateInput {
                                                                name,
                                                                ..Default::default()
                                                            })
                                                            .await;
                                                        shell
                                                            .update(cx, |shell, cx| {
                                                                shell
                                                                    .project_dialogs
                                                                    .is_submitting = false;
                                                                if result.is_ok() {
                                                                    shell.project_dialogs.close();
                                                                    shell
                                                                        .project_selector
                                                                        .load_data(cx);
                                                                }
                                                                cx.notify();
                                                            })
                                                            .ok();
                                                    })
                                                    .detach();
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
        })
    }
}
