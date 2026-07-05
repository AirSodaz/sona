use serde_json::json;
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
        project_id: Some("project-1".to_string()),
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
