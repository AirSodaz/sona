use serde_json::json;
use sona_core::recovery::normalization::{
    SourcePathStatus, SourcePathStatusProvider, recovered_item_from_queue_input_with_source_paths,
    snapshot_from_input_with_source_paths_at, snapshot_from_items_with_timestamp,
};
use sona_core::recovery::types::{RECOVERY_VERSION, RecoveryItemInput, RecoverySnapshotInput};
use sona_core::transcription::transcript::TranscriptTimingLevel;

struct DirectorySourcePaths;

impl SourcePathStatusProvider for DirectorySourcePaths {
    fn status_for_path(&self, _path: &str) -> SourcePathStatus {
        SourcePathStatus::Directory
    }
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
