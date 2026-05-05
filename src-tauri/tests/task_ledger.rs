#![allow(dead_code)]

#[path = "../src/task_ledger/mod.rs"]
mod task_ledger;

#[path = "../src/storage.rs"]
mod storage;

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use task_ledger::repository::TaskLedgerRepository;
use task_ledger::types::{TaskLedgerKind, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus};
use tempfile::tempdir;

fn ledger_file(root: &Path) -> std::path::PathBuf {
    root.join("task-ledger").join("tasks.json")
}

fn read_ledger_json(root: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(ledger_file(root)).unwrap()).unwrap()
}

fn base_record(id: &str, status: TaskLedgerStatus) -> TaskLedgerRecord {
    TaskLedgerRecord {
        id: id.to_string(),
        kind: TaskLedgerKind::BatchImport,
        status,
        title: format!("Task {id}"),
        progress: 12.0,
        created_at: 100,
        updated_at: 100,
        retryable: true,
        cancelable: true,
        recoverable: false,
        stage: Some("transcribing".to_string()),
        history_id: None,
        project_id: None,
        file_path: Some(format!("C:\\audio\\{id}.wav")),
        automation_rule_id: None,
        source_fingerprint: None,
        error_message: None,
        template_id: None,
        target_language: None,
    }
}

#[test]
fn load_snapshot_creates_empty_task_ledger_file() {
    let dir = tempdir().unwrap();
    let repository = TaskLedgerRepository::new(dir.path().to_path_buf());

    let snapshot = repository.load_snapshot().unwrap();

    assert_eq!(snapshot.version, 1);
    assert_eq!(snapshot.updated_at, None);
    assert!(snapshot.tasks.is_empty());
    assert_eq!(
        read_ledger_json(dir.path()),
        json!({
            "version": 1,
            "updatedAt": null,
            "tasks": []
        })
    );
}

#[test]
fn upsert_persists_only_actionable_task_statuses() {
    let dir = tempdir().unwrap();
    let repository = TaskLedgerRepository::new(dir.path().to_path_buf());

    repository
        .upsert_task(base_record("running", TaskLedgerStatus::Running))
        .unwrap();
    repository
        .upsert_task(base_record("failed", TaskLedgerStatus::Failed))
        .unwrap();
    repository
        .upsert_task(base_record("succeeded", TaskLedgerStatus::Succeeded))
        .unwrap();
    repository
        .upsert_task(base_record("cancelled", TaskLedgerStatus::Cancelled))
        .unwrap();

    let persisted = read_ledger_json(dir.path());
    let tasks = persisted["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0]["id"], "running");
    assert_eq!(tasks[1]["id"], "failed");
}

#[test]
fn load_snapshot_replaces_malformed_task_ledger_with_empty_snapshot() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("task-ledger")).unwrap();
    fs::write(ledger_file(dir.path()), "{ not json").unwrap();
    let repository = TaskLedgerRepository::new(dir.path().to_path_buf());

    let snapshot = repository.load_snapshot().unwrap();

    assert!(snapshot.tasks.is_empty());
    assert_eq!(
        read_ledger_json(dir.path()),
        json!({
            "version": 1,
            "updatedAt": null,
            "tasks": []
        })
    );
}

#[test]
fn load_snapshot_marks_incomplete_running_tasks_as_interrupted() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("task-ledger")).unwrap();
    fs::write(
        ledger_file(dir.path()),
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "updatedAt": 100,
            "tasks": [
                {
                    "id": "running-task",
                    "kind": "llmSummary",
                    "status": "running",
                    "title": "Summary",
                    "progress": 45,
                    "createdAt": 50,
                    "updatedAt": 100,
                    "retryable": true,
                    "cancelable": true,
                    "recoverable": false,
                    "historyId": "history-1",
                    "templateId": "meeting"
                },
                {
                    "id": "cancel-requested-task",
                    "kind": "automation",
                    "status": "cancelRequested",
                    "title": "Automation",
                    "progress": 60,
                    "createdAt": 60,
                    "updatedAt": 101,
                    "retryable": true,
                    "cancelable": false,
                    "recoverable": false,
                    "automationRuleId": "rule-1",
                    "sourceFingerprint": "fp-1"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let repository = TaskLedgerRepository::new(dir.path().to_path_buf());

    let snapshot: TaskLedgerSnapshot = repository.load_snapshot().unwrap();

    assert_eq!(snapshot.tasks.len(), 2);
    assert_eq!(snapshot.tasks[0].status, TaskLedgerStatus::Interrupted);
    assert_eq!(snapshot.tasks[0].cancelable, false);
    assert_eq!(
        snapshot.tasks[0].error_message.as_deref(),
        Some("Task was interrupted before it finished.")
    );
    assert_eq!(snapshot.tasks[1].status, TaskLedgerStatus::Interrupted);
    assert_eq!(snapshot.tasks[1].cancelable, false);
}

#[test]
fn patch_remove_and_clear_resolved_update_the_persisted_snapshot() {
    let dir = tempdir().unwrap();
    let repository = TaskLedgerRepository::new(dir.path().to_path_buf());
    repository
        .upsert_task(base_record("failed", TaskLedgerStatus::Failed))
        .unwrap();
    repository
        .upsert_task(base_record("recoverable", TaskLedgerStatus::Recoverable))
        .unwrap();

    let after_patch = repository
        .patch_task(
            "failed",
            json!({
                "status": "running",
                "progress": 25,
                "errorMessage": null
            }),
        )
        .unwrap();
    assert_eq!(after_patch.tasks[0].status, TaskLedgerStatus::Running);
    assert_eq!(after_patch.tasks[0].progress, 25.0);
    assert_eq!(after_patch.tasks[0].error_message, None);

    repository.remove_task("recoverable").unwrap();
    let after_remove = repository.load_snapshot().unwrap();
    assert_eq!(after_remove.tasks.len(), 1);
    assert_eq!(after_remove.tasks[0].id, "failed");

    repository
        .patch_task("failed", json!({ "status": "succeeded", "progress": 100 }))
        .unwrap();
    let after_success = repository.load_snapshot().unwrap();
    assert!(after_success.tasks.is_empty());
}
