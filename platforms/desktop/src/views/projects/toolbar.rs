//! Projects Toolbar with search, filters, and view mode switcher.

use crate::app::GlobalDesktopServices;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::FocusableExt as _;
use gpui_kit::component::IndexPath;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};
use sona_core::history::{
    HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceSortOrder,
};

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
            sort_select,
            type_select,
            date_select,
            _filter_subscriptions: [sort_subscription, type_subscription, date_subscription],
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
        self.sync_filter_selects(window, cx);
        self.sort_select.update(cx, |state, cx| {
            state.set_selected_value(&"Newest first", window, cx);
        });
        cx.entity()
            .update(cx, |shell, cx| shell.projects_results.reset_filters(cx));
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
                    shell.global_dialog.show(
                        "Could Not Open History Folder",
                        error,
                        crate::views::dialogs::global_dialog::DialogKind::Alert,
                    );
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn render(
        &mut self,
        search_state: &Entity<InputState>,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        let mode = self.view_mode;
        let selection_active = self.is_selection_active;
        let filter_menu_open = self.is_filter_menu_open;
        let filter_type = cx.entity().read(cx).projects_results.filter_type;
        let date_filter = cx.entity().read(cx).projects_results.date_filter;
        let (filtered_count, scoped_count) = {
            let results = cx.entity().read(cx);
            (
                results.projects_results.filtered_count,
                results.projects_results.scoped_count,
            )
        };
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
            .flex_wrap()
            .gap(px(16.0))
            .w_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
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
                                    .rounded(px(12.0))
                                    .bg(cx.theme().foreground.opacity(0.06))
                                    .hover(|style| style.bg(cx.theme().foreground.opacity(0.08)))
                                    .child(
                                        gpui_kit::component::Icon::new(LucideIcon::Search)
                                            .size(px(16.0))
                                            .text_color(cx.theme().muted),
                                    )
                                    .child(
                                        div().flex_1().min_w(px(0.0)).child(
                                            Input::new(search_state)
                                                .cleanable(true)
                                                .focus_ring(true)
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
                                            .when(filter_menu_open, |button| button.outline())
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
                            .rounded(px(12.0))
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .child(
                                Button::new("btn-view-list")
                                    .ghost()
                                    .small()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .icon(LucideIcon::List)
                                    .when(mode == ViewMode::List, |button| {
                                        button.bg(cx.theme().background).shadow_sm()
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
                                        button.bg(cx.theme().background).shadow_sm()
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
                                    .icon(LucideIcon::LayoutList)
                                    .when(mode == ViewMode::Table, |button| {
                                        button.bg(cx.theme().background).shadow_sm()
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
                                    .bg(cx.theme().border),
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
                                        button.bg(cx.theme().background).shadow_sm()
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
