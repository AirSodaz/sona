//! Projects results grid & cards list for transcript history.

use chrono::TimeZone as _;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::{Disableable as _, FocusableExt as _};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::super::shell::MainShell;
use super::toolbar::ViewMode;
use crate::app::GlobalDesktopServices;
use sona_core::history::mutation_repository::{HistoryItemMetaPatch, HistoryUpdateItemMetaRequest};
use sona_core::history::mutation_repository::{
    HistoryRestoreItemsRequest, HistoryUpdateProjectAssignmentsRequest,
};
use sona_core::history::store::HistoryStore as _;
use sona_core::history::{
    HistoryItemKind, HistoryItemRecord, HistoryItemStatus, HistoryWorkspaceDateFilter,
    HistoryWorkspaceFilterType, HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
    HistoryWorkspaceSummary,
};
use sona_core::project::ProjectRecord;

#[derive(Clone, Debug)]
pub struct HistoryRecordItem {
    pub record: HistoryItemRecord,
    pub id: String,
    pub title: String,
    pub duration: String,
    pub date: String,
    pub excerpt: String,
}

pub struct ProjectsResults {
    pub items: Vec<HistoryRecordItem>,
    pub selected_id: Option<String>,
    pub selected_ids: Vec<String>,
    pub scope: HistoryWorkspaceScope,
    pub search_query: String,
    pub active_search_result_id: Option<String>,
    pub filter_type: HistoryWorkspaceFilterType,
    pub date_filter: HistoryWorkspaceDateFilter,
    pub sort_order: HistoryWorkspaceSortOrder,
    pub filtered_count: usize,
    pub scoped_count: usize,
    pub has_more: bool,
    pub trash_count: usize,
    pub summary: Option<HistoryWorkspaceSummary>,
    pub is_loading: bool,
    pub is_loading_more: bool,
    pub load_error: bool,
    pub load_more_error: bool,
    pub rename_input_state: Entity<InputState>,
    scroll_handle: ScrollHandle,
    search_matches: std::collections::BTreeMap<String, Option<HistoryWorkspaceItemSearchMatch>>,
    rename_target: Option<String>,
    rename_error: Option<String>,
    rename_busy: bool,
    load_generation: u64,
}

