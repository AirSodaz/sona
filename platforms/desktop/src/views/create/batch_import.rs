//! Batch audio file import, drag-and-drop dropzone, stream modal, and queue system.
//! Strict 1:1 parity with React `BatchImport.tsx`, `UrlStreamImportModal.tsx`, `QueueSummaryBar.tsx`,
//! `QueueItem.tsx`, and `QueueGroup.tsx` (Gaps G10, G11, G12).

use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::{Disableable as _, FocusableExt as _};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::options_capsule::TranscriptionOptionsModel;
use crate::app::{GlobalDesktopServices, GlobalEventBus};
use crate::theme::tokens::{COLOR_PROJECT_BLUE, RADIUS_FULL, RADIUS_LG, RADIUS_MD, RADIUS_SM};

#[inline]
fn icon(name: LucideIcon) -> Icon {
    Icon::default().path(name.path())
}

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "m4a", "aiff", "flac", "ogg", "wma", "aac", "opus", "amr", "mp4", "webm", "mov",
    "mkv", "avi", "wmv", "flv", "3gp",
];

/// Checks if a file extension is in the supported audio/video formats list.
pub fn is_supported_audio_extension(ext: &str) -> bool {
    let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    SUPPORTED_EXTENSIONS.contains(&clean.as_str())
}

