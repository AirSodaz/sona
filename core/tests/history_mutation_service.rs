use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryMutationError, HistoryMutationRepository,
    HistoryReassignProjectRequest, HistoryUpdateItemMetaRequest,
    HistoryUpdateProjectAssignmentsRequest, HistoryUpdateTranscriptRequest,
};
use sona_core::history::mutation_service::HistoryMutationService;
use sona_core::history::{
    HistoryAudioStatus, HistoryCreateLiveDraftRequest, HistoryItemKind, HistoryItemRecord,
    HistoryItemStatus, HistorySaveImportedFileRequest, HistorySaveRecordingRequest,
    LiveRecordingDraftResult, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
};

#[derive(Default)]
struct RecordingHistoryMutationRepository {
    calls: Mutex<Vec<&'static str>>,
    forwarded_segments: Mutex<Vec<Value>>,
    forwarded_details: Mutex<Vec<String>>,
}

impl RecordingHistoryMutationRepository {
    fn record(&self, operation: &'static str) {
        self.calls.lock().unwrap().push(operation);
    }
}

impl HistoryMutationRepository for RecordingHistoryMutationRepository {
    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, HistoryMutationError> {
        self.record("create_live_draft");
        self.forwarded_details.lock().unwrap().push(format!(
            "draft:{:?}:{}:{:?}",
            request.id, request.audio_extension, request.project_id
        ));
        Ok(LiveRecordingDraftResult {
            item: history_item("draft-1"),
            audio_absolute_path: "history/draft-1.wav".to_string(),
        })
    }

    fn complete_live_draft(
        &self,
        request: HistoryCompleteLiveDraftRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.record("complete_live_draft");
        self.forwarded_segments
            .lock()
            .unwrap()
            .push(request.segments);
        self.forwarded_details.lock().unwrap().push(format!(
            "complete:{}:{}",
            request.history_id, request.duration
        ));
        Ok(history_item("draft-1"))
    }

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.record("save_recording");
        self.forwarded_segments
            .lock()
            .unwrap()
            .push(request.segments);
        self.forwarded_details.lock().unwrap().push(format!(
            "recording:{}:{:?}:{:?}",
            request.duration,
            request.audio_extension,
            request.audio_bytes.as_ref().map(Vec::len)
        ));
        Ok(history_item("recording-1"))
    }

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.record("save_imported_file");
        self.forwarded_segments
            .lock()
            .unwrap()
            .push(request.segments);
        self.forwarded_details.lock().unwrap().push(format!(
            "import:{:?}:{}:{}",
            request.id, request.source_path, request.duration
        ));
        Ok(history_item("import-1"))
    }

    fn delete_items(&self, request: HistoryDeleteItemsRequest) -> Result<(), HistoryMutationError> {
        self.record("delete_items");
        self.forwarded_details
            .lock()
            .unwrap()
            .push(format!("delete:{:?}", request.ids));
        Ok(())
    }

    fn update_transcript(
        &self,
        request: HistoryUpdateTranscriptRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError> {
        self.record("update_transcript");
        self.forwarded_segments
            .lock()
            .unwrap()
            .push(request.segments);
        self.forwarded_details
            .lock()
            .unwrap()
            .push(format!("update-transcript:{}", request.history_id));
        Ok(history_item("history-1"))
    }

    fn create_transcript_snapshot(
        &self,
        request: HistoryCreateTranscriptSnapshotRequest,
    ) -> Result<TranscriptSnapshotMetadata, HistoryMutationError> {
        self.record("create_transcript_snapshot");
        self.forwarded_segments
            .lock()
            .unwrap()
            .push(request.segments);
        self.forwarded_details.lock().unwrap().push(format!(
            "snapshot:{}:{:?}",
            request.history_id, request.reason
        ));
        Ok(TranscriptSnapshotMetadata {
            id: "snapshot-1".to_string(),
            history_id: "history-1".to_string(),
            reason: TranscriptSnapshotReason::Polish,
            created_at: 1,
            segment_count: 1,
        })
    }

    fn update_item_meta(
        &self,
        request: HistoryUpdateItemMetaRequest,
    ) -> Result<(), HistoryMutationError> {
        self.record("update_item_meta");
        self.forwarded_details
            .lock()
            .unwrap()
            .push(format!("meta:{}:{}", request.history_id, request.updates));
        Ok(())
    }

    fn update_project_assignments(
        &self,
        request: HistoryUpdateProjectAssignmentsRequest,
    ) -> Result<(), HistoryMutationError> {
        self.record("update_project_assignments");
        self.forwarded_details
            .lock()
            .unwrap()
            .push(format!("assign:{:?}:{:?}", request.ids, request.project_id));
        Ok(())
    }

    fn reassign_project(
        &self,
        request: HistoryReassignProjectRequest,
    ) -> Result<(), HistoryMutationError> {
        self.record("reassign_project");
        self.forwarded_details.lock().unwrap().push(format!(
            "reassign:{}:{:?}",
            request.current_project_id, request.next_project_id
        ));
        Ok(())
    }
}

