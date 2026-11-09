//! Batch audio file import, drag-and-drop dropzone, stream modal, and queue system.
//! Strict 1:1 parity with React `BatchImport.tsx`, `UrlStreamImportModal.tsx`, `QueueSummaryBar.tsx`,
//! `QueueItem.tsx`, and `QueueGroup.tsx` (Gaps G10, G11, G12).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::motion::{Easing, Keyframe, Keyframes, Timing, animate_keyframes};
use gpui_kit::base::{Disableable as _, FocusableExt as _};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _, DropdownButton};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::project::ProjectRecord;
use sona_core::recovery::types::{
    RecoveredQueueItem, RecoveryItemInput, RecoveryItemStage, RecoverySource,
};
use sona_core::transcription::transcript::TranscriptSegment;

use super::options_capsule::TranscriptionOptionsModel;
use crate::app::{GlobalDesktopServices, GlobalEventBus};
use crate::integrations::media_server::{
    MediaServerSubtitleMode, MediaServerSubtitleUploadPayload,
};
use crate::theme::tokens::{
    COLOR_PROJECT_BLUE, RADIUS_FULL, RADIUS_LG, RADIUS_MD, RADIUS_SM, RADIUS_XL,
};

static RECOVERY_SNAPSHOT_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchAsrSetup {
    LocalModel,
    OnlineProvider,
}

#[derive(Clone, Debug)]
pub enum BatchImportEvent {
    AsrConfigurationRequired(BatchAsrSetup),
    OpenHistoryItem(String),
}

impl EventEmitter<BatchImportEvent> for BatchImportPanel {}

fn required_config_value_exists(config: &serde_json::Value, key: &str) -> bool {
    let snake_key = key
        .chars()
        .flat_map(|character| {
            if character.is_ascii_uppercase() {
                vec!['_', character.to_ascii_lowercase()]
            } else {
                vec![character]
            }
        })
        .collect::<String>();
    config
        .get(key)
        .or_else(|| config.get(snake_key))
        .is_some_and(|value| match value {
            serde_json::Value::String(value) => !value.trim().is_empty(),
            serde_json::Value::Null => false,
            _ => true,
        })
}

fn batch_asr_setup_required(config: &serde_json::Value) -> Option<BatchAsrSetup> {
    let selected_engine = config
        .pointer("/asr/selections/batch/engine")
        .or_else(|| config.pointer("/asr/batch/engine"))
        .and_then(serde_json::Value::as_str);
    let request =
        crate::platform::agent_control::facade::resolve_batch_asr_request_from_config(config, None);
    let Ok(request) = request else {
        return Some(if selected_engine == Some("online") {
            BatchAsrSetup::OnlineProvider
        } else {
            BatchAsrSetup::LocalModel
        });
    };

    match request.engine_config {
        sona_core::ports::asr::AsrEngineConfig::Local { model_path, .. } => model_path
            .trim()
            .is_empty()
            .then_some(BatchAsrSetup::LocalModel),
        sona_core::ports::asr::AsrEngineConfig::Online { provider } => {
            let Some(provider_spec) =
                sona_core::ports::asr::find_online_asr_provider(&provider.provider_id)
            else {
                return Some(BatchAsrSetup::OnlineProvider);
            };
            let capability = &provider_spec.batch;
            let has_api_key = required_config_value_exists(&provider.config, "apiKey")
                || provider_spec
                    .default_api_key_env()
                    .and_then(|name| std::env::var(name).ok())
                    .is_some_and(|value| !value.trim().is_empty());
            let fields_configured = capability
                .required_config_fields
                .iter()
                .filter(|field| !field.eq_ignore_ascii_case("apiKey"))
                .all(|field| required_config_value_exists(&provider.config, field));

            (!capability.local_file_mode.supported
                || (capability.requires_api_key && !has_api_key)
                || !fields_configured)
                .then_some(BatchAsrSetup::OnlineProvider)
        }
    }
}

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
        || rest.starts_with([' ', '/', '?', '#'])
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueSummaryStatus {
    Processing,
    Paused,
    Complete,
    Failed,
    Idle,
}

