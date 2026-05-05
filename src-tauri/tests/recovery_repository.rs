#![allow(dead_code)]

#[path = "../src/recovery/mod.rs"]
mod recovery;

#[path = "../src/storage.rs"]
mod storage;

use recovery::repository::RecoveryRepository;
use serde_json::{Value, json};
use std::fs::{self, File};
use std::path::Path;
use tempfile::tempdir;

fn recovery_file(root: &Path) -> std::path::PathBuf {
    root.join("recovery").join("queue-recovery.json")
}

fn read_recovery_json(root: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(recovery_file(root)).unwrap()).unwrap()
}

#[test]
fn load_snapshot_creates_compatible_queue_recovery_file() {
    let dir = tempdir().unwrap();
    let repository = RecoveryRepository::new(dir.path().to_path_buf());

    let snapshot = repository.load_snapshot().unwrap();

    assert_eq!(snapshot.version, 1);
    assert_eq!(snapshot.updated_at, None);
    assert!(snapshot.items.is_empty());
    assert_eq!(
        read_recovery_json(dir.path()),
        json!({
            "version": 1,
            "updatedAt": null,
            "items": []
        })
    );
}

#[test]
fn persist_queue_snapshot_filters_queue_items_and_normalizes_recovery_payloads() {
    let dir = tempdir().unwrap();
    let source_file = dir.path().join("pending.wav");
    File::create(&source_file).unwrap();
    let repository = RecoveryRepository::new(dir.path().to_path_buf());
    let queue_items = vec![
        json!({
            "id": "pending-1",
            "filename": "pending.wav",
            "filePath": source_file,
            "status": "pending",
            "progress": 48,
            "segments": [{
                "id": "segment-1",
                "text": "Hello",
                "start": -1.0,
                "end": 0.5,
                "isFinal": true
            }],
            "projectId": null,
            "origin": "automation",
            "automationRuleId": "rule-1",
            "automationRuleName": "Inbox",
            "sourceFingerprint": "fp-1",
            "lastKnownStage": "transcribing",
            "exportConfig": null,
            "stageConfig": null
        }),
        json!({
            "id": "processing-1",
            "recoveryId": "existing-recovery-id",
            "filename": "processing.wav",
            "filePath": dir.path().join("processing.wav"),
            "status": "processing",
            "progress": 12,
            "segments": [],
            "projectId": "project-1"
        }),
        json!({
            "id": "complete-1",
            "filename": "complete.wav",
            "filePath": dir.path().join("complete.wav"),
            "status": "complete",
            "progress": 100,
            "segments": [],
            "projectId": null
        }),
    ];

    let snapshot = repository.persist_queue_snapshot(queue_items).unwrap();

    assert_eq!(snapshot.version, 1);
    assert!(snapshot.updated_at.is_some());
    assert_eq!(snapshot.items.len(), 2);
    assert_eq!(snapshot.items[0].id, "pending-1");
    assert_eq!(snapshot.items[0].source, "automation");
    assert_eq!(snapshot.items[0].last_known_stage, "transcribing");
    assert_eq!(snapshot.items[0].has_source_file, true);
    assert_eq!(snapshot.items[0].can_resume, true);
    assert_eq!(snapshot.items[0].segments[0].start, 0.0);
    assert!(snapshot.items[0].segments[0].timing.is_some());
    assert_eq!(snapshot.items[1].id, "existing-recovery-id");
    assert_eq!(snapshot.items[1].source, "batch_import");
    assert_eq!(snapshot.items[1].has_source_file, false);
    assert_eq!(snapshot.items[1].can_resume, false);

    let persisted = read_recovery_json(dir.path());
    assert_eq!(persisted["version"], 1);
    assert_eq!(persisted["items"].as_array().unwrap().len(), 2);
}