impl ProjectsResults {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
        let rename_input_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Transcript title"));
        Self {
            items: Vec::new(),
            selected_id: None,
            selected_ids: Vec::new(),
            scope: HistoryWorkspaceScope::Inbox,
            search_query: String::new(),
            active_search_result_id: None,
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            filtered_count: 0,
            scoped_count: 0,
            has_more: false,
            trash_count: 0,
            summary: None,
            is_loading: false,
            is_loading_more: false,
            load_error: false,
            load_more_error: false,
            rename_input_state,
            scroll_handle: ScrollHandle::new(),
            search_matches: std::collections::BTreeMap::new(),
            rename_target: None,
            rename_error: None,
            rename_busy: false,
            load_generation: 0,
        }
    }

    pub fn load_data(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;
        let request = HistoryWorkspaceQueryRequest {
            scope: self.scope.clone(),
            query: self.search_query.clone(),
            filter_type: self.filter_type,
            date_filter: self.date_filter,
            sort_order: self.sort_order,
            limit: 50,
            offset: 0,
        };
        self.is_loading = true;
        self.is_loading_more = false;
        self.load_error = false;
        self.load_more_error = false;
        cx.notify();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = services
                .history
                .query_db(move |query| query.query_workspace(request))
                .await;
            this.update(cx, |shell, cx| {
                let results = &mut shell.projects_results;
                if results.load_generation != generation {
                    return;
                }
                match result {
                    Ok(page) => results.set_page(page),
                    Err(_) => results.load_error = true,
                }
                results.is_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn load_more_data(&mut self, cx: &mut Context<MainShell>) {
        if !self.has_more || self.is_loading || self.is_loading_more {
            return;
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let generation = self.load_generation;
        let request = HistoryWorkspaceQueryRequest {
            scope: self.scope.clone(),
            query: self.search_query.clone(),
            filter_type: self.filter_type,
            date_filter: self.date_filter,
            sort_order: self.sort_order,
            limit: 50,
            offset: self.items.len(),
        };
        self.is_loading_more = true;
        self.load_more_error = false;
        cx.notify();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = services
                .history
                .query_db(move |query| query.query_workspace(request))
                .await;
            this.update(cx, |shell, cx| {
                let results = &mut shell.projects_results;
                if results.load_generation != generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        results.search_matches.extend(page.search_match_by_item_id);
                        results.filtered_count = page.filtered_item_count;
                        results.scoped_count = page.summary.total_items;
                        results.has_more = page.has_more;
                        results.trash_count = page.item_counts.trash;
                        results.summary = Some(page.summary);
                        results
                            .items
                            .extend(page.filtered_items.into_iter().map(history_item));
                    }
                    Err(_) => results.load_more_error = true,
                }
                results.is_loading_more = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_page(&mut self, page: HistoryWorkspaceQueryResult) {
        self.filtered_count = page.filtered_item_count;
        self.scoped_count = page.summary.total_items;
        self.has_more = page.has_more;
        self.trash_count = page.item_counts.trash;
        self.summary = Some(page.summary);
        self.search_matches = page.search_match_by_item_id;
        self.items = page.filtered_items.into_iter().map(history_item).collect();
        self.retain_visible_selection();
        if !self
            .active_search_result_id
            .as_ref()
            .is_some_and(|id| self.items.iter().any(|item| item.id == *id))
        {
            self.active_search_result_id = None;
        }
    }

    pub fn set_scope(&mut self, scope: HistoryWorkspaceScope, cx: &mut Context<MainShell>) {
        self.scope = scope;
        self.search_query.clear();
        self.active_search_result_id = None;
        self.filter_type = HistoryWorkspaceFilterType::All;
        self.date_filter = HistoryWorkspaceDateFilter::All;
        self.selected_id = None;
        self.selected_ids.clear();
        self.load_data(cx);
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<MainShell>) {
        if self.search_query == query {
            return;
        }
        self.active_search_result_id = None;
        self.search_query = query;
        self.load_data(cx);
    }

    pub fn navigate_search_result(
        &mut self,
        direction: i32,
        view_mode: ViewMode,
        cx: &mut Context<MainShell>,
    ) {
        if self.items.is_empty() {
            return;
        }
        let current_index = self
            .active_search_result_id
            .as_deref()
            .and_then(|current| self.items.iter().position(|item| item.id == current));
        let Some(next_index) =
            next_search_index(self.items.len(), current_index, direction.signum())
        else {
            return;
        };
        self.active_search_result_id = Some(self.items[next_index].id.clone());
        self.scroll_handle
            .scroll_to_item(next_index + usize::from(view_mode == ViewMode::Table));
        cx.notify();
    }

    pub fn set_filter_type(
        &mut self,
        filter_type: HistoryWorkspaceFilterType,
        cx: &mut Context<MainShell>,
    ) {
        self.filter_type = filter_type;
        self.active_search_result_id = None;
        self.load_data(cx);
    }

    pub fn set_date_filter(
        &mut self,
        date_filter: HistoryWorkspaceDateFilter,
        cx: &mut Context<MainShell>,
    ) {
        self.date_filter = date_filter;
        self.active_search_result_id = None;
        self.load_data(cx);
    }

    pub fn set_sort_order(
        &mut self,
        sort_order: HistoryWorkspaceSortOrder,
        cx: &mut Context<MainShell>,
    ) {
        self.sort_order = sort_order;
        self.active_search_result_id = None;
        self.load_data(cx);
    }

    pub fn reset_filters(&mut self, cx: &mut Context<MainShell>) {
        self.filter_type = HistoryWorkspaceFilterType::All;
        self.date_filter = HistoryWorkspaceDateFilter::All;
        self.sort_order = HistoryWorkspaceSortOrder::Newest;
        self.active_search_result_id = None;
        self.load_data(cx);
    }

    pub fn select_item(&mut self, id: String) {
        self.selected_id = Some(id);
    }

    pub fn toggle_selected_item(&mut self, id: String) {
        if let Some(index) = self
            .selected_ids
            .iter()
            .position(|selected| selected == &id)
        {
            self.selected_ids.remove(index);
        } else if self.items.iter().any(|item| item.id == id) {
            self.selected_ids.push(id);
        }
    }

    pub fn toggle_select_all_visible(&mut self) {
        if !self.items.is_empty() && self.selected_ids.len() == self.items.len() {
            self.selected_ids.clear();
        } else {
            self.selected_ids = self.items.iter().map(|item| item.id.clone()).collect();
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_ids.clear();
    }

    fn begin_rename(&mut self, id: String, window: &mut Window, cx: &mut Context<MainShell>) {
        let Some(item) = self.items.iter().find(|item| item.id == id) else {
            return;
        };
        self.rename_input_state.update(cx, |state, cx| {
            state.set_value(item.title.clone(), window, cx);
        });
        self.rename_target = Some(id);
        self.rename_error = None;
        cx.notify();
    }

    fn save_rename(&mut self, cx: &mut Context<MainShell>) {
        let Some(id) = self.rename_target.clone() else {
            return;
        };
        let title = self.rename_input_state.read(cx).value().trim().to_string();
        if title.is_empty() || self.rename_busy {
            return;
        }
        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
        self.rename_busy = true;
        self.rename_error = None;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = history
                .mutation_db(move |service| {
                    service.update_item_meta(HistoryUpdateItemMetaRequest {
                        history_id: id,
                        updates: HistoryItemMetaPatch {
                            title: Some(title),
                            ..Default::default()
                        },
                    })
                })
                .await;
            this.update(cx, |shell, cx| {
                let results = &mut shell.projects_results;
                results.rename_busy = false;
                match result {
                    Ok(()) => {
                        results.rename_target = None;
                        results.rename_error = None;
                        results.load_data(cx);
                    }
                    Err(error) => {
                        results.rename_error = Some(format!("Could not rename item: {error}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn retain_visible_selection(&mut self) {
        self.selected_ids
            .retain(|id| self.items.iter().any(|item| item.id == *id));
    }

    pub fn render(
        &mut self,
        view_mode: ViewMode,
        selection_mode: bool,
        projects: &[ProjectRecord],
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let selected_ids = self.selected_ids.clone();
        let selected_id = self.selected_id.clone();
        let active_search_result_id = self.active_search_result_id.clone();
        let search_query = self.search_query.trim().to_string();
        let show_project_badge = matches!(self.scope, HistoryWorkspaceScope::All);
        let is_empty_scope = self.scoped_count == 0 && self.search_query.is_empty();
        let content = if self.is_loading {
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(LucideIcon::LoaderCircle).size(px(16.0)))
                        .child("Loading transcripts…"),
                )
                .into_any_element()
        } else if self.load_error {
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .flex_col()
                .gap_2()
                .child(
                    Icon::new(LucideIcon::CircleAlert)
                        .size(px(28.0))
                        .text_color(cx.theme().danger),
                )
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Workspace items could not be loaded"),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted)
                        .child("Check the connection and try again."),
                )
                .child(
                    gpui_kit::component::button::Button::new("retry-history-load")
                        .secondary()
                        .small()
                        .icon(LucideIcon::RefreshCw)
                        .label("Retry")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.projects_results.load_data(cx)),
                        ),
                )
                .into_any_element()
        } else if self.items.is_empty() {
            div()
                .flex()
                .flex_1()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .text_color(cx.theme().muted)
                .child(if is_empty_scope {
                    Icon::new(LucideIcon::CirclePlus).size(px(32.0))
                } else {
                    Icon::new(LucideIcon::Search).size(px(32.0))
                })
                .child(if is_empty_scope {
                    "No items in this workspace yet."
                } else {
                    "No matching items"
                })
                .when(!is_empty_scope, |empty| {
                    empty
                        .child(
                            div()
                                .text_size(px(13.0))
                                .child("Try a different search or clear the current filters."),
                        )
                        .child(
                            Button::new("clear-history-filters")
                                .outline()
                                .small()
                                .label("Clear filters")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.projects_toolbar.reset_filters(window, cx);
                                })),
                        )
                })
                .into_any_element()
        } else {
            div()
                .id("projects-results-scroll")
                .flex()
                .flex_col()
                .size_full()
                .track_scroll(&self.scroll_handle)
                .overflow_y_scrollbar()
                .on_scroll(cx.listener(|this, event: &gpui::ScrollEvent, window, cx| {
                    let delta = event.delta.pixel_delta(window.line_height());
                    let offset = this.projects_results.scroll_handle.offset();
                    let max = this.projects_results.scroll_handle.max_offset().y;
                    let next_y = (offset.y.as_f32() + delta.y.as_f32()).clamp(-max.as_f32(), 0.0);
                    if should_load_more(max.as_f32(), next_y) {
                        this.projects_results.load_more_data(cx);
                    }
                }))
                .when(view_mode == ViewMode::Cards, |this| {
                    this.flex_wrap().content_start().gap_3().p_4()
                })
                .when(view_mode != ViewMode::Cards, |this| this.flex_col())
                .when(view_mode == ViewMode::Table, |this| {
                    this.child(render_table_header(selection_mode, show_project_badge, cx))
                })
                .children(self.items.iter().map(|item| {
                    let is_selected = if selection_mode {
                        selected_ids.iter().any(|id| id == &item.id)
                    } else {
                        selected_id.as_deref() == Some(&item.id)
                    };
                    render_history_item(
                        item,
                        view_mode,
                        selection_mode,
                        is_selected,
                        show_project_badge,
                        projects,
                        &selected_ids,
                        &self.items,
                        self.search_matches.get(&item.id).and_then(Option::as_ref),
                        !selection_mode && active_search_result_id.as_deref() == Some(&item.id),
                        !search_query.is_empty(),
                        cx,
                    )
                }))
                .when(self.is_loading_more || self.load_more_error, |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .py_3()
                            .when(self.is_loading_more, |footer| {
                                footer.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted)
                                        .child(Icon::new(LucideIcon::LoaderCircle).size(px(14.0)))
                                        .child("Loading more items…"),
                                )
                            })
                            .when(self.load_more_error, |footer| {
                                footer.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().danger)
                                        .child("Could not load more items."),
                                )
                            })
                            .when(self.load_more_error, |footer| {
                                footer.child(
                                    Button::new("load-more-history")
                                        .ghost()
                                        .small()
                                        .icon(LucideIcon::RefreshCw)
                                        .label("Retry")
                                        .disabled(self.is_loading_more)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.projects_results.load_more_data(cx)
                                        })),
                                )
                            }),
                    )
                })
                .into_any_element()
        };

        let rename_target = self.rename_target.clone();
        let rename_busy = self.rename_busy;
        let rename_error = self.rename_error.clone();

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(content)
            .when_some(rename_target, |view, id| {
                let title = self
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| item.title.clone())
                    .unwrap_or_default();
                view.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui_kit::black().opacity(0.45))
                        .child(
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
                                        .child(format!("Rename \"{title}\"")),
                                )
                                .child(Input::new(&self.rename_input_state).focus_ring(true))
                                .when_some(rename_error, |dialog, error| {
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
                                            Button::new("history-rename-cancel")
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                                .disabled(rename_busy)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.projects_results.rename_target = None;
                                                    this.projects_results.rename_error = None;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("history-rename-save")
                                                .primary()
                                                .small()
                                                .label(if rename_busy {
                                                    "Saving…"
                                                } else {
                                                    "Save"
                                                })
                                                .disabled(rename_busy)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.projects_results.save_rename(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            })
    }
}

