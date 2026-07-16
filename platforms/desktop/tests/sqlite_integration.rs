use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use sona_core::history::HistorySummaryPayload;
use sona_core::history::mutation_repository::{
    HistoryCreateTranscriptSnapshotRequest, HistoryDeleteItemsRequest, HistoryMutationRepository,
    HistoryUpdateTranscriptRequest,
};
use sona_core::history::query_repository::HistoryQueryRepository;
use sona_core::history_store::HistoryStore;
use sona_sqlite::Database;
use sona_sqlite::legacy_migration::migrate_legacy_to_sqlite;
use tauri_appsona_lib::platform::history_repository::sqlite_store::SqliteHistoryStore;
use tauri_appsona_lib::platform::history_repository::{
    HistorySaveRecordingRequest, TranscriptSnapshotReason,
};

fn test_db() -> Arc<Database> {
    Arc::new(Database::open_in_memory().unwrap())
}

fn clear_db(db: &Database) {
    db.with_transaction(|tx| {
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
                "description": "Work project",
                "icon": "folder",
                "color": "#ff0000",
                "sortOrder": 0,
                "createdAt": 1000,
                "updatedAt": 2000,
                "defaults": {
                    "summaryTemplateId": "detailed",
                    "translationLanguage": "en",
                    "polishPresetId": "formal",
                    "polishScenario": "meeting",
                    "polishContext": "weekly sync",
                    "exportFileNamePrefix": "work-",
                    "enabledTextReplacementSetIds": ["tr-1"],
                    "enabledHotwordSetIds": ["hw-1"],
                    "enabledPolishKeywordSetIds": ["kw-1"],
                    "enabledSpeakerProfileIds": ["sp-1"]
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
            {
                "id": "rule-1",
                "name": "Watch Docs",
                "watchDirectory": "/docs",
                "projectId": "proj-1",
                "presetId": "preset-1",
                "recursive": true,
                "enabled": true,
                "stageConfig": {
                    "autoPolish": true,
                    "polishPresetId": "formal",
                    "autoTranslate": true,
                    "translationLanguage": "ja",
                    "exportEnabled": true
                },
                "exportConfig": {
                    "directory": "/exports",
                    "format": "md",
                    "mode": "translated",
                    "prefix": "auto-"
                },
                "createdAt": 1000,
                "updatedAt": 2000
            },
            {"id": "rule-2", "name": "Watch Tmp", "watchDirectory": "/tmp", "projectId": "proj-2"}
        ]),
    );
    write_json(
        &dir.join("automation").join("processed.json"),
        &json!([
            {"id": "proc-1", "ruleId": "rule-1", "filePath": "/docs/file.txt", "sourceFingerprint": "fp-1", "size": 123, "mtimeMs": 456, "status": "complete", "processedAt": 789, "historyId": "hist-1", "exportPath": "/exports/file.md"},
            {"id": "proc-2", "filePath": "/tmp/data.txt", "processedAt": "2026-01-02"}
        ]),
    );

    write_json(
        &dir.join("task-ledger").join("tasks.json"),
        &json!({
            "version": 1,
            "updatedAt": 5000,
            "tasks": [
                {"id": "task-1", "kind": "llmPolish", "status": "pending", "title": "Polish task", "progress": 25.0, "createdAt": 1000, "updatedAt": 1000, "retryable": true, "cancelable": true, "recoverable": false, "stage": "polish", "historyId": "hist-1", "projectId": "proj-1", "filePath": "/docs/file.txt", "automationRuleId": "rule-1", "sourceFingerprint": "fp-1", "templateId": "tmpl-1", "targetLanguage": "ja"}
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
// Test 1: Legacy migration + store CRUD
// =========================================================================

#[test]
fn test_migration_and_crud() {
    let db = test_db();
    clear_db(db.as_ref());

    let root = tempfile::TempDir::new().unwrap();
    setup_legacy_data(root.path());

    let report = migrate_legacy_to_sqlite(db.as_ref(), root.path()).unwrap();
    assert!(report.migrated, "Migration should have found legacy data");
    assert_eq!(report.history_count, 2);
    assert_eq!(report.project_count, 2);
    assert!(
        report.errors.is_empty(),
        "Migration errors: {:?}",
        report.errors
    );

    db.with_connection(|conn| {
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

        let project = conn
            .query_row(
                "SELECT description, summary_template_id, translation_language,
                            polish_preset_id, polish_scenario, polish_context,
                            export_file_name_prefix
                     FROM projects WHERE id = 'proj-1'",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(project.0, "Work project");
        assert_eq!(project.1, "detailed");
        assert_eq!(project.2, "en");
        assert_eq!(project.3, "formal");
        assert_eq!(project.4.as_deref(), Some("meeting"));
        assert_eq!(project.5.as_deref(), Some("weekly sync"));
        assert_eq!(project.6, "work-");

        let link_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_default_links WHERE project_id = 'proj-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(link_count, 4, "project_default_links count");

        let rule = conn
            .query_row(
                "SELECT project_id, preset_id, recursive, enabled, stage_auto_polish,
                            stage_polish_preset_id, stage_auto_translate,
                            stage_translation_language, stage_export_enabled,
                            export_directory, export_format, export_mode, export_prefix,
                            created_at, updated_at
                     FROM automation_rules WHERE id = 'rule-1'",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, String>(7)?,
                        r.get::<_, i64>(8)?,
                        r.get::<_, String>(9)?,
                        r.get::<_, String>(10)?,
                        r.get::<_, String>(11)?,
                        r.get::<_, String>(12)?,
                        r.get::<_, i64>(13)?,
                        r.get::<_, i64>(14)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(rule.0, "proj-1");
        assert_eq!(rule.1, "preset-1");
        assert_eq!(rule.2, 1);
        assert_eq!(rule.3, 1);
        assert_eq!(rule.4, 1);
        assert_eq!(rule.5, "formal");
        assert_eq!(rule.6, 1);
        assert_eq!(rule.7, "ja");
        assert_eq!(rule.8, 1);
        assert_eq!(rule.9, "/exports");
        assert_eq!(rule.10, "md");
        assert_eq!(rule.11, "translated");
        assert_eq!(rule.12, "auto-");
        assert_eq!(rule.13, 1000);
        assert_eq!(rule.14, 2000);

        let processed = conn
            .query_row(
                "SELECT rule_id, file_path, source_fingerprint, size, mtime_ms,
                            processed_at, history_id, export_path
                     FROM automation_processed WHERE id = 'proc-1'",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(processed.0, "rule-1");
        assert_eq!(processed.1, "/docs/file.txt");
        assert_eq!(processed.2, "fp-1");
        assert_eq!(processed.3, 123);
        assert_eq!(processed.4, 456);
        assert_eq!(processed.5, 789);
        assert_eq!(processed.6.as_deref(), Some("hist-1"));
        assert_eq!(processed.7.as_deref(), Some("/exports/file.md"));

        let task = conn
            .query_row(
                "SELECT kind, status, title, progress, retryable, cancelable, stage,
                            history_id, project_id, automation_rule_id, source_fingerprint,
                            template_id, target_language
                     FROM task_ledger WHERE id = 'task-1'",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, f64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, Option<String>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, Option<String>>(11)?,
                        r.get::<_, Option<String>>(12)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(task.0, "llmPolish");
        assert_eq!(task.1, "pending");
        assert_eq!(task.2, "Polish task");
        assert_eq!(task.3, 25.0);
        assert_eq!(task.4, 1);
        assert_eq!(task.5, 1);
        assert_eq!(task.6.as_deref(), Some("polish"));
        assert_eq!(task.7.as_deref(), Some("hist-1"));
        assert_eq!(task.8.as_deref(), Some("proj-1"));
        assert_eq!(task.9.as_deref(), Some("rule-1"));
        assert_eq!(task.10.as_deref(), Some("fp-1"));
        assert_eq!(task.11.as_deref(), Some("tmpl-1"));
        assert_eq!(task.12.as_deref(), Some("ja"));
        Ok(())
    })
    .unwrap();

    let store = SqliteHistoryStore::new(root.path().to_path_buf(), Arc::clone(&db));
    store.ensure_ready().unwrap();

    let items = store.list_items().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Batch Import");
    assert_eq!(items[1].title, "First Recording");

    let transcript = store.load_transcript("hist-1").unwrap().unwrap();
    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].id, "seg-1");

    let summary = store.load_summary("hist-1").unwrap().unwrap();
    assert_eq!(summary.active_template_id, "general");

    // Save a new recording
    let recording = store
        .save_recording(HistorySaveRecordingRequest {
            segments: serde_json::from_value(json!([segment_value(
                "seg-new",
                "New recording test",
                0.0,
                2.0
            )]))
            .unwrap(),
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
        .update_transcript(HistoryUpdateTranscriptRequest {
            history_id: recording.id.clone(),
            segments: serde_json::from_value(json!([segment_value(
                "seg-new",
                "Updated text",
                0.0,
                3.0
            )]))
            .unwrap(),
        })
        .unwrap();
    assert_eq!(updated.preview_text, "Updated text...");

    // Save summary
    store
        .save_summary(
            &recording.id,
            HistorySummaryPayload {
                active_template_id: "summary-1".to_string(),
                record: None,
            },
        )
        .unwrap();
    let loaded_summary = store.load_summary(&recording.id).unwrap().unwrap();
    assert_eq!(loaded_summary.active_template_id, "summary-1");

    // Create snapshot
    let snapshot = store
        .create_transcript_snapshot(HistoryCreateTranscriptSnapshotRequest {
            history_id: recording.id.clone(),
            reason: TranscriptSnapshotReason::Polish,
            segments: serde_json::from_value(json!([segment_value(
                "seg-new",
                "Snapshot text",
                0.0,
                1.0
            )]))
            .unwrap(),
        })
        .unwrap();
    assert_eq!(snapshot.reason, TranscriptSnapshotReason::Polish);

    let snapshots = store.list_transcript_snapshots(&recording.id).unwrap();
    assert_eq!(snapshots.len(), 1);

    // Delete
    store
        .delete_items(HistoryDeleteItemsRequest {
            ids: vec![recording.id.clone()],
        })
        .unwrap();
    let items = store.list_items().unwrap();
    assert_eq!(items.len(), 2);
}
