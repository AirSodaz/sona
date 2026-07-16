use serde_json::json;
use sona_core::recovery::repository::RecoverySnapshotStore;
use sona_core::recovery::types::{RecoveryItemInput, RecoverySnapshot, RecoverySnapshotInput};
use sona_recovery_fs::{FsRecoveryAdapter, FsRecoverySnapshotStore};
use std::fs::{self, File};
use tempfile::tempdir;

#[test]
fn first_load_creates_the_canonical_empty_snapshot_file() {
    let dir = tempdir().unwrap();
    let store = FsRecoverySnapshotStore::new(dir.path().to_path_buf());

    let input = store.load_snapshot_input().unwrap();
    let path = store.queue_recovery_path();

    assert_eq!(input, RecoverySnapshotInput::default());
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "{\n  \"version\": 1,\n  \"updatedAt\": null,\n  \"items\": []\n}"
    );
}

#[test]
fn saved_canonical_snapshot_round_trips_as_typed_input() {
    let dir = tempdir().unwrap();
    let store = FsRecoverySnapshotStore::new(dir.path().to_path_buf());
    let snapshot: RecoverySnapshot = serde_json::from_value(json!({
        "version": 1,
        "updatedAt": 42,
        "items": [{
            "id": "recovery-1",
            "filename": "recording.wav",
            "filePath": "C:/recording.wav",
            "source": "batch_import",
            "resolution": "pending",
            "progress": 25,
            "segments": [],
            "projectId": null,
            "lastKnownStage": "transcribing",
            "updatedAt": 42,
            "hasSourceFile": true,
            "canResume": true,
            "exportConfig": null,
            "stageConfig": null
        }]
    }))
    .unwrap();

    store.save_snapshot(&snapshot).unwrap();

    assert_eq!(
        store.load_snapshot_input().unwrap(),
        serde_json::from_value::<RecoverySnapshotInput>(serde_json::to_value(snapshot).unwrap())
            .unwrap()
    );
}

#[test]
fn malformed_stored_json_loads_as_empty_input() {
    let dir = tempdir().unwrap();
    let store = FsRecoverySnapshotStore::new(dir.path().to_path_buf());
    fs::create_dir_all(store.recovery_dir()).unwrap();
    fs::write(store.queue_recovery_path(), "{not-json").unwrap();

    assert_eq!(
        store.load_snapshot_input().unwrap(),
        RecoverySnapshotInput::default()
    );
}

#[test]
fn load_returns_an_error_when_recovery_directory_cannot_be_created() {
    let dir = tempdir().unwrap();
    let blocked = dir.path().join("blocked");
    File::create(&blocked).unwrap();
    let store = FsRecoverySnapshotStore::new(blocked);

    assert!(store.load_snapshot_input().is_err());
}

#[test]
fn recovery_adapter_composes_snapshot_store_source_paths_and_clock() {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("recording.wav");
    fs::write(&source_path, b"audio").unwrap();
    let adapter = FsRecoveryAdapter::new(dir.path().to_path_buf());

    let saved = adapter
        .save_snapshot_at(
            vec![
                serde_json::from_value::<RecoveryItemInput>(json!({
                    "id": "recovery-1",
                    "filename": "recording.wav",
                    "filePath": source_path,
                    "resolution": "pending",
                    "segments": []
                }))
                .unwrap(),
            ],
            42,
        )
        .unwrap();

    assert_eq!(saved.updated_at, Some(42));
    assert_eq!(saved.items[0].updated_at, 42);
    assert!(saved.items[0].has_source_file);
    assert!(saved.items[0].can_resume);

    let loaded = adapter.load_snapshot_at(43).unwrap();

    assert_eq!(loaded.updated_at, Some(42));
    assert_eq!(loaded.items[0].updated_at, 42);
}
