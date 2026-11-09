//! Projects left sidebar rail.

use crate::theme::tokens::*;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::history::HistoryWorkspaceScope;
use sona_core::project::ProjectRecord;
use std::collections::HashMap;
use std::time::Duration;

const PROJECT_ROW_STEP: f32 = 40.0;
const PROJECT_REORDER_DURATION: Duration = Duration::from_millis(250);
const PROJECT_REORDER_OFFSET_FRAME: Duration = Duration::from_millis(16);

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
    drag_state: ProjectDragState,
}

#[derive(Default)]
struct ProjectDragState {
    dragged_id: Option<String>,
    is_keyboard: bool,
    preview_order: Option<Vec<String>>,
    offsets: HashMap<String, f32>,
    active_animation: Option<u64>,
    next_animation: u64,
}

pub struct ProjectsRailState<'a> {
    pub selected_scope: &'a HistoryWorkspaceScope,
    pub inbox_count: usize,
    pub all_count: usize,
    pub trash_count: usize,
    pub is_inbox_active: bool,
    pub is_recording: bool,
}

#[derive(Clone)]
struct ProjectDragPreview {
    name: String,
    count: usize,
    color: Hsla,
    icon: Option<String>,
    pipeline_enabled: bool,
    is_selected: bool,
}

impl Render for ProjectDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(render_project_icon(self.icon.as_deref(), self.color))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(13.0))
                            .font_weight(if self.is_selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(if self.is_selected {
                                cx.theme().primary
                            } else {
                                cx.theme().foreground
                            })
                            .child(self.name.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .when(self.pipeline_enabled, |this| {
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
                            .child(self.count.to_string()),
                    ),
            )
    }
}

fn render_project_icon(icon: Option<&str>, color: Hsla) -> AnyElement {
    let system_icon = icon
        .and_then(|value| value.strip_prefix("system:"))
        .and_then(|name| match name {
            "mic" => Some(LucideIcon::Mic),
            "file" => Some(LucideIcon::FileText),
            "folder" => Some(LucideIcon::Folder),
            "code" => Some(LucideIcon::Code),
            _ => None,
        });

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
        } else if let Some(emoji) = icon.filter(|value| !value.starts_with("system:")) {
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
        .into_any_element()
}

