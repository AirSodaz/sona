//! Projects left sidebar rail.

use crate::theme::tokens::*;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
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
    pub color: Option<String>,
    pub icon: Option<String>,
    pub pipeline_enabled: bool,
    pub record: ProjectRecord,
}

pub struct ProjectsRail {
    pub selected_project: Option<String>,
    pub projects: Vec<ProjectItem>,
    pub all_count: usize,
}

pub struct ProjectsRailState<'a> {
    pub selected_scope: &'a HistoryWorkspaceScope,
    pub inbox_count: usize,
    pub all_count: usize,
    pub trash_count: usize,
    pub is_inbox_active: bool,
    pub is_recording: bool,
}

struct ProjectDragPreview {
    name: String,
}

impl Render for ProjectDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .shadow_lg()
            .text_size(px(13.0))
            .text_color(cx.theme().foreground)
            .child(self.name.clone())
    }
}

fn reordered_project_ids(
    project_ids: &[String],
    dragged_id: &str,
    target_id: &str,
) -> Option<Vec<String>> {
    let from = project_ids
        .iter()
        .position(|project_id| project_id == dragged_id)?;
    let to = project_ids
        .iter()
        .position(|project_id| project_id == target_id)?;
    if from == to {
        return None;
    }
    let mut ids = project_ids.to_vec();
    let moved = ids.remove(from);
    ids.insert(to, moved);
    Some(ids)
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
                color: project.color.clone(),
                icon: project.icon.clone(),
                pipeline_enabled: project
                    .pipeline
                    .as_ref()
                    .map(|p| p.enabled)
                    .unwrap_or(false),
                record: project.clone(),
            })
            .collect();
        self.all_count = inbox_count + counts.values().sum::<usize>();
    }

    pub fn select_project(&mut self, id: Option<String>) {
        self.selected_project = id;
    }

    pub fn render(
        &mut self,
        state: ProjectsRailState<'_>,
        cx: &mut Context<super::super::shell::MainShell>,
    ) -> impl IntoElement {
        let ProjectsRailState {
            selected_scope,
            inbox_count,
            all_count,
            trash_count,
            is_inbox_active,
            is_recording,
        } = state;
        let is_all_selected = matches!(selected_scope, HistoryWorkspaceScope::All);
        let is_inbox_selected = matches!(selected_scope, HistoryWorkspaceScope::Inbox);
        let is_trash_selected = matches!(selected_scope, HistoryWorkspaceScope::Trash);

        div()
            .flex()
            .flex_col()
            .w(PANEL_LEFT_WIDTH)
            .h_full()
            .p_3()
            .gap_1()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                // Scopes group (All Items & Trash)
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
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
                            .context_menu({
                                let shell_entity = cx.entity().clone();
                                move |menu, _, _cx| {
                                    let shell = shell_entity.clone();
                                    menu.item(
                                        PopupMenuItem::new("Open All Items")
                                            .icon(Icon::new(LucideIcon::FolderOpen))
                                            .disabled(is_all_selected || is_recording)
                                            .on_click(move |_, window, cx| {
                                                shell.update(cx, |shell, cx| {
                                                    shell.set_project_scope(
                                                        HistoryWorkspaceScope::All,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::new(LucideIcon::ListFilter)
                                            .size(px(16.0))
                                            .text_color(if is_all_selected {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().muted
                                            }),
                                    )
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
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted)
                                    .child(all_count.to_string()),
                            ),
                    )
                    .child(
                        // "Trash" button
                        div()
                            .id("rail-trash")
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(if is_trash_selected {
                                cx.theme().accent
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .cursor_pointer()
                            .hover(|style| style.bg(cx.theme().accent))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_project_scope(HistoryWorkspaceScope::Trash, window, cx);
                            }))
                            .context_menu({
                                let shell_entity = cx.entity().clone();
                                move |menu, _, _cx| {
                                    let shell = shell_entity.clone();
                                    let open_shell = shell.clone();
                                    menu.item(
                                        PopupMenuItem::new("Open Trash")
                                            .icon(Icon::new(LucideIcon::FolderOpen))
                                            .disabled(is_trash_selected || is_recording)
                                            .on_click(move |_, window, cx| {
                                                open_shell.update(cx, |shell, cx| {
                                                    shell.set_project_scope(
                                                        HistoryWorkspaceScope::Trash,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )
                                    .separator()
                                    .item(
                                        PopupMenuItem::new("Empty Trash")
                                            .icon(Icon::new(LucideIcon::Trash))
                                            .disabled(trash_count == 0)
                                            .on_click(move |_, _, cx| {
                                                shell.update(cx, |shell, cx| {
                                                    shell.selection_bar.request_empty_trash(trash_count, cx);
                                                });
                                            }),
                                    )
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::new(LucideIcon::Trash)
                                            .size(px(16.0))
                                            .text_color(if is_trash_selected {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().muted
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_trash_selected {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().foreground
                                            })
                                            .child("Trash"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted)
                                    .child(trash_count.to_string()),
                            ),
                    ),
            )
            .child(
                // Divider
                div().my_2().h(px(1.0)).bg(cx.theme().border),
            )
            .child(
                // Projects group
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
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
                                    .child("PROJECTS"),
                            )
                            .child(
                                Button::new("btn-new-project")
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::Plus)
                                    .tooltip("New Project")
                                    .accessibility_label("New Project")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.project_dialogs.update(cx, |dialogs, cx| {
                                            dialogs.open_create(window, cx);
                                        });
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        // "Inbox" button
                        div()
                            .id("rail-inbox")
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(if is_inbox_selected {
                                cx.theme().accent
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .cursor_pointer()
                            .hover(|style| style.bg(cx.theme().accent))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_project_scope(HistoryWorkspaceScope::Inbox, window, cx);
                            }))
                            .context_menu({
                                let shell_entity = cx.entity().clone();
                                move |menu, _, _cx| {
                                    let open_shell = shell_entity.clone();
                                    let active_shell = shell_entity.clone();
                                    menu.item(
                                        PopupMenuItem::new("Open Inbox")
                                            .icon(Icon::new(LucideIcon::FolderOpen))
                                            .disabled(is_inbox_selected || is_recording)
                                            .on_click(move |_, window, cx| {
                                                open_shell.update(cx, |shell, cx| {
                                                    shell.set_project_scope(
                                                        HistoryWorkspaceScope::Inbox,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new("Set as Active")
                                            .icon(Icon::new(LucideIcon::Check))
                                            .disabled(is_inbox_active)
                                            .on_click(move |_, _, cx| {
                                                active_shell.update(cx, |shell, cx| {
                                                    shell.project_selector.select_project(None, cx);
                                                });
                                            }),
                                    )
                                }
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::new(LucideIcon::Inbox)
                                            .size(px(16.0))
                                            .text_color(if is_inbox_selected {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().muted
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_inbox_selected {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().foreground
                                            })
                                            .child("Inbox"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted)
                                    .child(inbox_count.to_string()),
                            ),
                    )
                    .child(
                        // Projects list
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .when(self.projects.is_empty(), |this| {
                                this.child(
                                    div()
                                        .p_3()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted)
                                        .text_center()
                                        .child("No tags yet."),
                                )
                            })
                            .children(self.projects.iter().map(|p| {
                                let is_selected = matches!(selected_scope, HistoryWorkspaceScope::Project { project_id } if project_id == &p.id);
                                let color = p
                                    .color
                                    .as_deref()
                                    .and_then(hex_to_hsla)
                                    .unwrap_or_else(|| project_color_from_index(p.color_index));
                                let project_icon = p.icon.as_deref().filter(|icon| !icon.is_empty());
                                let project_id = p.id.clone();
                                let shell_entity = cx.entity().clone();
                                let p_id_active = p.id.clone();
                                let p_id_del = p.id.clone();
                                let p_name_del = p.name.clone();
                                let p_count = p.count;
                                let p_record = p.record.clone();
                                let drag_preview_name = p.name.clone();
                                let drop_target_id = p.id.clone();

                                div()
                                    .id(ElementId::from(format!("rail-proj-{}", p.id)))
                                    .on_drag(p.id.clone(), move |_, _, _, cx| {
                                        cx.new(|_| ProjectDragPreview {
                                            name: drag_preview_name.clone(),
                                        })
                                    })
                                    .on_drop(cx.listener(move |this, dragged_id: &String, _, cx| {
                                        let current_ids: Vec<_> = this
                                            .projects_rail
                                            .projects
                                            .iter()
                                            .map(|project| project.id.clone())
                                            .collect();
                                        let Some(project_ids) = reordered_project_ids(
                                            &current_ids,
                                            dragged_id,
                                            &drop_target_id,
                                        ) else {
                                            return;
                                        };
                                        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
                                        cx.spawn(async move |this, cx| {
                                            match services.projects.reorder(project_ids).await {
                                                Ok(projects) => {
                                                    this.update(cx, |shell, cx| {
                                                        shell.project_selector.projects = projects;
                                                        shell.projects_rail.sync_projects(
                                                            &shell.project_selector.projects,
                                                            &shell.project_selector.project_counts,
                                                            shell.project_selector.count_inbox,
                                                        );
                                                        cx.notify();
                                                    })
                                                    .ok();
                                                }
                                                Err(_) => {
                                                    this.update(cx, |shell, cx| {
                                                        shell.project_selector.load_data(cx);
                                                    })
                                                    .ok();
                                                }
                                            }
                                        })
                                        .detach();
                                    }))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px(px(10.0))
                                    .h(px(36.0))
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
                                    .context_menu({
                                        let shell_for_active = shell_entity.clone();
                                        let shell_for_settings = shell_entity.clone();
                                        let shell_for_del = shell_entity.clone();
                                        let p_id_active = p_id_active.clone();
                                        let p_id_del = p_id_del.clone();
                                        let p_name_del = p_name_del.clone();
                                        let p_record = p_record.clone();
                                        move |menu, _, _cx| {
                                            let shell_for_active = shell_for_active.clone();
                                            let shell_for_settings = shell_for_settings.clone();
                                            let shell_for_del = shell_for_del.clone();
                                            let p_id_active = p_id_active.clone();
                                            let p_id_del = p_id_del.clone();
                                            let p_name_del = p_name_del.clone();
                                            let p_record = p_record.clone();
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
                                            .item(
                                                PopupMenuItem::new("Project Settings")
                                                    .icon(Icon::new(LucideIcon::Settings))
                                                    .on_click(move |_, window, cx| {
                                                        let p = p_record.clone();
                                                        shell_for_settings.update(cx, |shell, cx| {
                                                            shell.project_dialogs.update(cx, |dialogs, cx| {
                                                                dialogs.open_settings(&p, window, cx);
                                                            });
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
                                                            shell.project_dialogs.update(cx, |dialogs, cx| {
                                                                dialogs.open_delete(
                                                                    p_id,
                                                                    p_name,
                                                                    p_count,
                                                                    cx,
                                                                );
                                                            });
                                                        });
                                                    }),
                                            )
                                        }
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child({
                                                let icon_name = project_icon
                                                    .and_then(|value| value.strip_prefix("system:"));
                                                let system_icon = match icon_name {
                                                    Some("mic") => Some(LucideIcon::Mic),
                                                    Some("file") => Some(LucideIcon::FileText),
                                                    Some("folder") => Some(LucideIcon::Folder),
                                                    Some("code") => Some(LucideIcon::Code),
                                                    _ => None,
                                                };
                                                div()
                                                    .size(px(22.0))
                                                    .rounded(px(4.0))
                                                    .bg(color.opacity(0.14))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(if let Some(icon) = system_icon {
                                                        Icon::new(icon)
                                                            .size(px(16.0))
                                                            .text_color(color)
                                                            .into_any_element()
                                                    } else if let Some(emoji) = project_icon
                                                        .filter(|value| !value.starts_with("system:"))
                                                    {
                                                        div()
                                                            .text_size(px(14.0))
                                                            .child(emoji.to_string())
                                                            .into_any_element()
                                                    } else {
                                                        div()
                                                            .size(px(8.0))
                                                            .rounded_full()
                                                            .bg(color)
                                                            .into_any_element()
                                                    })
                                            })
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(if is_selected { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                                                    .text_color(if is_selected { cx.theme().primary } else { cx.theme().foreground })
                                                    .child(p.name.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .when(p.pipeline_enabled, |this| {
                                                this.child(
                                                    Icon::new(LucideIcon::Zap)
                                                        .size(px(13.0))
                                                        .text_color(cx.theme().muted),
                                                )
                                            })
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(cx.theme().muted)
                                                    .child(p.count.to_string()),
                                            ),
                                    )
                            })),
                    ),
            )
    }
}

impl Default for ProjectsRail {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::reordered_project_ids;

    #[test]
    fn reorders_project_to_target_position() {
        let ids = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        assert_eq!(
            reordered_project_ids(&ids, "a", "c"),
            Some(vec!["b".into(), "c".into(), "a".into(), "d".into()]),
        );
        assert_eq!(
            reordered_project_ids(&ids, "d", "b"),
            Some(vec!["a".into(), "d".into(), "b".into(), "c".into()]),
        );
    }

    #[test]
    fn ignores_same_or_unknown_project_ids() {
        let ids = vec!["a".into(), "b".into()];
        assert_eq!(reordered_project_ids(&ids, "a", "a"), None);
        assert_eq!(reordered_project_ids(&ids, "missing", "b"), None);
        assert_eq!(reordered_project_ids(&ids, "a", "missing"), None);
    }
}
