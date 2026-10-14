use std::sync::Arc;
use tokio::sync::Mutex;

use crate::services::DesktopServices;
use sona_application::live_transcription::{LiveInputTransform, LiveSourceEpoch};
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrTranscriptUpdateEvent, AsrTranscriptionRequest,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::TranscriptSegment;

use super::asr_config::resolve_live_asr_request_from_config;
use super::types::{
    AudioDevicesResult, StartRecordingRequest, StartRecordingResult, StopRecordingRequest,
    StopRecordingResult,
};

pub(crate) struct AgentAsrRuntimeObserver {
    inner: crate::integrations::asr::TauriAsrRuntimeObserver,
    segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
}

impl AgentAsrRuntimeObserver {
    pub(crate) fn new(
        inner: crate::integrations::asr::TauriAsrRuntimeObserver,
        segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
    ) -> Self {
        Self { inner, segments }
    }
}

impl AsrRuntimeObserver for AgentAsrRuntimeObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        self.inner.on_transcript_update(event);

        if let Ok(mut segs) = self.segments.lock() {
            if !event.update.remove_ids.is_empty() {
                segs.retain(|s| !event.update.remove_ids.contains(&s.id));
            }
            for upsert in &event.update.upsert_segments {
                if let Some(pos) = segs.iter().position(|s| s.id == upsert.id) {
                    segs[pos] = upsert.clone();
                } else {
                    segs.push(upsert.clone());
                }
            }
        }
    }

    fn on_model_load(&self, metric: &AsrModelLoadMetric) {
        self.inner.on_model_load(metric);
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        self.inner.on_live_inference(metric);
    }
}

#[derive(Clone)]
pub struct ActiveRecordingSession {
    pub history_id: String,
    pub consumer_id: String,
    pub source_kind: String,
    pub is_paused: bool,
    pub paused_at_instant: Option<std::time::Instant>,
    pub accumulated_pause_duration: std::time::Duration,
    pub asr_request: Option<AsrTranscriptionRequest>,
    pub gain: f32,
    pub started_at_epoch: u64,
    pub started_at_instant: std::time::Instant,
    pub segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
    pub has_coordinator_consumer: bool,
    pub coordinator_released: bool,
    pub audio_stopped: bool,
    pub frozen_duration_seconds: Option<f64>,
    pub _sleep_guard: Option<Arc<crate::platform::system::power::SleepPreventionGuard>>,
    pub is_ui_session: bool,
    pub saved_wav_path: Option<String>,
}

