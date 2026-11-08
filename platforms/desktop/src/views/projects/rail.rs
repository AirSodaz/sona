//! Projects left sidebar rail.

use crate::theme::tokens::*;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::*;
use sona_core::history::HistoryWorkspaceScope;
use sona_core::project::ProjectRecord;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectItem {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub color_index: usize,
}

pub struct ProjectsRail {
    pub selected_project: Option<String>,
    pub projects: Vec<ProjectItem>,
    pub all_count: usize,
}

impl ProjectsRail {
    pub fn new() -> Self {
        Self {
            selected_project: None,
            projects: Vec::new(),
            all_count: 0,
        }
    }

    pub fn sync_projects(
        &mut self,
        projects: &[ProjectRecord],
        counts: &HashMap<String, usize>,
        inbox_count: usize,
    ) {
        self.projects = projects
            .iter()
            .enumerate()
            .map(|(color_index, project)| ProjectItem {
                id: project.id.clone(),
                name: project.name.clone(),
                count: counts.get(&project.id).copied().unwrap_or_default(),
                color_index,
            })
            .collect();
        self.all_count = inbox_count + counts.values().sum::<usize>();
    }

    pub fn select_project(&mut self, id: Option<String>) {
        self.selected_project = id;
    }

    pub fn render(
        &mut self,
        selected_scope: &HistoryWorkspaceScope,
        inbox_count: usize,
        all_count: usize,
        trash_count: usize,
        cx: &mut Context<super::super::shell::MainShell>,
    ) -> impl IntoElement {
        let is_all_selected = matches!(selected_scope, HistoryWorkspaceScope::All);
        let is_inbox_selected = matches!(selected_scope, HistoryWorkspaceScope::Inbox);
        let is_trash_selected = matches!(selected_scope, HistoryWorkspaceScope::Trash);

        div()
            .flex()
            .flex_col()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .p_3()
            .gap_1()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                // Section header
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().muted)
                            .child("WORKSPACES"),
                    )
                    .child(
                        Button::new("btn-new-project")
                            .ghost()
                            .small()
                            .label("+")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.project_dialogs.open_create(window, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                // "All Items" button
                div()
                    .id(ElementId::from("rail-all-items"))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if is_all_selected {
                        cx.theme().accent
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().accent))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_project_scope(HistoryWorkspaceScope::All, window, cx);
                    }))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if is_all_selected {
                                cx.theme().primary
                            } else {
                                cx.theme().foreground
                            })
                            .child("All Items"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted)
                            .child(all_count.to_string()),
                    ),
            )
            .child(
                div()
                    .id("rail-inbox")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if is_inbox_selected { cx.theme().accent } else { gpui_kit::transparent_black() })
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().accent))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_project_scope(HistoryWorkspaceScope::Inbox, window, cx);
                    }))
                    .child(div().text_size(px(13.0)).child("Inbox"))
                    .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(inbox_count.to_string())),
            )
            .child(
                div()
                    .id("rail-trash")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if is_trash_selected { cx.theme().accent } else { gpui_kit::transparent_black() })
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().accent))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_project_scope(HistoryWorkspaceScope::Trash, window, cx);
                    }))
                    .context_menu({
                        let shell_entity = cx.entity().clone();
                        move |menu, _, _cx| {
                            let shell = shell_entity.clone();
                            menu.item(
                                PopupMenuItem::new("Empty Trash")
                                    .icon(Icon::new(LucideIcon::Trash))
                                    .disabled(trash_count == 0)
                                    .on_click(move |_, _, cx| {
                                        shell.update(cx, |shell, cx| {
                                            shell.projects_toolbar.is_selection_active = true;
                                            shell.selection_bar.request_delete(cx);
                                        });
                                    }),
                            )
                        }
                    })
                    .child(div().text_size(px(13.0)).child("Trash"))
                    .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(trash_count.to_string())),
            )
            .child(
                // Divider
                div().my_2().h(px(1.0)).bg(cx.theme().border),
            )
            .child(
                // Projects list
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.projects.iter().map(|p| {
                        let is_selected = matches!(selected_scope, HistoryWorkspaceScope::Project { project_id } if project_id == &p.id);
                        let color = project_color_from_index(p.color_index);
                        let project_id = p.id.clone();
                        let shell_entity = cx.entity().clone();
                        let p_id_active = p.id.clone();
                        let p_id_del = p.id.clone();
                        let p_name_del = p.name.clone();
                        let p_count = p.count;

                        div()
                            .id(ElementId::from(format!("rail-proj-{}", p.id)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(if is_selected {
                                cx.theme().accent
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .cursor_pointer()
                            .hover(|style| style.bg(cx.theme().accent))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_project_scope(
                                    HistoryWorkspaceScope::Project { project_id: project_id.clone() },
                                    window,
                                    cx,
                                );
                            }))
                            .context_menu(move |menu, _, _cx| {
                                let shell_for_active = shell_entity.clone();
                                let shell_for_del = shell_entity.clone();
                                let p_id_active = p_id_active.clone();
                                let p_id_del = p_id_del.clone();
                                let p_name_del = p_name_del.clone();
                                menu.item(
                                    PopupMenuItem::new("Set as Active")
                                        .icon(Icon::new(LucideIcon::Check))
                                        .on_click(move |_, _, cx| {
                                            let p_id = p_id_active.clone();
                                            shell_for_active.update(cx, |shell, cx| {
                                                shell
                                                    .project_selector
                                                    .select_project(Some(p_id), cx);
                                            });
                                        }),
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::new("Delete Workspace")
                                        .icon(Icon::new(LucideIcon::Trash))
                                        .on_click(move |_, _, cx| {
                                            let p_id = p_id_del.clone();
                                            let p_name = p_name_del.clone();
                                            shell_for_del.update(cx, |shell, cx| {
                                                shell.project_dialogs.open_delete(
                                                    p_id,
                                                    p_name,
                                                    p_count,
                                                    cx,
                                                );
                                            });
                                        }),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(color))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(p.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted)
                                    .child(p.count.to_string()),
                            )
                    })),
            )
    }
}

impl Default for ProjectsRail {
    fn default() -> Self {
        Self::new()
    }
}
