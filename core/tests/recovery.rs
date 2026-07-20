use serde_json::json;
use sona_core::recovery::normalization::{
    SourcePathStatus, SourcePathStatusProvider, recovered_item_from_queue_input_with_source_paths,
    snapshot_from_input_with_source_paths_at, snapshot_from_items_with_timestamp,
};
use sona_core::recovery::types::{
    RECOVERY_VERSION, RecoveredQueueItem, RecoveredTranscriptSegment, RecoveryFileStat,
    RecoveryItemInput, RecoveryItemStage, RecoveryResolution, RecoverySnapshot,
    RecoverySnapshotInput, RecoverySource,
};
use sona_core::transcription::transcript::TranscriptTimingLevel;

struct DirectorySourcePaths;

impl SourcePathStatusProvider for DirectorySourcePaths {
    fn status_for_path(&self, _path: &str) -> SourcePathStatus {
        SourcePathStatus::Directory
    }
}

#[test]
fn recovery_snapshot_transport_shape_lives_in_core() {
    let snapshot = RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: Some(3000),
        items: vec![RecoveredQueueItem {
            id: "recovery-1".to_string(),
            filename: "audio.wav".to_string(),
            file_path: "C:/audio.wav".to_string(),
            source: RecoverySource::BatchImport,
            resolution: RecoveryResolution::Pending,
            progress: 12.0,
            segments: vec![RecoveredTranscriptSegment {
                id: "seg-1".to_string(),
                text: "hello".to_string(),
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
            }],
            tag_ids: vec!["project-1".to_string()],
            history_id: Some("history-1".to_string()),
            history_title: Some("History".to_string()),
            last_known_stage: RecoveryItemStage::Transcribing,
            updated_at: 3000,
            has_source_file: true,
            can_resume: true,
            automation_rule_id: None,
            automation_rule_name: None,
            resolved_config_snapshot: Some(json!({"engine": "local"})),
            automation_resolution_snapshot: Some(json!({
                "tagRuleId": "tag-rule-1",
                "profileId": "profile-1",
                "profileSource": "tag"
            })),
            export_config: json!({"format": "txt"}),
            stage_config: json!({"chunk": 1}),
            source_fingerprint: Some("fingerprint".to_string()),
            file_stat: Some(RecoveryFileStat {
                size: 1024,
                mtime_ms: 2000,
            }),
            export_file_name_prefix: Some("meeting".to_string()),
        }],
    };

    let value = serde_json::to_value(snapshot).unwrap();

    assert_eq!(value["version"], 2);
    assert_eq!(value["items"][0]["filePath"], "C:/audio.wav");
    assert_eq!(value["items"][0]["hasSourceFile"], true);
    assert_eq!(value["items"][0]["fileStat"]["mtimeMs"], 2000);
    assert!(value["items"][0].get("file_path").is_none());
    assert!(value["items"][0]["fileStat"].get("mtime_ms").is_none());
}

#[test]
fn recovery_queue_normalization_uses_injected_source_path_status() {
    let input: RecoveryItemInput = serde_json::from_value(json!({
        "id": "queue-1",
        "status": "processing",
        "filename": "folder",
        "filePath": "C:/not-a-file",
        "segments": [{"text": "hello", "start": -1.0, "end": 0.5}]
    }))
    .unwrap();
    let item =
        recovered_item_from_queue_input_with_source_paths(input, 4000, &DirectorySourcePaths)
            .unwrap();

    assert_eq!(item.id, "queue-1");
    assert!(!item.has_source_file);
    assert!(!item.can_resume);
    assert_eq!(item.segments[0].start, 0.0);
    assert_eq!(
        item.segments[0].timing.as_ref().unwrap().level,
        TranscriptTimingLevel::Segment
    );
}

#[test]
fn recovery_snapshot_normalization_uses_supplied_timestamps() {
    let input: RecoverySnapshotInput = serde_json::from_value(json!({
        "version": RECOVERY_VERSION,
        "items": [{
            "id": "recovery-1",
            "filename": "audio.wav",
            "filePath": "C:/audio.wav",
            "resolution": "pending",
            "segments": []
        }]
    }))
    .unwrap();
    let loaded =
        snapshot_from_input_with_source_paths_at(input, false, &DirectorySourcePaths, 5000);

    assert_eq!(loaded.items[0].updated_at, 5000);

    let saved = snapshot_from_items_with_timestamp(loaded.items, 6000);

    assert_eq!(saved.updated_at, Some(6000));
}