fn queue_summary_status(
    is_processing: bool,
    is_paused: bool,
    is_all_complete: bool,
    has_failures: bool,
) -> QueueSummaryStatus {
    if is_processing {
        QueueSummaryStatus::Processing
    } else if is_paused {
        QueueSummaryStatus::Paused
    } else if is_all_complete {
        QueueSummaryStatus::Complete
    } else if has_failures {
        QueueSummaryStatus::Failed
    } else {
        QueueSummaryStatus::Idle
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerTaskContext {
    pub profile_id: String,
    pub item_id: String,
    pub audio_stream_index: i32,
    pub subtitle_mode: MediaServerSubtitleMode,
    pub target_language: String,
    pub source_language: Option<String>,
    pub existing_external_subtitle_index: Option<i32>,
    pub container: Option<String>,
}

fn project_assignment(project: Option<&ProjectRecord>) -> (Option<String>, Option<String>) {
    project
        .map(|project| (Some(project.id.clone()), Some(project.name.clone())))
        .unwrap_or_default()
}

fn available_batch_slots(processing_count: usize, max_concurrent: usize) -> usize {
    max_concurrent.saturating_sub(processing_count)
}

fn batch_concurrency_limit(config: Option<&serde_json::Value>) -> usize {
    config
        .and_then(|config| config.get("maxConcurrent"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(2)
}

fn build_batch_hotwords(
    config: &serde_json::Value,
    pipeline: &sona_core::project::EffectivePipelineSnapshot,
    project_id: Option<&str>,
    project_name: Option<&str>,
) -> Option<String> {
    if config
        .get("dictionaryContent")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| !content.trim().is_empty())
    {
        let content = crate::views::settings::vocabulary::effective_dictionary_content(config);
        let mut words = crate::views::settings::vocabulary::parse_dictionary_entries(&content)
            .into_iter()
            .filter(|entry| {
                entry.scope_id == "global"
                    || project_id.is_some_and(|id| entry.scope_id == id)
                    || project_name.is_some_and(|name| entry.scope_name == name)
            })
            .filter_map(|entry| {
                (!entry.raw.contains("->") && !entry.raw.contains("=>"))
                    .then_some(entry.raw.trim().to_string())
            })
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        let mut unique = std::collections::HashSet::new();
        words.retain(|word| unique.insert(word.clone()));
        return (!words.is_empty()).then(|| words.join(","));
    }

    let typed_config =
        serde_json::from_value::<sona_core::config::AppConfig>(config.clone()).unwrap_or_default();
    let mut words = typed_config
        .hotword_sets
        .unwrap_or_default()
        .into_iter()
        .filter(|set| set.enabled)
        .flat_map(|set| {
            set.rules
                .into_iter()
                .map(|rule| rule.text.trim().to_string())
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    for term in &pipeline.custom_terms {
        let term = term.trim();
        if term.is_empty() {
            continue;
        }
        let target = term
            .split_once("=>")
            .or_else(|| term.split_once("->"))
            .map(|(_, target)| target.trim())
            .filter(|target| !target.is_empty())
            .unwrap_or(term);
        words.push(target.to_string());
    }
    let mut unique = std::collections::HashSet::new();
    words.retain(|word| unique.insert(word.clone()));
    (!words.is_empty()).then(|| words.join(","))
}

fn attempt_is_current(current: Option<&str>, completed: &str) -> bool {
    current == Some(completed)
}

fn retry_progress(error_stage: Option<BatchItemErrorStage>, progress: f32) -> f32 {
    if error_stage == Some(BatchItemErrorStage::Upload) {
        progress
    } else {
        0.0
    }
}

fn queue_error_display(filename: &str, file_path: &str, message: &str) -> String {
    let target = if filename.is_empty() {
        file_path
    } else {
        filename
    };
    let target = target.to_ascii_lowercase();
    let message_lower = message.to_ascii_lowercase();
    let transcode_failure = [
        ".mp4", ".webm", ".mov", ".mkv", ".avi", ".wmv", ".flv", ".3gp", ".m4v", ".ts", ".mts",
        ".m2ts",
    ]
    .iter()
    .any(|extension| target.ends_with(extension))
        || [
            "decode",
            "transcode",
            "ffmpeg",
            "format",
            "container",
            "codec",
            "unsupported",
            "转码",
            "解码",
            "symphonia",
        ]
        .iter()
        .any(|term| message_lower.contains(term));

    if transcode_failure
        && (message.starts_with("Failed to decode audio file")
            || message.starts_with("Failed to extract audio slice"))
    {
        "Transcode failed".to_string()
    } else {
        message.to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchItemErrorStage {
    Transcribe,
    Upload,
}

#[derive(Clone, Debug)]
pub struct BatchQueueItem {
    pub id: String,
    pub attempt_id: Option<String>,
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
    pub asr_request: Option<sona_core::ports::asr::AsrTranscriptionRequest>,
    pub pipeline_snapshot: Option<sona_core::project::EffectivePipelineSnapshot>,
    pub export_path: Option<String>,
    pub error_message: Option<String>,
    pub error_stage: Option<BatchItemErrorStage>,
    pub history_id: Option<String>,
    pub segments: Vec<TranscriptSegment>,
    pub media_server_context: Option<MediaServerTaskContext>,
    pub cancel_requested: Option<Arc<AtomicBool>>,
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
    active_project: Option<ProjectRecord>,
    active_task_count: usize,
    pub queue: Vec<BatchQueueItem>,
    pub active_history_id: Option<String>,
    pub is_drag_over: bool,
    is_dropzone_hovered: bool,
    dropzone_hover_generation: u64,
    pub file_drop_error: Option<String>,
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
            active_project: None,
            active_task_count: 0,
            queue: Vec::new(),
            active_history_id: None,
            is_drag_over: false,
            is_dropzone_hovered: false,
            dropzone_hover_generation: 0,
            file_drop_error: None,
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

    pub fn set_active_project(&mut self, project: Option<ProjectRecord>) {
        self.active_project = project;
    }

    fn active_project_assignment(&self) -> (Option<String>, Option<String>) {
        project_assignment(self.active_project.as_ref())
    }

    fn capture_asr_request(
        &self,
        cx: &Context<Self>,
    ) -> Option<sona_core::ports::asr::AsrTranscriptionRequest> {
        let config = cx
            .global::<GlobalDesktopServices>()
            .0
            .config
            .load()
            .ok()??;
        self.capture_asr_request_for_project(&config, self.active_project.as_ref(), cx)
    }

    fn capture_asr_request_for_project(
        &self,
        config: &serde_json::Value,
        project: Option<&ProjectRecord>,
        cx: &Context<Self>,
    ) -> Option<sona_core::ports::asr::AsrTranscriptionRequest> {
        let options = self.options_model.read(cx);
        let language =
            (options.batch_language != "auto").then_some(options.batch_language.as_str());
        crate::platform::agent_control::facade::resolve_batch_asr_request_from_config(
            config, language,
        )
        .ok()
        .map(|mut request| {
            let pipeline = self.capture_pipeline_snapshot(config, project);
            request.hotwords = build_batch_hotwords(
                config,
                &pipeline,
                project.map(|project| project.id.as_str()),
                project.map(|project| project.name.as_str()),
            );
            request
        })
    }

    fn capture_pipeline_snapshot(
        &self,
        config: &serde_json::Value,
        project: Option<&ProjectRecord>,
    ) -> sona_core::project::EffectivePipelineSnapshot {
        let global_config = serde_json::from_value::<sona_core::config::AppConfig>(config.clone())
            .unwrap_or_default();
        let projects = project.into_iter().cloned().collect::<Vec<_>>();
        let mut snapshot = sona_core::project::resolve_item_pipeline(
            project.map(|project| project.id.as_str()),
            &projects,
            &global_config,
        );
        if !snapshot.is_project_pipeline {
            snapshot.auto_translate = config
                .get("autoTranslate")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
        }
        snapshot
    }

    fn capture_pipeline_snapshot_for_active_project(
        &self,
        cx: &Context<Self>,
    ) -> Option<sona_core::project::EffectivePipelineSnapshot> {
        let config = cx
            .global::<GlobalDesktopServices>()
            .0
            .config
            .load()
            .ok()??;
        Some(self.capture_pipeline_snapshot(&config, self.active_project.as_ref()))
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
    ///
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

            if is_match && let Some(prog) = progress_opt {
                item.progress = prog.clamp(0.0, 100.0);
                if prog >= 100.0 && item.media_server_context.is_none() {
                    item.status = BatchItemStatus::Complete;
                } else if item.status == BatchItemStatus::Pending {
                    item.status = BatchItemStatus::Processing;
                }
                updated = true;
            }
        }
        updated
    }

    /// Applies a streaming recognizer update to the queue item that owns it.
    /// Returns a snapshot for a transcript view when that item has a history record.
    pub fn apply_transcript_update(
        queue: &mut [BatchQueueItem],
        instance_id: &str,
        update: &sona_core::transcription::transcript::TranscriptUpdate,
    ) -> Option<(String, Vec<TranscriptSegment>, f64)> {
        let item = queue
            .iter_mut()
            .find(|item| item.instance_id.as_deref() == Some(instance_id))?;
        item.segments
            .retain(|segment| !update.remove_ids.contains(&segment.id));
        for segment in &update.upsert_segments {
            if let Some(existing) = item
                .segments
                .iter_mut()
                .find(|existing| existing.id == segment.id)
            {
                *existing = segment.clone();
            } else {
                item.segments.push(segment.clone());
            }
        }
        item.segments.sort_by(|left, right| {
            left.start
                .total_cmp(&right.start)
                .then_with(|| left.id.cmp(&right.id))
        });
        let history_id = item.history_id.clone()?;
        let duration = item.duration_seconds.unwrap_or_else(|| {
            item.segments
                .iter()
                .map(|segment| segment.end)
                .fold(0.0, f64::max)
        });
        Some((history_id, item.segments.clone(), duration))
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
                        && arr.len() >= 2
                    {
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
                                view.persist_recovery_snapshot(cx, Vec::new());
                                cx.notify();
                            }
                        })
                        .ok();
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

    fn require_asr_configuration(&mut self, cx: &mut Context<Self>) -> bool {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let config = services
            .config
            .load()
            .ok()
            .flatten()
            .unwrap_or(serde_json::Value::Null);
        let Some(setup) = batch_asr_setup_required(&config) else {
            return true;
        };

        cx.emit(BatchImportEvent::AsrConfigurationRequired(setup));
        false
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
                    if paths.iter().any(|path| {
                        is_supported_audio_extension(
                            path.extension()
                                .and_then(|ext| ext.to_str())
                                .unwrap_or_default(),
                        )
                    }) && !view.require_asr_configuration(cx)
                    {
                        return;
                    }
                    let asr_request = view.capture_asr_request(cx);
                    let pipeline_snapshot = view.capture_pipeline_snapshot_for_active_project(cx);
                    for path in paths {
                        view.add_file_path_with_request(
                            path,
                            asr_request.clone(),
                            pipeline_snapshot.clone(),
                        );
                    }
                    view.persist_recovery_snapshot(cx, Vec::new());
                    view.start_next_pending(cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn add_file_path(&mut self, path: PathBuf, cx: &Context<Self>) -> bool {
        let asr_request = self.capture_asr_request(cx);
        let pipeline_snapshot = self.capture_pipeline_snapshot_for_active_project(cx);
        self.add_file_path_with_request(path, asr_request, pipeline_snapshot)
    }

    fn add_file_path_with_request(
        &mut self,
        path: PathBuf,
        asr_request: Option<sona_core::ports::asr::AsrTranscriptionRequest>,
        pipeline_snapshot: Option<sona_core::project::EffectivePipelineSnapshot>,
    ) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();

        if !is_supported_audio_extension(&ext) {
            return false;
        }

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio_file".to_string());
        let file_path = path.to_string_lossy().to_string();
        let file_size_bytes = std::fs::metadata(&path).map(|m| m.len()).ok();
        let id = format!("batch-{}", uuid::Uuid::new_v4());
        let (project_id, project_name) = self.active_project_assignment();

        self.queue.push(BatchQueueItem {
            id,
            attempt_id: None,
            instance_id: None,
            filename,
            file_path,
            source_type: BatchSourceType::File,
            status: BatchItemStatus::Pending,
            progress: 0.0,
            file_size_bytes,
            duration_seconds: None,
            project_id,
            project_name,
            project_color_index: None,
            asr_request,
            pipeline_snapshot,
            export_path: None,
            error_message: None,
            error_stage: None,
            history_id: None,
            segments: Vec::new(),
            media_server_context: None,
            cancel_requested: None,
            is_media_server: false,
        });
        true
    }

    fn enqueue_dropped_paths(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let paths = paths.into_iter().collect::<Vec<_>>();
        let has_supported_file = paths.iter().any(|path| {
            is_supported_audio_extension(
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default(),
            )
        });
        if has_supported_file && !self.require_asr_configuration(cx) {
            self.is_drag_over = false;
            cx.notify();
            return;
        }

        let asr_request = self.capture_asr_request(cx);
        let pipeline_snapshot = self.capture_pipeline_snapshot_for_active_project(cx);
        let mut accepted = 0;
        let mut rejected = 0;
        for path in paths {
            if self.add_file_path_with_request(path, asr_request.clone(), pipeline_snapshot.clone())
            {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }

        self.file_drop_error = (rejected > 0).then(|| {
            format!(
                "{rejected} dropped file(s) were skipped. Supported formats: {}.",
                SUPPORTED_EXTENSIONS.join(", "),
            )
        });

        if accepted > 0 {
            self.persist_recovery_snapshot(cx, Vec::new());
            self.start_next_pending(cx);
        }
        cx.notify();
    }

    pub fn enqueue_retry_file(
        &mut self,
        path: PathBuf,
        project_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self.require_asr_configuration(cx) {
            return;
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let projects = services.projects.list().await.unwrap_or_default();
            let project = project_id
                .as_ref()
                .and_then(|id| projects.into_iter().find(|project| &project.id == id));
            let config = services.config.load().ok().flatten();
            let _ = this.update(cx, |view, cx| {
                let Some(config) = config.as_ref() else {
                    return;
                };
                let asr_request =
                    view.capture_asr_request_for_project(config, project.as_ref(), cx);
                let pipeline_snapshot =
                    Some(view.capture_pipeline_snapshot(config, project.as_ref()));
                let previous_len = view.queue.len();
                if !view.add_file_path_with_request(path, asr_request, pipeline_snapshot)
                    || view.queue.len() == previous_len
                {
                    return;
                }
                if let Some(item) = view.queue.last_mut() {
                    item.project_id = project_id;
                    item.project_name = project.map(|project| project.name);
                }
                view.start_next_pending(cx);
                view.persist_recovery_snapshot(cx, Vec::new());
                cx.notify();
            });
        })
        .detach();
    }

    pub fn cancel_task_instance(&mut self, task_id: &str, cx: &mut Context<Self>) -> bool {
        let item_id = self
            .queue
            .iter()
            .find(|item| item.instance_id.as_deref() == Some(task_id))
            .map(|item| item.id.clone());
        if let Some(item_id) = item_id {
            self.cancel_item(item_id, cx);
            true
        } else {
            false
        }
    }

    pub fn enqueue_recovered_items(
        &mut self,
        items: impl IntoIterator<Item = RecoveredQueueItem>,
        cx: &mut Context<Self>,
    ) {
        let asr_request = self.capture_asr_request(cx);
        for item in items {
            if item.source != sona_core::recovery::types::RecoverySource::BatchImport
                || !item.can_resume
                || self.queue.iter().any(|queued| queued.id == item.id)
            {
                continue;
            }

            self.queue.push(BatchQueueItem {
                id: item.id,
                attempt_id: None,
                instance_id: None,
                filename: item.filename,
                file_path: item.file_path,
                source_type: BatchSourceType::File,
                status: BatchItemStatus::Pending,
                progress: 0.0,
                file_size_bytes: None,
                duration_seconds: None,
                project_id: item.tag_ids.first().cloned(),
                project_name: None,
                project_color_index: None,
                asr_request: asr_request.clone(),
                pipeline_snapshot: None,
                export_path: None,
                error_message: None,
                error_stage: None,
                history_id: item.history_id,
                segments: item
                    .segments
                    .into_iter()
                    .filter_map(|segment| {
                        serde_json::from_value::<TranscriptSegment>(
                            serde_json::to_value(segment).ok()?,
                        )
                        .ok()
                    })
                    .collect(),
                media_server_context: item
                    .source_metadata
                    .as_ref()
                    .and_then(|value| serde_json::from_value(value.clone()).ok()),
                cancel_requested: None,
                is_media_server: item.source_metadata.is_some(),
            });
        }

        self.pending_expanded = true;
        self.persist_recovery_snapshot(cx, Vec::new());
        self.start_next_pending(cx);
        cx.notify();
    }

    pub fn add_stream_url(
        &mut self,
        url: String,
        alias: Option<String>,
        retain_audio: bool,
        cx: &Context<Self>,
    ) {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        let filename = alias
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| trimmed.clone());
        let id = format!("stream-{}", uuid::Uuid::new_v4());
        let (project_id, project_name) = self.active_project_assignment();
        let asr_request = self.capture_asr_request(cx);
        let pipeline_snapshot = self.capture_pipeline_snapshot_for_active_project(cx);

        self.queue.push(BatchQueueItem {
            id,
            attempt_id: None,
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
            project_id,
            project_name,
            project_color_index: None,
            asr_request,
            pipeline_snapshot,
            export_path: None,
            error_message: None,
            error_stage: None,
            history_id: None,
            segments: Vec::new(),
            media_server_context: None,
            cancel_requested: None,
            is_media_server: false,
        });
    }

    pub fn add_media_server_items(
        &mut self,
        items: Vec<(String, MediaServerTaskContext)>,
        cx: &mut Context<Self>,
    ) -> usize {
        if !items.is_empty() && !self.require_asr_configuration(cx) {
            return 0;
        }
        let mut existing = self
            .queue
            .iter()
            .filter(|item| {
                matches!(
                    item.status,
                    BatchItemStatus::Pending | BatchItemStatus::Processing
                )
            })
            .filter_map(|item| item.media_server_context.as_ref())
            .map(|context| format!("{}::{}", context.profile_id, context.item_id))
            .collect::<std::collections::HashSet<_>>();
        let mut added = 0;
        let asr_request = self.capture_asr_request(cx);
        let pipeline_snapshot = self.capture_pipeline_snapshot_for_active_project(cx);
        for (filename, context) in items {
            let key = format!("{}::{}", context.profile_id, context.item_id);
            if !existing.insert(key) {
                continue;
            }
            let (project_id, project_name) = self.active_project_assignment();
            self.queue.push(BatchQueueItem {
                id: format!("media-server-{}", uuid::Uuid::new_v4()),
                attempt_id: None,
                instance_id: None,
                filename,
                file_path: String::new(),
                source_type: BatchSourceType::File,
                status: BatchItemStatus::Pending,
                progress: 0.0,
                file_size_bytes: None,
                duration_seconds: None,
                project_id,
                project_name,
                project_color_index: None,
                asr_request: asr_request.clone(),
                pipeline_snapshot: pipeline_snapshot.clone(),
                export_path: None,
                error_message: None,
                error_stage: None,
                history_id: None,
                segments: Vec::new(),
                media_server_context: Some(context),
                cancel_requested: None,
                is_media_server: true,
            });
            added += 1;
        }

        if added > 0 {
            self.media_server_expanded = true;
            self.pending_expanded = true;
            self.persist_recovery_snapshot(cx, Vec::new());
            self.start_next_pending(cx);
            cx.notify();
        }
        added
    }

    /// Fills available queue slots and tracks each task through its terminal ledger state.
    pub fn start_next_pending(&mut self, cx: &mut Context<Self>) {
        if self.is_paused
            || !self
                .queue
                .iter()
                .any(|item| item.status == BatchItemStatus::Pending)
        {
            return;
        }
        if !self.require_asr_configuration(cx) {
            return;
        }

        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let config = services.config.load().ok().flatten();
        let configured_concurrency = batch_concurrency_limit(config.as_ref());
        let slots = available_batch_slots(self.active_task_count, configured_concurrency);
        if slots == 0 {
            return;
        }

        for _ in 0..slots {
            let Some(idx) = self
                .queue
                .iter()
                .position(|item| item.status == BatchItemStatus::Pending)
            else {
                break;
            };
            let item = &mut self.queue[idx];
            let item_id = item.id.clone();
            let filename = item.filename.clone();
            let mut file_path = item.file_path.clone();
            let media_context = item.media_server_context.clone();
            let upload_retry = media_context.is_some()
                && item.error_stage == Some(BatchItemErrorStage::Upload)
                && item.history_id.is_some();
            let instance_id =
                (!upload_retry).then(|| format!("batch-task-{}", uuid::Uuid::new_v4()));
            let cancel_requested = Arc::new(AtomicBool::new(false));
            item.cancel_requested = Some(Arc::clone(&cancel_requested));
            item.instance_id = instance_id.clone();
            item.status = BatchItemStatus::Processing;
            item.progress = 0.0;
            let attempt_id = uuid::Uuid::new_v4().to_string();
            item.attempt_id = Some(attempt_id.clone());
            self.active_task_count += 1;

            let options = self.options_model.read(cx);
            let language = if options.batch_language == "auto" {
                None
            } else {
                Some(options.batch_language.clone())
            };
            let project_id = item.project_id.clone();
            let history_id = item.history_id.clone();
            let asr_request = item.asr_request.clone();
            let pipeline_snapshot = item.pipeline_snapshot.clone();
            let mut segments = item.segments.clone();
            let services = services.clone();
            self.persist_recovery_snapshot(cx, Vec::new());

            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let mut downloaded_path: Option<std::path::PathBuf> = None;
                let mut error_stage = if upload_retry {
                    BatchItemErrorStage::Upload
                } else {
                    BatchItemErrorStage::Transcribe
                };
                let result: Result<(String, Vec<TranscriptSegment>, Option<String>), String> = async {
                    let resolved_history_id;
                    let mut export_path = None;

                    if let Some(context) = media_context.as_ref() {
                        if upload_retry {
                            if segments.is_empty() {
                                segments =
                                    load_history_transcript(&services, history_id.clone()).await?;
                            }
                        } else {
                            if cancel_requested.load(Ordering::Acquire) {
                                return Err("Task cancelled.".to_string());
                            }
                            let extension = context
                                .container
                                .as_deref()
                                .map(safe_media_extension)
                                .unwrap_or_else(|| "mkv".to_string());
                            let path = std::env::temp_dir().join(format!(
                                "sona-media-server-{}.{}",
                                uuid::Uuid::new_v4(),
                                extension
                            ));
                            this.update(cx, |view, cx| {
                                if let Some(target) =
                                    view.queue.iter_mut().find(|item| item.id == item_id)
                                {
                                    target.progress = 2.0;
                                }
                                cx.notify();
                            })
                            .ok();
                            services
                                .media_server
                                .download_audio(
                                    Some(&context.profile_id),
                                    &context.item_id,
                                    Some(context.audio_stream_index),
                                    &path,
                                )
                                .await?;
                            file_path = path.to_string_lossy().into_owned();
                            downloaded_path = Some(path);
                            if cancel_requested.load(Ordering::Acquire) {
                                return Err("Task cancelled.".to_string());
                            }
                        }
                    }

                    if !upload_retry {
                        let Some(instance_id) = instance_id.as_ref() else {
                            return Err("Batch task instance ID is missing.".to_string());
                        };
                        let request =
                            crate::platform::agent_control::facade::types::TranscribeFileRequest {
                                file_path: file_path.clone(),
                                language: language.clone(),
                                project_id: project_id.clone(),
                                history_id: None,
                                instance_id: Some(instance_id.clone()),
                                save_to_path: None,
                            };
                        let started =
                            crate::platform::agent_control::facade::batch::transcribe_file_with_asr_request(
                                &services,
                                request,
                                asr_request.clone(),
                            )
                            .await?;
                        resolved_history_id = Some(started.history_id.clone());

                        if let Some(context) = media_context.as_ref() {
                            let history_id_for_title = started.history_id.clone();
                            let title = filename.clone();
                            let request =
                            sona_core::history::mutation_repository::HistoryUpdateItemMetaRequest {
                                history_id: history_id_for_title,
                                updates:
                                    sona_core::history::mutation_repository::HistoryItemMetaPatch {
                                        title: Some(title),
                                        ..Default::default()
                                    },
                            };
                            let _ = services
                                .history
                                .mutation_file(move |repository| {
                                    repository.update_item_meta(request)
                                })
                                .await;
                            let _ = context;
                        }

                        let history_id_for_queue = started.history_id.clone();
                        let task_id_for_queue = instance_id.clone();
                        this.update(cx, |view, cx| {
                            if let Some(target) =
                                view.queue.iter_mut().find(|item| item.id == item_id)
                            {
                                target.history_id = Some(history_id_for_queue);
                                target.instance_id = Some(task_id_for_queue);
                            }
                            view.persist_recovery_snapshot(cx, Vec::new());
                        })
                        .ok();

                        let mut cancel_sent = false;
                        loop {
                            if cancel_requested.load(Ordering::Acquire) && !cancel_sent {
                                let _ =
                                crate::platform::agent_control::facade::batch::cancel_batch_task(
                                    &services,
                                    instance_id.clone(),
                                )
                                .await;
                                cancel_sent = true;
                            }
                            let records = services.task_ledger.list_tasks().await?;
                            let task = records
                                .iter()
                                .find(|task| task.id == *instance_id)
                                .ok_or_else(|| {
                                    format!(
                                        "Batch task {instance_id} disappeared from the task ledger."
                                    )
                                })?;
                            let progress = task.progress.clamp(0.0, 100.0) as f32;
                            this.update(cx, |view, cx| {
                                if let Some(target) =
                                    view.queue.iter_mut().find(|item| item.id == item_id)
                                    && target.status == BatchItemStatus::Processing
                                {
                                    target.progress = if media_context.is_some() {
                                        progress.min(92.0)
                                    } else {
                                        progress
                                    };
                                }
                                cx.notify();
                            })
                            .ok();

                            match &task.status {
                                sona_core::task_ledger::types::TaskLedgerStatus::Succeeded => break,
                                sona_core::task_ledger::types::TaskLedgerStatus::Failed
                                | sona_core::task_ledger::types::TaskLedgerStatus::Interrupted
                                | sona_core::task_ledger::types::TaskLedgerStatus::Recoverable => {
                                    return Err(task.error_message.clone().unwrap_or_else(|| {
                                        format!("Transcription failed for {filename}.")
                                    }));
                                }
                                sona_core::task_ledger::types::TaskLedgerStatus::Cancelled => {
                                    return Err("Task cancelled.".to_string());
                                }
                                _ => {
                                    tokio::time::sleep(std::time::Duration::from_millis(250)).await
                                }
                            }
                        }

                        let history_id_for_load = started.history_id.clone();
                        segments =
                            load_history_transcript(&services, Some(history_id_for_load)).await?;
                    } else {
                        resolved_history_id = history_id.clone();
                    }

                    let resolved_history_id = resolved_history_id.ok_or_else(|| {
                        "Transcription did not create a history record.".to_string()
                    })?;
                    if !upload_retry {
                        let config = services
                            .config
                            .load()
                            .ok()
                            .flatten()
                            .unwrap_or_else(|| serde_json::json!({}));
                        let pipeline = if let Some(snapshot) = pipeline_snapshot.clone() {
                            snapshot
                        } else {
                            let projects = services.projects.list().await?;
                            let app_config = serde_json::from_value::<sona_core::config::AppConfig>(config.clone()).unwrap_or_default();
                            let mut snapshot = sona_core::project::resolve_item_pipeline(
                                project_id.as_deref(),
                                &projects,
                                &app_config,
                            );
                            if !snapshot.is_project_pipeline {
                                snapshot.auto_translate = config
                                    .get("autoTranslate")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false);
                            }
                            snapshot
                        };
                        this.update(cx, |view, cx| {
                            if let Some(target) = view.queue.iter_mut().find(|item| item.id == item_id) {
                                target.progress = 95.0;
                            }
                            cx.notify();
                        }).ok();
                        let pipeline_result = crate::services::batch_pipeline::execute(
                            &services,
                            crate::services::batch_pipeline::BatchPipelineOptions {
                                config: &config,
                                pipeline: &pipeline,
                                project_id: project_id.as_deref(),
                                history_id: &resolved_history_id,
                                filename: &filename,
                                segments: segments.clone(),
                                cancelled: &|| cancel_requested.load(Ordering::Acquire),
                            },
                        ).await?;
                        segments = pipeline_result.segments;
                        export_path = pipeline_result.export_path;
                    }
                    if let Some(context) = media_context.as_ref() {
                        if cancel_requested.load(Ordering::Acquire) {
                            return Err("Task cancelled.".to_string());
                        }
                        error_stage = BatchItemErrorStage::Upload;
                        this.update(cx, |view, cx| {
                            if let Some(target) =
                                view.queue.iter_mut().find(|item| item.id == item_id)
                            {
                                target.progress = 96.0;
                                target.segments = segments.clone();
                            }
                            cx.notify();
                        })
                        .ok();
                        translate_media_server_segments(
                            &services,
                            context,
                            &resolved_history_id,
                            &mut segments,
                        )
                        .await?;
                        upload_media_server_subtitles(&services, context, &segments).await?;
                    }
                    Ok((resolved_history_id, segments.clone(), export_path))
                }
                .await;

                if let Some(path) = downloaded_path {
                    let _ = std::fs::remove_file(path);
                }

                this.update(cx, |view, cx| {
                    view.active_task_count = view.active_task_count.saturating_sub(1);
                    if let Some(target) = view.queue.iter_mut().find(|item| item.id == item_id)
                        && attempt_is_current(target.attempt_id.as_deref(), &attempt_id)
                    {
                        target.attempt_id = None;
                            match result {
                                Ok((history_id, segments, export_path)) => {
                                    target.history_id = Some(history_id);
                                    target.segments = segments;
                                    target.export_path = export_path;
                                    target.duration_seconds = target
                                        .segments
                                        .iter()
                                        .map(|segment| segment.end)
                                        .reduce(f64::max);
                                    target.status = BatchItemStatus::Complete;
                                    target.progress = 100.0;
                                    target.error_message = None;
                                    target.error_stage = None;
                                }
                                Err(message)
                                    if cancel_requested.load(Ordering::Acquire)
                                        || message.contains("Task cancelled") =>
                                {
                                    target.status = BatchItemStatus::Cancelled;
                                    target.progress = 0.0;
                                    target.error_message = None;
                                }
                                Err(message) => {
                                    target.status = BatchItemStatus::Error;
                                    target.error_message = Some(message);
                                    target.error_stage = Some(error_stage);
                                    if error_stage == BatchItemErrorStage::Transcribe {
                                        target.history_id = None;
                                        target.segments.clear();
                                    } else {
                                        target.segments = segments.clone();
                                    }
                                }
                            }
                        target.instance_id = None;
                        target.cancel_requested = None;
                    }
                    if view.queue.iter().all(|item| {
                        item.status != BatchItemStatus::Processing
                            && item.status != BatchItemStatus::Pending
                    }) {
                        view.completed_expanded = true;
                    }
                    view.start_next_pending(cx);
                    view.persist_recovery_snapshot(cx, Vec::new());
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
            if let Some(cancel_requested) = &item.cancel_requested {
                cancel_requested.store(true, Ordering::Release);
            }
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
        self.persist_recovery_snapshot(cx, Vec::new());
    }

    /// Removes a queue entry and signals cancellation first when it is running.
    pub fn remove_item(&mut self, item_id: String, cx: &mut Context<Self>) {
        let Some(index) = self.queue.iter().position(|item| item.id == item_id) else {
            return;
        };
        let item = self.queue.remove(index);
        if let Some(cancel_requested) = item.cancel_requested.as_ref() {
            cancel_requested.store(true, Ordering::Release);
        }
        if item.status == BatchItemStatus::Processing
            && let Some(instance_id) = item.instance_id
        {
            let services = cx.global::<GlobalDesktopServices>().0.clone();
            cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                let _ = crate::platform::agent_control::facade::batch::cancel_batch_task(
                    &services,
                    instance_id,
                )
                .await;
            })
            .detach();
        }
        self.persist_recovery_snapshot(cx, vec![item.id]);
        self.start_next_pending(cx);
        cx.notify();
    }

    /// Retries a failed or cancelled item.
    pub fn retry_item(&mut self, item_id: String, cx: &mut Context<Self>) {
        if let Some(item) = self.queue.iter_mut().find(|i| i.id == item_id) {
            item.status = BatchItemStatus::Pending;
            item.progress = retry_progress(item.error_stage, item.progress);
            item.error_message = None;
            item.instance_id = None;
            item.cancel_requested = None;
            item.attempt_id = None;
        }
        self.start_next_pending(cx);
        self.persist_recovery_snapshot(cx, Vec::new());
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
                item.progress = retry_progress(item.error_stage, item.progress);
                item.error_message = None;
                item.instance_id = None;
                item.cancel_requested = None;
                item.attempt_id = None;
            }
        }
        self.start_next_pending(cx);
        self.persist_recovery_snapshot(cx, Vec::new());
        cx.notify();
    }

    /// Clears completed items from the queue.
    pub fn clear_completed(&mut self, cx: &mut Context<Self>) {
        let resolved_ids = self
            .queue
            .iter()
            .filter(|item| item.status == BatchItemStatus::Complete)
            .map(|item| item.id.clone())
            .collect();
        self.queue.retain(|i| i.status != BatchItemStatus::Complete);
        self.persist_recovery_snapshot(cx, resolved_ids);
        cx.notify();
    }

    /// Clears all items from the queue.
    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let resolved_ids = self.queue.iter().map(|item| item.id.clone()).collect();
        for item in &self.queue {
            if item.status == BatchItemStatus::Processing
                && let Some(inst_id) = item.instance_id.clone()
            {
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
        self.queue.clear();
        self.persist_recovery_snapshot(cx, resolved_ids);
        self.is_clear_menu_open = false;
        self.is_clear_confirm_open = false;
        cx.notify();
    }

    pub fn toggle_queue_pause(&mut self, cx: &mut Context<Self>) {
        self.is_paused = !self.is_paused;
        if !self.is_paused {
            self.start_next_pending(cx);
        }
        self.persist_recovery_snapshot(cx, Vec::new());
        cx.notify();
    }

    fn persist_recovery_snapshot(&self, cx: &mut Context<Self>, resolved_ids: Vec<String>) {
        let items = self
            .queue
            .iter()
            .map(recovery_input_for_queue_item)
            .collect::<Vec<_>>();
        let provider = crate::platform::system::paths::DesktopPathProvider::from_default();
        let generation = RECOVERY_SNAPSHOT_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let immediate = !resolved_ids.is_empty();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            if !immediate {
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                if RECOVERY_SNAPSHOT_GENERATION.load(Ordering::Relaxed) != generation {
                    return;
                }
            }
            if let Err(error) = crate::platform::recovery_repository::persist_queue_snapshot(
                &provider,
                items,
                Some(resolved_ids),
            )
            .await
            {
                log::warn!("Failed to persist batch recovery snapshot: {error}");
            }
        })
        .detach();
    }
}

fn recovery_input_for_queue_item(item: &BatchQueueItem) -> RecoveryItemInput {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or_default();
    let source_available = item.media_server_context.is_some()
        || match &item.source_type {
            BatchSourceType::File => Path::new(&item.file_path).is_file(),
            BatchSourceType::UrlStream { url, .. } => is_valid_stream_url(url),
        };
    let stage = if item.status == BatchItemStatus::Processing {
        RecoveryItemStage::Transcribing
    } else {
        RecoveryItemStage::Queued
    };
    let stage = serde_json::to_value(stage)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned));
    let status = match item.status {
        BatchItemStatus::Pending => "pending",
        BatchItemStatus::Processing => "processing",
        BatchItemStatus::Complete => "complete",
        BatchItemStatus::Error => "error",
        BatchItemStatus::Cancelled => "cancelled",
    };
    RecoveryItemInput {
        id: Some(item.id.clone()),
        recovery_id: Some(item.id.clone()),
        filename: Some(item.filename.clone()),
        file_path: Some(item.file_path.clone()),
        source: Some(RecoverySource::BatchImport.to_string()),
        origin: Some("manual".to_string()),
        resolution: Some("pending".to_string()),
        status: Some(status.to_string()),
        progress: Some(item.progress as f64),
        tag_ids: item.project_id.iter().cloned().collect(),
        project_id: item.project_id.clone(),
        history_id: item.history_id.clone(),
        history_title: None,
        source_metadata: item
            .media_server_context
            .as_ref()
            .and_then(|context| serde_json::to_value(context).ok()),
        last_known_stage: stage,
        updated_at: Some(now_ms),
        has_source_file: Some(source_available),
        can_resume: Some(source_available),
        ..RecoveryItemInput::default()
    }
}