fn build_project_actions_menu(
    menu: PopupMenu,
    shell: Entity<super::super::shell::MainShell>,
    project_id: String,
    project_name: String,
    project: ProjectRecord,
    project_count: usize,
) -> PopupMenu {
    let shell_for_active = shell.clone();
    let shell_for_settings = shell.clone();
    let shell_for_delete = shell;
    let active_id = project_id.clone();
    let delete_id = project_id;
    let delete_name = project_name;

    menu.item(
        PopupMenuItem::new("Set as Active")
            .icon(Icon::new(LucideIcon::Check))
            .on_click(move |_, _, cx| {
                let project_id = active_id.clone();
                shell_for_active.update(cx, |shell, cx| {
                    shell.project_selector.select_project(Some(project_id), cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new("Project Settings")
            .icon(Icon::new(LucideIcon::Settings))
            .on_click(move |_, window, cx| {
                let project = project.clone();
                shell_for_settings.update(cx, |shell, cx| {
                    shell.project_dialogs.update(cx, |dialogs, cx| {
                        dialogs.open_settings(&project, window, cx);
                    });
                });
            }),
    )
    .separator()
    .item(
        PopupMenuItem::new("Delete Workspace")
            .icon(Icon::new(LucideIcon::Trash))
            .on_click(move |_, _, cx| {
                let project_id = delete_id.clone();
                let project_name = delete_name.clone();
                shell_for_delete.update(cx, |shell, cx| {
                    shell.project_dialogs.update(cx, |dialogs, cx| {
                        dialogs.open_delete(project_id, project_name, project_count, cx);
                    });
                });
            }),
    )
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

fn reorder_animation_offsets(before: &[String], after: &[String]) -> HashMap<String, f32> {
    after
        .iter()
        .enumerate()
        .filter_map(|(new_index, id)| {
            let old_index = before.iter().position(|old_id| old_id == id)?;
            let offset = (old_index as f32 - new_index as f32) * PROJECT_ROW_STEP;
            (offset != 0.0).then(|| (id.clone(), offset))
        })
        .collect()
}

fn move_project_by(order: &[String], dragged_id: &str, direction: isize) -> Option<Vec<String>> {
    let current_index = order.iter().position(|id| id == dragged_id)?;
    let target_index = current_index.checked_add_signed(direction)?;
    let target_id = order.get(target_index)?;
    reordered_project_ids(order, dragged_id, target_id)
}

fn schedule_reorder_animation_reset(
    generation: u64,
    cx: &mut Context<super::super::shell::MainShell>,
) {
    cx.spawn(async move |this, cx: &mut AsyncApp| {
        cx.background_executor()
            .timer(PROJECT_REORDER_OFFSET_FRAME)
            .await;
        this.update(cx, |shell, cx| {
            if shell.projects_rail.clear_reorder_offsets(generation) {
                cx.notify();
            }
        })
        .ok();

        cx.background_executor()
            .timer(PROJECT_REORDER_DURATION)
            .await;
        this.update(cx, |shell, cx| {
            if shell.projects_rail.finish_reorder_animation(generation) {
                cx.notify();
            }
        })
        .ok();
    })
    .detach();
}

fn persist_project_order(
    project_ids: Vec<String>,
    cx: &mut Context<super::super::shell::MainShell>,
) {
    let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
    cx.spawn(
        async move |this, cx| match services.projects.reorder(project_ids).await {
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
        },
    )
    .detach();
}

impl ProjectsRail {
    pub fn new() -> Self {
        Self {
            selected_project: None,
            projects: Vec::new(),
            all_count: 0,
            drag_state: ProjectDragState::default(),
        }
    }

    pub fn sync_projects(
        &mut self,
        projects: &[ProjectRecord],
        counts: &HashMap<String, usize>,
        inbox_count: usize,
    ) {
        self.reset_drag_state();
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

    pub fn begin_drag(&mut self, dragged_id: &str) {
        self.drag_state.dragged_id = Some(dragged_id.to_string());
        self.drag_state.is_keyboard = false;
        self.drag_state.preview_order = None;
    }

    pub fn begin_keyboard_drag(&mut self, dragged_id: &str) {
        self.drag_state.dragged_id = Some(dragged_id.to_string());
        self.drag_state.is_keyboard = true;
        self.drag_state.preview_order = None;
    }

    pub fn preview_drag_target(&mut self, dragged_id: &str, target_id: &str) -> Option<u64> {
        if self.drag_state.dragged_id.as_deref() != Some(dragged_id) {
            return None;
        }

        let displayed_order = self
            .drag_state
            .preview_order
            .clone()
            .unwrap_or_else(|| self.project_ids());
        let next_order = reordered_project_ids(&displayed_order, dragged_id, target_id)
            .unwrap_or_else(|| displayed_order.clone());
        if displayed_order == next_order {
            return None;
        }

        let generation = self.start_reorder_transition(&displayed_order, &next_order);
        self.drag_state.preview_order = Some(next_order);
        Some(generation)
    }

    pub fn move_keyboard_drag(&mut self, direction: isize) -> Option<u64> {
        if !self.drag_state.is_keyboard {
            return None;
        }
        let dragged_id = self.drag_state.dragged_id.clone()?;
        let displayed_order = self
            .drag_state
            .preview_order
            .clone()
            .unwrap_or_else(|| self.project_ids());
        let next_order = move_project_by(&displayed_order, &dragged_id, direction)?;
        let generation = self.start_reorder_transition(&displayed_order, &next_order);
        self.drag_state.preview_order = Some(next_order);
        Some(generation)
    }

    pub fn finish_drag(
        &mut self,
        dragged_id: &str,
        target_id: &str,
    ) -> (Option<Vec<String>>, Option<u64>) {
        if self.drag_state.dragged_id.as_deref() != Some(dragged_id) {
            return (None, None);
        }

        let canonical_order = self.project_ids();
        let displayed_order = self
            .drag_state
            .preview_order
            .clone()
            .unwrap_or_else(|| canonical_order.clone());
        let committed_order = reordered_project_ids(&displayed_order, dragged_id, target_id)
            .unwrap_or_else(|| canonical_order.clone());
        let animation = if displayed_order != committed_order {
            Some(self.start_reorder_transition(&displayed_order, &committed_order))
        } else {
            self.drag_state.active_animation
        };

        let changed = committed_order != canonical_order;
        if changed {
            self.apply_project_order(&committed_order);
        }
        self.drag_state.dragged_id = None;
        self.drag_state.is_keyboard = false;
        self.drag_state.preview_order = None;
        (changed.then_some(committed_order), animation)
    }

    pub fn finish_keyboard_drag(&mut self) -> (Option<Vec<String>>, Option<u64>) {
        if !self.drag_state.is_keyboard {
            return (None, None);
        }
        let canonical_order = self.project_ids();
        let committed_order = self
            .drag_state
            .preview_order
            .clone()
            .unwrap_or_else(|| canonical_order.clone());
        let changed = committed_order != canonical_order;
        if changed {
            self.apply_project_order(&committed_order);
        }
        let animation = self.drag_state.active_animation;
        self.drag_state.dragged_id = None;
        self.drag_state.is_keyboard = false;
        self.drag_state.preview_order = None;
        (changed.then_some(committed_order), animation)
    }

    pub fn cancel_drag(&mut self) -> (bool, Option<u64>) {
        let canonical_order = self.project_ids();
        let displayed_order = self
            .drag_state
            .preview_order
            .clone()
            .unwrap_or_else(|| canonical_order.clone());
        let was_dragging = self.drag_state.dragged_id.take().is_some();
        self.drag_state.is_keyboard = false;
        self.drag_state.preview_order = None;

        if displayed_order != canonical_order {
            let animation = self.start_reorder_transition(&displayed_order, &canonical_order);
            (true, Some(animation))
        } else {
            (was_dragging, self.drag_state.active_animation)
        }
    }

    pub fn cancel_pointer_drag(&mut self) -> (bool, Option<u64>) {
        if self.drag_state.is_keyboard {
            return (false, None);
        }
        self.cancel_drag()
    }

    pub fn clear_reorder_offsets(&mut self, generation: u64) -> bool {
        if self.drag_state.active_animation != Some(generation)
            || self.drag_state.offsets.is_empty()
        {
            return false;
        }
        self.drag_state.offsets.clear();
        true
    }

    pub fn finish_reorder_animation(&mut self, generation: u64) -> bool {
        if self.drag_state.active_animation != Some(generation) {
            return false;
        }
        self.drag_state.active_animation = None;
        true
    }

    fn project_ids(&self) -> Vec<String> {
        self.projects
            .iter()
            .map(|project| project.id.clone())
            .collect()
    }

    fn apply_project_order(&mut self, order: &[String]) {
        self.projects = order
            .iter()
            .filter_map(|id| {
                self.projects
                    .iter()
                    .find(|project| &project.id == id)
                    .cloned()
            })
            .collect();
    }

    fn start_reorder_transition(&mut self, before: &[String], after: &[String]) -> u64 {
        self.drag_state.offsets = reorder_animation_offsets(before, after);
        self.drag_state.next_animation = self.drag_state.next_animation.wrapping_add(1);
        self.drag_state.active_animation = Some(self.drag_state.next_animation);
        self.drag_state.next_animation
    }

    fn reset_drag_state(&mut self) {
        self.drag_state.dragged_id = None;
        self.drag_state.is_keyboard = false;
        self.drag_state.preview_order = None;
        self.drag_state.offsets.clear();
        self.drag_state.active_animation = None;
    }

    pub fn select_project(&mut self, id: Option<String>) {
        self.selected_project = id;
    }

    pub fn render(
        &mut self,
        state: ProjectsRailState<'_>,
        window: &mut Window,
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
        let project_indices = self
            .drag_state
            .preview_order
            .as_ref()
            .map(|order| {
                order
                    .iter()
                    .filter_map(|id| self.projects.iter().position(|project| &project.id == id))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| (0..self.projects.len()).collect());

        div()
            .flex()
            .flex_col()
            .w(PANEL_LEFT_WIDTH)
            .h_full()
            .p_3()
            .gap_1()
            .bg(COLOR_BG_SECONDARY_LIGHT)
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
                            .px(px(10.0))
                            .h(px(36.0))
                            .rounded_md()
                            .bg(if is_all_selected {
                                COLOR_BG_ELEVATED_LIGHT
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .border_1()
                            .border_color(if is_all_selected {
                                COLOR_BORDER_LIGHT
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .when(is_all_selected, |style| style.shadow_sm())
                            .cursor_pointer()
                            .hover(|style| {
                                if !is_all_selected {
                                    style.bg(COLOR_BG_HOVER_LIGHT)
                                } else {
                                    style
                                }
                            })
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
                                        Icon::new(LucideIcon::Menu)
                                            .size(px(16.0))
                                            .text_color(if is_all_selected {
                                                COLOR_TEXT_PRIMARY_LIGHT
                                            } else {
                                                COLOR_TEXT_SECONDARY_LIGHT
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(if is_all_selected {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .text_color(if is_all_selected {
                                                COLOR_TEXT_PRIMARY_LIGHT
                                            } else {
                                                COLOR_TEXT_SECONDARY_LIGHT
                                            })
                                            .child("All Items"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
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
                            .px(px(10.0))
                            .h(px(36.0))
                            .rounded_md()
                            .bg(if is_trash_selected {
                                COLOR_BG_ELEVATED_LIGHT
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .border_1()
                            .border_color(if is_trash_selected {
                                COLOR_BORDER_LIGHT
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .when(is_trash_selected, |style| style.shadow_sm())
                            .cursor_pointer()
                            .hover(|style| {
                                if !is_trash_selected {
                                    style.bg(COLOR_BG_HOVER_LIGHT)
                                } else {
                                    style
                                }
                            })
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
                                                COLOR_TEXT_PRIMARY_LIGHT
                                            } else {
                                                COLOR_TEXT_SECONDARY_LIGHT
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(if is_trash_selected {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .text_color(if is_trash_selected {
                                                COLOR_TEXT_PRIMARY_LIGHT
                                            } else {
                                                COLOR_TEXT_SECONDARY_LIGHT
                                            })
                                            .child("Trash"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                                    .child(trash_count.to_string()),
                            ),
                    ),
            )
            .child(
                // Divider
                div().my_1().h(px(1.0)).bg(COLOR_BORDER_LIGHT),
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
                            .px(px(10.0))
                            .py(px(4.0))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
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
                            .px(px(10.0))
                            .h(px(36.0))
                            .rounded_md()
                            .bg(if is_inbox_selected {
                                COLOR_BG_ELEVATED_LIGHT
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .border_1()
                            .border_color(if is_inbox_selected {
                                COLOR_BORDER_LIGHT
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .when(is_inbox_selected, |style| style.shadow_sm())
                            .cursor_pointer()
                            .hover(|style| {
                                if !is_inbox_selected {
                                    style.bg(COLOR_BG_HOVER_LIGHT)
                                } else {
                                    style
                                }
                            })
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
                                                COLOR_TEXT_PRIMARY_LIGHT
                                            } else {
                                                COLOR_TEXT_SECONDARY_LIGHT
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(if is_inbox_selected {
                                                FontWeight::SEMIBOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .text_color(if is_inbox_selected {
                                                COLOR_TEXT_PRIMARY_LIGHT
                                            } else {
                                                COLOR_TEXT_SECONDARY_LIGHT
                                            })
                                            .child("Inbox"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(COLOR_TEXT_MUTED_LIGHT)
                                    .child(inbox_count.to_string()),
                            ),
                    )
                    .child(
                        // Projects list
                        div()
                            .id("projects-rail-list")
                            .flex()
                            .flex_col()
                            .gap_1()
                            .on_drag_move::<String>(cx.listener(
                                |shell, event: &DragMoveEvent<String>, _, cx| {
                                    let dragged_id = event.drag(cx).clone();
                                    if !event.bounds.contains(&event.event.position) {
                                        return;
                                    }
                                    let first_project_y = event.bounds.origin.y + px(40.0);
                                    let relative_y = event.event.position.y - first_project_y;
                                    if relative_y < px(0.0) {
                                        return;
                                    }
                                    let target_index = (relative_y / px(PROJECT_ROW_STEP))
                                        .floor()
                                        .max(0.0) as usize;
                                    let rail = &shell.projects_rail;
                                    let visible_order = rail
                                        .drag_state
                                        .preview_order
                                        .clone()
                                        .unwrap_or_else(|| rail.project_ids());
                                    let Some(target_id) = visible_order
                                        .get(target_index.min(visible_order.len().saturating_sub(1)))
                                        .cloned()
                                    else {
                                        return;
                                    };
                                    if let Some(generation) = shell
                                        .projects_rail
                                        .preview_drag_target(&dragged_id, &target_id)
                                    {
                                        cx.notify();
                                        schedule_reorder_animation_reset(generation, cx);
                                    }
                                },
                            ))
                            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                let (changed, generation) =
                                    this.projects_rail.cancel_pointer_drag();
                                if changed {
                                    cx.notify();
                                }
                                if let Some(generation) = generation {
                                    schedule_reorder_animation_reset(generation, cx);
                                }
                            }))
                            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| {
                                let (changed, generation) =
                                    this.projects_rail.cancel_pointer_drag();
                                if changed {
                                    cx.notify();
                                }
                                if let Some(generation) = generation {
                                    schedule_reorder_animation_reset(generation, cx);
                                }
                            }))
                            .when(self.projects.is_empty(), |this| {
                                this.child(
                                    div()
                                        .px(px(10.0))
                                        .py(px(8.0))
                                        .text_size(px(12.5))
                                        .text_color(COLOR_TEXT_MUTED_LIGHT)
                                        .child("No projects yet. Click + above to create one."),
                                )
                            })
                            .children(project_indices.iter().map(|index| {
                                let p = &self.projects[*index];
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
                                let p_name_del = p.name.clone();
                                let p_count = p.count;
                                let p_record = p.record.clone();
                                let drag_preview = ProjectDragPreview {
                                    name: p.name.clone(),
                                    count: p.count,
                                    color,
                                    icon: p.icon.clone(),
                                    pipeline_enabled: p.pipeline_enabled,
                                    is_selected,
                                };
                                let drop_target_id = p.id.clone();
                                let drag_target_id = p.id.clone();
                                let drag_source_id = p.id.clone();
                                let keyboard_project_id = p.id.clone();
                                let project_row_group = format!("project-rail-row-{}", p.id);
                                let menu_row_group = project_row_group.clone();
                                let count_row_group = project_row_group.clone();
                                let drag_shell = cx.entity().clone();
                                let is_dragged =
                                    self.drag_state.dragged_id.as_deref() == Some(p.id.as_str());
                                let y_offset = if let Some(generation) =
                                    self.drag_state.active_animation
                                {
                                    let offset = self
                                        .drag_state
                                        .offsets
                                        .get(&p.id)
                                        .copied()
                                        .unwrap_or(0.0);
                                    gpui_kit::base::motion::transition(
                                        format!("projects-rail-sort-{}-{generation}", p.id),
                                        px(offset),
                                        gpui_kit::base::motion::Transition::new(
                                            PROJECT_REORDER_DURATION,
                                        ),
                                        window,
                                        cx,
                                    )
                                } else {
                                    px(0.0)
                                };

                                div()
                                    .id(ElementId::from(format!("rail-proj-{}", p.id)))
                                    .group(project_row_group)
                                    .focusable()
                                    .relative()
                                    .top(y_offset)
                                    .on_key_down(cx.listener(
                                        move |shell, event: &KeyDownEvent, window, cx| {
                                            let key = event.keystroke.key.to_ascii_lowercase();
                                            let is_drag_key = matches!(
                                                key.as_str(),
                                                " " | "space" | "enter"
                                            );
                                            let mut handled = false;
                                            let mut animation = None;
                                            let mut committed_order = None;

                                            if shell.projects_rail.drag_state.is_keyboard {
                                                match key.as_str() {
                                                    "arrowup" | "up" => {
                                                        animation = shell
                                                            .projects_rail
                                                            .move_keyboard_drag(-1);
                                                        handled = true;
                                                    }
                                                    "arrowdown" | "down" => {
                                                        animation = shell
                                                            .projects_rail
                                                            .move_keyboard_drag(1);
                                                        handled = true;
                                                    }
                                                    "escape" if !event.is_held => {
                                                        let (_, generation) =
                                                            shell.projects_rail.cancel_drag();
                                                        animation = generation;
                                                        handled = true;
                                                    }
                                                    _ if is_drag_key && !event.is_held => {
                                                        (committed_order, animation) = shell
                                                            .projects_rail
                                                            .finish_keyboard_drag();
                                                        handled = true;
                                                    }
                                                    _ => {}
                                                }
                                            } else if is_drag_key && !event.is_held {
                                                shell
                                                    .projects_rail
                                                    .begin_keyboard_drag(&keyboard_project_id);
                                                handled = true;
                                            }

                                            if handled {
                                                window.prevent_default();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }
                                            if let Some(generation) = animation {
                                                schedule_reorder_animation_reset(generation, cx);
                                            }
                                            if let Some(project_ids) = committed_order {
                                                persist_project_order(project_ids, cx);
                                            }
                                        },
                                    ))
                                    .on_drag(p.id.clone(), move |_, _, _, app| {
                                        let source_id = drag_source_id.clone();
                                        drag_shell
                                            .update(app, |shell, cx| {
                                                shell.projects_rail.begin_drag(&source_id);
                                                cx.notify();
                                            });
                                        let preview = drag_preview.clone();
                                        app.new(|_| preview)
                                    })
                                    .drag_over::<String>(move |style, dragged_id, _, cx| {
                                        if dragged_id == &drag_target_id {
                                            style
                                        } else {
                                            style.border_1().border_color(cx.theme().primary)
                                        }
                                    })
                                    .on_drop(cx.listener(move |this, dragged_id: &String, _, cx| {
                                        let (project_ids, generation) = this
                                            .projects_rail
                                            .finish_drag(dragged_id, &drop_target_id);
                                        cx.notify();
                                        if let Some(generation) = generation {
                                            schedule_reorder_animation_reset(generation, cx);
                                        }
                                        let Some(project_ids) = project_ids else { return };
                                        persist_project_order(project_ids, cx);
                                    }))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px(px(10.0))
                                    .h(px(36.0))
                                    .rounded_md()
                                    .when(is_dragged, |this| this.opacity(0.4))
                                    .bg(if is_selected {
                                        COLOR_BG_ELEVATED_LIGHT
                                    } else {
                                        gpui_kit::transparent_black()
                                    })
                                    .border_1()
                                    .border_color(if is_selected {
                                        COLOR_BORDER_LIGHT
                                    } else {
                                        gpui_kit::transparent_black()
                                    })
                                    .when(is_selected, |style| style.shadow_sm())
                                    .cursor_pointer()
                                    .hover(|style| {
                                        if !is_selected {
                                            style.bg(COLOR_BG_HOVER_LIGHT)
                                        } else {
                                            style
                                        }
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.set_project_scope(
                                            HistoryWorkspaceScope::Project { project_id: project_id.clone() },
                                            window,
                                            cx,
                                        );
                                    }))
                                    .context_menu({
                                        let shell = shell_entity.clone();
                                        let project_id = p_id_active.clone();
                                        let project_name = p_name_del.clone();
                                        let project = p_record.clone();
                                        move |menu, _, _| {
                                            build_project_actions_menu(
                                                menu,
                                                shell.clone(),
                                                project_id.clone(),
                                                project_name.clone(),
                                                project.clone(),
                                                p_count,
                                            )
                                        }
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(render_project_icon(project_icon, color))
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
                                                    .group_hover(count_row_group, |style| {
                                                        style.opacity(0.0)
                                                    })
                                                    .child(p.count.to_string()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id(ElementId::Name(
                                                format!("project-menu-trigger-{}", p.id).into(),
                                            ))
                                            .absolute()
                                            .right(px(8.0))
                                            .top(px(7.0))
                                            .on_click(|_, _, cx| cx.stop_propagation())
                                            .child(
                                                Button::new(ElementId::Name(
                                                    format!("project-menu-{}", p.id).into(),
                                                ))
                                                .ghost()
                                                .small()
                                                .icon(LucideIcon::Ellipsis)
                                                .tooltip("More options")
                                                .accessibility_label(format!(
                                                    "More options for {}",
                                                    p.name
                                                ))
                                                .opacity(0.0)
                                                .group_hover(menu_row_group, |button| {
                                                    button.opacity(1.0)
                                                })
                                                .in_focus(|button| button.opacity(1.0))
                                                .focus_visible(|button| button.opacity(1.0))
                                                .dropdown_menu_with_anchor(
                                                    Anchor::BottomRight,
                                                    move |menu, _, _| {
                                                        build_project_actions_menu(
                                                            menu,
                                                            shell_entity.clone(),
                                                            p_id_active.clone(),
                                                            p_name_del.clone(),
                                                            p_record.clone(),
                                                            p_count,
                                                        )
                                                    },
                                                ),
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
    use super::{
        PROJECT_ROW_STEP, move_project_by, reorder_animation_offsets, reordered_project_ids,
    };

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

    #[test]
    fn reorder_offsets_start_each_row_at_its_previous_position() {
        let before = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let after = vec!["b".into(), "c".into(), "a".into(), "d".into()];
        let offsets = reorder_animation_offsets(&before, &after);

        assert_eq!(offsets.get("b"), Some(&PROJECT_ROW_STEP));
        assert_eq!(offsets.get("c"), Some(&PROJECT_ROW_STEP));
        assert_eq!(offsets.get("a"), Some(&(-2.0 * PROJECT_ROW_STEP)));
        assert!(!offsets.contains_key("d"));
    }

    #[test]
    fn keyboard_moves_one_slot_and_stops_at_the_ends() {
        let ids = vec!["a".into(), "b".into(), "c".into()];

        assert_eq!(
            move_project_by(&ids, "a", 1),
            Some(vec!["b".into(), "a".into(), "c".into()]),
        );
        assert_eq!(
            move_project_by(&ids, "c", -1),
            Some(vec!["a".into(), "c".into(), "b".into()]),
        );
        assert_eq!(move_project_by(&ids, "a", -1), None);
        assert_eq!(move_project_by(&ids, "c", 1), None);
    }
}