pub async fn start_recording(
    services: &DesktopServices,
    app_handle: Option<&tauri::AppHandle>,
    active_session: &Arc<Mutex<Option<ActiveRecordingSession>>>,
    recording_op_lock: &Arc<Mutex<()>>,
    req: StartRecordingRequest,
) -> Result<StartRecordingResult, String> {
    let _op_guard = recording_op_lock.lock().await;
    if services.audio.has_active_captures() || services.asr.is_busy().await {
        return Err("DEVICE_BUSY: Audio capture or ASR engine is currently in use".to_string());
    }

    {
        let session_guard = active_session.lock().await;
        if session_guard.is_some() {
            return Err(
                "RECORDING_ALREADY_ACTIVE: A recording session is already in progress".to_string(),
            );
        }
    }

    // 1. Resolve ASR request FIRST before creating draft or starting hardware capture
    let resolved_asr_req = match req.asr_request {
        Some(explicit) => Some(explicit),
        None => match services
            .config
            .load()
            .map_err(|e| format!("Failed to load config: {e}"))?
        {
            Some(cfg) => resolve_live_asr_request_from_config(&cfg).ok(),
            None => None,
        },
    };

    let history_id = uuid::Uuid::new_v4().to_string();
    let now_epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let draft_req = crate::platform::history_repository::HistoryCreateLiveDraftRequest {
        id: Some(history_id.clone()),
        audio_extension: "wav".to_string(),
        tag_ids: req.project_id.clone().into_iter().collect(),
        project_id: req.project_id.clone(),
        icon: None,
    };

    let draft = services
        .history
        .mutation_file(move |s| s.create_live_draft(draft_req))
        .await
        .map_err(|e| e.to_string())?;

    let audio_state = services.audio.clone();
    let asr_state = services.asr.clone();
    let emitter = services.emitter.clone();
    let app_data_dir = services
        .sqlite
        .current_context()
        .map_err(|e| e.to_string())?
        .app_data_dir()
        .to_path_buf();

    let consumer_id = format!("agent-recording-{}", draft.item.id);
    let output_path = Some(draft.audio_absolute_path.clone());
    let device_name = req.device_name.clone();
    let consumer_id_clone = consumer_id.clone();

    let source_kind = match req.source.as_deref().unwrap_or("microphone") {
        "system" => "system".to_string(),
        "microphone" => "microphone".to_string(),
        other => {
            return Err(format!(
                "INVALID_SOURCE: Unsupported audio source '{other}', expected 'microphone' or 'system'"
            ));
        }
    };
    let source_kind_capture = source_kind.clone();
    let lease = match crate::integrations::audio::start_native_live_capture(
        audio_state,
        asr_state,
        emitter,
        app_data_dir,
        &source_kind_capture,
        device_name,
        consumer_id_clone,
        output_path,
    )
    .await
    {
        Ok(lease) => lease,
        Err(e) => {
            let hid = draft.item.id.clone();
            let _ = services
                .history
                .mutation_file(move |s| {
                    s.purge_items(
                        sona_core::history::mutation_repository::HistoryDeleteItemsRequest {
                            ids: vec![hid],
                        },
                    )
                })
                .await;
            return Err(format!("Failed to start native live capture: {e}"));
        }
    };

    let segments = Arc::new(std::sync::Mutex::new(Vec::new()));
    let inner_observer = crate::integrations::asr::TauriAsrRuntimeObserver::new(
        services.emitter.clone(),
        services.asr.metrics_store(),
    );
    let observer = Arc::new(AgentAsrRuntimeObserver::new(
        inner_observer,
        segments.clone(),
    )) as Arc<dyn AsrRuntimeObserver>;

    let mut has_coordinator_consumer = false;
    if let Some(asr_req) = resolved_asr_req.clone() {
        if services
            .asr
            .live_coordinator()
            .has_consumer(&consumer_id)
            .await
        {
            let _ = services.asr.live_coordinator().release(&consumer_id).await;
        }
        match services
            .asr
            .live_coordinator()
            .acquire(
                consumer_id.clone(),
                LiveSourceEpoch::new(lease.source_id.clone(), lease.source_generation),
                lease.source_cursor,
                LiveInputTransform { gain: 1.0 },
                asr_req,
                observer,
            )
            .await
        {
            Ok(_) => {
                has_coordinator_consumer = true;
            }
            Err(e) => {
                let _ = crate::integrations::audio::stop_native_live_capture(
                    &services.audio,
                    &source_kind,
                    consumer_id.clone(),
                )
                .await;
                let hid = draft.item.id.clone();
                let _ = services
                    .history
                    .mutation_file(move |s| {
                        s.purge_items(
                            sona_core::history::mutation_repository::HistoryDeleteItemsRequest {
                                ids: vec![hid],
                            },
                        )
                    })
                    .await;
                return Err(format!(
                    "ASR_ACQUIRE_FAILED: Failed to acquire ASR coordinator for live transcription: {e}"
                ));
            }
        }
    }

    let sleep_guard = Arc::new(
        crate::platform::system::power::SleepPreventionGuard::acquire("Agent Live Audio Recording"),
    );

    {
        let mut session_guard = active_session.lock().await;
        *session_guard = Some(ActiveRecordingSession {
            history_id: draft.item.id.clone(),
            consumer_id,
            source_kind,
            is_paused: false,
            paused_at_instant: None,
            accumulated_pause_duration: std::time::Duration::ZERO,
            asr_request: resolved_asr_req,
            gain: 1.0,
            started_at_epoch: now_epoch_secs,
            started_at_instant: std::time::Instant::now(),
            segments,
            has_coordinator_consumer,
            coordinator_released: false,
            audio_stopped: false,
            frozen_duration_seconds: None,
            _sleep_guard: Some(sleep_guard),
            is_ui_session: false,
            saved_wav_path: None,
        });
    }

    let _ = services.emitter.emit(
        "agent-control-recording-status",
        serde_json::json!({ "active": true, "isPaused": false, "historyId": &draft.item.id }),
    );
    if let Some(app) = app_handle {
        crate::app::tray::set_recording_active(app, true);
    }

    Ok(StartRecordingResult {
        history_id: draft.item.id,
        started_at: now_epoch_secs,
    })
}