const PROJECTS_LOAD_MORE_THRESHOLD_PX: f32 = 720.0;

fn should_load_more(max_offset: f32, next_scroll_y: f32) -> bool {
    max_offset > 0.0 && -next_scroll_y >= max_offset - PROJECTS_LOAD_MORE_THRESHOLD_PX
}

fn next_search_index(
    item_count: usize,
    current_index: Option<usize>,
    direction: i32,
) -> Option<usize> {
    if item_count == 0 {
        return None;
    }

    match (current_index.filter(|index| *index < item_count), direction) {
        (None, 1) => Some(0),
        (None, -1) => Some(item_count - 1),
        (Some(index), 1) => Some((index + 1) % item_count),
        (Some(index), -1) => Some((index + item_count - 1) % item_count),
        _ => None,
    }
}

fn render_table_header(
    selection_mode: bool,
    show_project_badge: bool,
    cx: &Context<MainShell>,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .px_4()
        .py(px(10.0))
        .bg(cx.theme().secondary)
        .border_b_1()
        .border_color(cx.theme().border)
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .when(selection_mode, |header| header.child(div().w(px(40.0))))
        .child(div().flex_1().min_w_0().px_2().child("NAME"))
        .when(show_project_badge, |header| {
            header.child(div().w(px(160.0)).px_2().child("TAGS"))
        })
        .child(
            div()
                .flex()
                .items_center()
                .child(div().w(px(160.0)).px_2().child("DATE"))
                .child(div().w(px(96.0)).px_2().child("DURATION")),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_history_item(
    item: &HistoryRecordItem,
    view_mode: ViewMode,
    selection_mode: bool,
    is_selected: bool,
    show_project_badge: bool,
    projects: &[ProjectRecord],
    selected_ids: &[String],
    all_items: &[HistoryRecordItem],
    search_match: Option<&HistoryWorkspaceItemSearchMatch>,
    is_keyboard_active: bool,
    is_searching: bool,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let title = item.title.clone();
    let preview = if is_searching {
        search_match
            .map(|matched| matched.display_snippet.text.clone())
            .unwrap_or_else(|| item.excerpt.clone())
    } else if item.excerpt.trim().is_empty() {
        if item.record.status == HistoryItemStatus::Draft {
            "Interrupted live recording draft".to_string()
        } else {
            "No transcript".to_string()
        }
    } else {
        item.excerpt.clone()
    };
    let is_deleted = item.record.deleted_at.is_some();
    let project = item
        .record
        .project_id
        .as_deref()
        .and_then(|project_id| projects.iter().find(|project| project.id == project_id));
    let type_icon = history_icon(&item.record, cx);
    let title_text = search_match
        .and_then(|matched| matched.title_match.as_ref())
        .and_then(|range| utf16_range_to_bytes(&title, range.start, range.end))
        .map(|range| highlighted_text(&title, Some(range), cx))
        .unwrap_or_else(|| StyledText::new(title));
    let title_content = div()
        .flex()
        .items_center()
        .gap_2()
        .min_w_0()
        .when(selection_mode && view_mode == ViewMode::Cards, |title| {
            title.pl(px(28.0))
        })
        .child(type_icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .when(view_mode == ViewMode::Cards, |title| {
                    title.line_height(px(19.6)).line_clamp(2)
                })
                .when(view_mode != ViewMode::Cards, |title| title.truncate())
                .child(title_text),
        )
        .when(item.record.status == HistoryItemStatus::Draft, |title| {
            title.child(
                div()
                    .px_2()
                    .py(px(1.0))
                    .min_h(px(20.0))
                    .rounded_full()
                    .bg(cx.theme().warning.opacity(0.12))
                    .text_size(px(11.0))
                    .line_height(px(12.0))
                    .text_color(cx.theme().warning)
                    .child("Draft"),
            )
        });

    let mut row = div()
        .group("history-item-row")
        .id(ElementId::from(format!("history-item-{}", item.id)))
        .flex()
        .items_start()
        .gap_4()
        .when(view_mode == ViewMode::Cards, |row| {
            row.w(px(280.0))
                .flex_1()
                .h(px(220.0))
                .flex_col()
                .p_4()
                .gap_3()
                .rounded_md()
        })
        .when(view_mode == ViewMode::List, |row| {
            row.w_full().px_4().py(px(14.0)).border_b_1()
        })
        .when(view_mode == ViewMode::Table, |row| {
            row.w_full().items_center().px_4().py(px(10.0)).border_b_1()
        })
        .bg(if is_selected {
            cx.theme().primary.opacity(0.1)
        } else {
            cx.theme().popover
        })
        .border_1()
        .border_color(if is_selected {
            cx.theme().primary
        } else {
            cx.theme().border
        })
        .when(view_mode != ViewMode::Table, |row| row.rounded_md())
        .when(view_mode == ViewMode::Table, |row| {
            row.gap_0()
                .border_0()
                .border_b_1()
                .rounded_none()
                .border_color(cx.theme().border)
        })
        .hover(|style| {
            style
                .bg(cx.theme().secondary)
                .border_color(cx.theme().muted)
                .shadow_md()
        })
        .when(selection_mode, |row| {
            row.child(
                div()
                    .when(view_mode == ViewMode::Cards, |checkbox| {
                        checkbox
                            .absolute()
                            .top(px(14.0))
                            .left(px(14.0))
                            .w(px(16.0))
                            .h(px(16.0))
                    })
                    .w(if view_mode == ViewMode::Table {
                        px(40.0)
                    } else if view_mode == ViewMode::Cards {
                        px(16.0)
                    } else {
                        px(24.0)
                    })
                    .flex_shrink_0()
                    .child(
                        Icon::new(if is_selected {
                            LucideIcon::SquareCheck
                        } else {
                            LucideIcon::Square
                        })
                        .size(px(16.0))
                        .text_color(if is_selected {
                            cx.theme().primary
                        } else {
                            cx.theme().muted
                        }),
                    ),
            )
        });
    row = row.when(is_keyboard_active, |row| {
        row.border_color(cx.theme().primary).shadow_md()
    });

    if view_mode == ViewMode::Table {
        row = row.child(
            div()
                .flex()
                .items_center()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .flex_1()
                        .min_w_0()
                        .line_clamp(1)
                        .px_2()
                        .child(title_content)
                        .when(
                            !preview.trim().is_empty()
                                && (is_searching || !item.excerpt.trim().is_empty()),
                            |title| {
                                title.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .line_clamp(1)
                                        .child(highlighted_preview(
                                            &preview,
                                            search_match,
                                            is_searching,
                                            cx,
                                        )),
                                )
                            },
                        ),
                )
                .when(show_project_badge, |row| {
                    row.child(div().w(px(160.0)).px_2().child(project_badge(project, cx)))
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .w(px(160.0))
                                .px_2()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(Icon::new(LucideIcon::CalendarDays).size(px(12.0)))
                                .child(item.date.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .w(px(96.0))
                                .px_2()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(Icon::new(LucideIcon::Clock3).size(px(12.0)))
                                .child(item.duration.clone()),
                        ),
                ),
        );
    } else {
        row = row.child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_3()
                        .when(view_mode == ViewMode::Cards, |header| {
                            header
                                .flex_col()
                                .gap_2()
                                .when(!selection_mode, |header| header.pr_6())
                        })
                        .child(title_content)
                        .when(show_project_badge, |row| {
                            row.child(project_badge(project, cx))
                        }),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(cx.theme().muted_foreground)
                        .line_height(px(21.0))
                        .line_clamp(if view_mode == ViewMode::Cards { 3 } else { 2 })
                        .child(highlighted_preview(
                            &preview,
                            search_match,
                            is_searching,
                            cx,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .mt_auto()
                        .text_size(px(12.6))
                        .text_color(cx.theme().muted)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(Icon::new(LucideIcon::CalendarDays).size(px(12.0)))
                                .child(item.date.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(Icon::new(LucideIcon::Clock3).size(px(12.0)))
                                .child(item.duration.clone()),
                        ),
                ),
        );
    }

    if !selection_mode {
        let rename_id = item.id.clone();
        let delete_id = item.id.clone();
        row = row.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .when(view_mode == ViewMode::Cards, |actions| {
                    actions.absolute().top_3().right_3()
                })
                .when(view_mode != ViewMode::Cards, |actions| {
                    actions.ml_auto().flex_shrink_0()
                })
                .when(!is_deleted, |actions| {
                    actions.child(
                        Button::new(ElementId::Name(
                            format!("history-rename-{}", item.id).into(),
                        ))
                        .ghost()
                        .small()
                        .icon(LucideIcon::Pencil)
                        .tooltip("Rename")
                        .accessibility_label(format!("Rename {}", item.title))
                        .when(!is_selected, |button| {
                            button
                                .invisible()
                                .group_hover("history-item-row", |button| button.visible())
                        })
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.projects_results
                                    .begin_rename(rename_id.clone(), window, cx);
                            },
                        )),
                    )
                })
                .child(
                    Button::new(ElementId::Name(
                        format!("history-delete-{}", item.id).into(),
                    ))
                    .ghost()
                    .small()
                    .icon(LucideIcon::Trash)
                    .tooltip("Delete")
                    .accessibility_label(format!("Delete {}", item.title))
                    .when(!is_selected, |button| {
                        button
                            .invisible()
                            .group_hover("history-item-row", |button| button.visible())
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.selection_bar
                            .request_delete_item(delete_id.clone(), cx);
                    })),
                ),
        );
    }

    if !selection_mode && !is_deleted {
        let open_id = item.id.clone();
        row = row.on_click(cx.listener(move |this, _, _, cx| {
            this.projects_results.select_item(open_id.clone());
            this.open_history_item(open_id.clone(), cx);
        }));
    } else if selection_mode {
        let toggle_id = item.id.clone();
        row = row.on_click(cx.listener(move |this, _, _, cx| {
            this.projects_results
                .toggle_selected_item(toggle_id.clone());
            cx.notify();
        }));
    }

    let item_id = item.id.clone();
    let item_title = item.title.clone();
    let item_excerpt = item.excerpt.clone();
    let shell = cx.entity().downgrade();
    let selected_live_drafts = selected_ids.iter().any(|selected_id| {
        all_items
            .iter()
            .find(|candidate| candidate.id == *selected_id)
            .is_some_and(|candidate| {
                candidate.record.status == HistoryItemStatus::Draft
                    && candidate.record.draft_source
                        == Some(sona_core::history::HistoryDraftSource::LiveRecord)
            })
    });
    let selected_copy_items = selected_ids
        .iter()
        .filter_map(|selected_id| {
            all_items
                .iter()
                .find(|candidate| candidate.id == *selected_id)
                .map(|candidate| {
                    (
                        candidate.id.clone(),
                        candidate.title.clone(),
                        candidate.excerpt.clone(),
                    )
                })
        })
        .collect::<Vec<_>>();
    let is_batch = selected_ids.len() > 1 && selected_ids.contains(&item_id);
    let all_visible_selected = !all_items.is_empty() && selected_ids.len() == all_items.len();
    let is_draft = item.record.status == HistoryItemStatus::Draft;
    let has_project = item.record.project_id.is_some();
    let is_live_draft = is_draft
        && item.record.draft_source == Some(sona_core::history::HistoryDraftSource::LiveRecord);
    let selected_ids = selected_ids.to_vec();
    row.context_menu(move |menu, _, cx| {
        let mut menu = menu;
        if !is_batch && !selected_ids.is_empty() && !selected_ids.contains(&item_id) {
            let _ = shell.update(cx, |shell, cx| {
                shell.projects_results.clear_selection();
                shell.projects_toolbar.is_selection_active = false;
                cx.notify();
            });
        }
        if is_batch && is_deleted {
            let restore_shell = shell.clone();
            let restore_ids = selected_ids.clone();
            menu = menu.item(
                PopupMenuItem::new(format!("Restore selected ({})", selected_ids.len()))
                    .icon(Icon::new(LucideIcon::RotateCcw))
                    .on_click(move |_, _, cx| {
                        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
                        let ids = restore_ids.clone();
                        restore_shell
                            .update(cx, move |_shell, cx| {
                                let history = history.clone();
                                cx.spawn(async move |this, cx: &mut AsyncApp| {
                                    let result = history
                                        .mutation_db(move |service| {
                                            service
                                                .restore_items(HistoryRestoreItemsRequest { ids })
                                        })
                                        .await;
                                    this.update(cx, |shell, cx| {
                                        if result.is_ok() {
                                            shell.projects_results.clear_selection();
                                            shell.projects_toolbar.is_selection_active = false;
                                            shell.projects_results.load_data(cx);
                                            shell.project_selector.load_data(cx);
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                                })
                                .detach();
                            })
                            .ok();
                    }),
            );
            let clear_shell = shell.clone();
            menu = menu.item(
                PopupMenuItem::new("Clear Selection")
                    .icon(Icon::new(LucideIcon::X))
                    .on_click(move |_, _, cx| {
                        clear_shell
                            .update(cx, |shell, cx| {
                                shell.projects_results.clear_selection();
                                shell.projects_toolbar.is_selection_active = false;
                                cx.notify();
                            })
                            .ok();
                    }),
            );
            menu = menu.separator();
            let purge_shell = shell.clone();
            menu = menu.item(
                PopupMenuItem::new(format!("Delete selected ({})", selected_ids.len()))
                    .icon(Icon::new(LucideIcon::Trash))
                    .on_click(move |_, _, cx| {
                        purge_shell
                            .update(cx, |shell, cx| {
                                shell.projects_toolbar.is_selection_active = true;
                                shell.selection_bar.request_delete(&shell.projects_results.selected_ids, cx);
                            })
                            .ok();
                    }),
            );
        } else if is_batch {
            let assign_shell = shell.clone();
            let assign_ids = selected_ids.clone();
            menu = menu.item(
                PopupMenuItem::new(format!("Assign Project ({})", selected_ids.len()))
                    .icon(Icon::new(LucideIcon::Tags))
                    .disabled(selected_live_drafts)
                    .on_click(move |_, _, cx| {
                        let _ = assign_shell.update(cx, |shell, cx| {
                            shell.projects_results.selected_ids = assign_ids.to_vec();
                            shell.projects_toolbar.is_selection_active = true;
                            shell.selection_bar.open_assignment(cx);
                        });
                    }),
            );
            let inbox_shell = shell.clone();
            let inbox_ids = selected_ids.clone();
            menu = menu.item(
                PopupMenuItem::new(format!("Move to Inbox ({})", selected_ids.len()))
                    .icon(Icon::new(LucideIcon::Inbox))
                    .disabled(selected_live_drafts)
                    .on_click(move |_, _, cx| {
                        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
                        let ids = inbox_ids.clone();
                        inbox_shell
                            .update(cx, move |_shell, cx| {
                                let history = history.clone();
                                cx.spawn(async move |this, cx: &mut AsyncApp| {
                                    let result = history
                                        .mutation_db(move |service| {
                                            service.update_project_assignments(
                                                HistoryUpdateProjectAssignmentsRequest {
                                                    ids,
                                                    project_id: None,
                                                },
                                            )
                                        })
                                        .await;
                                    this.update(cx, |shell, cx| {
                                        if result.is_ok() {
                                            shell.projects_results.clear_selection();
                                            shell.projects_toolbar.is_selection_active = false;
                                            shell.projects_results.load_data(cx);
                                            shell.project_selector.load_data(cx);
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                                })
                                .detach();
                            })
                            .ok();
                    }),
            );
            menu = menu.separator();
            let copy_shell = shell.clone();
            let copy_items = selected_copy_items.clone();
            menu = menu.item(
                PopupMenuItem::new(format!("Copy Transcripts ({})", copy_items.len()))
                    .icon(Icon::new(LucideIcon::Copy))
                    .on_click(move |_, _, cx| {
                        copy_history_transcripts(copy_shell.clone(), copy_items.clone(), true, cx);
                    }),
            );
            menu = menu.separator();
            let selection_shell = shell.clone();
            menu = menu.item(
                PopupMenuItem::new(if all_visible_selected {
                    "Clear Selection"
                } else {
                    "Select All"
                })
                .icon(Icon::new(if all_visible_selected {
                    LucideIcon::X
                } else {
                    LucideIcon::ListChecks
                }))
                .on_click(move |_, _, cx| {
                    selection_shell
                        .update(cx, |shell, cx| {
                            if all_visible_selected {
                                shell.projects_results.clear_selection();
                                shell.projects_toolbar.is_selection_active = false;
                            } else {
                                shell.projects_results.toggle_select_all_visible();
                            }
                            cx.notify();
                        })
                        .ok();
                }),
            );
            let delete_shell = shell.clone();
            menu = menu.item(
                PopupMenuItem::new(format!("Delete selected ({})", selected_ids.len()))
                    .icon(Icon::new(LucideIcon::Trash))
                    .disabled(selected_live_drafts)
                    .on_click(move |_, _, cx| {
                        delete_shell
                            .update(cx, |shell, cx| {
                                shell.projects_toolbar.is_selection_active = true;
                                shell.selection_bar.request_delete(&shell.projects_results.selected_ids, cx);
                            })
                            .ok();
                    }),
            );
        } else if is_deleted {
            let restore_shell = shell.clone();
            let restore_id = item_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Restore")
                    .icon(Icon::new(LucideIcon::RotateCcw))
                    .on_click(move |_, _, cx| {
                        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
                        let id = restore_id.clone();
                        restore_shell
                            .update(cx, move |_shell, cx| {
                                let history = history.clone();
                                cx.spawn(async move |this, cx: &mut AsyncApp| {
                                    let result = history
                                        .mutation_db(move |service| {
                                            service.restore_items(HistoryRestoreItemsRequest {
                                                ids: vec![id],
                                            })
                                        })
                                        .await;
                                    this.update(cx, |shell, cx| {
                                        if result.is_ok() {
                                            shell.projects_results.load_data(cx);
                                            shell.project_selector.load_data(cx);
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                                })
                                .detach();
                            })
                            .ok();
                    }),
            );
            let purge_shell = shell.clone();
            let purge_id = item_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Delete Permanently")
                    .icon(Icon::new(LucideIcon::Trash))
                    .on_click(move |_, _, cx| {
                        purge_shell
                            .update(cx, |shell, cx| {
                                shell
                                    .selection_bar
                                    .request_delete_item(purge_id.clone(), cx);
                            })
                            .ok();
                    }),
            );
        } else {
            let open_shell = shell.clone();
            let open_id = item_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Open")
                    .icon(Icon::new(LucideIcon::FolderOpen))
                    .on_click(move |_, _, cx| {
                        open_shell
                            .update(cx, |shell, cx| {
                                shell.projects_results.select_item(open_id.clone());
                                shell.open_history_item(open_id.clone(), cx);
                            })
                            .ok();
                    }),
            );
            menu = menu.separator();
            let copy_title = item_title.clone();
            menu = menu.item(
                PopupMenuItem::new("Copy Title")
                    .icon(Icon::new(LucideIcon::FileText))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_title.clone()));
                    }),
            );
            let transcript_shell = shell.clone();
            let transcript_id = item_id.clone();
            let transcript_title = item_title.clone();
            let transcript_excerpt = item_excerpt.clone();
            menu = menu.item(
                PopupMenuItem::new("Copy Transcript")
                    .icon(Icon::new(LucideIcon::Copy))
                    .on_click(move |_, _, cx| {
                        copy_history_transcripts(
                            transcript_shell.clone(),
                            vec![(
                                transcript_id.clone(),
                                transcript_title.clone(),
                                transcript_excerpt.clone(),
                            )],
                            false,
                            cx,
                        );
                    }),
            );
            menu = menu.separator();
            let rename_shell = shell.clone();
            let rename_id = item_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Rename")
                    .icon(Icon::new(LucideIcon::Pencil))
                    .disabled(is_live_draft)
                    .on_click(move |_, window, cx| {
                        rename_shell
                            .update(cx, |shell, cx| {
                                shell
                                    .projects_results
                                    .begin_rename(rename_id.clone(), window, cx);
                            })
                            .ok();
                    }),
            );
            let assign_shell = shell.clone();
            let assign_id = item_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Assign Project")
                    .icon(Icon::new(LucideIcon::Tags))
                    .disabled(is_live_draft)
                    .on_click(move |_, _, cx| {
                        let _ = assign_shell.update(cx, |shell, cx| {
                            shell.projects_results.selected_ids = vec![assign_id.clone()];
                            shell.projects_toolbar.is_selection_active = true;
                            shell.selection_bar.open_assignment(cx);
                        });
                    }),
            );
            if has_project {
                let inbox_shell = shell.clone();
                let inbox_id = item_id.clone();
                menu = menu.item(
                    PopupMenuItem::new("Move to Inbox")
                        .icon(Icon::new(LucideIcon::Inbox))
                        .disabled(is_live_draft)
                        .on_click(move |_, _, cx| {
                            let history = cx.global::<GlobalDesktopServices>().0.history.clone();
                            let id = inbox_id.clone();
                            inbox_shell
                                .update(cx, move |_shell, cx| {
                                    let history = history.clone();
                                    cx.spawn(async move |this, cx: &mut AsyncApp| {
                                        let result = history
                                            .mutation_db(move |service| {
                                                service.update_project_assignments(
                                                    HistoryUpdateProjectAssignmentsRequest {
                                                        ids: vec![id],
                                                        project_id: None,
                                                    },
                                                )
                                            })
                                            .await;
                                        this.update(cx, |shell, cx| {
                                            if result.is_ok() {
                                                shell.projects_results.load_data(cx);
                                                shell.project_selector.load_data(cx);
                                            }
                                            cx.notify();
                                        })
                                        .ok();
                                    })
                                    .detach();
                                })
                                .ok();
                        }),
                );
            }
            menu = menu.separator();
            let reveal_shell = shell.clone();
            let reveal_id = item_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Show in File Explorer")
                    .icon(Icon::new(LucideIcon::FolderSearch))
                    .on_click(move |_, _, cx| {
                        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
                        let id = reveal_id.clone();
                        reveal_shell
                            .update(cx, move |_shell, cx| {
                                let history = history.clone();
                                cx.spawn(async move |this, cx: &mut AsyncApp| {
                                    let path = history
                                        .file_task(move |store| store.resolve_audio_path(&id))
                                        .await
                                        .ok()
                                        .flatten();
                                    let result = match path {
                                        Some(path) => {
                                            crate::platform::storage::location::open_storage_path(path)
                                                .or_else(|_| {
                                                    history
                                                        .ensure_history_folder()
                                                        .and_then(|path| {
                                                            crate::platform::opener::open_path(path)
                                                                .map_err(|error| error.to_string())
                                                        })
                                                })
                                        }
                                        None => match history.ensure_history_folder() {
                                            Ok(path) => crate::platform::opener::open_path(path)
                                                .map_err(|error| error.to_string()),
                                            Err(error) => Err(error),
                                        },
                                    };
                                    if let Err(error) = result {
                                        this.update(cx, |shell, cx| {
                                            shell.global_dialog.update(cx, |d, _| {
                                                d.show(
                                                    "Could Not Reveal History Item",
                                                    error,
                                                    crate::views::dialogs::global_dialog::DialogKind::Alert,
                                                );
                                            });
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                })
                                .detach();
                            })
                            .ok();
                    }),
            );
            menu = menu.separator();
            let delete_shell = shell.clone();
            let delete_id = item_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Delete")
                    .icon(Icon::new(LucideIcon::Trash))
                    .disabled(is_live_draft)
                    .on_click(move |_, _, cx| {
                        delete_shell
                            .update(cx, |shell, cx| {
                                shell
                                    .selection_bar
                                    .request_delete_item(delete_id.clone(), cx);
                            })
                            .ok();
                    }),
            );
        }
        menu
    })
    .into_any_element()
}