async fn load_history_transcript(
    services: &crate::services::DesktopServices,
    history_id: Option<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    let history_id = history_id.ok_or_else(|| "History ID is missing.".to_string())?;
    let lookup_id = history_id.clone();
    services
        .history
        .query_db(move |repository| repository.load_transcript(&lookup_id))
        .await?
        .ok_or_else(|| format!("Transcript not found for history ID: {history_id}"))
}

fn safe_media_extension(container: &str) -> String {
    let extension = container
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if !extension.is_empty()
        && extension.len() <= 8
        && extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        extension
    } else {
        "mkv".to_string()
    }
}

fn clean_subtitle_text(text: &str) -> String {
    text.replace(['\0', '\r'], "").trim().to_string()
}

async fn translate_media_server_segments(
    services: &crate::services::DesktopServices,
    context: &MediaServerTaskContext,
    history_id: &str,
    segments: &mut [TranscriptSegment],
) -> Result<(), String> {
    let missing: Vec<_> = segments
        .iter()
        .enumerate()
        .filter(|(_, segment)| {
            segment
                .translation
                .as_deref()
                .is_none_or(|translation| clean_subtitle_text(translation).is_empty())
                && !clean_subtitle_text(&segment.text).is_empty()
        })
        .map(|(index, segment)| (index, segment.id.clone(), segment.text.clone()))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }

    use sona_core::ports::llm::{LlmTranslationPort, LlmTranslationRequest};
    let app_config = services
        .config
        .load()?
        .ok_or_else(|| "Application configuration is unavailable for translation.".to_string())?;
    let config = crate::platform::repositories::api_server_config::
        resolve_feature_llm_config_from_app_config(&app_config, "translation")
        .ok_or_else(|| "No translation model is configured for media server subtitles.".to_string())?;
    let adapter = crate::integrations::llm::DesktopLlmAdapter::with_pool(
        sona_runtime_fs::default_desktop_models_dir(),
        None,
        Some(Arc::clone(&services.translation_pool)),
    );
    let translations = adapter
        .translate_batch(LlmTranslationRequest {
            config,
            texts: missing.iter().map(|(_, _, text)| text.clone()).collect(),
            target_language: context.target_language.clone(),
            source_language: context.source_language.clone(),
        })
        .await
        .map_err(|error| error.to_string())?;
    if translations.len() != missing.len() {
        return Err(format!(
            "Translation returned {} results for {} transcript segments.",
            translations.len(),
            missing.len()
        ));
    }

    let mut persisted = Vec::with_capacity(missing.len());
    for ((index, id, _), translation) in missing.into_iter().zip(translations) {
        segments[index].translation = Some(translation.clone());
        persisted.push(
            crate::platform::agent_control::facade::SegmentTranslationInput {
                id,
                translation: Some(translation),
            },
        );
    }
    crate::platform::agent_control::facade::update_translations(
        services,
        crate::platform::agent_control::facade::UpdateTranslationsRequest {
            history_id: history_id.to_string(),
            translations: persisted,
            clear_all: Some(false),
        },
    )
    .await?;
    Ok(())
}