pub async fn stop_recording(
    services: &DesktopServices,
    app_handle: Option<&tauri::AppHandle>,
    active_session: &Arc<Mutex<Option<ActiveRecordingSession>>>,
    recording_op_lock: &Arc<Mutex<()>>,
    req: StopRecordingRequest,
) -> Result<StopRecordingResult, String> {
    let _op_guard = recording_op_lock.lock().await;
    let (session_snapshot, duration_seconds) = {
        let mut session_guard = active_session.lock().await;
        if let Some(session) = session_guard.as_mut() {
            let dur = session.frozen_duration_seconds.unwrap_or_else(|| {
                let total_elapsed = session.started_at_instant.elapsed();
                let pause_duration = session.accumulated_pause_duration
                    + if session.is_paused {
                        session
                            .paused_at_instant
                            .map(|p| p.elapsed())
                            .unwrap_or_default()
                    } else {
                        std::time::Duration::ZERO
                    };
                total_elapsed.saturating_sub(pause_duration).as_secs_f64()
            });
            session.frozen_duration_seconds = Some(dur);
            (Some(session.clone()), dur)
        } else if services.audio.has_active_captures() {
            (None, 0.0)
        } else {
            return Err("NO_ACTIVE_RECORDING: No recording is currently in progress".to_string());
        }
    };

    if session_snapshot.is_none() {
        log::info!("[AgentControl] Stopping active audio captures without active session");
        crate::integrations::audio::stop_all_captures(&services.audio).await;
        let _ = services.emitter.emit(
            "agent-control-recording-status",
            serde_json::json!({ "active": false, "isPaused": false }),
        );
        if let Some(app) = app_handle {
            crate::app::tray::set_recording_active(app, false);
        }
        return Ok(StopRecordingResult {
            history_id: String::new(),
            duration_seconds: 0.0,
            segment_count: 0,
        });
    }
    let session_snapshot = session_snapshot.unwrap();
    let is_ui_session =
        session_snapshot.is_ui_session || session_snapshot.history_id.starts_with("ui-record-");
    // Delegate UI-session teardown to its recorder before backend teardown so that
    // the recorder stops native capture, receives the saved WAV path, and persists
    // the recording draft cleanly.
    if is_ui_session {
        let mut final_history_id = session_snapshot.history_id.clone();
        let notified = services.recording_session.notified_cleared();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let _ = services.emitter.emit(
            "agent-control-ui-recording-stop-requested",
            serde_json::json!({
                "historyId": session_snapshot.history_id,
                "discard": req.discard
            }),
        );

        // Wait for UI to finish teardown (active_session cleared by stop_native_live_transcription)
        let ui_teardown_completed = if active_session.lock().await.is_none() {
            true
        } else {
            tokio::time::timeout(std::time::Duration::from_millis(1000), notified)
                .await
                .is_ok()
                || active_session.lock().await.is_none()
        };

        if !ui_teardown_completed {
            log::warn!(
                "[AgentControl] UI recording teardown timed out after 1000ms for history_id {}; falling back to clean backend teardown",
                session_snapshot.history_id
            );
            if session_snapshot.has_coordinator_consumer && !session_snapshot.coordinator_released {
                let _ = services
                    .asr
                    .live_coordinator()
                    .release(&session_snapshot.consumer_id)
                    .await;
            }
            let mut captured_path: Option<String> = None;
            if !session_snapshot.audio_stopped {
                let path = crate::integrations::audio::stop_native_live_capture(
                    &services.audio,
                    &session_snapshot.source_kind,
                    session_snapshot.consumer_id.clone(),
                )
                .await;
                if let Ok(saved_path) = path {
                    if req.discard {
                        let _ = tokio::fs::remove_file(&saved_path).await;
                    } else {
                        captured_path = Some(saved_path.clone());
                        services
                            .recording_session
                            .cache_saved_wav_path(session_snapshot.consumer_id.clone(), saved_path)
                            .await;
                    }
                }
            } else if req.discard {
                if let Some(path) = services
                    .recording_session
                    .take_cached_saved_wav_path(&session_snapshot.consumer_id)
                    .await
                {
                    let _ = tokio::fs::remove_file(path).await;
                }
            } else {
                captured_path = services
                    .recording_session
                    .get_cached_saved_wav_path(&session_snapshot.consumer_id)
                    .await;
            }
            let mut session_guard = active_session.lock().await;
            *session_guard = None;
            services.recording_session.notify_cleared();
            if !req.discard {
                let segments = session_snapshot
                    .segments
                    .lock()
                    .map(|s| s.clone())
                    .unwrap_or_default();

                let hid = session_snapshot.history_id.clone();
                let exists = if is_ui_session {
                    false
                } else {
                    services
                        .history
                        .query_db(move |s| s.get_item(&hid))
                        .await
                        .ok()
                        .flatten()
                        .is_some()
                };

                let native_audio_path = captured_path.filter(|p| std::path::Path::new(p).is_file());

                if exists {
                    let hid = session_snapshot.history_id.clone();
                    let complete_req =
                        sona_core::history::mutation_repository::HistoryCompleteLiveDraftRequest {
                            history_id: hid,
                            segments,
                            duration: duration_seconds,
                        };
                    if let Err(e) = services
                        .history
                        .mutation_file(move |s| s.complete_live_draft(complete_req))
                        .await
                    {
                        log::error!("Failed to complete fallback recording draft: {e}");
                    }
                } else if let Some(path) = native_audio_path {
                    let save_req = sona_core::history::HistorySaveRecordingRequest {
                        segments,
                        duration: duration_seconds,
                        tag_ids: Vec::new(),
                        project_id: None,
                        audio_bytes: None,
                        native_audio_path: Some(path),
                        audio_extension: Some("wav".to_string()),
                    };
                    match services
                        .history
                        .mutation_file(move |s| s.save_recording(save_req))
                        .await
                    {
                        Ok(record) => {
                            final_history_id = record.id;
                        }
                        Err(e) => {
                            log::error!("Failed to save fallback recording draft: {e}");
                        }
                    }
                } else {
                    let draft_id = if session_snapshot.history_id.starts_with("ui-record-") {
                        None
                    } else {
                        Some(session_snapshot.history_id.clone())
                    };
                    let draft_req =
                        crate::platform::history_repository::HistoryCreateLiveDraftRequest {
                            id: draft_id,
                            audio_extension: "wav".to_string(),
                            tag_ids: Vec::new(),
                            project_id: None,
                            icon: None,
                        };
                    let dur = duration_seconds;
                    match services
                        .history
                        .mutation_file(move |s| {
                            let draft = s.create_live_draft(draft_req)?;
                            s.complete_live_draft(
                                sona_core::history::mutation_repository::HistoryCompleteLiveDraftRequest {
                                    history_id: draft.item.id,
                                    segments,
                                    duration: dur,
                                },
                            )
                        })
                        .await
                    {
                        Ok(record) => {
                            final_history_id = record.id;
                        }
                        Err(e) => {
                            log::error!("Failed to save fallback recording draft: {e}");
                        }
                    }
                }
            } else if !session_snapshot.history_id.starts_with("ui-record-") {
                let hid = session_snapshot.history_id.clone();
                let deleted_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let _ = services
                    .history
                    .mutation_file(move |s| {
                        s.trash_items(
                            sona_core::history::mutation_repository::HistoryTrashItemsRequest {
                                ids: vec![hid],
                                deleted_at,
                            },
                        )
                    })
                    .await;
            }
        }

        let _ = services.emitter.emit(
            "agent-control-recording-status",
            serde_json::json!({ "active": false, "isPaused": false }),
        );
        if let Some(app) = app_handle {
            crate::app::tray::set_recording_active(app, false);
        }

        let segment_count = if req.discard {
            0
        } else {
            session_snapshot
                .segments
                .lock()
                .map(|s| s.len())
                .unwrap_or(0)
        };

        return Ok(StopRecordingResult {
            history_id: final_history_id,
            duration_seconds,
            segment_count,
        });
    }

    // 1. Release coordinator if active and not already released
    if session_snapshot.has_coordinator_consumer && !session_snapshot.coordinator_released {
        services
            .asr
            .live_coordinator()
            .release(&session_snapshot.consumer_id)
            .await
            .map_err(|e| format!("ASR_RELEASE_FAILED: Failed to release ASR coordinator: {e}"))?;
        if let Some(s) = active_session.lock().await.as_mut() {
            s.coordinator_released = true;
        }
    }

    // 2. Stop audio capture if not already stopped
    if !session_snapshot.audio_stopped {
        crate::integrations::audio::stop_native_live_capture(
            &services.audio,
            &session_snapshot.source_kind,
            session_snapshot.consumer_id.clone(),
        )
        .await
        .map_err(|e| format!("AUDIO_STOP_FAILED: Failed to stop native audio capture: {e}"))?;
        if let Some(s) = active_session.lock().await.as_mut() {
            s.audio_stopped = true;
        }
    }

    // 3. Finalize or discard draft with strict error checking
    let segment_count = if req.discard {
        let deleted_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis() as u64;
        let hid = session_snapshot.history_id.clone();
        services
            .history
            .mutation_file(move |s| {
                s.trash_items(
                    sona_core::history::mutation_repository::HistoryTrashItemsRequest {
                        ids: vec![hid],
                        deleted_at,
                    },
                )
            })
            .await
            .map_err(|e| format!("TRASH_DRAFT_FAILED: Failed to discard recording draft: {e}"))?;
        0
    } else {
        let collected = session_snapshot
            .segments
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let segments = if !collected.is_empty() {
            collected
        } else {
            let hid = session_snapshot.history_id.clone();
            services
                .history
                .query_db(move |s| s.load_transcript(&hid))
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        let count = segments.len();

        let complete_req =
            sona_core::history::mutation_repository::HistoryCompleteLiveDraftRequest {
                history_id: session_snapshot.history_id.clone(),
                segments,
                duration: duration_seconds,
            };

        services
            .history
            .mutation_file(move |s| s.complete_live_draft(complete_req))
            .await
            .map_err(|e| {
                format!("COMPLETE_DRAFT_FAILED: Failed to complete recording draft: {e}")
            })?;
        count
    };

    // All steps succeeded; clear active session
    {
        let mut session_guard = active_session.lock().await;
        *session_guard = None;
    }

    let _ = services.emitter.emit(
        "agent-control-recording-status",
        serde_json::json!({ "active": false, "isPaused": false }),
    );
    if let Some(app) = app_handle {
        crate::app::tray::set_recording_active(app, false);
    }

    Ok(StopRecordingResult {
        history_id: session_snapshot.history_id,
        duration_seconds,
        segment_count,
    })
}

pub async fn pause_recording(
    services: &DesktopServices,
    active_session: &Arc<Mutex<Option<ActiveRecordingSession>>>,
    recording_op_lock: &Arc<Mutex<()>>,
) -> Result<bool, String> {
    let _op_guard = recording_op_lock.lock().await;
    let (source_kind, consumer_id, has_coordinator_consumer, history_id) = {
        let session_guard = active_session.lock().await;
        let session = session_guard.as_ref().ok_or_else(|| {
            "NO_ACTIVE_RECORDING: No recording is currently in progress".to_string()
        })?;

        if session.is_paused {
            return Err(
                "RECORDING_ALREADY_PAUSED: Recording session is already paused".to_string(),
            );
        }
        (
            session.source_kind.clone(),
            session.consumer_id.clone(),
            session.has_coordinator_consumer && !session.coordinator_released,
            session.history_id.clone(),
        )
    };

    match crate::integrations::audio::set_native_live_capture_paused(
        &services.audio,
        &source_kind,
        &consumer_id,
        true,
    ) {
        Ok(_) | Err(crate::integrations::audio::AudioCaptureError::InstanceNotActive(_)) => {}
        Err(e) => return Err(format!("AUDIO_PAUSE_FAILED: {e}")),
    }

    if has_coordinator_consumer {
        let release_res = services.asr.live_coordinator().release(&consumer_id).await;
        if let Err(e) = release_res {
            let _ = crate::integrations::audio::set_native_live_capture_paused(
                &services.audio,
                &source_kind,
                &consumer_id,
                false,
            );
            return Err(format!(
                "ASR_RELEASE_FAILED: Failed to release ASR coordinator: {e}"
            ));
        }
    }

    {
        let mut session_guard = active_session.lock().await;
        if let Some(session) = session_guard.as_mut() {
            if has_coordinator_consumer {
                session.coordinator_released = true;
            }
            session.is_paused = true;
            session.paused_at_instant = Some(std::time::Instant::now());
        }
    }

    let _ = services.emitter.emit(
        "agent-control-recording-status",
        serde_json::json!({ "active": true, "isPaused": true, "historyId": &history_id }),
    );

    Ok(true)
}

pub async fn resume_recording(
    services: &DesktopServices,
    active_session: &Arc<Mutex<Option<ActiveRecordingSession>>>,
    recording_op_lock: &Arc<Mutex<()>>,
) -> Result<bool, String> {
    let _op_guard = recording_op_lock.lock().await;
    let (source_kind, consumer_id, asr_request, gain, segments, history_id) = {
        let session_guard = active_session.lock().await;
        let session = session_guard.as_ref().ok_or_else(|| {
            "NO_ACTIVE_RECORDING: No recording is currently in progress".to_string()
        })?;

        if !session.is_paused {
            return Err("RECORDING_NOT_PAUSED: Recording session is not paused".to_string());
        }
        (
            session.source_kind.clone(),
            session.consumer_id.clone(),
            session.asr_request.clone(),
            session.gain,
            session.segments.clone(),
            session.history_id.clone(),
        )
    };

    let lease = crate::integrations::audio::set_native_live_capture_paused(
        &services.audio,
        &source_kind,
        &consumer_id,
        false,
    )
    .map_err(|e| format!("AUDIO_RESUME_FAILED: {e}"))?;

    let mut coordinator_acquired = false;
    if let Some(asr_req) = asr_request {
        let inner_observer = crate::integrations::asr::TauriAsrRuntimeObserver::new(
            services.emitter.clone(),
            services.asr.metrics_store(),
        );
        let observer = Arc::new(AgentAsrRuntimeObserver::new(inner_observer, segments))
            as Arc<dyn AsrRuntimeObserver>;

        if let Err(e) = services
            .asr
            .live_coordinator()
            .acquire(
                consumer_id.clone(),
                LiveSourceEpoch::new(lease.source_id.clone(), lease.source_generation),
                lease.source_cursor,
                LiveInputTransform { gain },
                asr_req,
                observer,
            )
            .await
        {
            let _ = crate::integrations::audio::set_native_live_capture_paused(
                &services.audio,
                &source_kind,
                &consumer_id,
                true,
            );
            return Err(format!("ASR_ACQUIRE_FAILED: {e}"));
        }
        coordinator_acquired = true;
    }

    {
        let mut session_guard = active_session.lock().await;
        if let Some(session) = session_guard.as_mut() {
            if coordinator_acquired {
                session.has_coordinator_consumer = true;
                session.coordinator_released = false;
            }
            if let Some(paused_at) = session.paused_at_instant.take() {
                session.accumulated_pause_duration += paused_at.elapsed();
            }
            session.is_paused = false;
        }
    }

    let _ = services.emitter.emit(
        "agent-control-recording-status",
        serde_json::json!({ "active": true, "isPaused": false, "historyId": &history_id }),
    );

    Ok(true)
}

pub async fn list_audio_devices() -> Result<AudioDevicesResult, String> {
    let microphones = crate::platform::blocking::spawn_blocking_map(
        crate::integrations::audio::get_microphone_devices,
    )
    .await
    .map_err(|e| e.to_string())?;

    let system_devices = crate::platform::blocking::spawn_blocking_map(
        crate::integrations::audio::get_system_audio_devices,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(AudioDevicesResult {
        microphones,
        system_devices,
    })
}