fn copy_history_transcripts(
    shell: gpui::WeakEntity<MainShell>,
    items: Vec<(String, String, String)>,
    include_titles: bool,
    cx: &mut gpui::App,
) {
    let history = cx.global::<GlobalDesktopServices>().0.history.clone();
    let _ = shell.update(cx, move |_shell, cx| {
        let history = history.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let mut transcripts = Vec::new();
            for (id, title, preview) in items {
                let text = match history
                    .query_db(move |query| query.load_transcript(&id))
                    .await
                {
                    Ok(Some(segments)) => segments
                        .into_iter()
                        .map(|segment| segment.text)
                        .filter(|text| !text.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => String::new(),
                };
                let text = if text.trim().is_empty() {
                    if preview.trim().is_empty() {
                        title.clone()
                    } else {
                        preview
                    }
                } else {
                    text.trim().to_string()
                };
                if include_titles {
                    transcripts.push(format!("=== {title} ===\n{text}"));
                } else {
                    transcripts.push(text);
                }
            }
            let text = transcripts.join("\n\n");
            if !text.is_empty() {
                this.update(cx, move |_shell, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                })
                .ok();
            }
        })
        .detach();
    });
}

fn history_icon(record: &HistoryItemRecord, cx: &Context<MainShell>) -> AnyElement {
    if let Some(icon) = record.icon.as_deref() {
        if let Some(system_icon) = icon.strip_prefix("system:") {
            let icon = match system_icon {
                "mic" => Some(LucideIcon::Mic),
                "file" => Some(LucideIcon::FileText),
                "folder" => Some(LucideIcon::Folder),
                "code" => Some(LucideIcon::Code),
                _ => None,
            };
            if let Some(icon) = icon {
                return Icon::new(icon)
                    .size(px(16.0))
                    .text_color(cx.theme().muted)
                    .into_any_element();
            }
        } else {
            return div()
                .text_size(px(14.0))
                .child(icon.to_string())
                .into_any_element();
        }
    }
    let icon = match record.kind {
        HistoryItemKind::Batch => LucideIcon::FileText,
        HistoryItemKind::Recording => LucideIcon::Mic,
    };
    Icon::new(icon)
        .size(px(16.0))
        .text_color(cx.theme().muted)
        .into_any_element()
}

