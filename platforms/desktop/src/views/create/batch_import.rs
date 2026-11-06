//! Batch audio file import, drag-and-drop dropzone, and queue list.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct QueueItem {
    pub id: String,
    pub filename: String,
    pub progress: f32, // 0.0 to 1.0
    pub status: String,
}

pub struct BatchImportPanel {
    pub queue: Vec<QueueItem>,
    pub is_dragging: bool,
}

impl BatchImportPanel {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            is_dragging: false,
        }
    }

    pub fn pick_files(&mut self) {
        let files = rfd::FileDialog::new()
            .add_filter(
                "Audio Files",
                &["wav", "mp3", "m4a", "flac", "ogg", "aac", "mp4", "mkv"],
            )
            .pick_files();

        if let Some(paths) = files {
            for path in paths {
                self.add_file(path);
            }
        }
    }

    pub fn add_file(&mut self, path: PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio_file".to_string());
        let id = format!("{}_{}", name, self.queue.len());
        self.queue.push(QueueItem {
            id,
            filename: name,
            progress: 0.0,
            status: "Queued".to_string(),
        });
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_4()
            .child(
                // Dropzone area
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .h(px(180.0))
                    .w_full()
                    .rounded_lg()
                    .border_2()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child("Drop audio or video files here"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted)
                            .child("Supports MP3, WAV, M4A, FLAC, MP4, MKV"),
                    )
                    .child(Button::new("btn-browse").outline().label("Browse Files...")),
            )
            .when(!self.queue.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .w_full()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format!("Batch Queue ({} files)", self.queue.len())),
                                )
                                .child(
                                    Button::new("btn-clear-queue")
                                        .ghost()
                                        .small()
                                        .label("Clear All"),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1p5()
                                .children(self.queue.iter().map(|item| {
                                    div()
                                        .id(ElementId::from(item.id.clone()))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().background)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(item.filename.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(cx.theme().muted)
                                                .child(item.status.clone()),
                                        )
                                })),
                        ),
                )
            })
    }
}