fn history_item(id: &str) -> HistoryItemRecord {
    HistoryItemRecord {
        id: id.to_string(),
        timestamp: 1,
        duration: 1.0,
        audio_path: format!("{id}.wav"),
        audio_status: HistoryAudioStatus::Available,
        transcript_path: format!("{id}.json"),
        title: "History item".to_string(),
        preview_text: "hello".to_string(),
        icon: None,
        kind: HistoryItemKind::Recording,
        search_content: "hello".to_string(),
        project_id: None,
        status: HistoryItemStatus::Complete,
        draft_source: None,
    }
}

fn segments() -> Value {
    json!([{
        "id": "segment-1",
        "text": "hello",
        "start": 0.0,
        "end": 1.0,
        "isFinal": true
    }])
}

fn recording_request() -> HistorySaveRecordingRequest {
    HistorySaveRecordingRequest {
        segments: segments(),
        duration: 1.0,
        project_id: Some("project-1".to_string()),
        audio_bytes: Some(vec![1, 2, 3]),
        native_audio_path: None,
        audio_extension: Some("wav".to_string()),
    }
}

fn imported_file_request() -> HistorySaveImportedFileRequest {
    HistorySaveImportedFileRequest {
        id: Some("import-1".to_string()),
        source_path: "input.wav".to_string(),
        segments: segments(),
        duration: 1.0,
        project_id: Some("project-1".to_string()),
        converted_source_path: None,
    }
}

#[test]
fn service_routes_every_history_mutation_through_the_focused_port() {
    let repository = Arc::new(RecordingHistoryMutationRepository::default());
    let service = HistoryMutationService::new(repository.clone());

    service
        .create_live_draft(HistoryCreateLiveDraftRequest {
            id: Some("draft-1".to_string()),
            audio_extension: "wav".to_string(),
            project_id: Some("project-1".to_string()),
            icon: Some("audio".to_string()),
        })
        .unwrap();
    service
        .complete_live_draft(HistoryCompleteLiveDraftRequest {
            history_id: "draft-1".to_string(),
            segments: segments(),
            duration: 1.0,
        })
        .unwrap();
    service.save_recording(recording_request()).unwrap();
    service.save_imported_file(imported_file_request()).unwrap();
    service
        .delete_items(HistoryDeleteItemsRequest {
            ids: vec!["history-1".to_string()],
        })
        .unwrap();
    service
        .update_transcript(HistoryUpdateTranscriptRequest {
            history_id: "history-1".to_string(),
            segments: segments(),
        })
        .unwrap();
    service
        .create_transcript_snapshot(HistoryCreateTranscriptSnapshotRequest {
            history_id: "history-1".to_string(),
            reason: TranscriptSnapshotReason::Polish,
            segments: segments(),
        })
        .unwrap();
    service
        .update_item_meta(HistoryUpdateItemMetaRequest {
            history_id: "history-1".to_string(),
            updates: json!({"title": "Renamed", "icon": null}),
        })
        .unwrap();
    service
        .update_project_assignments(HistoryUpdateProjectAssignmentsRequest {
            ids: vec!["history-1".to_string()],
            project_id: Some("project-2".to_string()),
        })
        .unwrap();
    service
        .reassign_project(HistoryReassignProjectRequest {
            current_project_id: "project-2".to_string(),
            next_project_id: None,
        })
        .unwrap();

    assert_eq!(
        *repository.calls.lock().unwrap(),
        [
            "create_live_draft",
            "complete_live_draft",
            "save_recording",
            "save_imported_file",
            "delete_items",
            "update_transcript",
            "create_transcript_snapshot",
            "update_item_meta",
            "update_project_assignments",
            "reassign_project",
        ]
    );
    assert_eq!(repository.forwarded_segments.lock().unwrap().len(), 5);
    assert_eq!(
        *repository.forwarded_details.lock().unwrap(),
        [
            "draft:Some(\"draft-1\"):wav:Some(\"project-1\")",
            "complete:draft-1:1",
            "recording:1:Some(\"wav\"):Some(3)",
            "import:Some(\"import-1\"):input.wav:1",
            "delete:[\"history-1\"]",
            "update-transcript:history-1",
            "snapshot:history-1:Polish",
            "meta:history-1:{\"icon\":null,\"title\":\"Renamed\"}",
            "assign:[\"history-1\"]:Some(\"project-2\")",
            "reassign:project-2:None",
        ]
    );
}