fn project_badge(project: Option<&ProjectRecord>, cx: &Context<MainShell>) -> AnyElement {
    let inbox = project.is_none();
    let (icon, name, color) = project
        .map(|project| {
            (
                project.icon.as_deref(),
                project.name.as_str(),
                project_color(project.color.as_deref()),
            )
        })
        .unwrap_or((None, "Inbox", rgb(0x64748b)));
    let color_hsla = Hsla::from(color);
    div()
        .flex()
        .items_center()
        .gap(px(5.0))
        .max_w(px(180.0))
        .px_2()
        .h(px(24.0))
        .rounded_full()
        .border_1()
        .border_color(if inbox {
            cx.theme().border
        } else {
            color_hsla.opacity(0.28)
        })
        .bg(if inbox {
            cx.theme().secondary
        } else {
            color_hsla.opacity(0.1)
        })
        .text_size(px(11.5))
        .text_color(if inbox {
            cx.theme().muted_foreground
        } else {
            cx.theme().foreground
        })
        .child(if inbox {
            Icon::new(LucideIcon::Inbox)
                .size(px(12.0))
                .into_any_element()
        } else {
            project_icon(icon, color)
        })
        .child(div().flex_1().min_w_0().truncate().child(name.to_string()))
        .into_any_element()
}

