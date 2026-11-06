//! Virtualized transcript segment editor list.

use gpui_kit::*;

use super::segment_row::{SegmentData, SegmentRow};

pub struct TranscriptEditor {
    pub segments: Vec<SegmentData>,
}

impl TranscriptEditor {
    pub fn new() -> Self {
        Self {
            segments: vec![
                SegmentData {
                    id: "seg-1".to_string(),
                    start_time: 0.0,
                    end_time: 4.8,
                    speaker: "Speaker 1".to_string(),
                    text: "Welcome everyone to our engineering sync on the pure Rust GPUI migration.".to_string(),
                    translation: Some("欢迎大家参加关于纯 Rust GPUI 迁移的工程同步会。".to_string()),
                },
                SegmentData {
                    id: "seg-2".to_string(),
                    start_time: 5.2,
                    end_time: 12.4,
                    speaker: "Speaker 2".to_string(),
                    text: "We have fully eliminated Tauri WebViews and IPC serialization overhead.".to_string(),
                    translation: Some("我们已经完全消除了 Tauri WebView 和 IPC 序列化开销。".to_string()),
                },
                SegmentData {
                    id: "seg-3".to_string(),
                    start_time: 13.0,
                    end_time: 21.6,
                    speaker: "Speaker 1".to_string(),
                    text: "Direct in-process access to SQLite WAL and Tokio broadcast channels gives us sub-millisecond responsiveness.".to_string(),
                    translation: Some("直接在进程内访问 SQLite WAL 和 Tokio 广播通道，为我们提供了亚毫秒级的响应速度。".to_string()),
                },
            ],
        }
    }

    pub fn render(&mut self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .p_4()
            .gap_2()
            .children(self.segments.iter().map(|seg| {
                SegmentRow::render(seg, false, cx)
            }))
    }
}
