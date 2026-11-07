//! In-editor floating search widget with match counter and navigation.

use super::super::shell::MainShell;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct SearchOverlay {
    pub query_state: Entity<InputState>,
    pub query: String,
    pub match_count: usize,
    pub current_match: usize,
    pub is_visible: bool,
}

impl SearchOverlay {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
        Self {
            query_state: cx.new(|cx| InputState::new(window, cx).placeholder("Search transcript")),
            query: String::new(),
            match_count: 0,
            current_match: 0,
            is_visible: false,
        }
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<MainShell>) {
        self.is_visible = true;
        self.query_state
            .update(cx, |state, cx| state.focus(window, cx));
    }

    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        self.query = self.query_state.read(cx).value().to_string();
        div().when(self.is_visible, |this| {
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
                    .child(div().text_size(px(13.0)).child("Find: "))
                    .child(Input::new(&self.query_state).small())
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted)
                            .child(format!(
                                "{}/{}",
                                if self.match_count == 0 {
                                    0
                                } else {
                                    self.current_match + 1
                                },
                                self.match_count
                            )),
                    )
                    .child(
                        Button::new("btn-search-prev")
                            .ghost()
                            .xsmall()
                            .label("▲")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.workbench.search.match_count > 0 {
                                    let count = this.workbench.search.match_count;
                                    this.workbench.search.current_match =
                                        (this.workbench.search.current_match + count - 1) % count;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Button::new("btn-search-next")
                            .ghost()
                            .xsmall()
                            .label("▼")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.workbench.search.match_count > 0 {
                                    this.workbench.search.current_match =
                                        (this.workbench.search.current_match + 1)
                                            % this.workbench.search.match_count;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Button::new("btn-search-close")
                            .ghost()
                            .xsmall()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workbench.search.is_visible = false;
                                cx.notify();
                            })),
                    ),
            )
        })
    }
}