#[test]
fn persist_queue_snapshot_merges_with_unresolved_recovery_items_from_previous_crashes() {
    let dir = tempdir().unwrap();
    let old_source = dir.path().join("old.wav");
    let same_source = dir.path().join("same.wav");
    let fresh_source = dir.path().join("fresh.wav");
    File::create(&old_source).unwrap();
    File::create(&same_source).unwrap();
    File::create(&fresh_source).unwrap();
    let repository = RecoveryRepository::new(dir.path().to_path_buf());

    repository
        .save_snapshot(vec![
            json!({
                "id": "old-recovery",
                "filename": "old.wav",
                "filePath": old_source,
                "source": "batch_import",
                "resolution": "pending",
                "progress": 30,
                "segments": [],
                "projectId": null,
                "lastKnownStage": "transcribing",
                "updatedAt": 10,
                "hasSourceFile": true,
                "canResume": true
            }),
            json!({
                "id": "same-recovery",
                "filename": "same-old.wav",
                "filePath": same_source,
                "source": "batch_import",
                "resolution": "pending",
                "progress": 40,
                "segments": [],
                "projectId": null,
                "lastKnownStage": "transcribing",
                "updatedAt": 11,
                "hasSourceFile": true,
                "canResume": true
            }),
        ])
        .unwrap();

    let snapshot = repository
        .persist_queue_snapshot(vec![
            json!({
                "id": "queue-same",
                "recoveryId": "same-recovery",
                "filename": "same-new.wav",
                "filePath": same_source,
                "status": "processing",
                "progress": 75,
                "segments": [],
                "projectId": null,
                "lastKnownStage": "exporting"
            }),
            json!({
                "id": "fresh-recovery",
                "filename": "fresh.wav",
                "filePath": fresh_source,
                "status": "pending",
                "progress": 5,
                "segments": [],
                "projectId": null,
                "lastKnownStage": "queued"
            }),
        ])
        .unwrap();

    assert_eq!(snapshot.items.len(), 3);
    assert!(snapshot.items.iter().any(|item| item.id == "old-recovery"));

    let same = snapshot
        .items
        .iter()
        .find(|item| item.id == "same-recovery")
        .unwrap();
    assert_eq!(same.filename, "same-new.wav");
    assert_eq!(same.progress, 75.0);
    assert_eq!(same.last_known_stage, "exporting");

    let fresh = snapshot
        .items
        .iter()
        .find(|item| item.id == "fresh-recovery")
        .unwrap();
    assert_eq!(fresh.filename, "fresh.wav");
}

