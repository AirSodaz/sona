use super::*;
use crate::platform::database::DesktopSqliteState;
use crate::platform::event::MockEventEmitter;
use sona_core::history::TranscriptSnapshotReason;
use sona_core::ports::asr::AsrMode;
use sona_core::transcription::transcript::{SpeakerTag, TranscriptSegment};

async fn create_test_facade() -> (AgentControlFacade, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
    let sqlite = DesktopSqliteState::new(ctx);
    let emitter = Arc::new(MockEventEmitter::new());

    let services = DesktopServices::builder()
        .sqlite(sqlite)
        .event_emitter(emitter)
        .sync_config_path(temp.path().join("sync.json"))
        .build()
        .unwrap();

    let facade = AgentControlFacade::new(services, None);
    (facade, temp)
}

#[tokio::test]
async fn test_client_state_and_focus() {
    let (facade, _dir) = create_test_facade().await;

    let state = facade.get_client_state().await.unwrap();
    assert!(state.online);
    assert!(!state.is_recording);
    assert_eq!(state.active_project_id, None);

    let focus = facade.focus_window().unwrap();
    assert!(!focus);
}

#[tokio::test]
async fn test_casing_deserialization() {
    // Test snake_case vs camelCase wire format deserialization
    let snake_start: StartRecordingRequest =
        serde_json::from_str(r#"{"project_id":"p-1","device_name":"mic-1"}"#).unwrap();
    assert_eq!(snake_start.project_id.as_deref(), Some("p-1"));
    assert_eq!(snake_start.device_name.as_deref(), Some("mic-1"));

    let camel_start: StartRecordingRequest =
        serde_json::from_str(r#"{"projectId":"p-2","deviceName":"mic-2"}"#).unwrap();
    assert_eq!(camel_start.project_id.as_deref(), Some("p-2"));
    assert_eq!(camel_start.device_name.as_deref(), Some("mic-2"));

    let snake_query: QueryHistoryRequest =
        serde_json::from_str(r#"{"query":"hi","project_id":"p-3"}"#).unwrap();
    assert_eq!(snake_query.project_id.as_deref(), Some("p-3"));

    let snake_edit: EditTranscriptRequest =
        serde_json::from_str(r#"{"history_id":"h-1","segments":[]}"#).unwrap();
    assert_eq!(snake_edit.history_id, "h-1");

    let camel_edit: EditTranscriptRequest =
        serde_json::from_str(r#"{"historyId":"h-2","segments":[]}"#).unwrap();
    assert_eq!(camel_edit.history_id, "h-2");
}

#[tokio::test]
async fn test_resolve_live_asr_config() {
    let local_cfg = serde_json::json!({
        "streamingModelPath": "C:/models/streaming_encoder.onnx",
        "language": "en"
    });
    let req = resolve_live_asr_request_from_config(&local_cfg).unwrap();
    assert_eq!(req.language, "en");
    assert_eq!(req.mode, AsrMode::Streaming);

    let online_cfg = serde_json::json!({
        "asr": {
            "live": {
                "engine": "online",
                "providerId": "volcengine-doubao",
                "apiKey": "test-key"
            }
        }
    });
    let req_online = resolve_live_asr_request_from_config(&online_cfg).unwrap();
    assert_eq!(req_online.mode, AsrMode::Streaming);

    let empty_cfg = serde_json::json!({});
    assert!(resolve_live_asr_request_from_config(&empty_cfg).is_err());
}

#[tokio::test]
async fn test_projects_and_settings() {
    let (facade, _dir) = create_test_facade().await;

    let projects = facade.list_projects().await.unwrap();
    assert!(projects.is_empty());

    facade
        .set_active_project(Some("proj-1".to_string()))
        .await
        .unwrap();
    let state = facade.get_client_state().await.unwrap();
    assert_eq!(state.active_project_id, Some("proj-1".to_string()));

    let initial_cfg = facade.get_settings(None).unwrap();
    assert_eq!(initial_cfg, serde_json::Value::Null);

    facade
        .update_setting("theme".to_string(), serde_json::json!("dark"))
        .unwrap();
    let updated_cfg = facade.get_settings(Some("theme".to_string())).unwrap();
    assert_eq!(updated_cfg, serde_json::json!("dark"));
}

#[tokio::test]
async fn test_query_history_empty() {
    let (facade, _dir) = create_test_facade().await;

    let items = facade
        .query_history(QueryHistoryRequest {
            query: String::new(),
            project_id: None,
            limit: Some(10),
            offset: Some(0),
        })
        .await
        .unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn test_resolve_batch_asr_config() {
    let batch_cfg = serde_json::json!({
        "asr": {
            "selections": {
                "batch": {
                    "engine": "local",
                    "modelPath": "C:/models/sense-voice.onnx",
                    "language": "zh"
                }
            }
        }
    });
    let req = resolve_batch_asr_request_from_config(&batch_cfg, None).unwrap();
    assert_eq!(req.language, "zh");
    assert_eq!(req.mode, AsrMode::Batch);

    // Language override
    let req_override = resolve_batch_asr_request_from_config(&batch_cfg, Some("ja")).unwrap();
    assert_eq!(req_override.language, "ja");

    // Fallback to live setting with batch mode
    let live_cfg = serde_json::json!({
        "streamingModelPath": "C:/models/streaming_encoder.onnx",
        "language": "en"
    });
    let req_fallback = resolve_batch_asr_request_from_config(&live_cfg, None).unwrap();
    assert_eq!(req_fallback.language, "en");
    assert_eq!(req_fallback.mode, AsrMode::Batch);
}

#[tokio::test]
async fn test_project_crud_lifecycle() {
    let (facade, _dir) = create_test_facade().await;

    // Create project
    let created = facade
        .create_project(CreateProjectRequest {
            name: "Test Project".to_string(),
            description: Some("Description".to_string()),
            icon: Some("folder".to_string()),
            color: Some("#ff0000".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(created.name, "Test Project");

    // Update project
    let updated = facade
        .update_project(
            created.id.clone(),
            UpdateProjectRequest {
                name: Some("Updated Project".to_string()),
                description: None,
                icon: None,
                color: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.unwrap().name, "Updated Project");

    // Delete project
    let deleted = facade
        .delete_project(created.id, Some("moveToInbox".to_string()))
        .await
        .unwrap();
    assert!(deleted);
}

#[tokio::test]
async fn test_summary_and_trash() {
    let (facade, _dir) = create_test_facade().await;

    // Save a base history item first
    let dummy_path = _dir.path().join("dummy.wav");
    std::fs::write(&dummy_path, b"dummy audio content").unwrap();
    let history_id = "test-hist-1".to_string();
    let hid_clone = history_id.clone();
    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![],
                duration: 1.0,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();
    // Save and load summary
    let saved = facade
        .save_summary(SaveSummaryRequest {
            history_id: history_id.clone(),
            content: "This is a summary".to_string(),
            template_id: Some("meeting".to_string()),
            thought: Some("Thinking process".to_string()),
        })
        .await
        .unwrap();
    assert!(saved);

    let loaded = facade.load_summary(history_id.clone()).await.unwrap();
    assert!(loaded.is_some());
    let payload = loaded.unwrap();
    assert_eq!(payload.active_template_id, "meeting");
    let record = payload.record.unwrap();
    assert_eq!(record.content, "This is a summary");
    assert_eq!(record.thought.as_deref(), Some("Thinking process"));

    // Query trash initially empty
    let trash_items = facade
        .query_trash(QueryTrashRequest {
            query: String::new(),
            limit: Some(10),
            offset: Some(0),
        })
        .await
        .unwrap();
    assert!(trash_items.is_empty());
}

#[tokio::test]
async fn test_model_catalog_and_sync_status() {
    let (facade, _dir) = create_test_facade().await;

    let catalog = facade.get_model_catalog().await.unwrap();
    assert!(catalog.is_object());

    let sync_status = facade.get_sync_status().await.unwrap();
    assert!(sync_status.is_object());
}

#[tokio::test]
async fn test_patch_segments() {
    let (facade, _dir) = create_test_facade().await;
    let dummy_path = _dir.path().join("dummy.wav");
    std::fs::write(&dummy_path, b"audio").unwrap();
    let history_id = "test-hist-patch".to_string();
    let hid_clone = history_id.clone();
    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![
                    TranscriptSegment {
                        id: "seg-1".to_string(),
                        text: "Hello".to_string(),
                        start: 0.0,
                        end: 2.0,
                        is_final: true,
                        timing: None,
                        tokens: None,
                        timestamps: None,
                        durations: None,
                        translation: None,
                        speaker: None,
                        speaker_attribution: None,
                    },
                    TranscriptSegment {
                        id: "seg-2".to_string(),
                        text: "world".to_string(),
                        start: 2.0,
                        end: 4.0,
                        is_final: true,
                        timing: None,
                        tokens: None,
                        timestamps: None,
                        durations: None,
                        translation: None,
                        speaker: None,
                        speaker_attribution: None,
                    },
                ],
                duration: 4.0,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();

    // Patch seg-1, append seg-3, remove seg-2, with polish reason
    let patch_result = facade
        .patch_segments(PatchSegmentsRequest {
            history_id: history_id.clone(),
            segments: vec![
                SegmentPatchInput {
                    id: "seg-1".to_string(),
                    text: Some("Hello modified".to_string()),
                    start: None,
                    end: None,
                    translation: Some("Bonjour".to_string()),
                },
                SegmentPatchInput {
                    id: "seg-3".to_string(),
                    text: Some("appended segment".to_string()),
                    start: None,
                    end: None,
                    translation: None,
                },
            ],
            remove_ids: Some(vec!["seg-2".to_string()]),
            reason: Some("polish".to_string()),
        })
        .await
        .unwrap();

    assert!(patch_result.success);
    assert!(patch_result.snapshot_id.starts_with("polish-"));
    assert_eq!(patch_result.updated_count, 2);
    assert_eq!(patch_result.total_segments, 2);

    // Verify read_transcript
    let read = facade.read_transcript(history_id.clone()).await.unwrap();
    assert_eq!(read.segments.len(), 2);
    assert_eq!(read.segments[0].id, "seg-1");
    assert_eq!(read.segments[0].text, "Hello modified");
    assert_eq!(read.segments[0].translation.as_deref(), Some("Bonjour"));
    assert_eq!(read.segments[1].id, "seg-3");
    assert_eq!(read.segments[1].text, "appended segment");
    assert_eq!(read.translation_text.as_deref(), Some("Bonjour"));
}

#[tokio::test]
async fn test_update_translations() {
    let (facade, _dir) = create_test_facade().await;
    let dummy_path = _dir.path().join("dummy.wav");
    std::fs::write(&dummy_path, b"audio").unwrap();
    let history_id = "test-hist-trans".to_string();
    let hid_clone = history_id.clone();
    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![
                    TranscriptSegment {
                        id: "seg-1".to_string(),
                        text: "Apple".to_string(),
                        start: 0.0,
                        end: 1.0,
                        is_final: true,
                        timing: None,
                        tokens: None,
                        timestamps: None,
                        durations: None,
                        translation: None,
                        speaker: None,
                        speaker_attribution: None,
                    },
                    TranscriptSegment {
                        id: "seg-2".to_string(),
                        text: "Banana".to_string(),
                        start: 1.0,
                        end: 2.0,
                        is_final: true,
                        timing: None,
                        tokens: None,
                        timestamps: None,
                        durations: None,
                        translation: None,
                        speaker: None,
                        speaker_attribution: None,
                    },
                ],
                duration: 2.0,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();

    // Write translations
    let trans_result = facade
        .update_translations(UpdateTranslationsRequest {
            history_id: history_id.clone(),
            translations: vec![
                SegmentTranslationInput {
                    id: "seg-1".to_string(),
                    translation: Some("Pomme".to_string()),
                },
                SegmentTranslationInput {
                    id: "seg-2".to_string(),
                    translation: Some("Banane".to_string()),
                },
            ],
            clear_all: None,
        })
        .await
        .unwrap();

    assert!(trans_result.success);
    assert!(trans_result.snapshot_id.starts_with("translate-"));
    assert_eq!(trans_result.updated_count, 2);

    let read = facade.read_transcript(history_id.clone()).await.unwrap();
    assert_eq!(read.translation_text.as_deref(), Some("Pomme Banane"));

    // Verify snapshot reason is Translate in DB
    let hid_for_snap = history_id.clone();
    let snapshots = facade
        .services
        .history
        .query_db(move |repo| repo.list_transcript_snapshots(&hid_for_snap))
        .await
        .unwrap();
    assert!(!snapshots.is_empty());
    let translate_snap = snapshots
        .iter()
        .find(|s| s.id == trans_result.snapshot_id)
        .expect("must find translate snapshot");
    assert_eq!(translate_snap.reason, TranscriptSnapshotReason::Translate);

    // Clear all translations and set only seg-1
    let clear_result = facade
        .update_translations(UpdateTranslationsRequest {
            history_id: history_id.clone(),
            translations: vec![SegmentTranslationInput {
                id: "seg-1".to_string(),
                translation: Some("NewPomme".to_string()),
            }],
            clear_all: Some(true),
        })
        .await
        .unwrap();

    assert!(clear_result.success);
    let read_cleared = facade.read_transcript(history_id.clone()).await.unwrap();
    assert_eq!(
        read_cleared.segments[0].translation.as_deref(),
        Some("NewPomme")
    );
    assert_eq!(read_cleared.segments[1].translation, None);
    assert_eq!(read_cleared.translation_text.as_deref(), Some("NewPomme"));
}

#[tokio::test]
async fn test_summary_edit_and_delete() {
    let (facade, _dir) = create_test_facade().await;
    let dummy_path = _dir.path().join("dummy.wav");
    std::fs::write(&dummy_path, b"audio").unwrap();
    let history_id = "test-hist-sum-edit".to_string();
    let hid_clone = history_id.clone();
    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![],
                duration: 1.0,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();

    // Initial save with explicit template_id and thought
    let saved = facade
        .save_summary(SaveSummaryRequest {
            history_id: history_id.clone(),
            content: "Initial summary".to_string(),
            template_id: Some("meeting".to_string()),
            thought: Some("Initial thought".to_string()),
        })
        .await
        .unwrap();
    assert!(saved);

    let loaded = facade
        .load_summary(history_id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.active_template_id, "meeting");
    let record = loaded.record.unwrap();
    assert_eq!(record.content, "Initial summary");
    assert_eq!(record.thought.as_deref(), Some("Initial thought"));

    // Edit summary: content only, omit template_id and thought -> must be preserved!
    let edited = facade
        .save_summary(SaveSummaryRequest {
            history_id: history_id.clone(),
            content: "Updated summary content".to_string(),
            template_id: None,
            thought: None,
        })
        .await
        .unwrap();
    assert!(edited);

    let loaded_edited = facade
        .load_summary(history_id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded_edited.active_template_id, "meeting");
    let record_edited = loaded_edited.record.unwrap();
    assert_eq!(record_edited.content, "Updated summary content");
    assert_eq!(record_edited.thought.as_deref(), Some("Initial thought"));

    // Delete summary
    let deleted = facade.delete_summary(history_id.clone()).await.unwrap();
    assert!(deleted);

    let loaded_deleted = facade.load_summary(history_id.clone()).await.unwrap();
    assert!(loaded_deleted.is_none());
}

#[tokio::test]
async fn test_recording_pause_and_resume() {
    let (facade, _dir) = create_test_facade().await;

    // 1. Initially no active recording
    assert!(facade.pause_recording().await.is_err());
    assert!(facade.resume_recording().await.is_err());

    // 2. Set an active session
    let history_id = "test-rec-pause-resume".to_string();
    let segments = Arc::new(std::sync::Mutex::new(Vec::new()));
    {
        let mut guard = facade.active_session.lock().await;
        *guard = Some(ActiveRecordingSession {
            history_id: history_id.clone(),
            consumer_id: "test-consumer-1".to_string(),
            source_kind: "microphone".to_string(),
            is_paused: false,
            paused_at_instant: None,
            accumulated_pause_duration: std::time::Duration::ZERO,
            asr_request: None,
            gain: 1.0,
            started_at_epoch: 1700000000,
            started_at_instant: std::time::Instant::now(),
            segments: segments.clone(),
            has_coordinator_consumer: false,
            coordinator_released: false,
            audio_stopped: true,
            frozen_duration_seconds: None,
            _sleep_guard: None,
            is_ui_session: false,
            saved_wav_path: None,
        });
    }

    let state_before = facade.get_client_state().await.unwrap();
    assert!(state_before.is_recording);
    assert!(!state_before.is_paused);

    // 3. Pause recording
    let paused = facade.pause_recording().await.unwrap();
    assert!(paused);

    let state_paused = facade.get_client_state().await.unwrap();
    assert!(state_paused.is_recording);
    assert!(state_paused.is_paused);

    // Pausing again must error
    let pause_err = facade.pause_recording().await;
    assert!(pause_err.is_err());
    assert!(pause_err.unwrap_err().contains("RECORDING_ALREADY_PAUSED"));

    // 4. In this unit test environment without real native audio hardware capture,
    // resume_recording must strictly fail with AUDIO_RESUME_FAILED (InstanceNotActive)
    // and must NOT falsely mark the session as active/unpaused!
    let resume_res = facade.resume_recording().await;
    assert!(resume_res.is_err());
    assert!(resume_res.unwrap_err().contains("AUDIO_RESUME_FAILED"));

    // Verify the session remains paused
    let state_after_failed_resume = facade.get_client_state().await.unwrap();
    assert!(state_after_failed_resume.is_recording);
    assert!(state_after_failed_resume.is_paused);
    // 5. Clean up via stop_recording (with dummy draft created so stop succeeds)
    let dummy_path = _dir.path().join("dummy.wav");
    std::fs::write(&dummy_path, b"audio").unwrap();
    let hid_clone = history_id.clone();
    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![],
                duration: 1.0,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();

    let stop_res = facade
        .stop_recording(StopRecordingRequest { discard: false })
        .await
        .unwrap();
    assert_eq!(stop_res.history_id, history_id);
}

#[tokio::test]
async fn test_ui_session_stop_delegation() {
    let (facade, _dir) = create_test_facade().await;
    let history_id = "test-ui-rec-session".to_string();
    let segments = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Set an active UI recording session
    {
        let mut guard = facade.active_session.lock().await;
        *guard = Some(ActiveRecordingSession {
            history_id: history_id.clone(),
            consumer_id: "test-ui-consumer".to_string(),
            source_kind: "microphone".to_string(),
            is_paused: false,
            paused_at_instant: None,
            accumulated_pause_duration: std::time::Duration::ZERO,
            asr_request: None,
            gain: 1.0,
            started_at_epoch: 1700000000,
            started_at_instant: std::time::Instant::now(),
            segments: segments.clone(),
            has_coordinator_consumer: false,
            coordinator_released: false,
            audio_stopped: false,
            frozen_duration_seconds: None,
            _sleep_guard: None,
            is_ui_session: true,
            saved_wav_path: None,
        });
    }

    // Simulate UI recorder reacting to the event by clearing active_session
    let active_session_clone = facade.active_session.clone();
    let services_clone = facade.services.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut guard = active_session_clone.lock().await;
        *guard = None;
        services_clone.recording_session.notify_cleared();
    });
    let stop_res = facade
        .stop_recording(StopRecordingRequest { discard: false })
        .await
        .unwrap();

    assert_eq!(stop_res.history_id, history_id);
    assert_eq!(stop_res.segment_count, 0);

    let state_after = facade.get_client_state().await.unwrap();
    assert!(!state_after.is_recording);
}

#[tokio::test]
async fn test_ui_session_stop_returns_accumulated_segment_count() {
    let (facade, _dir) = create_test_facade().await;
    let history_id = "test-ui-segments-session".to_string();
    let segments = Arc::new(std::sync::Mutex::new(vec![
        TranscriptSegment {
            id: "seg-1".to_string(),
            text: "Hello world".to_string(),
            start: 0.0,
            end: 1.5,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        },
        TranscriptSegment {
            id: "seg-2".to_string(),
            text: "Second segment".to_string(),
            start: 1.5,
            end: 3.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        },
    ]));

    {
        let mut guard = facade.active_session.lock().await;
        *guard = Some(ActiveRecordingSession {
            history_id: history_id.clone(),
            consumer_id: "test-ui-consumer-seg".to_string(),
            source_kind: "microphone".to_string(),
            is_paused: false,
            paused_at_instant: None,
            accumulated_pause_duration: std::time::Duration::ZERO,
            asr_request: None,
            gain: 1.0,
            started_at_epoch: 1700000000,
            started_at_instant: std::time::Instant::now(),
            segments: segments.clone(),
            has_coordinator_consumer: false,
            coordinator_released: false,
            audio_stopped: false,
            frozen_duration_seconds: None,
            _sleep_guard: None,
            is_ui_session: true,
            saved_wav_path: None,
        });
    }

    let active_session_clone = facade.active_session.clone();
    let services_clone = facade.services.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut guard = active_session_clone.lock().await;
        *guard = None;
        services_clone.recording_session.notify_cleared();
    });

    let stop_res = facade
        .stop_recording(StopRecordingRequest { discard: false })
        .await
        .unwrap();

    assert_eq!(stop_res.history_id, history_id);
    assert_eq!(stop_res.segment_count, 2);
}

#[tokio::test]
async fn test_ui_session_stop_fallback_caches_saved_path() {
    let (facade, _dir) = create_test_facade().await;
    let history_id = "test-ui-fallback-session".to_string();
    let consumer_id = "test-ui-fallback-consumer".to_string();
    let segments = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Set an active UI session where audio is already marked stopped,
    // simulating a timeout where UI never clears active_session
    {
        let mut guard = facade.active_session.lock().await;
        *guard = Some(ActiveRecordingSession {
            history_id: history_id.clone(),
            consumer_id: consumer_id.clone(),
            source_kind: "microphone".to_string(),
            is_paused: false,
            paused_at_instant: None,
            accumulated_pause_duration: std::time::Duration::ZERO,
            asr_request: None,
            gain: 1.0,
            started_at_epoch: 1700000000,
            started_at_instant: std::time::Instant::now(),
            segments: segments.clone(),
            has_coordinator_consumer: false,
            coordinator_released: false,
            audio_stopped: true,
            frozen_duration_seconds: None,
            _sleep_guard: None,
            is_ui_session: true,
            saved_wav_path: None,
        });
    }

    facade
        .services
        .recording_session
        .cache_saved_wav_path(
            consumer_id.clone(),
            "C:/recordings/fallback.wav".to_string(),
        )
        .await;

    let stop_res = facade
        .stop_recording(StopRecordingRequest { discard: false })
        .await
        .unwrap();

    assert_eq!(stop_res.history_id, history_id);
    assert!(facade.active_session.lock().await.is_none());

    assert_eq!(
        facade
            .services
            .recording_session
            .take_cached_saved_wav_path(&consumer_id)
            .await,
        Some("C:/recordings/fallback.wav".to_string())
    );
    assert_eq!(
        facade
            .services
            .recording_session
            .take_cached_saved_wav_path(&consumer_id)
            .await,
        None
    );
}

#[tokio::test]
async fn test_history_meta_update() {
    let (facade, _dir) = create_test_facade().await;
    let dummy_path = _dir.path().join("dummy.wav");
    std::fs::write(&dummy_path, b"audio").unwrap();
    let history_id = "test-hist-meta-upd".to_string();
    let hid_clone = history_id.clone();
    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![],
                duration: 1.0,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();

    // Create a valid project to associate
    let proj = facade
        .create_project(CreateProjectRequest {
            name: "Meta Test Project".to_string(),
            description: None,
            icon: None,
            color: None,
        })
        .await
        .unwrap();
    let proj_id = proj.id;

    // Update title, icon, and project_id
    let upd = facade
        .update_history_meta(
            history_id.clone(),
            UpdateHistoryMetaRequest {
                title: Some("Updated Title".to_string()),
                project_id: Some(proj_id.clone()),
                icon: Some("bookmark".to_string()),
            },
        )
        .await
        .unwrap();
    assert!(upd);

    // Query back and verify
    let hid_q = history_id.clone();
    let items = facade
        .services
        .history
        .query_db(move |repo| repo.list_items(sona_core::history::HistoryListOptions::default()))
        .await
        .unwrap();
    let item = items
        .into_iter()
        .find(|it| it.id == hid_q)
        .expect("must exist");
    assert_eq!(item.title, "Updated Title");
    assert_eq!(item.project_id.as_deref(), Some(proj_id.as_str()));
    assert_eq!(item.icon.as_deref(), Some("bookmark"));
}

#[tokio::test]
async fn test_snapshot_revert() {
    let (facade, _dir) = create_test_facade().await;
    let dummy_path = _dir.path().join("dummy.wav");
    std::fs::write(&dummy_path, b"audio").unwrap();
    let history_id = "test-snap-revert".to_string();
    let hid_clone = history_id.clone();
    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![TranscriptSegment {
                    id: "seg-1".to_string(),
                    text: "Version 1 text".to_string(),
                    start: 0.0,
                    end: 2.0,
                    is_final: true,
                    timing: None,
                    tokens: None,
                    timestamps: None,
                    durations: None,
                    translation: None,
                    speaker: None,
                    speaker_attribution: None,
                }],
                duration: 2.0,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();

    // Edit to version 2 (creates a snapshot of version 1)
    let edit_v2 = facade
        .edit_transcript(EditTranscriptRequest {
            history_id: history_id.clone(),
            segments: vec![RelaxedTranscriptSegmentInput {
                id: Some("seg-1".to_string()),
                text: Some("Version 2 text".to_string()),
                start: None,
                end: None,
                is_final: None,
                translation: None,
            }],
            reason: Some("Edit to v2".to_string()),
        })
        .await
        .unwrap();
    assert!(edit_v2.success);

    // Verify snapshots list
    let snaps = facade
        .list_transcript_snapshots(history_id.clone())
        .await
        .unwrap();
    assert!(!snaps.is_empty());
    let snap_id = snaps[0].id.clone();

    // Edit to version 3
    let edit_v3 = facade
        .edit_transcript(EditTranscriptRequest {
            history_id: history_id.clone(),
            segments: vec![RelaxedTranscriptSegmentInput {
                id: Some("seg-1".to_string()),
                text: Some("Version 3 text".to_string()),
                start: None,
                end: None,
                is_final: None,
                translation: None,
            }],
            reason: Some("Edit to v3".to_string()),
        })
        .await
        .unwrap();
    assert!(edit_v3.success);

    let current_v3 = facade.read_transcript(history_id.clone()).await.unwrap();
    assert_eq!(current_v3.segments[0].text, "Version 3 text");

    // Revert to snapshot
    let revert_res = facade
        .revert_transcript_snapshot(history_id.clone(), snap_id)
        .await
        .unwrap();
    assert!(revert_res.success);

    // Verify text was reverted back
    let current_reverted = facade.read_transcript(history_id.clone()).await.unwrap();
    assert_eq!(current_reverted.segments[0].text, "Version 1 text");
}

#[tokio::test]
async fn test_audio_device_listing() {
    let (facade, _dir) = create_test_facade().await;
    let devices = facade.list_audio_devices().await.unwrap();
    // Even if empty in headless/container environments, lists must be returned without errors
    let val = serde_json::to_value(&devices).unwrap();
    assert!(val.get("microphones").is_some());
    assert!(val.get("systemDevices").is_some());
    assert!(val.get("microphones").unwrap().is_array());
    assert!(val.get("systemDevices").unwrap().is_array());
}

#[tokio::test]
async fn test_edit_transcript_preserves_speaker_and_timestamps() {
    let (facade, _dir) = create_test_facade().await;
    let dummy_path = _dir.path().join("dummy_speaker.wav");
    std::fs::write(&dummy_path, b"audio").unwrap();
    let history_id = "test-speaker-preserve".to_string();
    let hid_clone = history_id.clone();

    facade
        .services
        .history
        .mutation_file(move |s| {
            s.save_imported_file(sona_core::history::HistorySaveImportedFileRequest {
                id: Some(hid_clone),
                source_path: dummy_path.to_string_lossy().to_string(),
                segments: vec![TranscriptSegment {
                    id: "seg-preserve-1".to_string(),
                    text: "Original text before edit".to_string(),
                    start: 0.0,
                    end: 3.5,
                    is_final: true,
                    timing: None,
                    tokens: Some(vec!["Original".to_string(), "text".to_string()]),
                    timestamps: Some(vec![0.1, 0.8]),
                    durations: Some(vec![0.5, 0.4]),
                    translation: Some("Original translation".to_string()),
                    speaker: Some(SpeakerTag {
                        id: "spk-alice".to_string(),
                        label: "Alice".to_string(),
                        kind: "known".to_string(),
                        score: Some(0.95),
                    }),
                    speaker_attribution: None,
                }],
                duration: 3.5,
                tag_ids: vec![],
                project_id: None,
                converted_source_path: None,
            })
        })
        .await
        .unwrap();

    let edit_res = facade
        .edit_transcript(EditTranscriptRequest {
            history_id: history_id.clone(),
            segments: vec![RelaxedTranscriptSegmentInput {
                id: Some("seg-preserve-1".to_string()),
                text: Some("Polished text after edit".to_string()),
                start: None,
                end: None,
                is_final: None,
                translation: None,
            }],
            reason: Some("Polish text".to_string()),
        })
        .await
        .unwrap();
    assert!(edit_res.success);

    let current = facade.read_transcript(history_id).await.unwrap();
    assert_eq!(current.segments.len(), 1);
    let seg = &current.segments[0];
    assert_eq!(seg.text, "Polished text after edit");
    assert_eq!(seg.start, 0.0);
    assert_eq!(seg.end, 3.5);
    assert_eq!(seg.translation, Some("Original translation".to_string()));
    let speaker = seg.speaker.as_ref().expect("speaker should be preserved");
    assert_eq!(speaker.id, "spk-alice");
    assert_eq!(speaker.label, "Alice");
    assert_eq!(speaker.kind, "known");
    assert_eq!(speaker.score, Some(0.95));
    assert_eq!(seg.timestamps, Some(vec![0.1, 0.8]));
    assert_eq!(
        seg.tokens,
        Some(vec!["Original".to_string(), "text".to_string()])
    );
    assert_eq!(seg.durations, Some(vec![0.5, 0.4]));
}
#[tokio::test]
async fn test_agent_control_error_typing_and_cancellation() {
    let err1 = AgentControlError::from("TASK_CANCELLED: Task cancelled.");
    assert!(err1.is_cancelled());
    assert_eq!(err1.rpc_code(), -32800);
    assert_eq!(err1.to_string(), "TASK_CANCELLED: Task cancelled.");

    let err2 = AgentControlError::from("Task cancelled.");
    assert!(err2.is_cancelled());
    assert_eq!(err2.rpc_code(), -32800);

    let err3 =
        AgentControlError::from("NO_ACTIVE_RECORDING: No recording is currently in progress");
    assert!(!err3.is_cancelled());
    assert_eq!(err3.rpc_code(), -32000);
    assert_eq!(
        err3.to_string(),
        "NO_ACTIVE_RECORDING: No recording is currently in progress"
    );

    let (code, msg): (i32, String) = err1.into();
    assert_eq!(code, -32800);
    assert_eq!(msg, "TASK_CANCELLED: Task cancelled.");

    let err_exact = AgentControlError::from("cancelled");
    assert!(err_exact.is_cancelled());
    assert_eq!(err_exact.rpc_code(), -32800);

    let err_unrelated = AgentControlError::from(
        "Database operation failed: transaction cancelled due to lock timeout",
    );
    assert!(!err_unrelated.is_cancelled());
    assert_eq!(err_unrelated.rpc_code(), -32000);
}
#[tokio::test]
async fn test_stop_recording_fallback_when_no_active_session() {
    let (facade, _dir) = create_test_facade().await;

    // Initially no active session and no active captures -> should return NO_ACTIVE_RECORDING
    let res = facade
        .stop_recording(StopRecordingRequest { discard: false })
        .await;
    assert!(res.is_err());
    assert!(res.unwrap_err().starts_with("NO_ACTIVE_RECORDING"));

    // Simulate active hardware captures running without an active session
    {
        let mut reg = facade.services.audio.registry.lock().unwrap();
        let key = crate::integrations::audio::CaptureKey {
            kind: crate::integrations::audio::CaptureKind::Microphone,
            device_name: "default".to_string(),
        };
        let epoch = sona_application::live_transcription::LiveSourceEpoch::new("test", 1);
        reg.mark_stopping(key, epoch);
    }
    assert!(facade.services.audio.has_active_captures());

    // When facade.stop_recording is called, it cleanly falls back and returns Ok
    let res = facade
        .stop_recording(StopRecordingRequest { discard: false })
        .await
        .expect("stop_recording should succeed with fallback");
    assert_eq!(res.history_id, "");
    assert_eq!(res.duration_seconds, 0.0);
    assert_eq!(res.segment_count, 0);
}

#[tokio::test]
async fn test_facade_shares_recording_operation_lock_with_services() {
    let (facade, _dir) = create_test_facade().await;

    // Acquire lock via services.recording_session
    let guard = facade.services.recording_session.lock_operation().await;

    // Verify facade's recording_op_lock cannot be acquired concurrently
    let try_lock = facade.recording_op_lock.try_lock();
    assert!(
        try_lock.is_err(),
        "facade.recording_op_lock must share the same mutex with services.recording_session"
    );

    drop(guard);

    // After dropping guard, lock should be available
    let try_lock_after = facade.recording_op_lock.try_lock();
    assert!(
        try_lock_after.is_ok(),
        "facade.recording_op_lock must be free after guard is dropped"
    );
}
