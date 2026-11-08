//! AI Summary Sheet drawer.

use super::workbench::TranscriptWorkbench;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

pub struct SummarySheet {
    pub is_open: bool,
    pub template: String,
    pub summary_text: String,
    pub is_generating: bool,
    pub error_message: Option<String>,
    pub editor_state: Entity<TextareaState>,
    pub is_saving: bool,
    last_synced_text: String,
}

impl SummarySheet {
    pub fn new(window: &mut Window, cx: &mut Context<TranscriptWorkbench>) -> Self {
        let editor_state =
            cx.new(|cx| TextareaState::new(window, cx).placeholder("Write or generate a summary"));
        Self {
            is_open: false,
            template: "general".to_string(),
            summary_text: String::new(),
            is_generating: false,
            error_message: None,
            editor_state,
            is_saving: false,
            last_synced_text: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn sync_editor(&mut self, window: &mut Window, cx: &mut Context<TranscriptWorkbench>) {
        if self.summary_text != self.last_synced_text {
            self.editor_state.update(cx, |state, cx| {
                state.set_value(self.summary_text.clone(), window, cx)
            });
            self.last_synced_text = self.summary_text.clone();
        }
    }

    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<TranscriptWorkbench>,
    ) -> impl IntoElement {
        self.sync_editor(window, cx);
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
                                    .child(format!("AI Summary · {}", self.template)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new("btn-copy-summary")
                                            .ghost()
                                            .small()
                                            .label("Copy")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let text = this
                                                    .summary_sheet
                                                    .editor_state
                                                    .read(cx)
                                                    .value()
                                                    .to_string();
                                                if !text.is_empty() {
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(text),
                                                    );
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("btn-close-summary")
                                            .ghost()
                                            .small()
                                            .label("Close")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.summary_sheet.is_open = false;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("btn-gen-summary")
                                    .primary()
                                    .small()
                                    .label(if self.is_generating {
                                        "Generating…"
                                    } else {
                                        "Regenerate Summary"
                                    })
                                    .disabled(self.is_generating || self.is_saving)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.refresh_summary(cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-save-summary")
                                    .outline()
                                    .small()
                                    .label(if self.is_saving { "Saving…" } else { "Save" })
                                    .disabled(self.is_saving || self.is_generating)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_summary_edit(cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-delete-summary")
                                    .ghost()
                                    .small()
                                    .label("Delete")
                                    .disabled(self.is_saving || self.is_generating)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.delete_summary(cx);
                                    })),
                            ),
                    )
                    .child(if self.is_generating {
                        div()
                            .flex()
                            .flex_col()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().input)
                            .text_size(px(13.0))
                            .text_color(cx.theme().muted)
                            .child("Generating summary…")
                            .into_any_element()
                    } else if let Some(error) = self.error_message.clone() {
                        div()
                            .flex()
                            .flex_col()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().input)
                            .text_size(px(13.0))
                            .text_color(cx.theme().danger)
                            .child(error)
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h(px(220.0))
                            .p_2()
                            .rounded_md()
                            .bg(cx.theme().input)
                            .when(self.summary_text.trim().is_empty(), |this| {
                                this.child(
                                    div()
                                        .px_2()
                                        .pt_2()
                                        .text_size(px(13.0))
                                        .text_color(cx.theme().muted)
                                        .child("No summary available"),
                                )
                            })
                            .child(
                                Textarea::new(&self.editor_state)
                                    .h_full()
                                    .aria_label("Summary"),
                            )
                            .into_any_element()
                    }),
            )
        })
    }
}