fn format_srt_timestamp(seconds: f64) -> String {
    let milliseconds = if seconds.is_finite() {
        (seconds.max(0.0) * 1000.0).floor() as u64
    } else {
        0
    };
    let hours = milliseconds / 3_600_000;
    let minutes = (milliseconds % 3_600_000) / 60_000;
    let seconds = (milliseconds % 60_000) / 1_000;
    let millis = milliseconds % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

fn build_srt(
    segments: &[TranscriptSegment],
    select_text: impl Fn(&TranscriptSegment) -> String,
) -> String {
    let mut blocks = Vec::new();
    for segment in segments {
        let text = select_text(segment);
        if text.trim().is_empty() {
            continue;
        }
        blocks.push(format!(
            "{}\n{} --> {}\n{}",
            blocks.len() + 1,
            format_srt_timestamp(segment.start),
            format_srt_timestamp(segment.end.max(segment.start)),
            text
        ));
    }
    if blocks.is_empty() {
        String::new()
    } else {
        format!("{}\n", blocks.join("\n\n"))
    }
}

async fn upload_media_server_subtitles(
    services: &crate::services::DesktopServices,
    context: &MediaServerTaskContext,
    segments: &[TranscriptSegment],
) -> Result<(), String> {
    let upload = |language: String, content: String, delete_existing_index| {
        let service = Arc::clone(&services.media_server);
        let profile_id = context.profile_id.clone();
        let payload = MediaServerSubtitleUploadPayload {
            item_id: context.item_id.clone(),
            language,
            format: "srt".to_string(),
            is_forced: false,
            content,
        };
        async move {
            service
                .upload_subtitle(Some(&profile_id), payload, delete_existing_index)
                .await
        }
    };

    match context.subtitle_mode {
        MediaServerSubtitleMode::PureTranslation => {
            let content = build_srt(segments, |segment| {
                let translation = segment
                    .translation
                    .as_deref()
                    .map(clean_subtitle_text)
                    .unwrap_or_default();
                if translation.is_empty() {
                    clean_subtitle_text(&segment.text)
                } else {
                    translation
                }
            });
            if !content.trim().is_empty() {
                upload(
                    context.target_language.clone(),
                    content,
                    context.existing_external_subtitle_index,
                )
                .await?;
            }
        }
        MediaServerSubtitleMode::Bilingual => {
            let content = build_srt(segments, |segment| {
                let translation = segment
                    .translation
                    .as_deref()
                    .map(clean_subtitle_text)
                    .unwrap_or_default();
                let original = clean_subtitle_text(&segment.text);
                if !translation.is_empty()
                    && !original.is_empty()
                    && !translation.eq_ignore_ascii_case(&original)
                {
                    format!("{translation}\n{original}")
                } else if translation.is_empty() {
                    original
                } else {
                    translation
                }
            });
            if !content.trim().is_empty() {
                upload(
                    context.target_language.clone(),
                    content,
                    context.existing_external_subtitle_index,
                )
                .await?;
            }
        }
        MediaServerSubtitleMode::SeparateTracks => {
            let has_translation = segments.iter().any(|segment| {
                segment
                    .translation
                    .as_deref()
                    .is_some_and(|translation| !clean_subtitle_text(translation).is_empty())
            });
            if has_translation {
                let translation = build_srt(segments, |segment| {
                    let translation = segment
                        .translation
                        .as_deref()
                        .map(clean_subtitle_text)
                        .unwrap_or_default();
                    if translation.is_empty() {
                        clean_subtitle_text(&segment.text)
                    } else {
                        translation
                    }
                });
                if !translation.trim().is_empty() {
                    upload(
                        context.target_language.clone(),
                        translation,
                        context.existing_external_subtitle_index,
                    )
                    .await?;
                }
            }
            let original = build_srt(segments, |segment| clean_subtitle_text(&segment.text));
            if !original.trim().is_empty() {
                upload(
                    context
                        .source_language
                        .clone()
                        .unwrap_or_else(|| "und".to_string()),
                    original,
                    None,
                )
                .await?;
            }
        }
    }
    Ok(())
}

impl BatchImportPanel {
    pub fn render_item_card(
        item: &BatchQueueItem,
        active_history_id: Option<&str>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let item_id = item.id.clone();
        let status = item.status;
        let progress = item.progress;
        let is_stream = matches!(item.source_type, BatchSourceType::UrlStream { .. });
        let is_pulling_media_audio = item.is_media_server
            && status == BatchItemStatus::Processing
            && item.file_path.is_empty();
        let filename = item.filename.clone();
        let project_name = item.project_name.clone();
        let error_message = item
            .error_message
            .as_ref()
            .map(|message| queue_error_display(&item.filename, &item.file_path, message));
        let is_active = item
            .history_id
            .as_deref()
            .is_some_and(|id| active_history_id == Some(id));

        div()
            .id(ElementId::from(item.id.clone()))
            .role(Role::ListItem)
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(RADIUS_SM)
            .border_1()
            .border_color(if is_active {
                cx.theme().border
            } else {
                cx.theme().transparent
            })
            .when(is_active, |row| {
                row.bg(cx.theme().secondary.opacity(0.38)).shadow_sm()
            })
            .hover(|s| s.bg(cx.theme().secondary.opacity(0.45)))
            .when(item.history_id.is_some(), |row| {
                let history_id = item.history_id.clone().expect("checked above");
                row.cursor_pointer()
                    .tab_stop(true)
                    .on_click(cx.listener({
                        let history_id = history_id.clone();
                        move |_, _, _, cx| {
                            cx.emit(BatchImportEvent::OpenHistoryItem(history_id.clone()));
                        }
                    }))
                    .on_key_down(cx.listener(move |_, event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.to_ascii_lowercase();
                        if key == "enter" || key == "space" {
                            window.prevent_default();
                            cx.stop_propagation();
                            cx.emit(BatchImportEvent::OpenHistoryItem(history_id.clone()));
                        }
                    }))
            })
            // Status icon
            .child(
                div()
                    .size(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
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
                    }),
            )
            // Filename, processing progress, and source metadata
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(filename),
                            )
                            .when(
                                status == BatchItemStatus::Processing && !is_pulling_media_audio,
                                |row| {
                                    row.child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(COLOR_PROJECT_BLUE)
                                            .child(format!("{:.0}%", progress)),
                                    )
                                },
                            )
                            .when(is_pulling_media_audio, |row| {
                                row.child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(11.0))
                                        .text_color(COLOR_PROJECT_BLUE)
                                        .child("Fetching audio track..."),
                                )
                            }),
                    )
                    .when(status == BatchItemStatus::Processing, |content| {
                        content.child(
                            div()
                                .h(px(5.0))
                                .w_full()
                                .rounded_full()
                                .bg(cx.theme().secondary.opacity(0.5))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(progress.clamp(0.0, 100.0) / 100.0))
                                        .rounded_full()
                                        .bg(COLOR_PROJECT_BLUE),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .overflow_hidden()
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
                            .when(item.is_media_server, |this| {
                                this.child(
                                    div()
                                        .px(px(6.0))
                                        .py(px(1.0))
                                        .rounded(RADIUS_SM)
                                        .bg(cx.theme().success.opacity(0.12))
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(cx.theme().success)
                                        .flex()
                                        .items_center()
                                        .gap(px(3.0))
                                        .child(icon(LucideIcon::Tv).size(px(10.0)))
                                        .child("Media Server"),
                                )
                            })
                            .when(!is_stream && !item.is_media_server, |this| {
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
                            .when_some(project_name, |this, project_name| {
                                this.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(5.0))
                                        .max_w(px(140.0))
                                        .child(
                                            div()
                                                .size(px(6.0))
                                                .rounded_full()
                                                .bg(cx.theme().muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(cx.theme().secondary_foreground)
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .child(project_name),
                                        ),
                                )
                            })
                            .when_some(error_message, |this, err| {
                                let raw_error_message =
                                    item.error_message.clone().unwrap_or_else(|| err.clone());
                                this.child(
                                    div()
                                        .id(ElementId::from(format!("queue-error-{item_id}")))
                                        .max_w(px(280.0))
                                        .text_size(px(11.0))
                                        .text_color(cx.theme().danger)
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .role(Role::Note)
                                        .aria_label(raw_error_message)
                                        .child(err),
                                )
                            }),
                    ),
            )
            // Actions use stable per-item identities for predictable focus.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .flex_shrink_0()
                    .when(
                        matches!(status, BatchItemStatus::Error | BatchItemStatus::Cancelled),
                        |actions| {
                            let item_id = item_id.clone();
                            actions.child(
                                Button::new(ElementId::from(format!("btn-retry-{item_id}")))
                                    .ghost()
                                    .small()
                                    .icon(icon(LucideIcon::RotateCcw).size(px(12.0)))
                                    .tooltip("Retry transcription")
                                    .accessibility_label("Retry transcription")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.retry_item(item_id.clone(), cx);
                                    })),
                            )
                        },
                    )
                    .child(
                        Button::new(ElementId::from(format!("btn-cancel-{item_id}")))
                            .ghost()
                            .small()
                            .icon(
                                icon(LucideIcon::X)
                                    .size(px(12.0))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .tooltip(if status == BatchItemStatus::Processing {
                                "Stop and remove"
                            } else {
                                "Remove from queue"
                            })
                            .accessibility_label(if status == BatchItemStatus::Processing {
                                "Stop and remove"
                            } else {
                                "Remove from queue"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if status == BatchItemStatus::Processing {
                                    this.confirming_cancel_item_id = Some(item_id.clone());
                                    cx.notify();
                                } else {
                                    this.remove_item(item_id.clone(), cx);
                                }
                            })),
                    ),
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
        let pending_count = self
            .queue
            .iter()
            .filter(|i| i.status == BatchItemStatus::Pending)
            .count();
        let is_all_complete = completed_count == total_count && total_count > 0;
        let is_all_failed = failed_count == total_count && total_count > 0 && processing_count == 0;
        let is_paused = self.is_paused;
        let summary_status = queue_summary_status(
            processing_count > 0,
            is_paused,
            is_all_complete,
            failed_count > 0,
        );
        let overall_progress = self.overall_progress();

        let is_drag = self.is_drag_over;
        let is_dropzone_hovered = self.is_dropzone_hovered;
        let dropzone_icon_offset = if is_empty && is_dropzone_hovered {
            let keyframes = Keyframes::try_new([
                Keyframe::new(0.0, 0.0_f32),
                Keyframe::new(0.5, -6.0),
                Keyframe::new(1.0, 0.0),
            ])
            .expect("dropzone bounce keyframes are valid");
            animate_keyframes(
                format!("batch-dropzone-icon-{}", self.dropzone_hover_generation),
                &keyframes,
                Timing::new(std::time::Duration::from_millis(600)).ease(Easing::CubicBezier {
                    x1: 0.42,
                    y1: 0.0,
                    x2: 0.58,
                    y2: 1.0,
                }),
                window,
                cx,
            )
            .value
        } else {
            0.0
        };
        let clear_open = self.is_clear_menu_open;
        let cancel_confirmation = self
            .confirming_cancel_item_id
            .as_ref()
            .and_then(|id| self.queue.iter().find(|item| &item.id == id))
            .map(|item| {
                (
                    item.id.clone(),
                    item.filename.clone(),
                    item.progress.round() as u32,
                )
            });
        let file_drop_error = self.file_drop_error.clone();

        div()
            .id("batch-import-container")
            .flex()
            .flex_col()
            .size_full()
            .on_drag_move::<gpui_kit::ExternalPaths>(cx.listener(
                |this, event: &gpui_kit::DragMoveEvent<gpui_kit::ExternalPaths>, _, cx| {
                    let is_over = event.bounds.contains(&event.event.position);
                    if this.is_drag_over != is_over {
                        this.is_drag_over = is_over;
                        cx.notify();
                    }
                },
            ))
            .on_file_drop_exit(cx.listener(|this, _, _, cx| {
                if this.is_drag_over {
                    this.is_drag_over = false;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &gpui_kit::ExternalPaths, _, cx| {
                this.is_drag_over = false;
                this.enqueue_dropped_paths(paths.paths().iter().cloned(), cx);
            }))
            .px(px(16.0))
            .pt(px(16.0))
            .pb(px(8.0))
            .gap(px(16.0))
            .when(is_empty, |this| this.overflow_y_scroll())
            .when(!is_empty, |this| this.overflow_hidden())
            .when(!is_empty && is_drag, |this| {
                this.border_2()
                    .border_dashed()
                    .border_color(cx.theme().primary)
                    .rounded(RADIUS_LG)
                    .bg(cx.theme().primary.opacity(0.04))
            })
            .when_some(file_drop_error, |this, message| {
                this.child(
                    div()
                        .px(px(10.0))
                        .py(px(8.0))
                        .rounded(RADIUS_SM)
                        .bg(cx.theme().danger.opacity(0.1))
                        .border_1()
                        .border_color(cx.theme().danger.opacity(0.35))
                        .text_size(px(12.0))
                        .text_color(cx.theme().danger)
                        .child(message),
                )
            })
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
                                                    if !this.require_asr_configuration(cx) {
                                                        return;
                                                    }

                                                    this.add_stream_url(url, alias_opt, retain, cx);
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
            // Processing Item Removal Confirmation
            // ==========================================
            .when_some(cancel_confirmation, |this, (item_id, filename, progress)| {
                this.child(
                    div()
                        .id("cancel-item-confirm-overlay")
                        .absolute()
                        .inset_0()
                        .bg(gpui_kit::black().opacity(0.45))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(420.0))
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
                                        .child("Stop Transcription?"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "\"{filename}\" is currently being transcribed ({progress}%). Stopping will discard current progress. Are you sure you want to stop and remove this file?"
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap(px(8.0))
                                        .child(
                                            Button::new("btn-continue-transcription")
                                                .outline()
                                                .label("Continue Transcribing")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.confirming_cancel_item_id = None;
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-stop-and-remove")
                                                .danger()
                                                .label("Stop & Remove")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.confirming_cancel_item_id = None;
                                                    this.remove_item(item_id.clone(), cx);
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
                        .cursor_pointer()
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if this.is_dropzone_hovered != *hovered {
                                this.is_dropzone_hovered = *hovered;
                                if *hovered {
                                    this.dropzone_hover_generation =
                                        this.dropzone_hover_generation.wrapping_add(1);
                                }
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.pick_files_async(cx);
                        }))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .flex_1()
                        .min_h(px(300.0))
                        .rounded(RADIUS_XL)
                        .border_2()
                        .border_color(if is_drag || is_dropzone_hovered {
                            cx.theme().primary
                        } else {
                            crate::theme::tokens::COLOR_BORDER_HOVER_LIGHT
                        })
                        .bg(if is_drag || is_dropzone_hovered {
                            cx.theme().primary.opacity(0.04)
                        } else {
                            cx.theme().secondary
                        })
                        .px(px(16.0))
                        .py(px(32.0))
                        .gap(px(24.0))
                        // Central Upload Cloud Icon
                        .child(
                            div()
                                .size(px(48.0))
                                .when(dropzone_icon_offset != 0.0, |this| {
                                    this.relative().top(px(dropzone_icon_offset))
                                })
                                .child(
                                    Icon::default()
                                        .path("icons/upload-cloud.svg")
                                        .size(px(48.0))
                                        .text_color(if is_dropzone_hovered {
                                            cx.theme().primary
                                        } else {
                                            cx.theme().muted_foreground
                                        }),
                                ),
                        )
                        // Headings
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(8.0))
                                .max_w(px(320.0))
                                .text_center()
                                .child(
                                    div()
                                        .text_size(px(20.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child("Drop audio or video files here"),
                                )
                                .child(
                                    div()
                                        .text_size(px(15.0))
                                        .text_color(cx.theme().secondary_foreground)
                                        .text_center()
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
                                            cx.stop_propagation();
                                            this.pick_files_async(cx);
                                        })),
                                )
                                .child(
                                    Button::new("btn-select-stream-secondary")
                                        .outline()
                                        .label("Import Network Stream...")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.is_stream_modal_open = true;
                                            this.stream_error = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                        // Supported formats summary
                        .child(
                            div()
                                .mt(px(16.0))
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .opacity(if is_dropzone_hovered { 1.0 } else { 0.8 })
                                .text_center()
                                .child(format!(
                                    "Supports: {}",
                                    SUPPORTED_EXTENSIONS
                                        .iter()
                                        .map(|extension| format!(".{extension}"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )),
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
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .gap(px(12.0))
                        .w_full()
                        // Queue Summary Bar Card
                        .child(
                            div()
                                .id("queue-summary-bar")
                                .py(px(12.0))
                                .px(px(14.0))
                                .rounded(px(16.0))
                                .bg(cx.theme().popover)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_sm()
                                .flex_shrink_0()
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
                                                        .text_size(px(14.4))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(format!("Batch Queue ({total_count} files)")),
                                                )
                                                // Status Pill
                                                .child(
                                                    div()
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .rounded(RADIUS_FULL)
                                                        .text_size(px(11.84))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(5.0))
                                                        .when(summary_status == QueueSummaryStatus::Processing, |pill| {
                                                            pill.bg(hsla(196.0 / 360.0, 0.86, 0.322, 0.12))
                                                                .text_color(hsla(196.0 / 360.0, 0.86, 0.322, 1.0))
                                                                .child(icon(LucideIcon::Loader).size(px(12.0)))
                                                                .child(format!("Transcribing ({completed_count}/{total_count})"))
                                                        })
                                                        .when(summary_status == QueueSummaryStatus::Paused, |pill| {
                                                            pill.bg(cx.theme().warning.opacity(0.15))
                                                                .text_color(cx.theme().warning)
                                                                .child(icon(LucideIcon::Pause).size(px(12.0)))
                                                                .child("Paused")
                                                        })
                                                        .when(summary_status == QueueSummaryStatus::Complete, |pill| {
                                                            pill.bg(cx.theme().success.opacity(0.12))
                                                                .text_color(cx.theme().success)
                                                                .child(icon(LucideIcon::Check).size(px(12.0)))
                                                                .child("Completed")
                                                        })
                                                        .when(summary_status == QueueSummaryStatus::Failed, |pill| {
                                                            pill.bg(cx.theme().danger.opacity(0.12))
                                                                .text_color(cx.theme().danger)
                                                                .child(icon(LucideIcon::CircleAlert).size(px(12.0)))
                                                                .child(format!("{completed_count}/{total_count} · {failed_count} failed"))
                                                        })
                                                        .when(summary_status == QueueSummaryStatus::Idle, |pill| {
                                                            pill.bg(cx.theme().muted)
                                                                .text_color(cx.theme().secondary_foreground)
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
                                                .w(relative(if is_all_failed { 1.0 } else { overall_progress / 100.0 }))
                                                .rounded_full()
                                                .bg(if is_all_complete {
                                                    cx.theme().success
                                                } else if is_all_failed {
                                                    cx.theme().danger
                                                } else {
                                                    hsla(196.0 / 360.0, 0.86, 0.322, 1.0)
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
                                                .when(processing_count > 0 && !is_paused, |bar| bar.child(
                                                    Button::new("btn-pause-queue")
                                                        .small()
                                                        .outline()
                                                        .label("Pause Queue")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.toggle_queue_pause(cx);
                                                        })),
                                                ))
                                                .when(is_paused, |bar| bar.child(
                                                    Button::new("btn-resume-queue")
                                                        .small()
                                                        .primary()
                                                        .label("Resume Queue")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.toggle_queue_pause(cx);
                                                        })),
                                                ))
                                                .when(pending_count > 0 && processing_count == 0 && !is_paused, |bar| bar.child(
                                                    Button::new("btn-start-queue")
                                                        .small()
                                                        .primary()
                                                        .label("Start Queue")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.start_next_pending(cx);
                                                        })),
                                                ))
                                                // Match React's Add More split action and menu.
                                                .child({
                                                    let muted_foreground = cx.theme().muted_foreground;
                                                    DropdownButton::new("btn-add-more")
                                                        .button(
                                                            Button::new("btn-add-more-primary")
                                                                .small()
                                                                .outline()
                                                                .icon(icon(LucideIcon::Upload).size(px(13.0)))
                                                                .label("Add More")
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.pick_files_async(cx);
                                                                })),
                                                        )
                                                        .menu_accessibility_label("Add more options")
                                                        .dropdown_menu_with_anchor(Anchor::BottomLeft, {
                                                            let panel = cx.entity().downgrade();
                                                            move |menu, _, _| {
                                                                let files_panel = panel.clone();
                                                                let stream_panel = panel.clone();
                                                                menu.min_w(px(270.0))
                                                                    .item(
                                                                        PopupMenuItem::element(move |_, _| {
                                                                            div()
                                                                                .w(px(270.0))
                                                                                .flex()
                                                                                .items_center()
                                                                                .gap(px(10.0))
                                                                                .px(px(10.0))
                                                                                .py(px(7.0))
                                                                                .child(icon(LucideIcon::Upload).size(px(14.0)))
                                                                                .child(
                                                                                    div()
                                                                                        .flex()
                                                                                        .flex_col()
                                                                                        .gap(px(2.0))
                                                                                        .child(div().text_size(px(12.0)).child("Add More Files"))
                                                                                        .child(div().text_size(px(11.0)).text_color(muted_foreground).child("Add more audio/video files to queue")),
                                                                                )
                                                                        })
                                                                        .on_click(move |_, _, cx| {
                                                                            let _ = files_panel.update(cx, |this, cx| this.pick_files_async(cx));
                                                                        }),
                                                                    )
                                                                    .item(
                                                                        PopupMenuItem::element(move |_, _| {
                                                                            div()
                                                                                .w(px(270.0))
                                                                                .flex()
                                                                                .items_center()
                                                                                .gap(px(10.0))
                                                                                .px(px(10.0))
                                                                                .py(px(7.0))
                                                                                .child(icon(LucideIcon::Globe).size(px(14.0)))
                                                                                .child(
                                                                                    div()
                                                                                        .flex()
                                                                                        .flex_col()
                                                                                        .gap(px(2.0))
                                                                                        .child(div().text_size(px(12.0)).child("Import Network Stream..."))
                                                                                        .child(div().text_size(px(11.0)).text_color(muted_foreground).child("Enter HTTP, HTTPS, RTMP or HLS stream URL")),
                                                                                )
                                                                        })
                                                                        .on_click(move |_, _, cx| {
                                                                            let _ = stream_panel.update(cx, |this, cx| {
                                                                                this.is_stream_modal_open = true;
                                                                                this.stream_error = None;
                                                                                cx.notify();
                                                                            });
                                                                        }),
                                                                    )
                                                            }
                                                        })
                                                }),
                                        )
                                        // Clear Menu Dropdown trigger
                                        .child(
                                            div()
                                                .relative()
                                                .child(
                                                    div()
                                                        .id("btn-clear-menu-trigger")
                                                        .role(Role::Button)
                                                        .aria_label("Clear options")
                                                        .aria_expanded(clear_open)
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
                                                                .when(clear_open, |chevron| {
                                                                    chevron.rotate(Radians(std::f32::consts::PI))
                                                                })
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
                                                            .role(Role::Menu)
                                                            .aria_label("Clear options")
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
                                                                    .role(Role::MenuItem)
                                                                    .aria_label(format!(
                                                                        "Clear completed files ({completed_count})"
                                                                    ))
                                                                    .when(completed_count == 0, |item| {
                                                                        item.aria_description("Unavailable when there are no completed files")
                                                                    })
                                                                    .px(px(8.0))
                                                                    .py(px(5.0))
                                                                    .rounded(RADIUS_SM)
                                                                    .when(completed_count > 0, |item| {
                                                                        item.cursor_pointer()
                                                                            .hover(|s| s.bg(cx.theme().secondary))
                                                                    })
                                                                    .when(completed_count == 0, |item| {
                                                                        item.opacity(0.45).cursor_default()
                                                                    })
                                                                    .text_size(px(12.0))
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_col()
                                                                            .gap(px(1.0))
                                                                            .child(
                                                                                div()
                                                                                    .text_size(px(13.12))
                                                                                    .font_weight(FontWeight::MEDIUM)
                                                                                    .child("Clear Completed"),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_size(px(11.52))
                                                                                    .text_color(cx.theme().muted_foreground)
                                                                                    .child(format!("Clear all completed files ({completed_count})")),
                                                                            ),
                                                                    )
                                                                    .when(completed_count > 0, |item| {
                                                                        item.on_click(cx.listener(|this, _, _, cx| {
                                                                            this.clear_completed(cx);
                                                                            this.is_clear_menu_open = false;
                                                                            cx.notify();
                                                                        }))
                                                                    }),
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
                                                                    .role(Role::MenuItem)
                                                                    .aria_label(format!(
                                                                        "Clear all files ({total_count})"
                                                                    ))
                                                                    .px(px(8.0))
                                                                    .py(px(5.0))
                                                                    .rounded(RADIUS_SM)
                                                                    .cursor_pointer()
                                                                    .hover(|s| s.bg(cx.theme().danger.opacity(0.1)))
                                                                    .text_size(px(12.0))
                                                                    .text_color(cx.theme().danger)
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_col()
                                                                            .gap(px(1.0))
                                                                            .child(
                                                                                div()
                                                                                    .text_size(px(13.12))
                                                                                    .font_weight(FontWeight::MEDIUM)
                                                                                    .child("Clear All"),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_size(px(11.52))
                                                                                    .text_color(cx.theme().muted_foreground)
                                                                                    .child(format!("Clear all files in queue ({total_count})")),
                                                                            ),
                                                                    )
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
                            let active_history_id = self.active_history_id.clone();

                            div()
                                .id("queue-grouped-list")
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .pr(px(2.0))
                                .pb(px(40.0))
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
                                                            Self::render_item_card(item, active_history_id.as_deref(), cx)
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
                                                            Self::render_item_card(item, active_history_id.as_deref(), cx)
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
                                                            Self::render_item_card(item, active_history_id.as_deref(), cx)
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
                                                            Self::render_item_card(item, active_history_id.as_deref(), cx)
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
                                                            Self::render_item_card(item, active_history_id.as_deref(), cx)
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

