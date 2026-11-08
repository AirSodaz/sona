//! Version history and diff drawer.

use super::workbench::TranscriptWorkbench;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct VersionSheet {
    pub is_open: bool,
    pub versions: Vec<(String, String)>,
}

impl VersionSheet {
    pub fn new() -> Self {
        Self {
            is_open: false,
            versions: Vec::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn render(&mut self, cx: &mut Context<TranscriptWorkbench>) -> impl IntoElement {
        div().when(self.is_open, |this| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(360.0))
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
                                    .child("Version History"),
                            )
                            .child(
                                Button::new("btn-close-versions")
                                    .ghost()
                                    .small()
                                    .label("✕")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.version_sheet.is_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(if self.versions.is_empty() {
                        div()
                            .text_size(px(13.0))
                            .text_color(cx.theme().muted)
                            .child("No saved versions")
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(self.versions.iter().map(|(v, time)| {
                                div()
                                    .id(ElementId::from(format!("ver-card-{}", v)))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .p_3()
                                    .rounded_md()
                                    .bg(cx.theme().input)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(v.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(cx.theme().muted)
                                                    .child(time.clone()),
                                            ),
                                    )
                                    .child(
                                        Button::new(format!("btn-restore-{}", v))
                                            .outline()
                                            .xsmall()
                                            .label("Restore")
                                            .on_click(cx.listener({
                                                let snapshot_id = v
                                                    .split_whitespace()
                                                    .next()
                                                    .unwrap_or(v)
                                                    .to_string();
                                                move |this, _, _, cx| {
                                                    this.restore_version(&snapshot_id, cx);
                                                }
                                            })),
                                    )
                            }))
                            .into_any_element()
                    }),
            )
        })
    }
}

impl Default for VersionSheet {
    fn default() -> Self {
        Self::new()
    }
}
