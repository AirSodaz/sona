//! Projects batch selection action bar.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

pub struct SelectionBar {
    pub selected_count: usize,
}

impl SelectionBar {
    pub fn new() -> Self {
        Self { selected_count: 0 }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(48.0))
            .w_full()
            .px_4()
            .rounded_md()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_md()
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("{} items selected", self.selected_count)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Button::new("btn-sel-move").outline().small().label("Move to Project"))
                    .child(Button::new("btn-sel-export").outline().small().label("Export"))
                    .child(Button::new("btn-sel-delete").danger().small().label("Delete")),
            )
    }
}
