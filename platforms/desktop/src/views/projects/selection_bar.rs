//! Floating actions for the active Projects selection.

use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::history::mutation_repository::{
    HistoryPurgeItemsRequest, HistoryRestoreItemsRequest, HistoryTrashItemsRequest,
    HistoryUpdateProjectAssignmentsRequest,
};
use sona_core::project::ProjectRecord;

use super::super::shell::MainShell;
use crate::app::GlobalDesktopServices;

pub struct SelectionBar {
    assignment_open: bool,
    assignment_project_id: Option<String>,
    delete_confirmation_open: bool,
    permanently_delete: bool,
    pending_delete_ids: Option<Vec<String>>,
    is_busy: bool,
    error: Option<String>,
}

impl SelectionBar {
    pub fn new() -> Self {
        Self {
            assignment_open: false,
            assignment_project_id: None,
            delete_confirmation_open: false,
            permanently_delete: false,
            pending_delete_ids: None,
            is_busy: false,
            error: None,
        }
    }

    pub(crate) fn open_assignment(&mut self, cx: &mut Context<MainShell>) {
        self.assignment_project_id = None;
        self.assignment_open = true;
        self.error = None;
        cx.notify();
    }

    fn confirm_assignment(&mut self, cx: &mut Context<MainShell>) {
        let ids = cx.entity().read(cx).projects_results.selected_ids.clone();
        if ids.is_empty() || self.is_busy {
            return;
        }
        let project_id = self.assignment_project_id.clone();
        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
        self.is_busy = true;
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = history
                .mutation_db(move |service| {
                    service.update_project_assignments(HistoryUpdateProjectAssignmentsRequest {
                        ids,
                        project_id,
                    })
                })
                .await;
            this.update(cx, |shell, cx| {
                let bar = &mut shell.selection_bar;
                bar.is_busy = false;
                match result {
                    Ok(()) => {
                        bar.assignment_open = false;
                        shell.projects_results.clear_selection();
                        shell.projects_toolbar.is_selection_active = false;
                        shell.projects_results.load_data(cx);
                        shell.project_selector.load_data(cx);
                    }
                    Err(error) => {
                        bar.error = Some(format!("Could not update project assignments: {error}"))
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn request_delete(&mut self, cx: &mut Context<MainShell>) {
        let has_selection = !cx
            .entity()
            .read(cx)
            .projects_results
            .selected_ids
            .is_empty();
        if !has_selection || self.is_busy {
            return;
        }
        self.pending_delete_ids = None;
        self.delete_confirmation_open = true;
        self.permanently_delete = false;
        self.error = None;
        cx.notify();
    }

    pub(crate) fn request_delete_item(&mut self, id: String, cx: &mut Context<MainShell>) {
        if self.is_busy {
            return;
        }
        self.pending_delete_ids = Some(vec![id]);
        self.delete_confirmation_open = true;
        self.permanently_delete = false;
        self.error = None;
        cx.notify();
    }

    fn confirm_delete(&mut self, is_trash_scope: bool, cx: &mut Context<MainShell>) {
        let ids = self
            .pending_delete_ids
            .clone()
            .unwrap_or_else(|| cx.entity().read(cx).projects_results.selected_ids.clone());
        if ids.is_empty() || self.is_busy {
            return;
        }
        let permanently_delete = is_trash_scope || self.permanently_delete;
        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
        self.is_busy = true;
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = if permanently_delete {
                history
                    .mutation_file(move |service| {
                        service.purge_items(HistoryPurgeItemsRequest { ids })
                    })
                    .await
            } else {
                let deleted_at = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .map_err(|error| error.to_string());
                match deleted_at {
                    Ok(deleted_at) => {
                        history
                            .mutation_db(move |service| {
                                service.trash_items(HistoryTrashItemsRequest { ids, deleted_at })
                            })
                            .await
                    }
                    Err(error) => Err(error),
                }
            };
            this.update(cx, |shell, cx| {
                let bar = &mut shell.selection_bar;
                bar.is_busy = false;
                match result {
                    Ok(()) => {
                        bar.delete_confirmation_open = false;
                        bar.pending_delete_ids = None;
                        shell.projects_results.clear_selection();
                        shell.projects_toolbar.is_selection_active = false;
                        shell.projects_results.load_data(cx);
                        shell.project_selector.load_data(cx);
                    }
                    Err(error) => {
                        bar.error = Some(format!("Could not delete selected items: {error}"))
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn render(
        &mut self,
        is_trash_scope: bool,
        selected_count: usize,
        visible_count: usize,
        projects: &[ProjectRecord],
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let all_visible_selected = visible_count > 0 && selected_count == visible_count;
        let projects = projects.to_vec();
        let selected_project_id = self.assignment_project_id.clone();
        let assignment_open = self.assignment_open;
        let confirmation_open = self.delete_confirmation_open;
        let permanently_delete = self.permanently_delete;
        let is_busy = self.is_busy;
        let error = self.error.clone();
        let delete_count = self
            .pending_delete_ids
            .as_ref()
            .map_or(selected_count, Vec::len);

        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_end()
            .pb_6()
            .when(!assignment_open && !confirmation_open, |overlay| {
                overlay.when_some(error.clone(), |overlay, error| {
                    overlay.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(px(70.0))
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().danger)
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().danger)
                                    .child(error),
                            ),
                    )
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pl_3()
                    .pr(px(6.0))
                    .py_1()
                    .rounded_md()
                    .bg(cx.theme().popover)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_xl()
                    .child(
                        div()
                            .pr_1()
                            .text_size(px(13.0))
                            .line_height(px(15.6))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{selected_count} selected")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .child(
                                selection_button(
                                    "selection-select-all",
                                    LucideIcon::ListChecks,
                                    "Select All",
                                    all_visible_selected,
                                    false,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.projects_results.toggle_select_all_visible();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(div().mx_1().w(px(1.0)).h_4().bg(cx.theme().border))
                            .child(
                                selection_button(
                                    "selection-restore-or-assign",
                                    if is_trash_scope {
                                        LucideIcon::RotateCcw
                                    } else {
                                        LucideIcon::FolderKanban
                                    },
                                    if is_trash_scope {
                                        "Restore"
                                    } else {
                                        "Assign Project"
                                    },
                                    false,
                                    selected_count == 0 || is_busy,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if is_trash_scope {
                                            this.selection_bar.confirm_restore(cx);
                                        } else {
                                            this.selection_bar.open_assignment(cx);
                                        }
                                    },
                                )),
                            )
                            .child(
                                selection_button(
                                    "selection-delete",
                                    LucideIcon::Trash,
                                    "Delete",
                                    false,
                                    selected_count == 0 || is_busy,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.selection_bar.request_delete(cx),
                                )),
                            )
                            .child(
                                selection_button(
                                    "selection-cancel",
                                    LucideIcon::X,
                                    "Cancel",
                                    false,
                                    false,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.projects_results.clear_selection();
                                        this.projects_toolbar.is_selection_active = false;
                                        this.selection_bar.error = None;
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .when(assignment_open || confirmation_open, |overlay| {
                overlay.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui_kit::black().opacity(0.45))
                        .child(if assignment_open {
                            render_assignment_dialog(
                                selected_count,
                                &projects,
                                selected_project_id.clone(),
                                error.clone(),
                                is_busy,
                                cx,
                            )
                        } else {
                            render_delete_dialog(
                                delete_count,
                                is_trash_scope,
                                permanently_delete,
                                error.clone(),
                                is_busy,
                                cx,
                            )
                        }),
                )
            })
    }

    fn confirm_restore(&mut self, cx: &mut Context<MainShell>) {
        let ids = cx.entity().read(cx).projects_results.selected_ids.clone();
        if ids.is_empty() || self.is_busy {
            return;
        }
        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
        self.is_busy = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = history
                .mutation_db(move |service| {
                    service.restore_items(HistoryRestoreItemsRequest { ids })
                })
                .await;
            this.update(cx, |shell, cx| {
                let bar = &mut shell.selection_bar;
                bar.is_busy = false;
                match result {
                    Ok(()) => {
                        shell.projects_results.clear_selection();
                        shell.projects_toolbar.is_selection_active = false;
                        shell.projects_results.load_data(cx);
                        shell.project_selector.load_data(cx);
                    }
                    Err(error) => {
                        bar.error = Some(format!("Could not restore selected items: {error}"))
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn selection_button(
    id: &'static str,
    icon: LucideIcon,
    label: &'static str,
    selected: bool,
    disabled: bool,
    cx: &Context<MainShell>,
) -> Button {
    let button = Button::new(id)
        .ghost()
        .small()
        .w(px(30.0))
        .h(px(30.0))
        .icon(icon)
        .disabled(disabled)
        .when(selected, |button| {
            button
                .bg(cx.theme().primary.opacity(0.12))
                .text_color(cx.theme().primary)
        })
        .tooltip(label)
        .accessibility_label(label);
    if disabled {
        button
    } else if selected {
        button.hover(|style| style.bg(cx.theme().primary.opacity(0.18)))
    } else {
        button.hover(|style| style.bg(cx.theme().secondary))
    }
}

fn render_assignment_dialog(
    selected_count: usize,
    projects: &[ProjectRecord],
    selected_project_id: Option<String>,
    error: Option<String>,
    is_busy: bool,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let project_options = std::iter::once((None, "Inbox".to_string()))
        .chain(
            projects
                .iter()
                .map(|project| (Some(project.id.clone()), project.name.clone())),
        )
        .collect::<Vec<_>>();

    div()
        .w(px(420.0))
        .max_w(px(520.0))
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .rounded_lg()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .shadow_xl()
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Assign Project"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .text_color(cx.theme().muted_foreground)
                .child(format!("{selected_count} selected")),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(project_options.into_iter().map(|(project_id, name)| {
                    let selected = project_id == selected_project_id;
                    let click_id = project_id.clone();
                    Button::new(ElementId::Name(
                        format!(
                            "assignment-target-{}",
                            project_id.as_deref().unwrap_or("inbox")
                        )
                        .into(),
                    ))
                    .ghost()
                    .w_full()
                    .justify_start()
                    .when(selected, |button| button.bg(cx.theme().accent))
                    .label(name)
                    .disabled(is_busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selection_bar.assignment_project_id = click_id.clone();
                        cx.notify();
                    }))
                })),
        )
        .when_some(error, |this, error| {
            this.child(
                div()
                    .text_size(px(12.0))
                    .text_color(cx.theme().danger)
                    .child(error),
            )
        })
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("assignment-cancel")
                        .ghost()
                        .small()
                        .label("Cancel")
                        .disabled(is_busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.selection_bar.assignment_open = false;
                            this.selection_bar.error = None;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("assignment-save")
                        .primary()
                        .small()
                        .label(if is_busy { "Saving…" } else { "Save" })
                        .disabled(is_busy || selected_count == 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.selection_bar.confirm_assignment(cx);
                        })),
                ),
        )
        .into_any_element()
}

fn render_delete_dialog(
    selected_count: usize,
    is_trash_scope: bool,
    permanently_delete: bool,
    error: Option<String>,
    is_busy: bool,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let permanent = is_trash_scope || permanently_delete;
    div()
        .w(px(420.0))
        .max_w(px(520.0))
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .rounded_lg()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .shadow_xl()
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(if permanent {
                    "Delete Permanently"
                } else {
                    "Move to Trash"
                }),
        )
        .child(
            div()
                .text_size(px(14.0))
                .text_color(cx.theme().muted_foreground)
                .child(if permanent {
                    format!(
                        "Permanently delete {selected_count} {}? This cannot be undone.",
                        if selected_count == 1 { "item" } else { "items" }
                    )
                } else {
                    format!(
                        "Move {selected_count} {} to Trash?",
                        if selected_count == 1 { "item" } else { "items" }
                    )
                }),
        )
        .when(!is_trash_scope, |dialog| {
            dialog.child(
                gpui_kit::component::checkbox::Checkbox::new("selection-permanent-delete")
                    .checked(permanently_delete)
                    .label("Directly delete permanently (bypass Trash)")
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.selection_bar.permanently_delete = *checked;
                        cx.notify();
                    })),
            )
        })
        .when(permanently_delete && !is_trash_scope, |dialog| {
            dialog.child(
                div()
                    .text_size(px(12.0))
                    .text_color(cx.theme().danger)
                    .child("This permanently deletes the records and cannot be undone."),
            )
        })
        .when_some(error, |dialog, error| {
            dialog.child(
                div()
                    .text_size(px(12.0))
                    .text_color(cx.theme().danger)
                    .child(error),
            )
        })
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("selection-delete-cancel")
                        .ghost()
                        .small()
                        .label("Cancel")
                        .disabled(is_busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.selection_bar.delete_confirmation_open = false;
                            this.selection_bar.pending_delete_ids = None;
                            this.selection_bar.error = None;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("selection-delete-confirm")
                        .danger()
                        .small()
                        .label(if is_busy {
                            "Deleting…"
                        } else if permanent {
                            "Delete Permanently"
                        } else {
                            "Move to Trash"
                        })
                        .disabled(is_busy || selected_count == 0)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selection_bar.confirm_delete(is_trash_scope, cx);
                        })),
                ),
        )
        .into_any_element()
}