/// Validates whether a URL is a supported streaming source (http, https, rtmp, rtsp).
/// Strictly adheres to `/^(https?|rtmp|rtsp):\/\/[^\s/?#]\S*$/i`.
pub fn is_valid_stream_url(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() {
        return false;
    }
    let lower = u.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("http://") {
        r
    } else if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("rtmp://") {
        r
    } else if let Some(r) = lower.strip_prefix("rtsp://") {
        r
    } else {
        return false;
    };

    if rest.is_empty()
        || rest.starts_with(|c: char| matches!(c, ' ' | '/' | '?' | '#'))
        || rest.chars().any(|c| c.is_whitespace())
    {
        return false;
    }

    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchItemStatus {
    Pending,
    Processing,
    Complete,
    Error,
    Cancelled,
}

impl BatchItemStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Queued",
            Self::Processing => "Transcribing",
            Self::Complete => "Completed",
            Self::Error => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BatchSourceType {
    File,
    UrlStream { url: String, retain_audio: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchQueueGroupKind {
    MediaServer,
    Processing,
    Pending,
    Complete,
    Failed,
}

#[derive(Clone, Debug)]
pub struct BatchQueueItem {
    pub id: String,
    pub instance_id: Option<String>,
    pub filename: String,
    pub file_path: String,
    pub source_type: BatchSourceType,
    pub status: BatchItemStatus,
    pub progress: f32, // 0.0 to 100.0
    pub file_size_bytes: Option<u64>,
    pub duration_seconds: Option<f64>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub project_color_index: Option<usize>,
    pub error_message: Option<String>,
    pub history_id: Option<String>,
    pub is_media_server: bool,
}

impl BatchQueueItem {
    pub fn group_kind(&self) -> BatchQueueGroupKind {
        if self.is_media_server {
            BatchQueueGroupKind::MediaServer
        } else {
            match self.status {
                BatchItemStatus::Processing => BatchQueueGroupKind::Processing,
                BatchItemStatus::Pending => BatchQueueGroupKind::Pending,
                BatchItemStatus::Complete => BatchQueueGroupKind::Complete,
                BatchItemStatus::Error | BatchItemStatus::Cancelled => BatchQueueGroupKind::Failed,
            }
        }
    }
}

pub struct BatchImportPanel {
    pub options_model: Entity<TranscriptionOptionsModel>,
    pub queue: Vec<BatchQueueItem>,
    pub is_drag_over: bool,
    pub is_paused: bool,

    // UrlStreamImportModal state (Gap G10)
    pub is_stream_modal_open: bool,
    pub stream_url_input: String,
    pub stream_alias_input: String,
    pub stream_retain_audio: bool,
    pub stream_error: Option<String>,
    pub stream_url_state: Option<Entity<InputState>>,
    pub stream_alias_state: Option<Entity<InputState>>,
    pub stream_input_subscriptions: Vec<Subscription>,

    // QueueClearMenu & Confirmation dialog state (Gap G11)
    pub is_clear_menu_open: bool,
    pub is_clear_confirm_open: bool,

    // Cancel confirmation state (Gap G12)
    pub confirming_cancel_item_id: Option<String>,

    // Group collapse toggles (Gap G12)
    pub media_server_expanded: bool,
    pub processing_expanded: bool,
    pub pending_expanded: bool,
    pub completed_expanded: bool,
    pub failed_expanded: bool,
}

impl BatchImportPanel {
    pub fn new(options_model: Entity<TranscriptionOptionsModel>) -> Self {
        Self {
            options_model,
            queue: Vec::new(),
            is_drag_over: false,
            is_paused: false,

            is_stream_modal_open: false,
            stream_url_input: String::new(),
            stream_alias_input: String::new(),
            stream_retain_audio: false,
            stream_error: None,
            stream_url_state: None,
            stream_alias_state: None,
            stream_input_subscriptions: Vec::new(),

            is_clear_menu_open: false,
            is_clear_confirm_open: false,
            confirming_cancel_item_id: None,

            media_server_expanded: true,
            processing_expanded: true,
            pending_expanded: true,
            completed_expanded: false,
            failed_expanded: true,
        }
    }

    /// Returns the overall queue progress calculated across all items.
    pub fn overall_progress(&self) -> f32 {
        let total_count = self.queue.len();
        if total_count == 0 {
            return 0.0;
        }
        let progress_sum: f32 = self
            .queue
            .iter()
            .map(|i| match i.status {
                BatchItemStatus::Complete => 100.0,
                BatchItemStatus::Processing => i.progress,
                _ => 0.0,
            })
            .sum();
        (progress_sum / (total_count as f32 * 100.0) * 100.0).clamp(0.0, 100.0)
    }

    /// Lazily initializes interactive InputState entities and binds event listeners.
    pub fn ensure_stream_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stream_url_state.is_none() {
            let url_state = cx.new(|cx| {
                InputState::new(window, cx).placeholder("https://example.com/live/stream.m3u8")
            });
            let url_sub = cx.subscribe(&url_state, |this, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.stream_url_input = state.read(cx).value().to_string();
                    if this.stream_error.is_some() {
                        this.stream_error = None;
                    }
                    cx.notify();
                }
            });
            self.stream_url_state = Some(url_state);
            self.stream_input_subscriptions.push(url_sub);
        }

        if self.stream_alias_state.is_none() {
            let alias_state =
                cx.new(|cx| InputState::new(window, cx).placeholder("e.g., AI Podcast Episode 42"));
            let alias_sub = cx.subscribe(&alias_state, |this, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.stream_alias_input = state.read(cx).value().to_string();
                    cx.notify();
                }
            });
            self.stream_alias_state = Some(alias_state);
            self.stream_input_subscriptions.push(alias_sub);
        }
    }

    /// Closes the stream import modal and resets all related input states.
    pub fn close_stream_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_stream_modal_open = false;
        self.stream_url_input.clear();
        self.stream_alias_input.clear();
        self.stream_retain_audio = false;
        self.stream_error = None;
        if let Some(ref url_state) = self.stream_url_state {
            url_state.update(cx, |s, cx| s.set_value("", window, cx));
        }
        if let Some(ref alias_state) = self.stream_alias_state {
            alias_state.update(cx, |s, cx| s.set_value("", window, cx));
        }
        cx.notify();
    }

    /// Disambiguates and applies a batch progress event payload to queue items.
    ///
    /// Priority:
    /// 1. If `instance_id_opt` is Some, match strictly on `item.instance_id == instance_id_opt`.
    /// 2. If `instance_id_opt` is None, match on `item.file_path == path_opt` ONLY for items currently in `Processing`.
    /// Returns true if any item was updated.
    pub fn apply_progress_event(
        queue: &mut [BatchQueueItem],
        path_opt: Option<&str>,
        progress_opt: Option<f32>,
        instance_id_opt: Option<&str>,
    ) -> bool {
        let mut updated = false;
        for item in queue.iter_mut() {
            let is_match = if let Some(inst_id) = instance_id_opt {
                item.instance_id.as_deref() == Some(inst_id)
            } else if let Some(p) = path_opt {
                item.status == BatchItemStatus::Processing && item.file_path == p
            } else {
                false
            };

            if is_match {
                if let Some(prog) = progress_opt {
                    item.progress = prog.clamp(0.0, 100.0);
                    if prog >= 100.0 {
                        item.status = BatchItemStatus::Complete;
                    } else if item.status == BatchItemStatus::Pending {
                        item.status = BatchItemStatus::Processing;
                    }
                    updated = true;
                }
            }
        }
        updated
    }

    /// Subscribes to batch progress events emitted by the decoupled ASR pipeline.
    pub fn subscribe_batch_events(&mut self, cx: &mut Context<Self>) {
        let event_bus = cx.global::<GlobalEventBus>().0.clone();
        let mut rx = event_bus.subscribe();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            while let Ok(event) = rx.recv().await {
                if event.name == "batch-progress" {
                    // payload: [path, progress, instance_id]
                    if let Ok(arr) = serde_json::from_value::<Vec<serde_json::Value>>(event.payload)
                    {
                        if arr.len() >= 2 {
                            let path_opt = arr[0].as_str().map(|s| s.to_string());
                            let progress_opt = arr[1].as_f64().map(|p| p as f32);
                            let instance_id_opt =
                                arr.get(2).and_then(|v| v.as_str()).map(|s| s.to_string());

                            this.update(cx, |view, cx| {
                                if Self::apply_progress_event(
                                    &mut view.queue,
                                    path_opt.as_deref(),
                                    progress_opt,
                                    instance_id_opt.as_deref(),
                                ) {
                                    cx.notify();
                                }
                            })
                            .ok();
                        }
                    }
                }
            }
        })
        .detach();
    }

    pub fn is_processing(&self) -> bool {
        self.queue
            .iter()
            .any(|item| item.status == BatchItemStatus::Processing)
    }

    /// Native async file picker via rfd.
    pub fn pick_files_async(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let files = rfd::AsyncFileDialog::new()
                .add_filter("Audio & Video Files", SUPPORTED_EXTENSIONS)
                .pick_files()
                .await;

            if let Some(handles) = files {
                let paths: Vec<PathBuf> = handles
                    .into_iter()
                    .map(|h| h.path().to_path_buf())
                    .collect();
                this.update(cx, |view, cx| {
                    for path in paths {
                        view.add_file_path(path);
                    }
                    view.start_next_pending(cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn add_file_path(&mut self, path: PathBuf) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();

        if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
            return;
        }

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio_file".to_string());
        let file_path = path.to_string_lossy().to_string();
        let file_size_bytes = std::fs::metadata(&path).map(|m| m.len()).ok();
        let id = format!("batch-{}", uuid::Uuid::new_v4());

        self.queue.push(BatchQueueItem {
            id,
            instance_id: None,
            filename,
            file_path,
            source_type: BatchSourceType::File,
            status: BatchItemStatus::Pending,
            progress: 0.0,
            file_size_bytes,
            duration_seconds: None,
            project_id: None,
            project_name: None,
            project_color_index: None,
            error_message: None,
            history_id: None,
            is_media_server: false,
        });
    }

    pub fn add_stream_url(&mut self, url: String, alias: Option<String>, retain_audio: bool) {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        let filename = alias
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| trimmed.clone());
        let id = format!("stream-{}", uuid::Uuid::new_v4());

        self.queue.push(BatchQueueItem {
            id,
            instance_id: None,
            filename,
            file_path: trimmed.clone(),
            source_type: BatchSourceType::UrlStream {
                url: trimmed,
                retain_audio,
            },
            status: BatchItemStatus::Pending,
            progress: 0.0,
            file_size_bytes: None,
            duration_seconds: None,
            project_id: None,
            project_name: None,
            project_color_index: None,
            error_message: None,
            history_id: None,
            is_media_server: false,
        });
    }

    /// Triggers processing for the next pending queue item in strict serial order.
    pub fn start_next_pending(&mut self, cx: &mut Context<Self>) {
        if self.is_paused || self.is_processing() {
            return;
        }

        let pending_idx = self
            .queue
            .iter()
            .position(|i| i.status == BatchItemStatus::Pending);

        if let Some(idx) = pending_idx {
            let item = &mut self.queue[idx];
            let item_id = item.id.clone();
            let file_path = item.file_path.clone();
            let instance_id = format!("batch-task-{}", uuid::Uuid::new_v4());
            item.instance_id = Some(instance_id.clone());
            item.status = BatchItemStatus::Processing;
            item.progress = 0.0;

            let options = self.options_model.read(cx);
            let language = if options.batch_language == "auto" {
                None
            } else {
                Some(options.batch_language.clone())
            };
            let project_id = item.project_id.clone();
            let services = cx.global::<GlobalDesktopServices>().0.clone();

            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let req = crate::platform::agent_control::facade::types::TranscribeFileRequest {
                    file_path,
                    language,
                    project_id,
                    instance_id: Some(instance_id),
                    save_to_path: None,
                };

                let res =
                    crate::platform::agent_control::facade::batch::transcribe_file(&services, req)
                        .await;

                this.update(cx, |view, cx| {
                    if let Some(target) = view.queue.iter_mut().find(|i| i.id == item_id) {
                        // Only transition if the item is still Processing (not cancelled while in flight)
                        if target.status == BatchItemStatus::Processing {
                            match res {
                                Ok(transcribe_res) => {
                                    target.history_id = Some(transcribe_res.history_id);
                                    target.status = BatchItemStatus::Complete;
                                    target.progress = 100.0;
                                }
                                Err(err) => {
                                    target.status = BatchItemStatus::Error;
                                    target.error_message = Some(err);
                                }
                            }
                        }
                    }
                    // If all tasks are completed, auto-expand the Completed section
                    if view.queue.iter().all(|i| {
                        i.status != BatchItemStatus::Processing
                            && i.status != BatchItemStatus::Pending
                    }) {
                        view.completed_expanded = true;
                    }
                    view.start_next_pending(cx);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Cancels or removes an item from the queue.
    pub fn cancel_item(&mut self, item_id: String, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        if let Some(item) = self.queue.iter_mut().find(|i| i.id == item_id) {
            if let Some(inst_id) = item.instance_id.clone() {
                cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                    let _ = crate::platform::agent_control::facade::batch::cancel_batch_task(
                        &services, inst_id,
                    )
                    .await;
                })
                .detach();
            }
            item.status = BatchItemStatus::Cancelled;
            cx.notify();
        }
        self.start_next_pending(cx);
    }

    /// Retries a failed or cancelled item.
    pub fn retry_item(&mut self, item_id: String, cx: &mut Context<Self>) {
        if let Some(item) = self.queue.iter_mut().find(|i| i.id == item_id) {
            item.status = BatchItemStatus::Pending;
            item.progress = 0.0;
            item.error_message = None;
            item.instance_id = None;
        }
        self.start_next_pending(cx);
        cx.notify();
    }

    /// Retries all failed and cancelled items.
    pub fn retry_all_failed(&mut self, cx: &mut Context<Self>) {
        for item in &mut self.queue {
            if matches!(
                item.status,
                BatchItemStatus::Error | BatchItemStatus::Cancelled
            ) {
                item.status = BatchItemStatus::Pending;
                item.progress = 0.0;
                item.error_message = None;
                item.instance_id = None;
            }
        }
        self.start_next_pending(cx);
        cx.notify();
    }

    /// Clears completed items from the queue.
    pub fn clear_completed(&mut self, cx: &mut Context<Self>) {
        self.queue.retain(|i| i.status != BatchItemStatus::Complete);
        cx.notify();
    }

    /// Clears all items from the queue.
    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        for item in &self.queue {
            if item.status == BatchItemStatus::Processing {
                if let Some(inst_id) = item.instance_id.clone() {
                    let services = services.clone();
                    cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                        let _ = crate::platform::agent_control::facade::batch::cancel_batch_task(
                            &services, inst_id,
                        )
                        .await;
                    })
                    .detach();
                }
            }
        }
        self.queue.clear();
        self.is_clear_menu_open = false;
        self.is_clear_confirm_open = false;
        cx.notify();
    }

    pub fn toggle_queue_pause(&mut self, cx: &mut Context<Self>) {
        self.is_paused = !self.is_paused;
        if !self.is_paused {
            self.start_next_pending(cx);
        }
        cx.notify();
    }
}