#[cfg(test)]
mod queue_summary_tests {
    use super::{
        BatchAsrSetup, QueueSummaryStatus, batch_asr_setup_required, is_supported_audio_extension,
        queue_error_display, queue_summary_status,
    };

    #[test]
    fn file_drop_extension_validation_is_case_insensitive() {
        assert!(is_supported_audio_extension("mp3"));
        assert!(is_supported_audio_extension(".WAV"));
        assert!(!is_supported_audio_extension("txt"));
    }

    #[test]
    fn batch_import_requires_local_model_before_queueing() {
        assert_eq!(
            batch_asr_setup_required(&serde_json::Value::Null),
            Some(BatchAsrSetup::LocalModel)
        );
        assert_eq!(
            batch_asr_setup_required(&serde_json::json!({
                "batchModelPath": "C:/models/batch.onnx"
            })),
            None
        );
    }

    #[test]
    fn online_batch_import_requires_provider_credentials_and_batch_settings() {
        let base = serde_json::json!({
            "asr": {
                "selections": {
                    "batch": {
                        "engine": "online",
                        "providerId": "volcengine-doubao",
                        "modelId": "volc.bigasr.auc_turbo"
                    }
                },
                "providers": {
                    "online": {
                        "volcengine-doubao": {
                            "batchEndpoint": "https://asr.example/batch",
                            "batchResourceId": "volc.bigasr.auc_turbo"
                        }
                    }
                }
            }
        });
        assert_eq!(
            batch_asr_setup_required(&base),
            Some(BatchAsrSetup::OnlineProvider)
        );

        let configured = serde_json::json!({
            "asr": {
                "selections": {
                    "batch": {
                        "engine": "online",
                        "providerId": "volcengine-doubao",
                        "modelId": "volc.bigasr.auc_turbo"
                    }
                },
                "providers": {
                    "online": {
                        "volcengine-doubao": {
                            "apiKey": "configured-key",
                            "batchEndpoint": "https://asr.example/batch",
                            "batchResourceId": "volc.bigasr.auc_turbo"
                        }
                    }
                }
            }
        });
        assert_eq!(batch_asr_setup_required(&configured), None);
    }

