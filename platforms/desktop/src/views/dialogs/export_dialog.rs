//! Export modal dialog for saving transcripts into various subtitle and document formats.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ExportFormat {
    Srt,
    Vtt,
    Json,
    Txt,
    Markdown,
}

pub struct ExportModal {
    pub is_open: bool,
    pub format: ExportFormat,
    pub bilingual: bool,
    pub include_speakers: bool,
    pub include_timestamps: bool,
}

impl ExportModal {
    pub fn new() -> Self {
        Self {
            is_open: false,
            format: ExportFormat::Srt,
            bilingual: true,
            include_speakers: true,
            include_timestamps: true,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        let formats = [
            (ExportFormat::Srt, "SubRip (.srt)"),
            (ExportFormat::Vtt, "WebVTT (.vtt)"),
            (ExportFormat::Markdown, "Markdown (.md)"),
            (ExportFormat::Txt, "Plain Text (.txt)"),
            (ExportFormat::Json, "JSON Data (.json)"),
        ];

        div().when(self.is_open, |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(460.0))
                            .p_6()
                            .rounded_xl()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_xl()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::BOLD)
                                            .child("Export Transcript"),
                                    )
                                    .child(
                                        Button::new("btn-close-export").ghost().small().label("✕"),
                                    ),
                            )
                            .child(div().flex().flex_col().gap_2().children(
                                formats.into_iter().map(|(fmt, label)| {
                                    let is_sel = self.format == fmt;
                                    div()
                                        .id(ElementId::from(format!("fmt-choice-{:?}", fmt)))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .p_2p5()
                                        .rounded_md()
                                        .bg(if is_sel {
                                            cx.theme().accent
                                        } else {
                                            cx.theme().input
                                        })
                                        .cursor_pointer()
                                        .child(label)
                                        .child(if is_sel { "✓" } else { "" })
                                }),
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("btn-copy-clipboard")
                                            .outline()
                                            .small()
                                            .label("Copy to Clipboard"),
                                    )
                                    .child(
                                        Button::new("btn-save-export")
                                            .primary()
                                            .small()
                                            .label("Save File..."),
                                    ),
                            ),
                    ),
            )
        })
    }
}