fn project_color(color: Option<&str>) -> Rgba {
    let Some(hex) = color.and_then(parse_project_color) else {
        return rgb(0x64748b);
    };
    rgb(hex)
}

fn parse_project_color(color: &str) -> Option<u32> {
    let hex = color.trim().strip_prefix('#').unwrap_or(color.trim());
    match hex.len() {
        3 => {
            let mut expanded = 0_u32;
            for character in hex.chars() {
                let nibble = character.to_digit(16)?;
                expanded = (expanded << 8) | (nibble << 4) | nibble;
            }
            Some(expanded)
        }
        6 => u32::from_str_radix(hex, 16).ok(),
        _ => None,
    }
}

fn project_icon(icon: Option<&str>, color: Rgba) -> AnyElement {
    let icon = if let Some(icon) = icon {
        if let Some(system_icon) = icon.strip_prefix("system:") {
            let icon = match system_icon {
                "mic" => LucideIcon::Mic,
                "file" => LucideIcon::FileText,
                "code" => LucideIcon::Code,
                _ => LucideIcon::Folder,
            };
            Icon::new(icon)
                .size(px(12.0))
                .text_color(color)
                .into_any_element()
        } else {
            div()
                .text_size(px(11.0))
                .child(icon.to_string())
                .into_any_element()
        }
    } else {
        Icon::new(LucideIcon::Folder)
            .size(px(12.0))
            .text_color(color)
            .into_any_element()
    };
    div()
        .w(px(16.0))
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .child(icon)
        .into_any_element()
}

