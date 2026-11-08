//! Export modal dialog for saving transcripts into various subtitle and document formats.
//! Strict 1:1 parity with React `ExportModal.tsx`.

use std::path::PathBuf;

use gpui_kit::base::{Disableable as _, FocusableExt as _};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::export::{ExportFormat, ExportMode};
use sona_core::transcription::transcript::TranscriptSegment;

use super::super::shell::MainShell;
use crate::theme::tokens::{RADIUS_LG, RADIUS_SM};

pub struct ExportModal {
    pub is_open: bool,
    pub filename_state: Entity<InputState>,
    pub directory: PathBuf,
    pub format: ExportFormat,
    pub export_mode: ExportMode,
    pub has_translation: bool,
    pub is_exporting: bool,
    pub copied: bool,
    pub error_message: Option<String>,
}

impl ExportModal {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
        let default_dir = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(|home| PathBuf::from(home).join("Downloads"))
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        Self {
            is_open: false,
            filename_state: cx.new(|cx| InputState::new(window, cx).placeholder("File name")),
            directory: default_dir,
            format: ExportFormat::Srt,
            export_mode: ExportMode::Original,
            has_translation: false,
            is_exporting: false,
            copied: false,
            error_message: None,
        }
    }

    pub fn open(
        &mut self,
        title: &str,
        has_translation: bool,
        window: &mut Window,
        cx: &mut Context<MainShell>,
    ) {
        let clean_name = title
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
            .collect::<String>()
            .trim()
            .to_string();
        let initial_name = if clean_name.is_empty() {
            "transcript".to_string()
        } else {
            clean_name
        };

        self.filename_state.update(cx, |state, cx| {
            state.set_value(initial_name, window, cx);
            state.focus(window, cx);
        });

        self.has_translation = has_translation;
        self.copied = false;
        self.is_exporting = false;
        self.error_message = None;
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.copied = false;
        self.is_exporting = false;
        self.error_message = None;
    }

    pub fn browse_directory(&mut self, cx: &mut Context<MainShell>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let picked = tokio::task::spawn_blocking(|| rfd::FileDialog::new().pick_folder())
                .await
                .ok()
                .flatten();
            if let Some(folder) = picked {
                this.update(cx, |shell, cx| {
                    shell.export_modal.directory = folder;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn copy_to_clipboard(
        &mut self,
        segments: &[TranscriptSegment],
        cx: &mut Context<MainShell>,
    ) {
        match sona_core::export::export_segments_with_mode(segments, self.format, self.export_mode)
        {
            Ok(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.copied = true;
                self.error_message = None;
                cx.notify();
            }
            Err(e) => {
                self.error_message = Some(format!("Export format error: {e}"));
                cx.notify();
            }
        }
    }

    pub fn export_to_file(
        &mut self,
        segments: Vec<TranscriptSegment>,
        cx: &mut Context<MainShell>,
    ) {
        let filename = self.filename_state.read(cx).value().trim().to_string();
        let filename = if filename.is_empty() {
            "transcript".to_string()
        } else {
            filename
        };
        let ext = match self.format {
            ExportFormat::Srt => "srt",
            ExportFormat::Vtt => "vtt",
            ExportFormat::Json => "json",
            ExportFormat::Txt => "txt",
            ExportFormat::Md => "md",
        };
        let path = self.directory.join(format!("{}.{}", filename, ext));
        let text = match sona_core::export::export_segments_with_mode(
            &segments,
            self.format,
            self.export_mode,
        ) {
            Ok(text) => text,
            Err(e) => {
                self.error_message = Some(format!("Export serialization failed: {e}"));
                cx.notify();
                return;
            }
        };

        self.is_exporting = true;
        self.error_message = None;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let res = tokio::fs::write(&path, text.as_bytes()).await;
            this.update(cx, |shell, cx| {
                shell.export_modal.is_exporting = false;
                match res {
                    Ok(()) => {
                        shell.export_modal.close();
                    }
                    Err(e) => {
                        shell.export_modal.error_message =
                            Some(format!("Failed to write file to {}: {e}", path.display()));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let formats = [
            (ExportFormat::Srt, "SubRip (.srt)"),
            (ExportFormat::Vtt, "WebVTT (.vtt)"),
            (ExportFormat::Md, "Markdown (.md)"),
            (ExportFormat::Txt, "Plain Text (.txt)"),
            (ExportFormat::Json, "JSON Data (.json)"),
        ];

        let has_translation = self.has_translation;
        let is_exporting = self.is_exporting;
        let copied = self.copied;
        let current_format = self.format;
        let current_mode = self.export_mode;
        let dir_str = self.directory.display().to_string();
        let error_msg = self.error_message.clone();

        div().when(self.is_open, |this| {
            this.child(
                div()
                    .id("export-dialog-backdrop")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.export_modal.close();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id("export-dialog-panel")
                            .flex()
                            .flex_col()
                            .w(px(500.0))
                            .max_w(relative(0.92))
                            .p_6()
                            .rounded(RADIUS_LG)
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_2xl()
                            .gap_4()
                            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            // Header: Title + Close Button
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Export Transcript"),
                                    )
                                    .child(
                                        Button::new("btn-close-export")
                                            .ghost()
                                            .small()
                                            .label("✕")
                                            .on_click(cx.listener(|shell, _, _, cx| {
                                                shell.export_modal.close();
                                                cx.notify();
                                            })),
                                    ),
                            )
                            // Filename Row
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("File name"),
                                    )
                                    .child(Input::new(&self.filename_state).focus_ring(true)),
                            )
                            // Destination Directory Row
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("Export Directory"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .h(px(32.0))
                                                    .px_3()
                                                    .rounded(RADIUS_SM)
                                                    .bg(cx.theme().input)
                                                    .border_1()
                                                    .border_color(cx.theme().border)
                                                    .flex()
                                                    .items_center()
                                                    .overflow_hidden()
                                                    .text_ellipsis()
                                                    .whitespace_nowrap()
                                                    .text_size(px(12.0))
                                                    .text_color(cx.theme().muted)
                                                    .child(dir_str),
                                            )
                                            .child(
                                                Button::new("btn-export-browse")
                                                    .secondary()
                                                    .small()
                                                    .label("Browse...")
                                                    .on_click(cx.listener(|shell, _, _, cx| {
                                                        shell.export_modal.browse_directory(cx);
                                                    })),
                                            ),
                                    ),
                            )
                            // 2-Column Options: Format on left, Mode Selection on right
                            .child(
                                div()
                                    .flex()
                                    .gap_4()
                                    // Column 1: Format choices
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("Format"),
                                            )
                                            .children(formats.into_iter().map(|(fmt, label)| {
                                                let is_sel = current_format == fmt;
                                                div()
                                                    .id(ElementId::from(format!(
                                                        "fmt-choice-{:?}",
                                                        fmt
                                                    )))
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .h(px(28.0))
                                                    .px_2p5()
                                                    .rounded(RADIUS_SM)
                                                    .bg(if is_sel {
                                                        cx.theme().secondary
                                                    } else {
                                                        gpui_kit::transparent_black()
                                                    })
                                                    .border_1()
                                                    .border_color(if is_sel {
                                                        cx.theme().border
                                                    } else {
                                                        gpui_kit::transparent_black()
                                                    })
                                                    .cursor_pointer()
                                                    .text_size(px(12.0))
                                                    .text_color(if is_sel {
                                                        cx.theme().foreground
                                                    } else {
                                                        cx.theme().muted
                                                    })
                                                    .on_click(cx.listener(
                                                        move |shell, _, _, cx| {
                                                            shell.export_modal.format = fmt;
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child(label)
                                                    .child(if is_sel { "✓" } else { "" })
                                            })),
                                    )
                                    // Column 2: Export mode radio buttons
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().foreground)
                                                    .child("Content"),
                                            )
                                            .child(
                                                div()
                                                    .id("mode-opt-original")
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .h(px(28.0))
                                                    .px_2p5()
                                                    .rounded(RADIUS_SM)
                                                    .cursor_pointer()
                                                    .bg(if current_mode == ExportMode::Original {
                                                        cx.theme().secondary
                                                    } else {
                                                        gpui_kit::transparent_black()
                                                    })
                                                    .text_size(px(12.0))
                                                    .text_color(cx.theme().foreground)
                                                    .on_click(cx.listener(|shell, _, _, cx| {
                                                        shell.export_modal.export_mode =
                                                            ExportMode::Original;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        if current_mode == ExportMode::Original {
                                                            "●"
                                                        } else {
                                                            "○"
                                                        },
                                                    )
                                                    .child("Original transcript"),
                                            )
                                            .child(
                                                div()
                                                    .id("mode-opt-translation")
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .h(px(28.0))
                                                    .px_2p5()
                                                    .rounded(RADIUS_SM)
                                                    .bg(
                                                        if current_mode == ExportMode::Translation {
                                                            cx.theme().secondary
                                                        } else {
                                                            gpui_kit::transparent_black()
                                                        },
                                                    )
                                                    .text_size(px(12.0))
                                                    .when(has_translation, |this| {
                                                        this.cursor_pointer()
                                                            .text_color(cx.theme().foreground)
                                                            .on_click(cx.listener(
                                                                |shell, _, _, cx| {
                                                                    shell
                                                                        .export_modal
                                                                        .export_mode =
                                                                        ExportMode::Translation;
                                                                    cx.notify();
                                                                },
                                                            ))
                                                    })
                                                    .when(!has_translation, |this| {
                                                        this.opacity(0.4).cursor_not_allowed()
                                                    })
                                                    .child(
                                                        if current_mode == ExportMode::Translation {
                                                            "●"
                                                        } else {
                                                            "○"
                                                        },
                                                    )
                                                    .child("Translation only"),
                                            )
                                            .child(
                                                div()
                                                    .id("mode-opt-bilingual")
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .h(px(28.0))
                                                    .px_2p5()
                                                    .rounded(RADIUS_SM)
                                                    .bg(if current_mode == ExportMode::Bilingual {
                                                        cx.theme().secondary
                                                    } else {
                                                        gpui_kit::transparent_black()
                                                    })
                                                    .text_size(px(12.0))
                                                    .when(has_translation, |this| {
                                                        this.cursor_pointer()
                                                            .text_color(cx.theme().foreground)
                                                            .on_click(cx.listener(
                                                                |shell, _, _, cx| {
                                                                    shell
                                                                        .export_modal
                                                                        .export_mode =
                                                                        ExportMode::Bilingual;
                                                                    cx.notify();
                                                                },
                                                            ))
                                                    })
                                                    .when(!has_translation, |this| {
                                                        this.opacity(0.4).cursor_not_allowed()
                                                    })
                                                    .child(
                                                        if current_mode == ExportMode::Bilingual {
                                                            "●"
                                                        } else {
                                                            "○"
                                                        },
                                                    )
                                                    .child("Bilingual"),
                                            ),
                                    ),
                            )
                            // Error banner
                            .when_some(error_msg, |this, msg| {
                                this.child(
                                    div()
                                        .p_2p5()
                                        .rounded(RADIUS_SM)
                                        .bg(cx.theme().danger.opacity(0.1))
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().danger)
                                        .child(msg),
                                )
                            })
                            // Footer Buttons
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .pt_2()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        Button::new("btn-copy-clipboard")
                                            .ghost()
                                            .small()
                                            .label(if copied {
                                                "✓ Copied!"
                                            } else {
                                                "Copy to Clipboard"
                                            })
                                            .on_click(cx.listener(|shell, _, _, cx| {
                                                let segments =
                                                    shell.workbench.base_segments.clone();
                                                shell.export_modal.copy_to_clipboard(&segments, cx);
                                            })),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Button::new("btn-export-cancel")
                                                    .ghost()
                                                    .small()
                                                    .label("Cancel")
                                                    .on_click(cx.listener(|shell, _, _, cx| {
                                                        shell.export_modal.close();
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new("btn-save-export")
                                                    .primary()
                                                    .small()
                                                    .label(if is_exporting {
                                                        "Exporting..."
                                                    } else {
                                                        "Export"
                                                    })
                                                    .disabled(is_exporting)
                                                    .on_click(cx.listener(|shell, _, _, cx| {
                                                        let segments =
                                                            shell.workbench.base_segments.clone();
                                                        shell
                                                            .export_modal
                                                            .export_to_file(segments, cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    ),
            )
        })
    }
}