#[test]
fn service_forwards_a_canonical_transcript_to_the_repository() {
    let repository = Arc::new(RecordingHistoryMutationRepository::default());
    let service = HistoryMutationService::new(repository.clone());

    service
        .update_transcript(HistoryUpdateTranscriptRequest {
            history_id: "history-1".to_string(),
            segments: json!([{"text": "hello"}]),
        })
        .unwrap();

    assert_eq!(
        *repository.forwarded_segments.lock().unwrap(),
        [json!([{
            "id": "segment-0",
            "text": "hello",
            "start": 0.0,
            "end": 0.0,
            "isFinal": true,
            "timing": {
                "level": "segment",
                "source": "derived",
                "units": [{"text": "hello", "start": 0.0, "end": 0.0}]
            }
        }])]
    );
}

#[test]
fn service_rejects_invalid_ids_projects_and_extensions_before_the_port() {
    let repository = Arc::new(RecordingHistoryMutationRepository::default());
    let service = HistoryMutationService::new(repository.clone());

    let errors = [
        service
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: Some(" ".to_string()),
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap_err(),
        service
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: Some("CON".to_string()),
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap_err(),
        service
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: Some("history:1".to_string()),
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap_err(),
        service
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: Some("history-1.".to_string()),
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap_err(),
        service
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: None,
                audio_extension: "../wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap_err(),
        service
            .delete_items(HistoryDeleteItemsRequest {
                ids: vec!["history-1".to_string(), "".to_string()],
            })
            .unwrap_err(),
        service
            .update_project_assignments(HistoryUpdateProjectAssignmentsRequest {
                ids: vec!["history-1".to_string()],
                project_id: Some("\t".to_string()),
            })
            .unwrap_err(),
        service
            .reassign_project(HistoryReassignProjectRequest {
                current_project_id: "".to_string(),
                next_project_id: None,
            })
            .unwrap_err(),
    ];

    assert!(
        errors
            .iter()
            .all(|error| matches!(error, HistoryMutationError::InvalidRequest(_)))
    );
    assert!(repository.calls.lock().unwrap().is_empty());
}

#[test]
fn project_ids_remain_opaque_while_history_ids_obey_file_name_limits() {
    let repository = Arc::new(RecordingHistoryMutationRepository::default());
    let service = HistoryMutationService::new(repository.clone());

    service
        .create_live_draft(HistoryCreateLiveDraftRequest {
            id: Some("draft-1".to_string()),
            audio_extension: "wav".to_string(),
            project_id: Some("team:alpha".to_string()),
            icon: None,
        })
        .unwrap();
    service
        .update_item_meta(HistoryUpdateItemMetaRequest {
            history_id: "draft-1".to_string(),
            updates: json!({"audioPath": "meeting..draft.wav"}),
        })
        .unwrap();

    assert!(matches!(
        service
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: Some("a".repeat(239)),
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap_err(),
        HistoryMutationError::InvalidRequest(_)
    ));
    assert_eq!(
        *repository.calls.lock().unwrap(),
        ["create_live_draft", "update_item_meta"]
    );
}