    #[test]
    fn summary_status_follows_react_priority_without_overlapping_states() {
        assert_eq!(
            queue_summary_status(true, true, true, true),
            QueueSummaryStatus::Processing
        );
        assert_eq!(
            queue_summary_status(false, true, true, true),
            QueueSummaryStatus::Paused
        );
        assert_eq!(
            queue_summary_status(false, false, true, true),
            QueueSummaryStatus::Complete
        );
        assert_eq!(
            queue_summary_status(false, false, false, true),
            QueueSummaryStatus::Failed
        );
        assert_eq!(
            queue_summary_status(false, false, false, false),
            QueueSummaryStatus::Idle
        );
    }

    #[test]
    fn queue_error_shortens_transcode_failures_but_preserves_other_details() {
        assert_eq!(
            queue_error_display(
                "capture.mp4",
                "capture.mp4",
                "Failed to decode audio file: unsupported codec"
            ),
            "Transcode failed"
        );
        assert_eq!(
            queue_error_display("capture.wav", "capture.wav", "Network unavailable"),
            "Network unavailable"
        );
    }

    #[test]
    fn streaming_transcript_updates_preserve_queue_snapshot_and_order() {
        let segment = |id: &str, start: f64, text: &str| {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "text": text,
                "start": start,
                "end": start + 1.0,
                "isFinal": true
            }))
            .unwrap()
        };
        let mut queue = vec![super::BatchQueueItem {
            id: "queue-1".to_string(),
            attempt_id: None,
            instance_id: Some("batch-task-1".to_string()),
            filename: "sample.wav".to_string(),
            file_path: "sample.wav".to_string(),
            source_type: super::BatchSourceType::File,
            status: super::BatchItemStatus::Processing,
            progress: 20.0,
            file_size_bytes: None,
            duration_seconds: None,
            project_id: None,
            project_name: None,
            project_color_index: None,
            asr_request: None,
            pipeline_snapshot: None,
            export_path: None,
            error_message: None,
            error_stage: None,
            history_id: Some("history-1".to_string()),
            segments: vec![segment("later", 2.0, "old"), segment("removed", 3.0, "x")],
            media_server_context: None,
            cancel_requested: None,
            is_media_server: false,
        }];
        let update = sona_core::transcription::transcript::TranscriptUpdate {
            remove_ids: vec!["removed".to_string()],
            upsert_segments: vec![
                segment("earlier", 1.0, "first"),
                segment("later", 2.0, "updated"),
            ],
        };

        let (history_id, snapshot, duration) =
            super::BatchImportPanel::apply_transcript_update(&mut queue, "batch-task-1", &update)
                .unwrap();

        assert_eq!(history_id, "history-1");
        assert_eq!(
            snapshot
                .iter()
                .map(|segment| segment.id.as_str())
                .collect::<Vec<_>>(),
            ["earlier", "later"]
        );
        assert_eq!(snapshot[1].text, "updated");
        assert_eq!(duration, 3.0);
        assert_eq!(queue[0].segments, snapshot);
    }
}