impl BatchImportPanel {
    pub fn render_item_card(item: &BatchQueueItem, cx: &Context<Self>) -> impl IntoElement {
        let item_id = item.id.clone();
        let status = item.status;
        let progress = item.progress;
        let is_stream = matches!(item.source_type, BatchSourceType::UrlStream { .. });

        div()
            .id(ElementId::from(item.id.clone()))
            .flex()
            .flex_col()
            .p(px(8.0))
            .gap(px(6.0))
            .rounded(RADIUS_SM)
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .hover(|s| s.bg(cx.theme().secondary.opacity(0.4)))
            // Header Row: Status icon + filename + Actions
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(match status {
                                BatchItemStatus::Pending => icon(LucideIcon::Clock)
                                    .size(px(14.0))
                                    .text_color(cx.theme().muted_foreground),
                                BatchItemStatus::Processing => icon(LucideIcon::Loader)
                                    .size(px(14.0))
                                    .text_color(COLOR_PROJECT_BLUE),
                                BatchItemStatus::Complete => icon(LucideIcon::Check)
                                    .size(px(14.0))
                                    .text_color(cx.theme().success),
                                BatchItemStatus::Error => icon(LucideIcon::CircleAlert)
                                    .size(px(14.0))
                                    .text_color(cx.theme().danger),
                                BatchItemStatus::Cancelled => icon(LucideIcon::CircleSlash)
                                    .size(px(14.0))
                                    .text_color(cx.theme().muted_foreground),
                            })
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_w(px(200.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(item.filename.clone()),
                            ),
                    )
                    // Action buttons
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .when(
                                matches!(
                                    status,
                                    BatchItemStatus::Error | BatchItemStatus::Cancelled
                                ),
                                |act| {
                                    let item_id = item_id.clone();
                                    act.child(
                                        div()
                                            .id(ElementId::from(format!("btn-retry-{}", item_id)))
                                            .cursor_pointer()
                                            .p(px(2.0))
                                            .rounded(RADIUS_SM)
                                            .hover(|s| s.bg(cx.theme().secondary))
                                            .child(
                                                icon(LucideIcon::RotateCcw)
                                                    .size(px(12.0))
                                                    .text_color(COLOR_PROJECT_BLUE),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.retry_item(item_id.clone(), cx);
                                            })),
                                    )
                                },
                            )
                            .child(
                                div()
                                    .id(ElementId::from(format!("btn-cancel-{}", item_id)))
                                    .cursor_pointer()
                                    .p(px(2.0))
                                    .rounded(RADIUS_SM)
                                    .hover(|s| s.bg(cx.theme().secondary))
                                    .child(
                                        icon(LucideIcon::X)
                                            .size(px(12.0))
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .on_click(cx.listener({
                                        let item_id = item_id.clone();
                                        move |this, _, _, cx| {
                                            this.cancel_item(item_id.clone(), cx);
                                        }
                                    })),
                            ),
                    ),
            )
            // Individual Progress Bar (when processing)
            .when(status == BatchItemStatus::Processing, |this| {
                this.child(
                    div()
                        .h(px(4.0))
                        .w_full()
                        .rounded_full()
                        .bg(cx.theme().secondary.opacity(0.5))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(progress / 100.0))
                                .rounded_full()
                                .bg(COLOR_PROJECT_BLUE),
                        ),
                )
            })
            // Meta Tags Row: Format chip, Stream badge, Error text
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .when(is_stream, |this| {
                        this.child(
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded(RADIUS_SM)
                                .bg(COLOR_PROJECT_BLUE.opacity(0.15))
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(COLOR_PROJECT_BLUE)
                                .flex()
                                .items_center()
                                .gap(px(3.0))
                                .child(icon(LucideIcon::Globe).size(px(10.0)))
                                .child("Network Stream"),
                        )
                    })
                    .when(!is_stream, |this| {
                        let ext = Path::new(&item.file_path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("AUDIO")
                            .to_uppercase();
                        this.child(
                            div()
                                .px(px(5.0))
                                .py(px(1.0))
                                .rounded(RADIUS_SM)
                                .bg(cx.theme().secondary)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().muted_foreground)
                                .child(ext),
                        )
                    })
                    .when_some(item.error_message.as_ref(), |this, err| {
                        this.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().danger)
                                .child(err.clone()),
                        )
                    }),
            )
    }
}

