//! Complete Live Record Panel for Sona Desktop.
//! Strict 1:1 parity with React `LiveRecord.tsx` and `RecordingTimer.tsx` (Gaps G07, G08, G09).
//!
//! Features:
//! - Full-width 160px GPU waveform canvas visualizer with dual-harmonic sine paths (Gap G07)
//! - Real-time CPAL audio peak streaming over AppEventBus (Gap G07)
//! - MM:SS monospace timer (hidden when stopped to avoid layout jumping) (Gap G08)
//! - 64px circular record button with 24px inner white dot (start) / 24px square (stop) (Gap G08)
//! - 56px circular pause/resume button (Gap G08)
//! - Interactive Audio Source Dropdown (Microphone vs System Audio) (Gap G09)
//! - Auto-polish trigger loop on segment finalization (Gap G09)

use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::ports::event::EventEmitterPort;
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};

use super::options_capsule::TranscriptionOptionsModel;
use crate::app::{GlobalDesktopServices, GlobalEventBus};
use crate::theme::tokens::{RADIUS_LG, RADIUS_MD, RADIUS_SM};

#[inline]
fn icon(name: LucideIcon) -> Icon {
    Icon::default().path(name.path())
}

/// Selected audio hardware source kind.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AudioSourceKind {
    #[default]
    Microphone,
    SystemAudio,
}

impl AudioSourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Microphone => "Microphone",
            Self::SystemAudio => "System Audio",
        }
    }

    pub fn icon(self) -> LucideIcon {
        match self {
            Self::Microphone => LucideIcon::Mic,
            Self::SystemAudio => LucideIcon::Monitor,
        }
    }
}

pub struct LiveRecordPanel {
    pub options_model: Entity<TranscriptionOptionsModel>,

    pub is_recording: bool,
    pub is_paused: bool,
    pub is_initializing: bool,
    pub is_transitioning: bool,
    pub is_agent_recording: bool,
    pub elapsed_seconds: u64,
    pub started_at: Option<Instant>,
    pub paused_at: Option<Instant>,
    pub paused_duration: Duration,

    // Waveform visualizer state (Gap G07)
    pub target_amplitude: f32,
    pub current_amplitude: f32,
    pub phase: f32,

    // Audio source dropdown state (Gap G09)
    pub audio_source: AudioSourceKind,
    pub is_source_dropdown_open: bool,

    // Auto-polish state (Gap G09)
    pub auto_polish_enabled: bool,
    pub auto_polish_frequency: usize,
    pub unpolished_segments: Vec<TranscriptSegment>,
    pub polished_ids: HashSet<String>,
    pub active_session_id: Option<String>,
    pub recording_draft: Option<sona_core::history::LiveRecordingDraftResult>,
    pub transcript_segments: Vec<TranscriptSegment>,
    pub active_project_id: Option<String>,
}

/// Formats an elapsed duration in seconds as MM:SS with 2-digit padding.
pub fn format_duration_mmss(elapsed_seconds: u64) -> String {
    let mins = elapsed_seconds / 60;
    let secs = elapsed_seconds % 60;
    format!("{:02}:{:02}", mins, secs)
}

impl LiveRecordPanel {
    pub fn new(options_model: Entity<TranscriptionOptionsModel>) -> Self {
        Self {
            options_model,
            is_recording: false,
            is_paused: false,
            is_initializing: false,
            is_transitioning: false,
            is_agent_recording: false,
            elapsed_seconds: 0,
            started_at: None,
            paused_at: None,
            paused_duration: Duration::ZERO,

            target_amplitude: 0.05,
            current_amplitude: 0.05,
            phase: 0.0,

            audio_source: AudioSourceKind::Microphone,
            is_source_dropdown_open: false,

            auto_polish_enabled: true,
            auto_polish_frequency: 5,
            unpolished_segments: Vec::new(),
            polished_ids: HashSet::new(),
            active_session_id: None,
            recording_draft: None,
            transcript_segments: Vec::new(),
            active_project_id: None,
        }
    }