#[test]
fn load_snapshot_recomputes_source_file_status_and_resume_guard() {
    let dir = tempdir().unwrap();
    let existing_file = dir.path().join("existing.wav");
    let missing_file = dir.path().join("missing.wav");
    let directory_path = dir.path().join("source-dir");
    File::create(&existing_file).unwrap();
    fs::create_dir_all(&directory_path).unwrap();
    fs::create_dir_all(dir.path().join("recovery")).unwrap();
    fs::write(
        recovery_file(dir.path()),
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "updatedAt": 100,
            "items": [
                {
                    "id": "existing",
                    "filename": "existing.wav",
                    "filePath": existing_file,
                    "source": "batch_import",
                    "resolution": "pending",
                    "progress": 30,
                    "segments": [],
                    "projectId": null,
                    "lastKnownStage": "transcribing",
                    "updatedAt": 100,
                    "hasSourceFile": false,
                    "canResume": false,
                    "exportConfig": null,
                    "stageConfig": null
                },
                {
                    "id": "missing",
                    "filename": "missing.wav",
                    "filePath": missing_file,
                    "source": "batch_import",
                    "resolution": "pending",
                    "progress": 30,
                    "segments": [],
                    "projectId": null,
                    "lastKnownStage": "transcribing",
                    "updatedAt": 100,
                    "hasSourceFile": true,
                    "canResume": true,
                    "exportConfig": null,
                    "stageConfig": null
                },
                {
                    "id": "directory",
                    "filename": "source-dir",
                    "filePath": directory_path,
                    "source": "batch_import",
                    "resolution": "pending",
                    "progress": 30,
                    "segments": [],
                    "projectId": null,
                    "lastKnownStage": "transcribing",
                    "updatedAt": 100,
                    "hasSourceFile": true,
                    "canResume": true,
                    "exportConfig": null,
                    "stageConfig": null
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let repository = RecoveryRepository::new(dir.path().to_path_buf());

    let snapshot = repository.load_snapshot().unwrap();

    assert_eq!(snapshot.items[0].has_source_file, true);
    assert_eq!(snapshot.items[0].can_resume, true);
    assert_eq!(snapshot.items[1].has_source_file, false);
    assert_eq!(snapshot.items[1].can_resume, false);
    assert_eq!(snapshot.items[2].has_source_file, false);
    assert_eq!(snapshot.items[2].can_resume, false);
}

#[test]
fn load_snapshot_keeps_valid_items_when_one_recovery_item_is_unreadable() {
    let dir = tempdir().unwrap();
    let first_file = dir.path().join("first.wav");
    let second_file = dir.path().join("second.wav");
    File::create(&first_file).unwrap();
    File::create(&second_file).unwrap();
    fs::create_dir_all(dir.path().join("recovery")).unwrap();
    fs::write(
        recovery_file(dir.path()),
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "updatedAt": 100,
            "items": [
                {
                    "id": "first",
                    "filename": "first.wav",
                    "filePath": first_file,
                    "source": "batch_import",
                    "resolution": "pending",
                    "progress": 30,
                    "segments": [],
                    "projectId": null,
                    "updatedAt": 100,
                    "hasSourceFile": true,
                    "canResume": true,
                    "exportConfig": null,
                    "stageConfig": null
                },
                "locally-corrupt-item",
                {
                    "id": "second",
                    "filename": "second.wav",
                    "filePath": second_file,
                    "source": "batch_import",
                    "resolution": "pending",
                    "progress": 40,
                    "segments": [],
                    "projectId": null,
                    "updatedAt": 101,
                    "hasSourceFile": true,
                    "canResume": true,
                    "exportConfig": null,
                    "stageConfig": null
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let repository = RecoveryRepository::new(dir.path().to_path_buf());

    let snapshot = repository.load_snapshot().unwrap();

    assert_eq!(snapshot.items.len(), 2);
    assert_eq!(snapshot.items[0].id, "first");
    assert_eq!(snapshot.items[1].id, "second");
    assert_eq!(snapshot.updated_at, Some(100));
}

#[test]
fn load_snapshot_keeps_valid_segments_when_one_segment_is_unreadable() {
    let dir = tempdir().unwrap();
    let source_file = dir.path().join("recording.wav");
    File::create(&source_file).unwrap();
    fs::create_dir_all(dir.path().join("recovery")).unwrap();
    fs::write(
        recovery_file(dir.path()),
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "updatedAt": 100,
            "items": [
                {
                    "id": "recording",
                    "filename": "recording.wav",
                    "filePath": source_file,
                    "source": "batch_import",
                    "resolution": "pending",
                    "progress": 30,
                    "segments": [
                        {
                            "id": "segment-1",
                            "text": "Hello",
                            "start": 0.0,
                            "end": 0.5,
                            "isFinal": true
                        },
                        "locally-corrupt-segment",
                        {
                            "id": "segment-2",
                            "text": "World",
                            "start": 0.5,
                            "end": 1.0,
                            "isFinal": true
                        }
                    ],
                    "projectId": null,
                    "updatedAt": 100,
                    "hasSourceFile": true,
                    "canResume": true,
                    "exportConfig": null,
                    "stageConfig": null
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let repository = RecoveryRepository::new(dir.path().to_path_buf());

    let snapshot = repository.load_snapshot().unwrap();

    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].segments.len(), 2);
    assert_eq!(snapshot.items[0].segments[0].id, "segment-1");
    assert_eq!(snapshot.items[0].segments[1].id, "segment-2");
}

#[test]
fn save_snapshot_filters_non_pending_items_and_defaults_legacy_fields() {
    let dir = tempdir().unwrap();
    let source_file = dir.path().join("automation.wav");
    File::create(&source_file).unwrap();
    let repository = RecoveryRepository::new(dir.path().to_path_buf());
    let items = vec![
        json!({
            "id": "automation-1",
            "filename": "automation.wav",
            "filePath": source_file,
            "resolution": "pending",
            "progress": 20,
            "segments": [],
            "projectId": null,
            "updatedAt": 5,
            "automationRuleId": "rule-1",
            "sourceFingerprint": "fp-1"
        }),
        json!({
            "id": "discarded-1",
            "filename": "discarded.wav",
            "filePath": dir.path().join("discarded.wav"),
            "source": "batch_import",
            "resolution": "discarded",
            "progress": 80,
            "segments": [],
            "projectId": null,
            "lastKnownStage": "queued",
            "updatedAt": 6,
            "hasSourceFile": true,
            "canResume": true
        }),
    ];

    let snapshot = repository.save_snapshot(items).unwrap();

    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].source, "automation");
    assert_eq!(snapshot.items[0].last_known_stage, "queued");
    assert_eq!(snapshot.items[0].has_source_file, true);
    assert_eq!(snapshot.items[0].can_resume, true);
    assert!(snapshot.items[0].updated_at >= 5);
}
