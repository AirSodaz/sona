//! Notifications center flyout popover panel.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct NotificationCenter {
    pub is_open: bool,
    pub notifications: Vec<String>,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self {
            is_open: false,
            notifications: vec![
                "Transcription completed: Quarterly Engineering Sync (42m)".to_string(),
                "Emby Media Server sync finished (3 new episodes)".to_string(),
                "SenseVoice Small neural model loaded in 42ms".to_string(),
            ],
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
                    .w(px(320.0))
                    .p_3()
                    .rounded_lg()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_xl()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_1()
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Notifications"),
                            )
                            .child(Button::new("btn-notif-close").ghost().small().label("✕")),
                    )
                    .child(div().flex().flex_col().gap_1p5().children(
                        self.notifications.iter().enumerate().map(|(i, notif)| {
                            div()
                                .id(ElementId::from(format!("notif-item-{}", i)))
                                .p_2()
                                .rounded_md()
                                .bg(cx.theme().input)
                                .text_size(px(12.0))
                                .line_height(relative(1.3))
                                .child(notif.clone())
                        }),
                    )),
            )
        })
    }
}