fn history_item(item: HistoryItemRecord) -> HistoryRecordItem {
    let total_seconds = item.duration.max(0.0) as u64;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let duration = format!("{minutes}:{seconds:02}");
    let date = chrono::Utc
        .timestamp_millis_opt(item.timestamp as i64)
        .single()
        .map(|timestamp| timestamp.with_timezone(&chrono::Local))
        .map(format_relative_date)
        .unwrap_or_default();
    HistoryRecordItem {
        record: item.clone(),
        id: item.id,
        title: item.title,
        duration,
        date,
        excerpt: item.preview_text,
    }
}

fn format_relative_date(timestamp: chrono::DateTime<chrono::Local>) -> String {
    let seconds = timestamp.timestamp() - chrono::Local::now().timestamp();
    let absolute = seconds.abs();
    if absolute < 60 {
        return "just now".to_string();
    }
    if absolute < 3_600 {
        let minutes = (absolute + 30) / 60;
        return if seconds < 0 {
            format!("{minutes} minutes ago")
        } else {
            format!("in {minutes} minutes")
        };
    }
    if absolute < 86_400 {
        let hours = (absolute + 1_800) / 3_600;
        return if seconds < 0 {
            format!("{hours} hours ago")
        } else {
            format!("in {hours} hours")
        };
    }
    if absolute < 604_800 {
        let days = (absolute + 43_200) / 86_400;
        return match (seconds < 0, days) {
            (true, 1) => "yesterday".to_string(),
            (false, 1) => "tomorrow".to_string(),
            (true, _) => format!("{days} days ago"),
            (false, _) => format!("in {days} days"),
        };
    }
    timestamp.format("%b %-d").to_string()
}