    /// Subscribes to backend CPAL peak streaming and real-time ASR events from AppEventBus.
    pub fn subscribe_audio_peaks(&mut self, cx: &mut Context<Self>) {
        let event_bus = cx.global::<GlobalEventBus>().0.clone();
        let mut rx = event_bus.subscribe();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            while let Ok(event) = rx.recv().await {
                if event.name == "agent-control-recording-status" {
                    let active = event
                        .payload
                        .get("active")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);
                    this.update(cx, |view, cx| {
                        view.is_agent_recording = active;
                        cx.notify();
                    })
                    .ok();
                }
                // 1. Audio peak streaming (Microphone or System Audio)
                if event.name == "microphone-audio" || event.name == "system-audio" {
                    if let Ok(peak_i16) = serde_json::from_value::<i16>(event.payload) {
                        // Safe peak conversion: cast to f32 BEFORE abs() to avoid i16::MIN negation panic
                        let normalized = ((peak_i16 as f32).abs() / 32767.0).clamp(0.02, 1.0);
                        this.update(cx, |view, cx| {
                            view.target_amplitude = normalized;
                            cx.notify();
                        })
                        .ok();
                    }
                }
                // 2. Real-time ASR segment finalization & auto-polish ingestion
                else if event.name.starts_with("recognizer-output-") {
                    if let Ok(update) = serde_json::from_value::<TranscriptUpdate>(event.payload) {
                        this.update(cx, |view, cx| {
                            let matches_session = view
                                .active_session_id
                                .as_deref()
                                .map(|id| event.name == format!("recognizer-output-{id}"))
                                .unwrap_or(false);

                            if matches_session
                                || (view.is_recording && view.active_session_id.is_none())
                            {
                                view.handle_transcript_update(update, cx);
                            }
                        })
                        .ok();
                    }
                }
            }
        })
        .detach();
    }

    /// Ingests live transcript update, tracks unpolished finalized segments,
    /// and flushes auto-polish batch when configured frequency threshold is reached.
    pub fn handle_transcript_update(&mut self, update: TranscriptUpdate, cx: &mut Context<Self>) {
        // Retract any deleted segment IDs
        if !update.remove_ids.is_empty() {
            self.transcript_segments
                .retain(|segment| !update.remove_ids.contains(&segment.id));
            self.unpolished_segments
                .retain(|s| !update.remove_ids.contains(&s.id));
        }

        // Ingest newly finalized segments
        for new_seg in update.upsert_segments {
            if let Some(index) = self
                .transcript_segments
                .iter()
                .position(|segment| segment.id == new_seg.id)
            {
                self.transcript_segments[index] = new_seg.clone();
            } else {
                self.transcript_segments.push(new_seg.clone());
            }
            if new_seg.is_final && !self.polished_ids.contains(&new_seg.id) {
                if let Some(pos) = self
                    .unpolished_segments
                    .iter()
                    .position(|s| s.id == new_seg.id)
                {
                    self.unpolished_segments[pos] = new_seg;
                } else {
                    self.unpolished_segments.push(new_seg);
                }
            }
        }

        // Trigger auto-polish when threshold is reached
        if self.auto_polish_enabled
            && self.auto_polish_frequency > 0
            && self.unpolished_segments.len() >= self.auto_polish_frequency
        {
            self.flush_auto_polish(cx);
        }

        if let Some(session_id) = self.active_session_id.clone() {
            let event_bus = cx.global::<GlobalEventBus>().0.clone();
            let payload = serde_json::json!({
                "sessionId": session_id,
                "segments": self.transcript_segments,
                "duration": self.elapsed_duration().as_secs_f64(),
            });
            let _ = event_bus.emit("live-record:transcript-updated", payload);
        }

        cx.notify();
    }

    /// Formats timer as MM:SS with 2-digit padding.
    pub fn formatted_timer_text(&self) -> String {
        format_duration_mmss(self.elapsed_seconds)
    }

    fn elapsed_duration(&self) -> Duration {
        let Some(started_at) = self.started_at else {
            return Duration::ZERO;
        };
        let paused = self.paused_duration
            + self
                .paused_at
                .map(|paused_at| paused_at.elapsed())
                .unwrap_or_default();
        started_at.elapsed().saturating_sub(paused)
    }

    /// Status copy below controls.
    fn status_helper_text(&self) -> &'static str {
        if self.is_agent_recording {
            "AI Agent recording in progress, microphone occupied"
        } else if !self.is_recording {
            "Press Space or click to start recording"
        } else if self.is_paused {
            "Recording paused. Press Space to resume"
        } else {
            "Recording in progress... Press Space to pause"
        }
    }

    pub fn start_recording(&mut self, cx: &mut Context<Self>) {
        if self.is_recording || self.is_initializing || self.is_agent_recording {
            return;
        }

        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let session_id = uuid::Uuid::new_v4().to_string();
        let source_kind = self.audio_source;
        let project_id = self.active_project_id.clone();

        self.is_initializing = true;
        self.is_transitioning = true;
        self.is_paused = false;
        self.elapsed_seconds = 0;
        self.started_at = None;
        self.paused_at = None;
        self.paused_duration = Duration::ZERO;
        self.active_session_id = Some(session_id.clone());
        self.recording_draft = None;
        self.transcript_segments.clear();
        self.unpolished_segments.clear();
        self.polished_ids.clear();
        self.target_amplitude = 0.25;

        // Auto-close popover if open
        self.options_model.update(cx, |opt, cx| {
            opt.is_popover_open = false;
            cx.notify();
        });
        cx.notify();

        // Create the history draft first so native capture writes directly to its managed WAV path.
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let context = match services.sqlite.current_context() {
                Ok(context) => context,
                Err(error) => {
                    log::error!("[LiveRecord] failed to resolve app data directory: {error}");
                    let _ = services.emitter.emit(
                        "live-record:error",
                        serde_json::json!({ "message": error.to_string() }),
                    );
                    this.update(cx, |view, cx| {
                        view.is_initializing = false;
                        view.is_transitioning = false;
                        view.active_session_id = None;
                        view.target_amplitude = 0.05;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let app_data_dir = context.app_data_dir().to_path_buf();
            let draft_request = sona_core::history::HistoryCreateLiveDraftRequest {
                id: Some(session_id.clone()),
                audio_extension: "wav".to_string(),
                tag_ids: project_id.clone().into_iter().collect(),
                project_id,
                icon: Some("system:mic".to_string()),
            };
            let draft = match services
                .history
                .mutation_file(move |store| store.create_live_draft(draft_request))
                .await
            {
                Ok(draft) => draft,
                Err(error) => {
                    log::error!("[LiveRecord] failed to create recording draft: {error}");
                    let _ = services
                        .emitter
                        .emit("live-record:error", serde_json::json!({ "message": error }));
                    this.update(cx, |view, cx| {
                        view.is_initializing = false;
                        view.is_transitioning = false;
                        view.active_session_id = None;
                        view.target_amplitude = 0.05;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let output_path = draft.audio_absolute_path.clone();

            let result = match source_kind {
                AudioSourceKind::Microphone => {
                    crate::integrations::audio::start_microphone_capture(
                        services.audio.clone(),
                        services.asr.clone(),
                        services.emitter.clone(),
                        app_data_dir,
                        None,
                        session_id.clone(),
                        Some(output_path),
                    )
                    .await
                }
                AudioSourceKind::SystemAudio => {
                    crate::integrations::audio::start_system_audio_capture(
                        services.audio.clone(),
                        services.asr.clone(),
                        services.emitter.clone(),
                        app_data_dir,
                        None,
                        session_id.clone(),
                        Some(output_path),
                    )
                    .await
                }
            };

            if let Err(err) = result {
                log::error!("[LiveRecord] failed to start audio capture: {err}");
                let _ = services
                    .emitter
                    .emit("live-record:error", serde_json::json!({ "message": err }));
                let history_id = draft.item.id.clone();
                let deleted_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or_default();
                let _ = services
                    .history
                    .mutation_file(move |store| {
                        let ids = vec![history_id];
                        store.trash_items(
                            sona_core::history::mutation_repository::HistoryTrashItemsRequest {
                                ids: ids.clone(),
                                deleted_at,
                            },
                        )?;
                        store.purge_items(
                            sona_core::history::mutation_repository::HistoryPurgeItemsRequest {
                                ids,
                            },
                        )
                    })
                    .await;
                this.update(cx, |view, cx| {
                    view.is_initializing = false;
                    view.is_transitioning = false;
                    view.is_paused = false;
                    view.active_session_id = None;
                    view.recording_draft = None;
                    view.transcript_segments.clear();
                    view.unpolished_segments.clear();
                    view.polished_ids.clear();
                    view.target_amplitude = 0.05;
                    cx.notify();
                })
                .ok();
                return;
            }

            this.update(cx, |view, cx| {
                if view.active_session_id.as_deref() != Some(&session_id) {
                    return;
                }
                view.recording_draft = Some(draft);
                view.is_recording = true;
                view.is_initializing = false;
                view.is_transitioning = false;
                view.started_at = Some(Instant::now());
                view.start_recording_tickers(cx);
                let event_bus = cx.global::<GlobalEventBus>().0.clone();
                let _ = event_bus.emit(
                    "live-record:started",
                    serde_json::json!({ "sessionId": session_id, "title": "Recording" }),
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_recording_tickers(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let should_continue = this
                    .update(cx, |view, cx| {
                        if !view.is_recording {
                            return false;
                        }
                        if !view.is_paused {
                            view.elapsed_seconds += 1;
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !should_continue {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            loop {
                tokio::time::sleep(Duration::from_millis(30)).await;
                let should_continue = this
                    .update(cx, |view, cx| {
                        if !view.is_recording && view.current_amplitude <= 0.05 {
                            return false;
                        }
                        view.phase = (view.phase + 0.06) % (2.0 * std::f32::consts::PI);
                        view.current_amplitude +=
                            (view.target_amplitude - view.current_amplitude) * 0.08;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !should_continue {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if !self.is_recording {
            return;
        }

        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let source_kind = self.audio_source;
        let Some(session_id) = self.active_session_id.clone() else {
            return;
        };
        let draft = self.recording_draft.clone();
        let started_at = self.started_at;
        let paused_duration = self.paused_duration
            + self
                .paused_at
                .map(|paused_at| paused_at.elapsed())
                .unwrap_or_default();
        let duration = started_at
            .map(|started_at| {
                started_at
                    .elapsed()
                    .saturating_sub(paused_duration)
                    .as_secs_f64()
            })
            .unwrap_or_default();
        let event_bus = cx.global::<GlobalEventBus>().0.clone();

        self.is_recording = false;
        self.is_transitioning = true;
        self.is_paused = false;
        self.paused_at = None;
        self.target_amplitude = 0.05;
        cx.notify();

        // Finalize auto-polish for any remaining segments
        self.flush_auto_polish(cx);

        // Stop backend audio capture
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let stopped = match source_kind {
                AudioSourceKind::Microphone => {
                    crate::integrations::audio::stop_microphone_capture(
                        &services.audio,
                        session_id.clone(),
                    )
                        .await
                }
                AudioSourceKind::SystemAudio => {
                    crate::integrations::audio::stop_system_audio_capture(
                        &services.audio,
                        session_id.clone(),
                    )
                    .await
                }
            };

            let segments = this
                .update(cx, |view, _cx| view.transcript_segments.clone())
                .unwrap_or_default();
            let mut completed_title = None;
            if let Some(draft) = draft {
                match stopped {
                    Ok(_saved_path) if !segments.is_empty() => {
                        let request = sona_core::history::mutation_repository::HistoryCompleteLiveDraftRequest {
                            history_id: draft.item.id.clone(),
                            segments,
                            duration,
                        };
                        match services
                            .history
                            .mutation_file(move |store| store.complete_live_draft(request))
                            .await
                        {
                            Ok(item) => completed_title = Some(item.title),
                            Err(error) => {
                                log::error!("[LiveRecord] failed to complete recording history item: {error}");
                                let _ = event_bus.emit(
                                    "live-record:error",
                                    serde_json::json!({ "message": error }),
                                );
                            }
                        }
                    }
                    Ok(saved_path) => {
                        if let Err(error) = tokio::fs::remove_file(&saved_path).await {
                            if error.kind() != std::io::ErrorKind::NotFound {
                                log::warn!(
                                    "[LiveRecord] failed to remove empty recording file {saved_path}: {error}"
                                );
                            }
                        }
                        let id = draft.item.id.clone();
                        let deleted_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|duration| duration.as_millis() as u64)
                            .unwrap_or_default();
                        let _ = services
                            .history
                            .mutation_file(move |store| {
                                let ids = vec![id];
                                store.trash_items(sona_core::history::mutation_repository::HistoryTrashItemsRequest {
                                    ids: ids.clone(),
                                    deleted_at,
                                })?;
                                store.purge_items(sona_core::history::mutation_repository::HistoryPurgeItemsRequest { ids })
                            })
                            .await;
                    }
                    Err(error) => {
                        log::error!("[LiveRecord] failed to stop audio capture: {error}");
                        let _ = event_bus.emit(
                            "live-record:error",
                            serde_json::json!({ "message": error }),
                        );
                    }
                }
            }
            this.update(cx, |view, cx| {
                if view.active_session_id.as_deref() == Some(&session_id) {
                    view.active_session_id = None;
                    view.recording_draft = None;
                    view.started_at = None;
                    view.paused_duration = Duration::ZERO;
                    view.transcript_segments.clear();
                    view.unpolished_segments.clear();
                    view.polished_ids.clear();
                }
                view.is_transitioning = false;
                cx.notify();
            })
            .ok();
            let _ = event_bus.emit(
                "live-record:stopped",
                serde_json::json!({
                    "sessionId": session_id,
                    "title": completed_title,
                    "duration": duration,
                }),
            );
        })
        .detach();
    }

    pub fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        if !self.is_recording || self.is_transitioning {
            return;
        }

        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let source_kind = self.audio_source;
        let session_id = self.active_session_id.clone().unwrap_or_default();
        let target_paused = !self.is_paused;

        let result = match source_kind {
            AudioSourceKind::Microphone => {
                crate::integrations::audio::set_microphone_capture_paused(
                    &services.audio,
                    session_id,
                    target_paused,
                )
            }
            AudioSourceKind::SystemAudio => {
                crate::integrations::audio::set_system_audio_capture_paused(
                    &services.audio,
                    session_id,
                    target_paused,
                )
            }
        };
        if let Err(error) = result {
            log::error!("[LiveRecord] failed to change capture pause state: {error}");
            return;
        }

        self.is_paused = target_paused;
        if target_paused {
            self.paused_at = Some(Instant::now());
            self.target_amplitude = 0.02;
        } else if let Some(paused_at) = self.paused_at.take() {
            self.paused_duration += paused_at.elapsed();
        }
        cx.notify();
    }

    /// Dispatches unpolished segment batch to the LLM polish pipeline over AppEventBus.
    pub fn flush_auto_polish(&mut self, cx: &mut Context<Self>) {
        if !self.auto_polish_enabled || self.unpolished_segments.is_empty() {
            return;
        }

        let to_polish: Vec<TranscriptSegment> = self.unpolished_segments.drain(..).collect();
        for seg in &to_polish {
            self.polished_ids.insert(seg.id.clone());
        }

        let session_id = self.active_session_id.clone().unwrap_or_default();
        let event_bus = cx.global::<GlobalEventBus>().0.clone();

        let llm_inputs: Vec<sona_core::llm::tasks::LlmSegmentInput> = to_polish
            .iter()
            .map(|seg| sona_core::llm::tasks::LlmSegmentInput {
                id: seg.id.clone(),
                text: seg.text.clone(),
            })
            .collect();
        let prompt = sona_core::llm::tasks::build_polish_prompt(&llm_inputs, None, None);

        // Asynchronously dispatch polish request to event bus
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let request_payload = serde_json::json!({
                "sessionId": session_id,
                "segmentCount": to_polish.len(),
                "segments": to_polish,
                "prompt": prompt,
            });
            let _ = event_bus.emit("live-record:auto-polish-request", request_payload);

            // Acknowledge completion on event bus
            let complete_payload = serde_json::json!({
                "sessionId": session_id,
                "polishedIds": to_polish.iter().map(|s| &s.id).collect::<Vec<_>>(),
            });
            let _ = event_bus.emit("live-record:auto-polish-complete", complete_payload);

            this.update(cx, |_view, cx| {
                cx.notify();
            })
            .ok();
        })
        .detach();

        cx.notify();
    }
}

impl Render for LiveRecordPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_recording = self.is_recording;
        let is_paused = self.is_paused;
        let is_initializing = self.is_initializing;
        let is_transitioning = self.is_transitioning;
        let is_agent_recording = self.is_agent_recording;
        let amplitude = self.current_amplitude;
        let phase = self.phase;
        let is_source_open = self.is_source_dropdown_open;
        let current_source = self.audio_source;

        div()
            .id("live-record-container")
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .gap(px(16.0))
            .px(px(16.0))
            .pt(px(20.0))
            .pb(px(8.0))
            .overflow_y_scroll()
            .child(
                div()
                    .id("live-record-main-content")
                    .w_full()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        // ==========================================
                        // Full-width 160px Waveform Visualizer (Gap G07)
                        // ==========================================
                        div()
                            .id("visualizer-wrapper")
                            .w_full()
                            .h(px(160.0))
                            .rounded(RADIUS_LG)
                            .bg(cx.theme().secondary)
                            .border_1()
                            .border_color(cx.theme().border)
                            .overflow_hidden()
                            .child(
                                canvas(
                                    |_bounds, _window, _cx| (),
                                    move |bounds, (), window, _cx| {
                                        let width = bounds.size.width;
                                        let height = bounds.size.height;
                                        let width_f32 = f32::from(width);
                                        let height_f32 = f32::from(height);

                                        // Early return guard: prevent division by zero / NaN when unmeasured or collapsed
                                        if width_f32 <= 1.0 || height_f32 <= 1.0 {
                                            return;
                                        }

                                        // Defensive normalization to guarantee finite inputs
                                        let amplitude = if amplitude.is_nan() {
                                            0.02
                                        } else {
                                            amplitude.clamp(0.02, 1.0)
                                        };
                                        let phase = if phase.is_nan() { 0.0 } else { phase };

                                        // 1. Organic Waveform Fill
                                        let mut fill_builder = PathBuilder::fill();
                                        fill_builder.move_to(point(
                                            bounds.origin.x,
                                            bounds.origin.y + height,
                                        ));

                                        let step_f32 = 2.0_f32;
                                        let mut x_f32 = 0.0_f32;
                                        let cycles = 2.0_f32;

                                        while x_f32 <= width_f32 {
                                            let t = x_f32 / width_f32;
                                            let wave1 =
                                                (t * cycles * std::f32::consts::TAU + phase).sin();
                                            let wave2 =
                                                (t * (cycles * 0.5) * std::f32::consts::TAU
                                                    + phase * 1.5)
                                                    .sin()
                                                    * 0.5;
                                            let normalized_wave = (wave1 + wave2 + 1.5) / 3.0;
                                            let min_height = height_f32 * 0.05;
                                            let amp_px = amplitude * (height_f32 * 0.75);
                                            let y =
                                                height_f32 - min_height - normalized_wave * amp_px;

                                            fill_builder.line_to(point(
                                                bounds.origin.x + px(x_f32),
                                                bounds.origin.y + px(y),
                                            ));
                                            x_f32 += step_f32;
                                        }
                                        fill_builder.line_to(point(
                                            bounds.origin.x + width,
                                            bounds.origin.y + height,
                                        ));
                                        fill_builder.close();

                                        if let Ok(fill_path) = fill_builder.build() {
                                            let fill_color = if (is_recording || is_agent_recording)
                                                && !is_paused
                                            {
                                                hsla(0.0, 0.84, 0.60, 0.55) // Crimson Red (#ef4444)
                                            } else {
                                                hsla(0.0, 0.0, 0.50, 0.15) // Quiescent wave
                                            };
                                            window.paint_path(fill_path, fill_color);
                                        }

                                        // 2. Top Crest Contour Stroke
                                        let mut stroke_builder = PathBuilder::stroke(px(2.0));
                                        let mut x_f32 = 0.0_f32;
                                        let first_t = 0.0_f32;
                                        let first_wave = ((first_t
                                            * cycles
                                            * std::f32::consts::TAU
                                            + phase)
                                            .sin()
                                            + (first_t * (cycles * 0.5) * std::f32::consts::TAU
                                                + phase * 1.5)
                                                .sin()
                                                * 0.5
                                            + 1.5)
                                            / 3.0;
                                        let first_y = height_f32
                                            - (height_f32 * 0.05)
                                            - first_wave * (amplitude * (height_f32 * 0.75));
                                        stroke_builder.move_to(point(
                                            bounds.origin.x,
                                            bounds.origin.y + px(first_y),
                                        ));

                                        while x_f32 <= width_f32 {
                                            let t = x_f32 / width_f32;
                                            let wave1 =
                                                (t * cycles * std::f32::consts::TAU + phase).sin();
                                            let wave2 =
                                                (t * (cycles * 0.5) * std::f32::consts::TAU
                                                    + phase * 1.5)
                                                    .sin()
                                                    * 0.5;
                                            let normalized_wave = (wave1 + wave2 + 1.5) / 3.0;
                                            let min_height = height_f32 * 0.05;
                                            let amp_px = amplitude * (height_f32 * 0.75);
                                            let y =
                                                height_f32 - min_height - normalized_wave * amp_px;

                                            stroke_builder.line_to(point(
                                                bounds.origin.x + px(x_f32),
                                                bounds.origin.y + px(y),
                                            ));
                                            x_f32 += step_f32;
                                        }
                                        if let Ok(stroke_path) = stroke_builder.build() {
                                            let stroke_color = if (is_recording
                                                || is_agent_recording)
                                                && !is_paused
                                            {
                                                hsla(0.0, 0.84, 0.60, 0.95)
                                            } else {
                                                hsla(0.0, 0.0, 0.50, 0.40)
                                            };
                                            window.paint_path(stroke_path, stroke_color);
                                        }
                                    },
                                )
                                .size_full(),
                            ),
                    )
                    .child(
                        // ==========================================
                        // MM:SS Monospace Timer (Gap G08)
                        // ==========================================
                        div()
                            .id("recording-timer")
                            .h(px(32.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family("Consolas, monospace")
                            .text_size(px(24.0))
                            .text_color(if is_recording || is_agent_recording {
                                hsla(0.0, 0.84, 0.60, 1.0) // Red #ef4444
                            } else {
                                gpui_kit::transparent_black() // Hidden when stopped, retains 32px height!
                            })
                            .child(self.formatted_timer_text()),
                    )
                    .child(
                        // ==========================================
                        // Controls Row (64px Record + 56px Pause) (Gap G08)
                        // ==========================================
                        div()
                            .id("record-controls")
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(24.0))
                            // Pause/Resume button (56px circular) - only visible when recording!
                            .when(is_recording, |this| {
                                let pause_tooltip = if is_paused { "Resume" } else { "Pause" };
                                this.child(
                                    div()
                                        .id("btn-live-pause")
                                        .size(px(56.0))
                                        .rounded_full()
                                        .bg(cx.theme().secondary)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .tooltip(move |window, cx| {
                                            Tooltip::new(pause_tooltip).build(window, cx)
                                        })
                                        .child(if is_paused {
                                            icon(LucideIcon::Play).size(px(24.0))
                                        } else {
                                            icon(LucideIcon::Pause).size(px(24.0))
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_pause(cx);
                                        })),
                                )
                            })
                            // Start / Stop button (64px circular)
                            .child(if !is_recording {
                                let start_tooltip = if is_agent_recording {
                                    "AI Agent recording in progress"
                                } else if is_initializing {
                                    "Initializing..."
                                } else {
                                    "Start recording"
                                };
                                // Start Button: 64px circular red button with 24px white inner dot
                                div()
                                    .id("btn-live-start")
                                    .size(px(64.0))
                                    .rounded_full()
                                    .bg(hsla(0.0, 0.84, 0.60, 1.0)) // Red #ef4444
                                    .shadow_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(start_tooltip).build(window, cx)
                                    })
                                    .when(
                                        is_agent_recording || is_initializing || is_transitioning,
                                        |this| this.opacity(0.5).cursor_default(),
                                    )
                                    .child(
                                        // 24px circular white center dot
                                        div().size(px(24.0)).rounded_full().bg(gpui_kit::white()),
                                    )
                                    .when(
                                        !is_agent_recording
                                            && !is_initializing
                                            && !is_transitioning,
                                        |this| {
                                            this.on_click(cx.listener(|this, _, _, cx| {
                                                this.start_recording(cx);
                                            }))
                                        },
                                    )
                            } else {
                                // Stop Button: 64px circular red button with 24px white rounded square
                                div()
                                    .id("btn-live-stop")
                                    .size(px(64.0))
                                    .rounded_full()
                                    .bg(hsla(0.0, 0.84, 0.60, 1.0))
                                    .shadow_lg()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Stop recording").build(window, cx)
                                    })
                                    .when(is_transitioning, |this| {
                                        this.opacity(0.5).cursor_default()
                                    })
                                    .child(
                                        // 24px white stop square
                                        div()
                                            .size(px(24.0))
                                            .rounded(RADIUS_SM)
                                            .bg(gpui_kit::white()),
                                    )
                                    .when(!is_transitioning, |this| {
                                        this.on_click(cx.listener(|this, _, _, cx| {
                                            this.stop_recording(cx);
                                        }))
                                    })
                            }),
                    )
                    .child(
                        // ==========================================
                        // Audio Source Dropdown Selector (Gap G09)
                        // ==========================================
                        div()
                            .id("input-source-selector")
                            .h(px(32.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(!is_recording, |this| {
                                let is_open = is_source_open;

                                this.child(
                                    div()
                                        .relative()
                                        .child(
                                            // Trigger button
                                            div()
                                                .id("source-select-trigger")
                                                .min_w(px(180.0))
                                                .h(px(32.0))
                                                .px(px(12.0))
                                                .rounded(RADIUS_MD)
                                                .bg(cx.theme().secondary.opacity(0.6))
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .gap(px(8.0))
                                                .cursor_pointer()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(8.0))
                                                        .child(
                                                            icon(current_source.icon())
                                                                .size(px(16.0)),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(13.0))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .child(current_source.label()),
                                                        ),
                                                )
                                                .child(icon(LucideIcon::ChevronDown).size(px(14.0)))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.is_source_dropdown_open =
                                                        !this.is_source_dropdown_open;
                                                    cx.notify();
                                                })),
                                        )
                                        .when(is_open, |this| {
                                            // Menu popover
                                            this.child(
                                                div()
                                                    .id("source-select-menu")
                                                    .absolute()
                                                    .top(px(36.0))
                                                    .left_0()
                                                    .w_full()
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
                                                        // Option 1: Microphone
                                                        div()
                                                            .id("source-opt-mic")
                                                            .h(px(28.0))
                                                            .px(px(8.0))
                                                            .rounded(RADIUS_SM)
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(8.0))
                                                            .cursor_pointer()
                                                            .bg(
                                                                if current_source
                                                                    == AudioSourceKind::Microphone
                                                                {
                                                                    cx.theme().secondary
                                                                } else {
                                                                    gpui_kit::transparent_black()
                                                                },
                                                            )
                                                            .child(
                                                                icon(LucideIcon::Mic)
                                                                    .size(px(15.0)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(12.5))
                                                                    .child("Microphone"),
                                                            )
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.audio_source =
                                                                        AudioSourceKind::Microphone;
                                                                    this.is_source_dropdown_open =
                                                                        false;
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        // Option 2: System Audio
                                                        div()
                                                            .id("source-opt-sys")
                                                            .h(px(28.0))
                                                            .px(px(8.0))
                                                            .rounded(RADIUS_SM)
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(8.0))
                                                            .cursor_pointer()
                                                            .bg(
                                                                if current_source
                                                                    == AudioSourceKind::SystemAudio
                                                                {
                                                                    cx.theme().secondary
                                                                } else {
                                                                    gpui_kit::transparent_black()
                                                                },
                                                            )
                                                            .child(
                                                                icon(LucideIcon::Monitor)
                                                                    .size(px(15.0)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(12.5))
                                                                    .child("System Audio"),
                                                            )
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.audio_source =
                                                            AudioSourceKind::SystemAudio;
                                                                    this.is_source_dropdown_open =
                                                                        false;
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    ),
                                            )
                                        }),
                                )
                            }),
                    )
                    .child(
                        // Status helper copy
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(self.status_helper_text()),
                    ),
            )
    }
}
