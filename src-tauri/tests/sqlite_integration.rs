#![allow(dead_code)]

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, Once};

use tauri_appsona_lib::core::database::Database;
use tauri_appsona_lib::core::database::legacy_migration::migrate_legacy_to_sqlite;
use tauri_appsona_lib::core::history_store::HistoryStore;
use tauri_appsona_lib::repositories::history::backup::{
    apply_prepared_history_import_inner, export_backup_archive_inner, prepare_backup_import_inner,
};
use tauri_appsona_lib::repositories::history::sqlite_store::SqliteHistoryStore;
use tauri_appsona_lib::repositories::history::{
    ExportBackupArchiveRequest, HistorySaveRecordingRequest, TranscriptSnapshotReason,
};

// Serializes tests that use set_global() (OnceLock can only be set once).
static INTEGRATION_TEST_LOCK: Mutex<()> = Mutex::new(());
static INIT_GLOBAL_DB: Once = Once::new();

fn init_global_db() {
    INIT_GLOBAL_DB.call_once(|| {
        let db = Database::open_in_memory().unwrap();
        Database::set_global(db).unwrap();
    });
}

fn clear_global_db() {
    Database::global()
        .with_transaction(|tx| {
            for table in &[
                "transcript_snapshots",
                "history_summaries",
                "history_transcripts",
                "history_items",
                "projects",
                "automation_rules",
                "automation_processed",
                "task_ledger",
            ] {
                tx.execute(&format!("DELETE FROM {table}"), []).ok();
            }
            tx.execute("DELETE FROM analytics.llm_usage", []).ok();
            Ok(())
        })
        .unwrap();
}

