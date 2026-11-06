//! Projects Toolbar with search, filters, and view mode switcher.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

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
}

impl ProjectsToolbar {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            view_mode: ViewMode::Cards,
            is_selection_active: false,
        }
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    pub fn toggle_selection(&mut self) {
        self.is_selection_active = !self.is_selection_active;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(48.0))
            .w_full()
            .px_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                // Search bar placeholder
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1p5()
                    .w(px(280.0))
                    .rounded_md()
                    .bg(cx.theme().input)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(cx.theme().muted)
                            .child("Search transcripts..."),
                    ),
            )
            .child(
                // View controls
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("btn-view-cards")
                            .ghost()
                            .small()
                            .label("Cards"),
                    )
                    .child(
                        Button::new("btn-view-list")
                            .ghost()
                            .small()
                            .label("List"),
                    )
                    .child(
                        div()
                            .w(px(1.0))
                            .h(px(16.0))
                            .bg(cx.theme().border),
                    )
                    .child(
                        Button::new("btn-select-mode")
                            .outline()
                            .small()
                            .label(if self.is_selection_active { "Cancel Select" } else { "Select" }),
                    ),
            )
    }
}
