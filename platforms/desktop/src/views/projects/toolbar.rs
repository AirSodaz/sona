//! Projects Toolbar with search, filters, and view mode switcher.

use crate::app::GlobalDesktopServices;
use crate::theme::tokens::*;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::IndexPath;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};
use sona_core::history::HistoryWorkspaceScope;
use sona_core::history::{
    HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceSortOrder,
};
use sona_core::project::ProjectRecord;

use super::super::shell::MainShell;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ViewMode {
    Cards,
    List,
    Table,
}

pub struct ProjectsToolbar {
    pub search_query: String,
    pub view_mode: ViewMode,
    pub is_selection_active: bool,
    pub is_filter_menu_open: bool,
    pub current_placeholder: String,
    pub search_state: Entity<InputState>,
    _search_subscription: Subscription,
    sort_select: Entity<SelectState<Vec<&'static str>>>,
    type_select: Entity<SelectState<Vec<&'static str>>>,
    date_select: Entity<SelectState<Vec<&'static str>>>,
    _filter_subscriptions: [Subscription; 3],
}
impl ProjectsToolbar {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
        let view_mode = view_mode_from_config(
            cx.global::<GlobalDesktopServices>()
                .0
                .config
                .load()
                .ok()
                .flatten(),
        );
        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search Inbox..."));
        let search_subscription =
            cx.subscribe(&search_state, |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = state.read(cx).value().to_string();
                    shell.projects_results.set_search_query(query.clone(), cx);
                    shell.projects_toolbar.search_query = query;
                }
            });
        let sort_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    "Newest first",
                    "Oldest first",
                    "Longest first",
                    "Shortest first",
                    "Title A-Z",
                ],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let type_select = cx.new(|cx| {
            SelectState::new(
                vec!["All types", "Recordings", "Batch imports"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let date_select = cx.new(|cx| {
            SelectState::new(
                vec!["Any time", "Today", "Last 7 days", "Last 30 days"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let sort_subscription = cx.subscribe(
            &sort_select,
            |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let order = match *value {
                        "Oldest first" => HistoryWorkspaceSortOrder::Oldest,
                        "Longest first" => HistoryWorkspaceSortOrder::DurationDesc,
                        "Shortest first" => HistoryWorkspaceSortOrder::DurationAsc,
                        "Title A-Z" => HistoryWorkspaceSortOrder::TitleAsc,
                        _ => HistoryWorkspaceSortOrder::Newest,
                    };
                    shell.projects_results.set_sort_order(order, cx);
                }
            },
        );
        let type_subscription = cx.subscribe(
            &type_select,
            |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let filter = match *value {
                        "Recordings" => HistoryWorkspaceFilterType::Recording,
                        "Batch imports" => HistoryWorkspaceFilterType::Batch,
                        _ => HistoryWorkspaceFilterType::All,
                    };
                    shell.projects_results.set_filter_type(filter, cx);
                }
            },
        );
        let date_subscription = cx.subscribe(
            &date_select,
            |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let filter = match *value {
                        "Today" => HistoryWorkspaceDateFilter::Today,
                        "Last 7 days" => HistoryWorkspaceDateFilter::Week,
                        "Last 30 days" => HistoryWorkspaceDateFilter::Month,
                        _ => HistoryWorkspaceDateFilter::All,
                    };
                    shell.projects_results.set_date_filter(filter, cx);
                }
            },
        );
        Self {
            search_query: String::new(),
            view_mode,
            is_selection_active: false,
            is_filter_menu_open: false,
            current_placeholder: "Search Inbox...".to_string(),
            search_state,
            _search_subscription: search_subscription,
            sort_select,
            type_select,
            date_select,
            _filter_subscriptions: [sort_subscription, type_subscription, date_subscription],
        }
    }

    pub fn update_scope(
        &mut self,
        scope: &HistoryWorkspaceScope,
        browse_project: Option<&ProjectRecord>,
        window: &mut Window,
        cx: &mut Context<MainShell>,
    ) {
        let formatted = match scope {
            HistoryWorkspaceScope::Inbox => "Search Inbox...".to_string(),
            HistoryWorkspaceScope::All => "Search All Items...".to_string(),
            HistoryWorkspaceScope::Trash => "Search Trash...".to_string(),
            HistoryWorkspaceScope::Project { .. } => {
                if let Some(p) = browse_project {
                    format!("Search in {}...", p.name)
                } else {
                    "Search project...".to_string()
                }
            }
        };
        if self.current_placeholder != formatted {
            self.current_placeholder = formatted.clone();
            self.search_state.update(cx, |state, cx| {
                state.set_placeholder(formatted, window, cx);
            });
        }
    }

    pub fn sync_filter_selects(&mut self, window: &mut Window, cx: &mut Context<MainShell>) {
        self.type_select.update(cx, |state, cx| {
            state.set_selected_value(&"All types", window, cx);
        });
        self.date_select.update(cx, |state, cx| {
            state.set_selected_value(&"Any time", window, cx);
        });
    }

    pub fn reset_filters(&mut self, window: &mut Window, cx: &mut Context<MainShell>) {
        self.search_state
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.search_query.clear();
        self.sync_filter_selects(window, cx);
        self.sort_select.update(cx, |state, cx| {
            state.set_selected_value(&"Newest first", window, cx);
        });
    }
    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<MainShell>) {
        self.view_mode = mode;
        let config_service = cx.global::<GlobalDesktopServices>().0.config.clone();
        let stored_mode = match mode {
            ViewMode::Cards => "grid",
            ViewMode::List => "list",
            ViewMode::Table => "table",
        };
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut config = config_service
                .load()
                .ok()
                .flatten()
                .filter(Value::is_object)
                .unwrap_or_else(|| json!({}));
            config["projectsViewMode"] = json!(stored_mode);
            let _ = config_service.save(&config);
        })
        .detach();
    }

    pub fn toggle_selection(&mut self) {
        self.is_selection_active = !self.is_selection_active;
    }

    fn open_history_folder(&self, cx: &mut Context<MainShell>) {
        let history = cx.global::<GlobalDesktopServices>().0.history.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = match history.ensure_history_folder() {
                Ok(path) => crate::platform::opener::open_path_async(path)
                    .await
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                this.update(cx, |shell, cx| {
                    shell.global_dialog.update(cx, |d, _| {
                        d.show(
                            "Could Not Open History Folder",
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
    }

    pub fn render(
        &mut self,
        filter_type: HistoryWorkspaceFilterType,
        date_filter: HistoryWorkspaceDateFilter,
        filtered_count: usize,
        scoped_count: usize,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let mode = self.view_mode;
        let selection_active = self.is_selection_active;
        let filter_menu_open = self.is_filter_menu_open;
        let active_filter_count = usize::from(filter_type != HistoryWorkspaceFilterType::All)
            + usize::from(date_filter != HistoryWorkspaceDateFilter::All);
        let filter_hint = match (filter_type, date_filter) {
            (HistoryWorkspaceFilterType::Recording, HistoryWorkspaceDateFilter::All) => {
                "Recordings".to_string()
            }
            (HistoryWorkspaceFilterType::Batch, HistoryWorkspaceDateFilter::All) => {
                "Batch imports".to_string()
            }
            (HistoryWorkspaceFilterType::All, HistoryWorkspaceDateFilter::Today) => {
                "Today".to_string()
            }
            (HistoryWorkspaceFilterType::All, HistoryWorkspaceDateFilter::Week) => {
                "Last 7 days".to_string()
            }
            (HistoryWorkspaceFilterType::All, HistoryWorkspaceDateFilter::Month) => {
                "Last 30 days".to_string()
            }
            (HistoryWorkspaceFilterType::Recording, HistoryWorkspaceDateFilter::Today) => {
                "Recordings · Today".to_string()
            }
            (HistoryWorkspaceFilterType::Recording, HistoryWorkspaceDateFilter::Week) => {
                "Recordings · Last 7 days".to_string()
            }
            (HistoryWorkspaceFilterType::Recording, HistoryWorkspaceDateFilter::Month) => {
                "Recordings · Last 30 days".to_string()
            }
            (HistoryWorkspaceFilterType::Batch, HistoryWorkspaceDateFilter::Today) => {
                "Batch imports · Today".to_string()
            }
            (HistoryWorkspaceFilterType::Batch, HistoryWorkspaceDateFilter::Week) => {
                "Batch imports · Last 7 days".to_string()
            }
            (HistoryWorkspaceFilterType::Batch, HistoryWorkspaceDateFilter::Month) => {
                "Batch imports · Last 30 days".to_string()
            }
            _ => "Refine the current workspace view by type or time.".to_string(),
        };
        div()
            .relative()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            .w_full()
            .border_b_1()
            .border_color(COLOR_BORDER_LIGHT)
            .bg(COLOR_BG_PRIMARY_LIGHT)
            .px(px(24.0))
            .py(px(10.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_1()
                    .min_w(px(320.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_3()
                                    .w(px(260.0))
                                    .h(px(36.0))
                                    .rounded(RADIUS_MD)
                                    .bg(COLOR_BG_SECONDARY_LIGHT)
                                    .border_1()
                                    .border_color(COLOR_BORDER_LIGHT)
                                    .hover(|style| style.border_color(COLOR_BORDER_HOVER_LIGHT))
                                    .child(
                                        gpui_kit::component::Icon::new(LucideIcon::Search)
                                            .size(px(16.0))
                                            .text_color(COLOR_TEXT_MUTED_LIGHT),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .on_key_down(cx.listener(
                                                |this, event: &KeyDownEvent, window, cx| {
                                                    if event.is_held
                                                        || this.projects_toolbar.is_selection_active
                                                    {
                                                        return;
                                                    }

                                                    match event
                                                        .keystroke
                                                        .key
                                                        .to_ascii_lowercase()
                                                        .as_str()
                                                    {
                                                        "arrowdown" | "down" => {
                                                            this.projects_results
                                                                .navigate_search_result(
                                                                    1,
                                                                    this.projects_toolbar.view_mode,
                                                                    cx,
                                                                );
                                                            window.prevent_default();
                                                            cx.stop_propagation();
                                                        }
                                                        "arrowup" | "up" => {
                                                            this.projects_results
                                                                .navigate_search_result(
                                                                    -1,
                                                                    this.projects_toolbar.view_mode,
                                                                    cx,
                                                                );
                                                            window.prevent_default();
                                                            cx.stop_propagation();
                                                        }
                                                        "enter" => {
                                                            if let Some(id) = this
                                                                .projects_results
                                                                .active_search_result_id
                                                                .clone()
                                                            {
                                                                this.projects_results
                                                                    .active_search_result_id = None;
                                                                this.open_history_item(id, cx);
                                                                window.prevent_default();
                                                                cx.stop_propagation();
                                                            }
                                                        }
                                                        "escape" => {
                                                            if !this
                                                                .projects_toolbar
                                                                .search_query
                                                                .trim()
                                                                .is_empty()
                                                            {
                                                                this.projects_toolbar
                                                                    .search_state
                                                                    .update(cx, |state, cx| {
                                                                        state.set_value(
                                                                            "", window, cx,
                                                                        );
                                                                    });
                                                                this.projects_toolbar
                                                                    .search_query
                                                                    .clear();
                                                                this.projects_results
                                                                    .set_search_query(
                                                                        String::new(),
                                                                        cx,
                                                                    );
                                                            } else {
                                                                this.projects_results
                                                                    .active_search_result_id = None;
                                                                cx.notify();
                                                            }
                                                            window.prevent_default();
                                                            cx.stop_propagation();
                                                        }
                                                        _ => {}
                                                    }
                                                },
                                            ))
                                            .child(
                                                Input::new(&self.search_state)
                                                    .appearance(false)
                                                    .cleanable(true)
                                                    .small(),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .relative()
                                    .child(
                                        Button::new("btn-projects-filter")
                                            .ghost()
                                            .small()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .icon(LucideIcon::SlidersHorizontal)
                                            .border_1()
                                            .border_color(if filter_menu_open {
                                                COLOR_BORDER_HOVER_LIGHT
                                            } else {
                                                COLOR_BORDER_LIGHT
                                            })
                                            .bg(if filter_menu_open {
                                                COLOR_BG_HOVER_LIGHT
                                            } else {
                                                COLOR_BG_SECONDARY_LIGHT
                                            })
                                            .rounded(RADIUS_MD)
                                            .tooltip("Filter & Sort")
                                            .accessibility_label("Filter & Sort")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.projects_toolbar.is_filter_menu_open =
                                                    !this.projects_toolbar.is_filter_menu_open;
                                                cx.notify();
                                            })),
                                    )
                                    .when(active_filter_count > 0, |this| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .top(px(-3.0))
                                                .right(px(-3.0))
                                                .min_w(px(18.0))
                                                .h(px(18.0))
                                                .px(px(4.0))
                                                .rounded_full()
                                                .bg(cx.theme().accent)
                                                .text_size(px(10.4))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().background)
                                                .text_center()
                                                .child(active_filter_count.to_string()),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(1.0))
                            .px(px(2.0))
                            .py(px(2.0))
                            .rounded(RADIUS_MD)
                            .border_1()
                            .border_color(COLOR_BORDER_LIGHT)
                            .bg(COLOR_BG_SECONDARY_LIGHT)
                            .child(
                                Button::new("btn-view-list")
                                    .ghost()
                                    .small()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .icon(LucideIcon::List)
                                    .when(mode == ViewMode::List, |button| {
                                        button.bg(COLOR_BG_ELEVATED_LIGHT).shadow_sm()
                                    })
                                    .tooltip("List View")
                                    .accessibility_label("List View")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.projects_toolbar.set_view_mode(ViewMode::List, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-view-grid")
                                    .ghost()
                                    .small()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .icon(LucideIcon::LayoutGrid)
                                    .when(mode == ViewMode::Cards, |button| {
                                        button.bg(COLOR_BG_ELEVATED_LIGHT).shadow_sm()
                                    })
                                    .tooltip("Grid View")
                                    .accessibility_label("Grid View")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.projects_toolbar.set_view_mode(ViewMode::Cards, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-view-table")
                                    .ghost()
                                    .small()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .icon(LucideIcon::Columns3)
                                    .when(mode == ViewMode::Table, |button| {
                                        button.bg(COLOR_BG_ELEVATED_LIGHT).shadow_sm()
                                    })
                                    .tooltip("Table View")
                                    .accessibility_label("Table View")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.projects_toolbar.set_view_mode(ViewMode::Table, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .mx(px(4.0))
                                    .w(px(1.0))
                                    .h(px(18.0))
                                    .bg(COLOR_BORDER_LIGHT),
                            )
                            .child(
                                Button::new("btn-open-history-folder")
                                    .ghost()
                                    .small()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .icon(LucideIcon::Folder)
                                    .tooltip("Open File Directory")
                                    .accessibility_label("Open File Directory")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.projects_toolbar.open_history_folder(cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-select-mode")
                                    .ghost()
                                    .small()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .icon(LucideIcon::SquareCheck)
                                    .when(selection_active, |button| {
                                        button.bg(COLOR_BG_ELEVATED_LIGHT).shadow_sm()
                                    })
                                    .tooltip(if selection_active {
                                        "Cancel Select"
                                    } else {
                                        "Select"
                                    })
                                    .accessibility_label(if selection_active {
                                        "Cancel Select"
                                    } else {
                                        "Select"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.projects_toolbar.toggle_selection();
                                        this.projects_results.clear_selection();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when(filter_menu_open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(51.0))
                        .left(px(8.0))
                        .w(px(320.0))
                        .flex_col()
                        .gap(px(14.0))
                        .p(px(14.0))
                        .rounded(px(16.0))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .shadow_lg()
                        .child(
                            div()
                                .flex()
                                .items_start()
                                .justify_between()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_col()
                                        .child(
                                            div()
                                                .flex()
                                                .items_baseline()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child("Filter & Sort"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.8))
                                                        .text_color(cx.theme().muted)
                                                        .child(format!(
                                                            "{filtered_count} / {scoped_count}"
                                                        )),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .mt_1()
                                                .text_size(px(13.12))
                                                .text_color(cx.theme().muted)
                                                .child(filter_hint),
                                        ),
                                )
                                .child(
                                    Button::new("clear-project-filters")
                                        .ghost()
                                        .small()
                                        .label("Clear filters")
                                        .disabled(active_filter_count == 0)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.projects_toolbar.reset_filters(window, cx);
                                            this.projects_results.reset_filters(cx);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(11.52))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().muted)
                                        .child("SORT ITEMS"),
                                )
                                .child(
                                    Select::new(&self.sort_select)
                                        .placeholder("Sort items")
                                        .menu_width(px(290.0)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(11.52))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().muted)
                                        .child("FILTER BY TYPE"),
                                )
                                .child(
                                    Select::new(&self.type_select)
                                        .placeholder("Filter by type")
                                        .menu_width(px(290.0)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(11.52))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().muted)
                                        .child("FILTER BY DATE"),
                                )
                                .child(
                                    Select::new(&self.date_select)
                                        .placeholder("Filter by date")
                                        .menu_width(px(290.0)),
                                ),
                        ),
                )
            })
    }
}

fn view_mode_from_config(config: Option<Value>) -> ViewMode {
    match config
        .as_ref()
        .and_then(|config| config.get("projectsViewMode"))
        .and_then(Value::as_str)
    {
        Some("grid") => ViewMode::Cards,
        Some("table") => ViewMode::Table,
        _ => ViewMode::List,
    }
}

#[cfg(test)]
mod tests {
    use super::{ViewMode, view_mode_from_config};
    use serde_json::json;

    #[test]
    fn restores_react_projects_view_mode_from_config() {
        assert_eq!(
            view_mode_from_config(Some(json!({"projectsViewMode": "list"}))),
            ViewMode::List
        );
        assert_eq!(
            view_mode_from_config(Some(json!({"projectsViewMode": "grid"}))),
            ViewMode::Cards
        );
        assert_eq!(
            view_mode_from_config(Some(json!({"projectsViewMode": "table"}))),
            ViewMode::Table
        );
        assert_eq!(
            view_mode_from_config(Some(json!({"projectsViewMode": "invalid"}))),
            ViewMode::List
        );
        assert_eq!(view_mode_from_config(None), ViewMode::List);
    }
}