#[cfg(test)]
mod project_assignment_tests {
    use super::project_assignment;
    use sona_core::project::ProjectRecord;

    #[test]
    fn new_queue_entries_capture_the_current_project_and_inbox_stays_unassigned() {
        let project = ProjectRecord {
            id: "project-1".to_string(),
            name: "Research".to_string(),
            description: String::new(),
            icon: None,
            color: None,
            sort_order: 0,
            created_at: 1,
            updated_at: 1,
            pipeline: None,
        };

        assert_eq!(project_assignment(None), (None, None));
        assert_eq!(
            project_assignment(Some(&project)),
            (Some("project-1".to_string()), Some("Research".to_string()))
        );
    }
}

#[cfg(test)]
mod batch_scheduler_tests {
    use super::{
        BatchItemErrorStage, attempt_is_current, available_batch_slots, batch_concurrency_limit,
        retry_progress,
    };

    #[test]
    fn scheduler_uses_configured_limit_and_releases_slots_after_completion() {
        assert_eq!(batch_concurrency_limit(None), 2);
        assert_eq!(
            batch_concurrency_limit(Some(&serde_json::json!({"maxConcurrent": 4}))),
            4
        );
        assert_eq!(
            batch_concurrency_limit(Some(&serde_json::json!({"maxConcurrent": 0}))),
            2
        );
        assert_eq!(available_batch_slots(0, 2), 2);
        assert_eq!(available_batch_slots(1, 2), 1);
        assert_eq!(available_batch_slots(2, 2), 0);
        assert_eq!(available_batch_slots(3, 2), 0);
    }