fn highlighted_preview(
    text: &str,
    search_match: Option<&HistoryWorkspaceItemSearchMatch>,
    is_searching: bool,
    cx: &Context<MainShell>,
) -> StyledText {
    let range = search_match.filter(|_| is_searching).and_then(|matched| {
        utf16_range_to_bytes(
            text,
            matched.display_snippet.highlight_start,
            matched.display_snippet.highlight_end,
        )
    });
    highlighted_text(text, range, cx)
}

fn highlighted_text(
    text: &str,
    range: Option<std::ops::Range<usize>>,
    cx: &Context<MainShell>,
) -> StyledText {
    match range {
        Some(range) => StyledText::new(text.to_string()).with_highlights([(
            range,
            HighlightStyle {
                background_color: Some(cx.theme().warning.opacity(0.25)),
                ..Default::default()
            },
        )]),
        None => StyledText::new(text.to_string()),
    }
}

fn utf16_range_to_bytes(text: &str, start: usize, end: usize) -> Option<std::ops::Range<usize>> {
    if start >= end {
        return None;
    }

    let mut utf16_offset = 0;
    let mut byte_start = None;
    let mut byte_end = None;
    for (byte_index, character) in text.char_indices() {
        if utf16_offset == start {
            byte_start = Some(byte_index);
        }
        if utf16_offset == end {
            byte_end = Some(byte_index);
            break;
        }
        utf16_offset += character.len_utf16();
    }
    if byte_start.is_none() && utf16_offset == start {
        byte_start = Some(text.len());
    }
    if byte_end.is_none() && utf16_offset == end {
        byte_end = Some(text.len());
    }

    let range = byte_start?..byte_end?;
    (range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end))
    .then_some(range)
}

#[cfg(test)]
mod projects_results_tests {
    use super::{next_search_index, parse_project_color, should_load_more, utf16_range_to_bytes};

    #[test]
    fn loads_next_page_inside_react_viewport_prefetch_range() {
        assert!(should_load_more(2_000.0, -1_300.0));
        assert!(!should_load_more(2_000.0, -1_200.0));
    }

    #[test]
    fn does_not_load_when_content_cannot_scroll() {
        assert!(!should_load_more(0.0, 0.0));
    }

    #[test]
    fn search_navigation_wraps_in_both_directions() {
        assert_eq!(next_search_index(3, None, 1), Some(0));
        assert_eq!(next_search_index(3, Some(2), 1), Some(0));
        assert_eq!(next_search_index(3, None, -1), Some(2));
        assert_eq!(next_search_index(3, Some(0), -1), Some(2));
        assert_eq!(next_search_index(0, None, 1), None);
    }

    #[test]
    fn search_highlight_ranges_convert_utf16_offsets_to_utf8_bytes() {
        assert_eq!(utf16_range_to_bytes("A中B", 1, 2), Some(1..4));
        assert_eq!(utf16_range_to_bytes("a😀b", 1, 3), Some(1..5));
        assert_eq!(utf16_range_to_bytes("a😀b", 2, 3), None);
    }

    #[test]
    fn project_colors_accept_three_and_six_digit_hex_values() {
        assert_eq!(parse_project_color("#abc"), Some(0xaabbcc));
        assert_eq!(parse_project_color("#12aBcF"), Some(0x12abcf));
        assert_eq!(parse_project_color("invalid"), None);
    }
}