#[test]
fn service_rejects_invalid_durations_transcripts_and_recording_sources() {
    let repository = Arc::new(RecordingHistoryMutationRepository::default());
    let service = HistoryMutationService::new(repository.clone());

    let mut negative_duration = recording_request();
    negative_duration.duration = -1.0;
    let mut ambiguous_source = recording_request();
    ambiguous_source.native_audio_path = Some("recording.wav".to_string());
    let mut missing_source = recording_request();
    missing_source.audio_bytes = None;
    let mut empty_bytes = recording_request();
    empty_bytes.audio_bytes = Some(Vec::new());
    let mut empty_native_path = recording_request();
    empty_native_path.audio_bytes = None;
    empty_native_path.native_audio_path = Some("  ".to_string());
    let mut invalid_extension = recording_request();
    invalid_extension.audio_extension = Some("../wav".to_string());
    let mut invalid_import = imported_file_request();
    invalid_import.segments = json!({"text": "not an array"});
    let mut empty_import_source = imported_file_request();
    empty_import_source.source_path = " ".to_string();
    let mut empty_converted_source = imported_file_request();
    empty_converted_source.converted_source_path = Some("\t".to_string());
    let mut invalid_import_duration = imported_file_request();
    invalid_import_duration.duration = f64::INFINITY;
    let mut oversized_import_file_name = imported_file_request();
    oversized_import_file_name.id = Some("a".repeat(238));
    oversized_import_file_name.source_path = "input.abcdefghijklmnopq".to_string();

    let errors = [
        service.save_recording(negative_duration).unwrap_err(),
        service.save_recording(ambiguous_source).unwrap_err(),
        service.save_recording(missing_source).unwrap_err(),
        service.save_recording(empty_bytes).unwrap_err(),
        service.save_recording(empty_native_path).unwrap_err(),
        service.save_recording(invalid_extension).unwrap_err(),
        service.save_imported_file(invalid_import).unwrap_err(),
        service.save_imported_file(empty_import_source).unwrap_err(),
        service
            .save_imported_file(empty_converted_source)
            .unwrap_err(),
        service
            .save_imported_file(invalid_import_duration)
            .unwrap_err(),
        service
            .save_imported_file(oversized_import_file_name)
            .unwrap_err(),
        service
            .complete_live_draft(HistoryCompleteLiveDraftRequest {
                history_id: "history-1".to_string(),
                segments: segments(),
                duration: f64::NAN,
            })
            .unwrap_err(),
        service
            .update_transcript(HistoryUpdateTranscriptRequest {
                history_id: "history-1".to_string(),
                segments: json!(null),
            })
            .unwrap_err(),
        service
            .create_transcript_snapshot(HistoryCreateTranscriptSnapshotRequest {
                history_id: "history-1".to_string(),
                reason: TranscriptSnapshotReason::Polish,
                segments: json!({}),
            })
            .unwrap_err(),
    ];

    assert!(
        errors
            .iter()
            .all(|error| matches!(error, HistoryMutationError::InvalidRequest(_)))
    );
    assert!(repository.calls.lock().unwrap().is_empty());
}

#[test]
fn service_validates_metadata_shape_keys_and_value_types() {
    let repository = Arc::new(RecordingHistoryMutationRepository::default());
    let service = HistoryMutationService::new(repository.clone());

    for updates in [
        json!([]),
        json!({"unknown": true}),
        json!({"id": "renamed-history"}),
        json!({"title": 42}),
        json!({"projectId": ""}),
        json!({"duration": "one"}),
        json!({"audioPath": "../../outside.wav"}),
        json!({"transcriptPath": "C:\\outside.json"}),
        json!({"audioPath": "NUL"}),
    ] {
        assert!(matches!(
            service
                .update_item_meta(HistoryUpdateItemMetaRequest {
                    history_id: "history-1".to_string(),
                    updates,
                })
                .unwrap_err(),
            HistoryMutationError::InvalidRequest(_)
        ));
    }

    assert!(repository.calls.lock().unwrap().is_empty());
}

#[test]
fn empty_bulk_mutations_remain_no_ops_without_opening_the_repository() {
    let repository = Arc::new(RecordingHistoryMutationRepository::default());
    let service = HistoryMutationService::new(repository.clone());

    service
        .delete_items(HistoryDeleteItemsRequest { ids: Vec::new() })
        .unwrap();
    service
        .update_project_assignments(HistoryUpdateProjectAssignmentsRequest {
            ids: Vec::new(),
            project_id: Some("project-1".to_string()),
        })
        .unwrap();

    assert!(repository.calls.lock().unwrap().is_empty());
}
