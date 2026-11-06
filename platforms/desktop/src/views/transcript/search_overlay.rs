//! In-editor floating search widget with match counter and navigation.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct SearchOverlay {
    pub query: String,
    pub match_count: usize,
    pub current_match: usize,
    pub is_visible: bool,
}

impl SearchOverlay {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            match_count: 0,
            current_match: 0,
            is_visible: false,
        }
    }

    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .when(self.is_visible, |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_lg()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .child("Find: "),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(cx.theme().input)
                                .text_size(px(13.0))
                                .child(if self.query.is_empty() { "Search...".to_string() } else { self.query.clone() }),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted)
                                .child(format!("{}/{}", self.current_match, self.match_count)),
                        )
                        .child(Button::new("btn-search-prev").ghost().xsmall().label("▲"))
                        .child(Button::new("btn-search-next").ghost().xsmall().label("▼"))
                        .child(
                            Button::new("btn-search-close")
                                .ghost()
                                .xsmall()
                                .label("✕")
                        ),
                )
            })
    }
}
