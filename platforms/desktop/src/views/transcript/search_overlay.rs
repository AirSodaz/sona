//! In-editor floating search widget with match counter and navigation.

use super::workbench::TranscriptWorkbench;
use crate::theme::tokens::RADIUS_LG;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
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
    pub fn new(window: &mut Window, cx: &mut Context<TranscriptWorkbench>) -> Self {
        Self {
            query_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Find in transcript...")),
            query: String::new(),
            match_count: 0,
            current_match: 0,
            is_visible: false,
        }
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<TranscriptWorkbench>) {
        self.is_visible = true;
        self.query_state
            .update(cx, |state, cx| state.focus(window, cx));
    }

    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn render(&mut self, cx: &mut Context<TranscriptWorkbench>) -> impl IntoElement {
        self.query = self.query_state.read(cx).value().to_string();
        div().when(self.is_visible, |this| {
            let count_text = if self.match_count > 0 {
                format!("{}/{}", self.current_match + 1, self.match_count)
            } else if !self.query.is_empty() {
                "0/0".to_string()
            } else {
                String::new()
            };
            this.child(
                div()
                    .id("search-ui-bar")
                    .flex()
                    .items_center()
                    .min_w(px(320.0))
                    .p(px(4.0))
                    .rounded(RADIUS_LG)
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_xl()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(160.0))
                            .child(Input::new(&self.query_state).small()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.0))
                            .pr(px(4.0))
                            .when(!count_text.is_empty(), |actions| {
                                actions.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted)
                                        .px(px(8.0))
                                        .whitespace_nowrap()
                                        .child(count_text),
                                )
                            })
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h(px(16.0))
                                    .bg(cx.theme().border)
                                    .mx(px(4.0)),
                            )
                            .child(
                                Button::new("btn-search-prev")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::ChevronUp)
                                    .tooltip("Previous match (Shift+Enter)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.search.match_count > 0 {
                                            let count = this.search.match_count;
                                            this.search.current_match =
                                                (this.search.current_match + count - 1) % count;
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new("btn-search-next")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::ChevronDown)
                                    .tooltip("Next match (Enter)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.search.match_count > 0 {
                                            this.search.current_match = (this.search.current_match
                                                + 1)
                                                % this.search.match_count;
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new("btn-search-close")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::X)
                                    .tooltip("Close (Esc)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.search.is_visible = false;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
        })
    }
}