fn write_json(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn setup_legacy_data(dir: &Path) {
    write_json(
        &dir.join("history").join("index.json"),
        &json!([
            {
                "id": "hist-1",
                "timestamp": 1000,
                "duration": 5.0,
                "audioPath": "hist-1.wav",
                "transcriptPath": "hist-1.json",
                "title": "First Recording",
                "previewText": "Hello world...",
                "icon": "mic",
                "type": "recording",
                "searchContent": "Hello world this is a test",
                "projectId": "proj-1",
                "status": "complete"
            },
            {
                "id": "hist-2",
                "timestamp": 2000,
                "duration": 3.0,
                "audioPath": "hist-2.wav",
                "transcriptPath": "hist-2.json",
                "title": "Batch Import",
                "previewText": "Batch content...",
                "icon": null,
                "type": "batch",
                "searchContent": "batch imported data",
                "projectId": null,
                "status": "complete"
            }
        ]),
    );
    write_json(
        &dir.join("history").join("hist-1.json"),
        &json!([{"id": "seg-1", "text": "Hello", "start": 0.0, "end": 1.0, "isFinal": true}]),
    );
    write_json(
        &dir.join("history").join("hist-2.json"),
        &json!([{"id": "seg-2", "text": "Batch", "start": 0.0, "end": 2.0, "isFinal": true}]),
    );
    write_json(
        &dir.join("history").join("hist-1.summary.json"),
        &json!({"activeTemplateId": "general", "content": "Summary of first recording"}),
    );

    write_json(
        &dir.join("projects").join("index.json"),
        &json!([
            {
                "id": "proj-1",
                "name": "Work",
                "icon": "folder",
                "color": "#ff0000",
                "sortOrder": 0,
                "createdAt": 1000,
                "updatedAt": 2000,
                "defaults": {
                    "summaryTemplateId": "detailed",
                    "translationLanguage": "en",
                    "polishPresetId": "formal"
                }
            },
            {
                "id": "proj-2",
                "name": "Personal",
                "icon": "user",
                "color": "#00ff00",
                "sortOrder": 1,
                "createdAt": 3000,
                "updatedAt": 4000,
                "defaults": {}
            }
        ]),
    );

    write_json(
        &dir.join("automation").join("rules.json"),
        &json!([
            {"id": "rule-1", "name": "Watch Docs", "watchDirectory": "/docs", "projectId": "proj-1"},
            {"id": "rule-2", "name": "Watch Tmp", "watchDirectory": "/tmp", "projectId": "proj-2"}
        ]),
    );
    write_json(
        &dir.join("automation").join("processed.json"),
        &json!([
            {"id": "proc-1", "filePath": "/docs/file.txt", "processedAt": "2026-01-01"},
            {"id": "proc-2", "filePath": "/tmp/data.txt", "processedAt": "2026-01-02"}
        ]),
    );

    write_json(
        &dir.join("task-ledger").join("tasks.json"),
        &json!({
            "version": 1,
            "updatedAt": 5000,
            "tasks": [
                {"id": "task-1", "kind": "llmPolish", "status": "pending", "title": "Polish task", "progress": 0.0, "createdAt": 1000, "updatedAt": 1000, "retryable": false, "cancelable": true, "recoverable": false}
            ]
        }),
    );

    write_json(
        &dir.join("analytics").join("llm-usage.json"),
        &json!({
            "schemaVersion": 1,
            "startedAt": "2026-01-01T00:00:00Z",
            "byProvider": {
                "open_ai": {
                    "callCount": 5,
                    "callsWithUsage": 5,
                    "promptTokens": 1000,
                    "completionTokens": 500,
                    "totalTokens": 1500
                }
            },
            "byCategory": {},
            "daily": {}
        }),
    );
}

fn segment_value(id: &str, text: &str, start: f64, end: f64) -> Value {
    json!({
        "id": id,
        "text": text,
        "start": start,
        "end": end,
        "isFinal": true
    })
}

// =========================================================================
// Test 1: Legacy migration + store CRUD (uses global DB, serial via lock)
// =========================================================================

#[test]
fn test_migration_and_crud() {
    let _guard = INTEGRATION_TEST_LOCK.lock().unwrap();
    init_global_db();
    clear_global_db();

    let root = tempfile::TempDir::new().unwrap();
    setup_legacy_data(root.path());

    // Run legacy migration on the global DB
    let report = migrate_legacy_to_sqlite(Database::global(), root.path()).unwrap();
    assert!(report.migrated, "Migration should have found legacy data");
    assert_eq!(report.history_count, 2);
    assert_eq!(report.project_count, 2);
    assert!(
        report.errors.is_empty(),
        "Migration errors: {:?}",
        report.errors
    );

    // Verify all tables via global DB connection
    Database::global()
        .with_connection(|conn| {
            let h: i64 = conn
                .query_row("SELECT COUNT(*) FROM history_items", [], |r| r.get(0))
                .unwrap();
            assert_eq!(h, 2, "history_items count");
            let p: i64 = conn
                .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
                .unwrap();
            assert_eq!(p, 2, "projects count");
            let ar: i64 = conn
                .query_row("SELECT COUNT(*) FROM automation_rules", [], |r| r.get(0))
                .unwrap();
            assert_eq!(ar, 2, "automation_rules count");
            let ap: i64 = conn
                .query_row("SELECT COUNT(*) FROM automation_processed", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(ap, 2, "automation_processed count");
            let tl: i64 = conn
                .query_row("SELECT COUNT(*) FROM task_ledger", [], |r| r.get(0))
                .unwrap();
            assert_eq!(tl, 1, "task_ledger count");
            let llm: i64 = conn
                .query_row("SELECT COUNT(*) FROM analytics.llm_usage", [], |r| r.get(0))
                .unwrap();
            assert_eq!(llm, 1, "llm_usage count");
            Ok(())
        })
        .unwrap();

    // Test history store API (uses Database::global() implicitly)
    let store = SqliteHistoryStore::new(root.path().to_path_buf());
    store.ensure_ready().unwrap();

    let items = store.list_items().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Batch Import");
    assert_eq!(items[1].title, "First Recording");

    let transcript = store.load_transcript("hist-1").unwrap().unwrap();
    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].id, "seg-1");

    let summary = store.load_summary("hist-1").unwrap().unwrap();
    assert_eq!(summary["activeTemplateId"], "general");

    // Save a new recording
    let recording = store
        .save_recording(HistorySaveRecordingRequest {
            segments: json!([segment_value("seg-new", "New recording test", 0.0, 2.0)]),
            duration: 2.0,
            project_id: Some("proj-1".to_string()),
            audio_bytes: Some(vec![1, 2, 3]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        })
        .unwrap();
    assert_eq!(recording.preview_text, "New recording test...");
    assert_eq!(recording.project_id.as_deref(), Some("proj-1"));

    let items = store.list_items().unwrap();
    assert_eq!(items.len(), 3);

    // Update transcript
    let updated = store
        .update_transcript(
            &recording.id,
            json!([segment_value("seg-new", "Updated text", 0.0, 3.0)]),
        )
        .unwrap();
    assert_eq!(updated.preview_text, "Updated text...");

    // Save summary
    store
        .save_summary(&recording.id, json!({"activeTemplateId": "summary-1"}))
        .unwrap();
    let loaded_summary = store.load_summary(&recording.id).unwrap().unwrap();
    assert_eq!(loaded_summary["activeTemplateId"], "summary-1");

    // Create snapshot
    let snapshot = store
        .create_transcript_snapshot(
            &recording.id,
            TranscriptSnapshotReason::Polish,
            json!([segment_value("seg-new", "Snapshot text", 0.0, 1.0)]),
        )
        .unwrap();
    assert_eq!(snapshot.reason, TranscriptSnapshotReason::Polish);

    let snapshots = store.list_transcript_snapshots(&recording.id).unwrap();
    assert_eq!(snapshots.len(), 1);

    // Delete
    store.delete_items(std::slice::from_ref(&recording.id)).unwrap();
    let items = store.list_items().unwrap();
    assert_eq!(items.len(), 2);
}

// =========================================================================
// Test 2: Backup export/import roundtrip (uses global DB, serial via lock)
// =========================================================================

#[test]
fn test_backup_roundtrip() {
    let _guard = INTEGRATION_TEST_LOCK.lock().unwrap();
    init_global_db();
    clear_global_db();

    // --- Setup data ---
    let root = tempfile::TempDir::new().unwrap();
    let store = SqliteHistoryStore::new(root.path().to_path_buf());
    store.ensure_ready().unwrap();

    let item = store
        .save_recording(HistorySaveRecordingRequest {
            segments: json!([segment_value("seg-1", "Backup test", 0.0, 1.0)]),
            duration: 1.0,
            project_id: None,
            audio_bytes: Some(vec![1, 2, 3]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        })
        .unwrap();

    store
        .save_summary(&item.id, json!({"activeTemplateId": "general"}))
        .unwrap();

    let _snapshot = store
        .create_transcript_snapshot(
            &item.id,
            TranscriptSnapshotReason::Polish,
            json!([segment_value("seg-1", "Before backup", 0.0, 1.0)]),
        )
        .unwrap();

    // --- Export ---
    let archive_dir = tempfile::TempDir::new().unwrap();
    let archive_path = archive_dir.path().join("backup.tar.bz2");
    let manifest = export_backup_archive_inner(
        root.path(),
        ExportBackupArchiveRequest {
            archive_path: archive_path.to_string_lossy().into_owned(),
            app_version: "0.7.5".to_string(),
            config: json!({"theme": "auto"}),
            projects: vec![json!({"id": "project-1", "name": "Work", "defaults": {}})],
            automation_rules: vec![json!({"id": "rule-1", "name": "Watch"})],
            automation_processed_entries: vec![
                json!({"ruleId": "rule-1", "filePath": "/watch/file.wav"}),
            ],
            analytics_content: r#"{"schemaVersion":1,"startedAt":"2026-01-01T00:00:00Z"}"#
                .to_string(),
        },
    )
    .unwrap();

    assert_eq!(manifest.counts.history_items, 1);
    assert_eq!(manifest.counts.transcript_files, 1);
    assert_eq!(manifest.counts.summary_files, 1);

    // --- Clear DB and import ---
    clear_global_db();
    let restore_root = tempfile::TempDir::new().unwrap();

    let (prepared, backup_snapshot) = prepare_backup_import_inner(&archive_path).unwrap();
    apply_prepared_history_import_inner(
        restore_root.path(),
        &prepared.import_id,
        &backup_snapshot.extraction_dir,
    )
    .unwrap();

    // --- Verify imported data ---
    let restored_store = SqliteHistoryStore::new(restore_root.path().to_path_buf());
    let items = restored_store.list_items().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, item.id);
    assert_eq!(items[0].preview_text, "Backup test...");

    let transcript = restored_store.load_transcript(&item.id).unwrap().unwrap();
    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].text, "Backup test");

    let summary = restored_store.load_summary(&item.id).unwrap().unwrap();
    assert_eq!(summary["activeTemplateId"], "general");

    let snapshots = restored_store.list_transcript_snapshots(&item.id).unwrap();
    assert_eq!(snapshots.len(), 1);
}
