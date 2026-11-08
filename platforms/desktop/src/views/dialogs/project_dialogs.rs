//! Project creation, editing, and deletion modals.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::Disableable as _;
use gpui_kit::base::FocusableExt as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{RADIUS_LG, RADIUS_MD};
use sona_core::project::ProjectCreateInput;
pub struct ProjectDialogs {
    pub is_create_open: bool,
    pub is_delete_open: bool,
    pub project_name_state: Entity<InputState>,
    pub is_submitting: bool,
    pub delete_project_id: Option<String>,
    pub delete_project_name: String,
    pub delete_item_count: usize,
    pub delete_cascade_to_inbox: bool,
}

impl ProjectDialogs {
    pub fn new(window: &mut Window, cx: &mut Context<super::super::shell::MainShell>) -> Self {
        Self {
            is_create_open: false,
            is_delete_open: false,
            project_name_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Project name")),
            is_submitting: false,
            delete_project_id: None,
            delete_project_name: String::new(),
            delete_item_count: 0,
            delete_cascade_to_inbox: true,
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
    pub fn open_delete(
        &mut self,
        project_id: String,
        project_name: String,
        item_count: usize,
        cx: &mut Context<super::super::shell::MainShell>,
    ) {
        self.delete_project_id = Some(project_id);
        self.delete_project_name = project_name;
        self.delete_item_count = item_count;
        self.delete_cascade_to_inbox = true;
        self.is_delete_open = true;
        cx.notify();
    }

    pub fn close(&mut self) {
        self.is_create_open = false;
        self.is_delete_open = false;
    }

    pub fn render(&mut self, cx: &mut Context<super::super::shell::MainShell>) -> impl IntoElement {
        div()
            .when(self.is_create_open, |this| {
                this.child(
                    div()
                        .id("modal-backdrop-create-project")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.project_dialogs.close();
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(420.0))
                                .p_6()
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_2xl()
                                .gap_4()
                                .id("create-project-card")
                                .on_click(|_, _, cx| cx.stop_propagation())
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
            .when(self.is_delete_open, |this| {
                let project_name = self.delete_project_name.clone();
                let item_count = self.delete_item_count;
                let cascade_to_inbox = self.delete_cascade_to_inbox;
                let is_submitting = self.is_submitting;

                this.child(
                    div()
                        .id("modal-backdrop-delete-project")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.project_dialogs.close();
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(440.0))
                                .p_6()
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_2xl()
                                .gap_4()
                                .id("delete-project-card")
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Icon::new(LucideIcon::TriangleAlert)
                                                .size(px(20.0))
                                                .text_color(cx.theme().danger),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(16.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().danger)
                                                .child("Delete Workspace"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .line_height(relative(1.4))
                                        .text_color(cx.theme().foreground)
                                        .child(format!(
                                            "Are you sure you want to delete \"{}\"?",
                                            project_name
                                        )),
                                )
                                .when(item_count > 0, |card| {
                                    card.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .p_3()
                                            .rounded(RADIUS_MD)
                                            .bg(cx.theme().secondary)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().muted)
                                                    .child(format!(
                                                        "This workspace contains {} items. Choose how to handle them:",
                                                        item_count
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_start()
                                    .gap_2()
                                    .id("del-opt-inbox")
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.project_dialogs.delete_cascade_to_inbox = true;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        div()
                                                            .size(px(14.0))
                                                            .mt(px(2.0))
                                                            .rounded_full()
                                                            .border_2()
                                                            .border_color(if cascade_to_inbox {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().muted
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .when(cascade_to_inbox, |bullet| {
                                                                bullet.child(
                                                                    div()
                                                                        .size(px(6.0))
                                                                        .rounded_full()
                                                                        .bg(cx.theme().primary),
                                                                )
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_size(px(13.0))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .child("Move to Inbox (Recommended)"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .text_color(cx.theme().muted)
                                                                    .child("Keep all transcripts; reset their workspace to Inbox."),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_start()
                                    .gap_2()
                                    .id("del-opt-trash")
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.project_dialogs.delete_cascade_to_inbox = false;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        div()
                                                            .size(px(14.0))
                                                            .mt(px(2.0))
                                                            .rounded_full()
                                                            .border_2()
                                                            .border_color(if !cascade_to_inbox {
                                                                cx.theme().danger
                                                            } else {
                                                                cx.theme().muted
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .when(!cascade_to_inbox, |bullet| {
                                                                bullet.child(
                                                                    div()
                                                                        .size(px(6.0))
                                                                        .rounded_full()
                                                                        .bg(cx.theme().danger),
                                                                )
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_size(px(13.0))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(cx.theme().danger)
                                                                    .child("Delete items (Move to Trash)"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .text_color(cx.theme().muted)
                                                                    .child("Move all recordings and transcriptions in this workspace to Trash."),
                                                            ),
                                                    ),
                                            ),
                                    )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("btn-cancel-del-proj")
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                                .disabled(is_submitting)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.project_dialogs.close();
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-confirm-del-proj")
                                                .danger()
                                                .small()
                                                .label(if is_submitting {
                                                    "Deleting…"
                                                } else {
                                                    "Delete Workspace"
                                                })
                                                .disabled(is_submitting)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let Some(project_id) =
                                                        this.project_dialogs.delete_project_id.take()
                                                    else {
                                                        return;
                                                    };
                                                    let cascade_action =
                                                        if this.project_dialogs.delete_cascade_to_inbox {
                                                            "moveToInbox".to_string()
                                                        } else {
                                                            "deleteItems".to_string()
                                                        };
                                                    let projects = cx
                                                        .global::<GlobalDesktopServices>()
                                                        .0
                                                        .projects
                                                        .clone();
                                                    this.project_dialogs.is_submitting = true;
                                                    cx.spawn(async move |shell, cx| {
                                                        let result = projects
                                                            .delete_with_cascade(
                                                                project_id.clone(),
                                                                cascade_action,
                                                            )
                                                            .await;
                                                        shell
                                                            .update(cx, |shell, cx| {
                                                                shell
                                                                    .project_dialogs
                                                                    .is_submitting = false;
                                                                if result.is_ok() {
                                                                    shell.project_dialogs.close();
                                                                    if shell
                                                                        .project_selector
                                                                        .active_project_id
                                                                        .as_deref()
                                                                        == Some(&project_id)
                                                                    {
                                                                        shell
                                                                            .project_selector
                                                                            .active_project_id =
                                                                            None;
                                                                    }
                                                                    shell
                                                                        .project_selector
                                                                        .load_data(cx);
                                                                    shell
                                                                        .projects_results
                                                                        .load_data(cx);
                                                                }
                                                                cx.notify();
                                                            })
                                                            .ok();
                                                    })
                                                    .detach();
                                                })),
                                        ),
                                ),
                        ),
                )
            })
    }
}
