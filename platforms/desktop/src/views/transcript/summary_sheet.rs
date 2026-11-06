//! AI Summary Sheet drawer.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct SummarySheet {
    pub is_open: bool,
    pub template: String,
    pub summary_text: String,
    pub is_generating: bool,
}

impl SummarySheet {
    pub fn new() -> Self {
        Self {
            is_open: false,
            template: "Executive Overview".to_string(),
            summary_text: "• Architecture refactored cleanly into pure Rust with GPUI Kit.\n• All 17 Tauri dependencies excised in-process.\n• Real-time broadcast event bus handles streaming transcription.".to_string(),
            is_generating: false,
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div().when(self.is_open, |this| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(380.0))
                    .h_full()
                    .p_4()
                    .border_l_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child("AI Summary"),
                            )
                            .child(Button::new("btn-close-summary").ghost().small().label("✕")),
                    )
                    .child(
                        Button::new("btn-gen-summary")
                            .primary()
                            .small()
                            .label("Regenerate Summary"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().input)
                            .text_size(px(13.0))
                            .line_height(relative(1.4))
                            .child(self.summary_text.clone()),
                    ),
            )
        })
    }
}