impl Render for BatchImportPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_stream_modal_open {
            self.ensure_stream_inputs(window, cx);
        }

        let is_empty = self.queue.is_empty();
        let total_count = self.queue.len();
        let completed_count = self
            .queue
            .iter()
            .filter(|i| i.status == BatchItemStatus::Complete)
            .count();
        let failed_count = self
            .queue
            .iter()
            .filter(|i| {
                matches!(
                    i.status,
                    BatchItemStatus::Error | BatchItemStatus::Cancelled
                )
            })
            .count();
        let processing_count = self
            .queue
            .iter()
            .filter(|i| i.status == BatchItemStatus::Processing)
            .count();
        let is_paused = self.is_paused;
        let overall_progress = self.overall_progress();

        let is_drag = self.is_drag_over;
        let clear_open = self.is_clear_menu_open;

        div()
            .id("batch-import-container")
            .flex()
            .flex_col()
            .size_full()
            .p(px(16.0))
            .gap(px(14.0))
            .overflow_y_scroll()
            // ==========================================
            // UrlStreamImportModal (500px width dialog) (Gap G10)
            // ==========================================
            .when(self.is_stream_modal_open, |this| {
                let err_opt = self.stream_error.clone();
                let retain_audio = self.stream_retain_audio;
                let is_url_empty = self.stream_url_input.trim().is_empty();

                this.child(
                    div()
                        .id("stream-import-overlay")
                        .absolute()
                        .inset_0()
                        .bg(gpui_kit::black().opacity(0.45))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("stream-import-modal")
                                .w(px(500.0))
                                .p(px(20.0))
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_xl()
                                .flex()
                                .flex_col()
                                .gap(px(14.0))
                                // Title Header
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.0))
                                                .child(icon(LucideIcon::Globe).size(px(18.0)))
                                                .child(
                                                    div()
                                                        .text_size(px(15.0))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child("Import Network Audio/Video Stream"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("btn-close-stream-modal")
                                                .cursor_pointer()
                                                .child(icon(LucideIcon::X).size(px(16.0)))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.close_stream_modal(window, cx);
                                                })),
                                        ),
                                )
                                // Hint
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            "Transcribes directly from network stream; processed audio slices are discarded immediately without consuming permanent disk space.",
                                        ),
                                )
                                // Interactive Stream URL Input
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Stream URL (http, https, rtmp, rtsp, m3u8)"),
                                        )
                                        .when_some(self.stream_url_state.as_ref(), |this, url_state| {
                                            this.child(
                                                Input::new(url_state)
                                                    .cleanable(true)
                                                    .focus_ring(true),
                                            )
                                        }),
                                )
                                // Interactive Custom Name (Optional Alias) Input
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Custom Name (Optional)"),
                                        )
                                        .when_some(self.stream_alias_state.as_ref(), |this, alias_state| {
                                            this.child(
                                                Input::new(alias_state)
                                                    .cleanable(true)
                                                    .focus_ring(true),
                                            )
                                        }),
                                )
                                // Retain Transcoded Audio Copy Checkbox & Description
                                .child(
                                    div()
                                        .p(px(10.0))
                                        .rounded(RADIUS_MD)
                                        .bg(cx.theme().secondary.opacity(0.12))
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            Checkbox::new("stream-retain-audio")
                                                .checked(retain_audio)
                                                .label("Keep Transcoded Audio Copy (allows workbench playback after saving)")
                                                .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                                    this.stream_retain_audio = *checked;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            div()
                                                .pl(px(26.0))
                                                .text_size(px(11.5))
                                                .text_color(cx.theme().muted_foreground)
                                                .child("When unchecked, audio is kept only in memory during processing and discarded immediately with zero local disk footprint."),
                                        ),
                                )
                                // Error Banner
                                .when_some(err_opt, |this, err| {
                                    this.child(
                                        div()
                                            .p(px(8.0))
                                            .rounded(RADIUS_MD)
                                            .bg(cx.theme().danger.opacity(0.1))
                                            .border_1()
                                            .border_color(cx.theme().danger)
                                            .text_size(px(12.0))
                                            .text_color(cx.theme().danger)
                                            .child(format!("⚠️ {}", err)),
                                    )
                                })
                                // Actions
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(10.0))
                                        .child(
                                            Button::new("btn-cancel-stream")
                                                .outline()
                                                .label("Cancel")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.close_stream_modal(window, cx);
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-submit-stream")
                                                .primary()
                                                .disabled(is_url_empty)
                                                .label("Add to Queue")
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    let url = this.stream_url_input.trim().to_string();
                                                    if url.is_empty() {
                                                        this.stream_error = Some("Please enter at least one audio or video stream URL.".to_string());
                                                        cx.notify();
                                                        return;
                                                    }

                                                    if !is_valid_stream_url(&url) {
                                                        this.stream_error = Some("Stream URL must begin with http://, https://, rtmp://, or rtsp://".to_string());
                                                        cx.notify();
                                                        return;
                                                    }

                                                    let alias_opt = if this.stream_alias_input.trim().is_empty() {
                                                        None
                                                    } else {
                                                        Some(this.stream_alias_input.trim().to_string())
                                                    };
                                                    let retain = this.stream_retain_audio;

                                                    this.add_stream_url(url, alias_opt, retain);
                                                    this.close_stream_modal(window, cx);
                                                    this.start_next_pending(cx);
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        ),
                )
            })
            // ==========================================
            // Clear Confirmation Dialog (Gap G11)
            // ==========================================
            .when(self.is_clear_confirm_open, |this| {
                this.child(
                    div()
                        .id("clear-confirm-overlay")
                        .absolute()
                        .inset_0()
                        .bg(gpui_kit::black().opacity(0.45))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(400.0))
                                .p(px(20.0))
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_xl()
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child("Clear Transcription Queue?"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            "There are files currently being transcribed. Clearing the queue will stop ongoing transcriptions and discard progress. Are you sure you want to clear all?",
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.0))
                                        .child(
                                            Button::new("btn-cancel-clear")
                                                .outline()
                                                .label("Cancel")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.is_clear_confirm_open = false;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-confirm-clear")
                                                .danger()
                                                .label("Clear All")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.clear_all(cx);
                                                })),
                                        ),
                                ),
                        ),
                )
            })
            // ==========================================
            // Empty State Dropzone (Gap G10)
            // ==========================================
            .when(is_empty, |this| {
                this.child(
                    div()
                        .id("batch-dropzone")
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .flex_1()
                        .min_h(px(240.0))
                        .rounded(RADIUS_LG)
                        .border_2()
                        .border_color(if is_drag {
                            cx.theme().primary
                        } else {
                            cx.theme().border
                        })
                        .bg(if is_drag {
                            cx.theme().primary.opacity(0.04)
                        } else {
                            cx.theme().background
                        })
                        .p(px(24.0))
                        .gap(px(14.0))
                        // Central Upload Cloud Icon
                        .child(
                            div()
                                .size(px(48.0))
                                .rounded_full()
                                .bg(cx.theme().secondary.opacity(0.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    icon(LucideIcon::Upload)
                                        .size(px(24.0))
                                        .text_color(cx.theme().primary),
                                ),
                        )
                        // Headings
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(3.0))
                                .child(
                                    div()
                                        .text_size(px(15.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child("Drop audio or video files here"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Drag files here or choose an option below"),
                                ),
                        )
                        // Action Buttons Row
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    Button::new("btn-select-file-primary")
                                        .primary()
                                        .label("Select File")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.pick_files_async(cx);
                                        })),
                                )
                                .child(
                                    Button::new("btn-select-stream-secondary")
                                        .outline()
                                        .label("Import Network Stream...")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.is_stream_modal_open = true;
                                            this.stream_error = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                        // Format Chips Row (Full 18 formats from SUPPORTED_EXTENSIONS)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .flex_wrap()
                                .justify_center()
                                .gap(px(5.0))
                                .pt(px(6.0))
                                .children(SUPPORTED_EXTENSIONS.iter().map(|ext| {
                                    let ext_upper = ext.to_ascii_uppercase();
                                    div()
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(RADIUS_SM)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().secondary.opacity(0.3))
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(cx.theme().muted_foreground)
                                        .child(ext_upper)
                                })),
                        ),
                )
            })
            // ==========================================
            // Queue Summary Bar & Grouped Queue List (Gaps G11 & G12)
            // ==========================================
            .when(!is_empty, |this| {
                this.child(
                    div()
                        .id("batch-queue-section")
                        .flex()
                        .flex_col()
                        .gap(px(12.0))
                        .w_full()
                        // Queue Summary Bar Card
                        .child(
                            div()
                                .id("queue-summary-bar")
                                .p(px(12.0))
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .flex()
                                .flex_col()
                                .gap(px(10.0))
                                // Top row: Title, Status Pill, Progress %
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.0))
                                                .child(
                                                    div()
                                                        .text_size(px(13.5))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(format!("Batch Queue ({total_count} files)")),
                                                )
                                                // Status Pill
                                                .child(
                                                    div()
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .rounded(RADIUS_FULL)
                                                        .text_size(px(11.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.0))
                                                        .when(processing_count > 0 && !is_paused, |p| {
                                                            p.bg(COLOR_PROJECT_BLUE.opacity(0.15))
                                                                .text_color(COLOR_PROJECT_BLUE)
                                                                .child(icon(LucideIcon::Loader).size(px(12.0)))
                                                                .child(format!("Transcribing ({completed_count}/{total_count})"))
                                                        })
                                                        .when(is_paused, |p| {
                                                            p.bg(cx.theme().warning.opacity(0.15))
                                                                .text_color(cx.theme().warning)
                                                                .child(icon(LucideIcon::Pause).size(px(12.0)))
                                                                .child("Paused")
                                                        })
                                                        .when(completed_count == total_count && total_count > 0, |p| {
                                                            p.bg(cx.theme().success.opacity(0.15))
                                                                .text_color(cx.theme().success)
                                                                .child(icon(LucideIcon::Check).size(px(12.0)))
                                                                .child("Completed")
                                                        })
                                                        .when(failed_count > 0 && processing_count == 0, |p| {
                                                            p.bg(cx.theme().danger.opacity(0.15))
                                                                .text_color(cx.theme().danger)
                                                                .child(icon(LucideIcon::CircleAlert).size(px(12.0)))
                                                                .child(format!("{completed_count}/{total_count} · {failed_count} failed"))
                                                        })
                                                        .when(processing_count == 0 && failed_count == 0 && completed_count < total_count && !is_paused, |p| {
                                                            p.bg(cx.theme().secondary)
                                                                .text_color(cx.theme().muted_foreground)
                                                                .child(format!("{completed_count}/{total_count}"))
                                                        }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_family("Consolas, monospace")
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("{:.0}%", overall_progress)),
                                        ),
                                )
                                // Overall Progress Track (6px)
                                .child(
                                    div()
                                        .h(px(6.0))
                                        .w_full()
                                        .rounded_full()
                                        .bg(cx.theme().secondary.opacity(0.5))
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .h_full()
                                                .w(relative(overall_progress / 100.0))
                                                .rounded_full()
                                                .bg(if completed_count == total_count && total_count > 0 {
                                                    cx.theme().success
                                                } else if failed_count == total_count && total_count > 0 {
                                                    cx.theme().danger
                                                } else {
                                                    COLOR_PROJECT_BLUE
                                                }),
                                        ),
                                )
                                // Toolbar: Start/Pause, Add More Files, Clear Menu
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(6.0))
                                                // Start / Pause toggle
                                                .child(
                                                    Button::new("btn-pause-queue")
                                                        .small()
                                                        .outline()
                                                        .label(if is_paused { "Resume" } else { "Pause" })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.toggle_queue_pause(cx);
                                                        })),
                                                )
                                                // Add More Files button
                                                .child(
                                                    Button::new("btn-add-files")
                                                        .small()
                                                        .outline()
                                                        .label("Add Files...")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.pick_files_async(cx);
                                                        })),
                                                )
                                                // Add Stream URL button
                                                .child(
                                                    Button::new("btn-add-stream")
                                                        .small()
                                                        .outline()
                                                        .label("Stream...")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.is_stream_modal_open = true;
                                                            this.stream_error = None;
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                        // Clear Menu Dropdown trigger
                                        .child(
                                            div()
                                                .relative()
                                                .child(
                                                    div()
                                                        .id("btn-clear-menu-trigger")
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.0))
                                                        .px(px(8.0))
                                                        .py(px(4.0))
                                                        .rounded(RADIUS_SM)
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(cx.theme().secondary))
                                                        .child(
                                                            icon(LucideIcon::Trash)
                                                                .size(px(14.0))
                                                                .text_color(cx.theme().muted_foreground),
                                                        )
                                                        .child(
                                                            icon(LucideIcon::ChevronDown)
                                                                .size(px(12.0))
                                                                .text_color(cx.theme().muted_foreground),
                                                        )
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.is_clear_menu_open =
                                                                 !this.is_clear_menu_open;
                                                            cx.notify();
                                                        })),
                                                )
                                                .when(clear_open, |this| {
                                                    this.child(
                                                        div()
                                                            .id("clear-menu-popover")
                                                            .absolute()
                                                            .bottom(px(32.0))
                                                            .right_0()
                                                            .w(px(160.0))
                                                            .p(px(4.0))
                                                            .rounded(RADIUS_MD)
                                                            .bg(cx.theme().background)
                                                            .border_1()
                                                            .border_color(cx.theme().border)
                                                            .shadow_lg()
                                                            .flex()
                                                            .flex_col()
                                                            .gap(px(2.0))
                                                            .child(
                                                                div()
                                                                    .id("btn-clear-completed-opt")
                                                                    .px(px(8.0))
                                                                    .py(px(5.0))
                                                                    .rounded(RADIUS_SM)
                                                                    .cursor_pointer()
                                                                    .hover(|s| s.bg(cx.theme().secondary))
                                                                    .text_size(px(12.0))
                                                                    .child(format!(
                                                                        "Clear Completed ({completed_count})"
                                                                    ))
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.clear_completed(cx);
                                                                        this.is_clear_menu_open = false;
                                                                    })),
                                                            )
                                                            .child(
                                                                div()
                                                                    .h(px(1.0))
                                                                    .w_full()
                                                                    .bg(cx.theme().border),
                                                            )
                                                            .child(
                                                                div()
                                                                    .id("btn-clear-all-opt")
                                                                    .px(px(8.0))
                                                                    .py(px(5.0))
                                                                    .rounded(RADIUS_SM)
                                                                    .cursor_pointer()
                                                                    .hover(|s| s.bg(cx.theme().danger.opacity(0.1)))
                                                                    .text_size(px(12.0))
                                                                    .text_color(cx.theme().danger)
                                                                    .child(format!("Clear All ({total_count})"))
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.is_clear_menu_open = false;
                                                                        let is_running = this.queue.iter().any(|i| i.status == BatchItemStatus::Processing);
                                                                        if is_running {
                                                                            this.is_clear_confirm_open = true;
                                                                        } else {
                                                                            this.clear_all(cx);
                                                                        }
                                                                        cx.notify();
                                                                    })),
                                                            ),
                                                    )
                                                }),
                                        ),
                                ),
                        )
                        // Grouped Queue Items List (Gap G12, Parity with QueueGroupedList.tsx)
                        .child({
                            let media_server_items: Vec<_> = self.queue.iter().filter(|i| i.group_kind() == BatchQueueGroupKind::MediaServer).collect();
                            let processing_items: Vec<_> = self.queue.iter().filter(|i| i.group_kind() == BatchQueueGroupKind::Processing).collect();
                            let pending_items: Vec<_> = self.queue.iter().filter(|i| i.group_kind() == BatchQueueGroupKind::Pending).collect();
                            let completed_items: Vec<_> = self.queue.iter().filter(|i| i.group_kind() == BatchQueueGroupKind::Complete).collect();
                            let failed_items: Vec<_> = self.queue.iter().filter(|i| i.group_kind() == BatchQueueGroupKind::Failed).collect();

                            let media_server_expanded = self.media_server_expanded;
                            let processing_expanded = self.processing_expanded;
                            let pending_expanded = self.pending_expanded;
                            let completed_expanded = self.completed_expanded;
                            let failed_expanded = self.failed_expanded;

                            div()
                                .id("queue-grouped-list")
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                // 0. Media Server Tasks Group
                                .when(!media_server_items.is_empty(), |list| {
                                    list.child(
                                        div()
                                            .id("queue-group-media-server")
                                            .flex()
                                            .flex_col()
                                            .rounded(RADIUS_MD)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .px(px(10.0))
                                                    .py(px(7.0))
                                                    .bg(cx.theme().secondary.opacity(0.3))
                                                    .child(
                                                        div()
                                                            .id("queue-group-header-media-server")
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.media_server_expanded = !this.media_server_expanded;
                                                                cx.notify();
                                                            }))
                                                            .child(icon(if media_server_expanded { LucideIcon::ChevronDown } else { LucideIcon::ChevronRight }).size(px(12.0)).text_color(cx.theme().muted_foreground))
                                                            .child(icon(LucideIcon::Tv).size(px(13.0)).text_color(cx.theme().success))
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child("MEDIA SERVER TASKS"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(1.0))
                                                                    .rounded_full()
                                                                    .bg(cx.theme().success.opacity(0.14))
                                                                    .text_size(px(11.0))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().success)
                                                                    .child(media_server_items.len().to_string()),
                                                            ),
                                                    ),
                                            )
                                            .when(media_server_expanded, |grp| {
                                                grp.child(
                                                    div()
                                                        .p(px(6.0))
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(4.0))
                                                        .border_t_1()
                                                        .border_color(cx.theme().border)
                                                        .children(media_server_items.into_iter().map(|item| {
                                                            Self::render_item_card(item, cx)
                                                        })),
                                                )
                                            }),
                                    )
                                })
                                // 1. Processing Group
                                .when(!processing_items.is_empty(), |list| {
                                    list.child(
                                        div()
                                            .id("queue-group-processing")
                                            .flex()
                                            .flex_col()
                                            .rounded(RADIUS_MD)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .px(px(10.0))
                                                    .py(px(7.0))
                                                    .bg(cx.theme().secondary.opacity(0.3))
                                                    .child(
                                                        div()
                                                            .id("queue-group-header-processing")
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.processing_expanded = !this.processing_expanded;
                                                                cx.notify();
                                                            }))
                                                            .child(icon(if processing_expanded { LucideIcon::ChevronDown } else { LucideIcon::ChevronRight }).size(px(12.0)).text_color(cx.theme().muted_foreground))
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child("PROCESSING"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(1.0))
                                                                    .rounded_full()
                                                                    .bg(COLOR_PROJECT_BLUE.opacity(0.14))
                                                                    .text_size(px(11.0))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(COLOR_PROJECT_BLUE)
                                                                    .child(processing_items.len().to_string()),
                                                            ),
                                                    ),
                                            )
                                            .when(processing_expanded, |grp| {
                                                grp.child(
                                                    div()
                                                        .p(px(6.0))
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(4.0))
                                                        .border_t_1()
                                                        .border_color(cx.theme().border)
                                                        .children(processing_items.into_iter().map(|item| {
                                                            Self::render_item_card(item, cx)
                                                        })),
                                                )
                                            }),
                                    )
                                })
                                // 2. Pending Group
                                .when(!pending_items.is_empty(), |list| {
                                    list.child(
                                        div()
                                            .id("queue-group-pending")
                                            .flex()
                                            .flex_col()
                                            .rounded(RADIUS_MD)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .px(px(10.0))
                                                    .py(px(7.0))
                                                    .bg(cx.theme().secondary.opacity(0.3))
                                                    .child(
                                                        div()
                                                            .id("queue-group-header-pending")
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.pending_expanded = !this.pending_expanded;
                                                                cx.notify();
                                                            }))
                                                            .child(icon(if pending_expanded { LucideIcon::ChevronDown } else { LucideIcon::ChevronRight }).size(px(12.0)).text_color(cx.theme().muted_foreground))
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child("PENDING"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(1.0))
                                                                    .rounded_full()
                                                                    .bg(cx.theme().secondary)
                                                                    .text_size(px(11.0))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child(pending_items.len().to_string()),
                                                            ),
                                                    ),
                                            )
                                            .when(pending_expanded, |grp| {
                                                grp.child(
                                                    div()
                                                        .p(px(6.0))
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(4.0))
                                                        .border_t_1()
                                                        .border_color(cx.theme().border)
                                                        .children(pending_items.into_iter().map(|item| {
                                                            Self::render_item_card(item, cx)
                                                        })),
                                                )
                                            }),
                                    )
                                })
                                // 3. Completed Group
                                .when(!completed_items.is_empty(), |list| {
                                    list.child(
                                        div()
                                            .id("queue-group-complete")
                                            .flex()
                                            .flex_col()
                                            .rounded(RADIUS_MD)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .px(px(10.0))
                                                    .py(px(7.0))
                                                    .bg(cx.theme().secondary.opacity(0.3))
                                                    .child(
                                                        div()
                                                            .id("queue-group-header-complete")
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.completed_expanded = !this.completed_expanded;
                                                                cx.notify();
                                                            }))
                                                            .child(icon(if completed_expanded { LucideIcon::ChevronDown } else { LucideIcon::ChevronRight }).size(px(12.0)).text_color(cx.theme().muted_foreground))
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child("COMPLETED"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(1.0))
                                                                    .rounded_full()
                                                                    .bg(cx.theme().success.opacity(0.14))
                                                                    .text_size(px(11.0))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().success)
                                                                    .child(completed_items.len().to_string()),
                                                            ),
                                                    ),
                                            )
                                            .when(completed_expanded, |grp| {
                                                grp.child(
                                                    div()
                                                        .p(px(6.0))
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(4.0))
                                                        .border_t_1()
                                                        .border_color(cx.theme().border)
                                                        .children(completed_items.into_iter().map(|item| {
                                                            Self::render_item_card(item, cx)
                                                        })),
                                                )
                                            }),
                                    )
                                })
                                // 4. Failed Group
                                .when(!failed_items.is_empty(), |list| {
                                    list.child(
                                        div()
                                            .id("queue-group-failed")
                                            .flex()
                                            .flex_col()
                                            .rounded(RADIUS_MD)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().background)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .px(px(10.0))
                                                    .py(px(7.0))
                                                    .bg(cx.theme().secondary.opacity(0.3))
                                                    .child(
                                                        div()
                                                            .id("queue-group-header-failed")
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .cursor_pointer()
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.failed_expanded = !this.failed_expanded;
                                                                cx.notify();
                                                            }))
                                                            .child(icon(if failed_expanded { LucideIcon::ChevronDown } else { LucideIcon::ChevronRight }).size(px(12.0)).text_color(cx.theme().muted_foreground))
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child("FAILED"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(1.0))
                                                                    .rounded_full()
                                                                    .bg(cx.theme().danger.opacity(0.14))
                                                                    .text_size(px(11.0))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(cx.theme().danger)
                                                                    .child(failed_items.len().to_string()),
                                                            ),
                                                    )
                                                    // Action: Retry All
                                                    .child(
                                                        div()
                                                            .id("btn-retry-all-failed")
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(4.0))
                                                            .px(px(8.0))
                                                            .py(px(2.0))
                                                            .rounded(RADIUS_SM)
                                                            .bg(cx.theme().secondary)
                                                            .hover(|s| s.bg(cx.theme().secondary.opacity(0.8)))
                                                            .cursor_pointer()
                                                            .child(icon(LucideIcon::RotateCcw).size(px(11.0)).text_color(COLOR_PROJECT_BLUE))
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(COLOR_PROJECT_BLUE)
                                                                    .child("Retry All"),
                                                            )
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.retry_all_failed(cx);
                                                            })),
                                                    ),
                                            )
                                            .when(failed_expanded, |grp| {
                                                grp.child(
                                                    div()
                                                        .p(px(6.0))
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(4.0))
                                                        .border_t_1()
                                                        .border_color(cx.theme().border)
                                                        .children(failed_items.into_iter().map(|item| {
                                                            Self::render_item_card(item, cx)
                                                        })),
                                                )
                                            }),
                                    )
                                })
                        }),
                )
            })
    }
}