    #[test]
    fn completed_attempt_cannot_mutate_a_retried_queue_item() {
        assert!(attempt_is_current(Some("attempt-2"), "attempt-2"));
        assert!(!attempt_is_current(Some("attempt-2"), "attempt-1"));
        assert!(!attempt_is_current(None, "attempt-1"));
    }

    #[test]
    fn retries_preserve_upload_progress_and_reset_transcription_progress() {
        assert_eq!(
            retry_progress(Some(BatchItemErrorStage::Upload), 96.0),
            96.0
        );
        assert_eq!(
            retry_progress(Some(BatchItemErrorStage::Transcribe), 42.0),
            0.0
        );
        assert_eq!(retry_progress(None, 42.0), 0.0);
    }
}

#[cfg(test)]
mod batch_pipeline_tests {
    use super::build_batch_hotwords;
    use sona_core::project::EffectivePipelineSnapshot;

    fn pipeline(custom_terms: Vec<String>) -> EffectivePipelineSnapshot {
        EffectivePipelineSnapshot {
            is_project_pipeline: true,
            auto_polish: false,
            polish_preset_id: None,
            polish_prompt_override: None,
            auto_translate: false,
            target_language: None,
            auto_summary: false,
            summary_template_id: None,
            hotword_set_ids: Vec::new(),
            replacement_set_ids: Vec::new(),
            custom_terms,
            auto_export: false,
            export_format: None,
            export_directory: None,
            export_file_name_prefix: None,
        }
    }

    #[test]
    fn merges_enabled_hotwords_and_custom_term_targets_without_duplicates() {
        let config = serde_json::json!({
            "hotwordSets": [{
                "id": "global",
                "name": "Global",
                "enabled": true,
                "rules": [{"id": "1", "text": "Sona"}]
            }, {
                "id": "disabled",
                "name": "Disabled",
                "enabled": false,
                "rules": [{"id": "2", "text": "ignored"}]
            }]
        });
        assert_eq!(
            build_batch_hotwords(
                &config,
                &pipeline(vec!["K8s -> Kubernetes".into(), "Sona".into()]),
                Some("p1"),
                Some("Research")
            ),
            Some("Sona,Kubernetes".into())
        );
    }

    #[test]
    fn unified_dictionary_uses_global_and_assigned_project_hotwords_only() {
        let config = serde_json::json!({
            "dictionaryContent": "# Global\n- Sona\n- speech -> text\nprojects:\n  Research (id:p1):\n    - K8s\n  Other (id:p2):\n    - unrelated"
        });
        assert_eq!(
            build_batch_hotwords(&config, &pipeline(Vec::new()), Some("p1"), Some("Research")),
            Some("Sona,K8s".into())
        );
    }
}
