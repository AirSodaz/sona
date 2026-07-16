use serde_json::json;
use sona_core::history::item_factory::{
    HistoryItemGeneratedValues, create_imported_file_item, create_live_draft_item,
    create_recording_item,
};
use sona_core::history::{
    HistoryAudioStatus, HistoryItemKind, HistoryItemRecord, HistoryItemStatus,
    TranscriptSnapshotMetadata, TranscriptSnapshotReason, TranscriptSnapshotRecord,
};

#[test]
fn history_item_transport_shape_lives_in_core() {
    let item = HistoryItemRecord {
        id: "history-1".to_string(),
        timestamp: 42,
        duration: 12.5,
        audio_path: "history/history-1.wav".to_string(),
        audio_status: HistoryAudioStatus::Available,
        transcript_path: "history/history-1.json".to_string(),
        title: "Meeting".to_string(),
        preview_text: "hello".to_string(),
        icon: Some("Mic".to_string()),
        kind: HistoryItemKind::Recording,
        search_content: "hello".to_string(),
        tag_ids: vec!["tag-1".to_string()],
        deleted_at: None,
        status: HistoryItemStatus::Complete,
        draft_source: None,
    };

    let value = serde_json::to_value(item).unwrap();

    assert_eq!(value["type"], "recording");
    assert_eq!(value["audioStatus"], "available");
    assert_eq!(value["previewText"], "hello");
    assert!(value.get("audio_status").is_none());
}

#[test]
fn transcript_snapshot_record_uses_core_transcript_segments() {
    let record: TranscriptSnapshotRecord = serde_json::from_value(json!({
        "metadata": {
            "id": "snapshot-1",
            "historyId": "history-1",
            "reason": "polish",
            "createdAt": 123,
            "segmentCount": 1
        },
        "segments": [{
            "id": "seg-1",
            "text": "hello",
            "start": 0.0,
            "end": 1.0,
            "isFinal": true
        }]
    }))
    .unwrap();

    assert_eq!(
        record.metadata,
        TranscriptSnapshotMetadata {
            id: "snapshot-1".to_string(),
            history_id: "history-1".to_string(),
            reason: TranscriptSnapshotReason::Polish,
            created_at: 123,
            segment_count: 1,
        }
    );
    assert_eq!(record.segments[0].text, "hello");
}

#[test]
fn history_item_factory_uses_supplied_id_and_timestamp_for_recordings() {
    let item = create_recording_item(
        HistoryItemGeneratedValues {
            fallback_id: "recording-1".to_string(),
            timestamp: 1_700_000_000_000,
            recording_title: "Recording From Adapter".to_string(),
        },
        12.5,
        vec!["tag-1".to_string()],
        Some(".WEBM!"),
        None,
    )
    .unwrap();

    assert_eq!(item.id, "recording-1");
    assert_eq!(item.timestamp, 1_700_000_000_000);
    assert_eq!(item.audio_path, "recording-1.webm");
    assert_eq!(item.transcript_path, "recording-1.json");
    assert_eq!(item.title, "Recording From Adapter");
    assert_eq!(item.tag_ids, vec!["tag-1"]);
    assert_eq!(item.kind, HistoryItemKind::Recording);
}

#[test]
fn history_item_factory_prefers_request_ids_over_generated_fallback_ids() {
    let draft = create_live_draft_item(
        sona_core::history::HistoryCreateLiveDraftRequest {
            id: Some("draft-request-id".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: Some("Mic".to_string()),
        },
        HistoryItemGeneratedValues {
            fallback_id: "unused-fallback-id".to_string(),
            timestamp: 42,
            recording_title: "Draft Title From Adapter".to_string(),
        },
    )
    .unwrap();

    assert_eq!(draft.id, "draft-request-id");
    assert_eq!(draft.timestamp, 42);
    assert_eq!(draft.title, "Draft Title From Adapter");
    assert_eq!(draft.audio_path, "draft-request-id.wav");
    assert_eq!(draft.status, HistoryItemStatus::Draft);
    assert_eq!(draft.icon.as_deref(), Some("Mic"));

    let imported = create_imported_file_item(
        Some("import-request-id".to_string()),
        "C:/audio/Meeting.MP3".to_string(),
        None,
        7.0,
        Vec::new(),
        HistoryItemGeneratedValues {
            fallback_id: "unused-import-fallback".to_string(),
            timestamp: 43,
            recording_title: "Unused Imported Recording Title".to_string(),
        },
    )
    .unwrap();

    assert_eq!(imported.item.id, "import-request-id");
    assert_eq!(imported.item.timestamp, 43);
    assert_eq!(imported.item.audio_path, "import-request-id.mp3");
    assert_eq!(imported.copy_source_path, "C:/audio/Meeting.MP3");
}
