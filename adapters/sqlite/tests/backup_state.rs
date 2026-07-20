use std::fs::OpenOptions;
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use fs3::FileExt;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde_json::{Value, json};
use sona_core::automation::repository::{
    AutomationProcessedRecord, AutomationProfileRecord, AutomationRepositoryState,
    AutomationRuleInputActions, AutomationRuleRecord, AutomationRuleRecordExportConfig,
    AutomationRuleRecordStageConfig, AutomationStore,
};
use sona_core::backup::{
    BackupDataset, BackupError, BackupRestoreDataset, BackupStateRepository, build_backup_manifest,
};
use sona_core::config::service::app_config_stored_state_from_value;
use sona_core::config::{AppConfigStore, AppConfigStoredState};
use sona_core::history::{HistoryBackupSnapshot, HistoryItemStatus};
use sona_core::tag::{TagRecord, TagStore};
use sona_sqlite::ports::Database as DatabasePort;
use sona_sqlite::{
    Database, DatabaseError, LazySqliteBackupStateRepository, SqliteAutomationRepository,
    SqliteBackupStateRepository, SqliteConfigStore, SqliteTagRepository, llm_usage,
    validate_backup_restore_dataset,
};
use tempfile::TempDir;

const ACTIVE_TAG_KEY: &str = "sona-active-tag-id";

struct Fixture {
    root: TempDir,
    db: Arc<Database>,
    repository: Arc<SqliteBackupStateRepository>,
}

struct CoordinatedDatabase {
    inner: Arc<Database>,
    transaction_started: Mutex<Option<mpsc::Sender<()>>>,
    continue_transaction: Mutex<mpsc::Receiver<()>>,
}

impl DatabasePort for CoordinatedDatabase {
    fn with_connection<F, T>(&self, operation: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        self.inner.with_connection(operation)
    }

    fn with_read_connection<F, T>(&self, operation: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        self.inner.with_read_connection(operation)
    }

    fn with_write_connection<F, T>(&self, operation: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        self.inner.with_write_connection(operation)
    }

    fn with_transaction<F, T>(&self, operation: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, DatabaseError>,
    {
        self.inner.with_transaction(operation)
    }

    fn with_rw_transaction<F, T>(&self, operation: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, DatabaseError>,
    {
        self.inner.with_rw_transaction(|transaction| {
            if let Some(started) = self.transaction_started.lock().unwrap().take() {
                started.send(()).unwrap();
                self.continue_transaction.lock().unwrap().recv().unwrap();
            }
            operation(transaction)
        })
    }

    fn run_optimize(&self) -> Result<(), DatabaseError> {
        self.inner.run_optimize()
    }

    fn vacuum(&self) -> Result<(), DatabaseError> {
        self.inner.vacuum()
    }

    fn is_for_app_local_data_dir(&self, app_local_data_dir: &Path) -> bool {
        self.inner.is_for_app_local_data_dir(app_local_data_dir)
    }
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(root.path()).unwrap());
        let repository = Arc::new(SqliteBackupStateRepository::new(
            root.path().to_path_buf(),
            Arc::clone(&db),
        ));
        Self {
            root,
            db,
            repository,
        }
    }

    fn seed(&self, label: &str) -> Value {
        let config = config_value(label);
        let config_store = SqliteConfigStore::new(Arc::clone(&self.db));
        config_store
            .replace_state(stored_config(&config, timestamp(label)))
            .unwrap();

        self.db
            .with_rw_transaction(|tx| {
                tx.execute("DELETE FROM transcript_snapshots", [])?;
                tx.execute("DELETE FROM history_summaries", [])?;
                tx.execute("DELETE FROM history_transcripts", [])?;
                tx.execute("DELETE FROM history_items", [])?;
                Ok(())
            })
            .unwrap();

        let tags = tags(label);
        let tag_store = SqliteTagRepository::new(Arc::clone(&self.db));
        tag_store.replace_tags(tags).unwrap();
        seed_history(self.db.as_ref(), label);

        let automation_store = SqliteAutomationRepository::new(Arc::clone(&self.db));
        automation_store
            .replace_state(&automation_state(label))
            .unwrap();
        llm_usage::replace_raw(self.db.as_ref(), &analytics_content(label)).unwrap();
        config_store
            .set_setting_json(
                ACTIVE_TAG_KEY,
                serde_json::to_string(&format!("{label}-tag-b")).unwrap(),
            )
            .unwrap();
        config
    }
}

fn timestamp(label: &str) -> i64 {
    match label {
        "before" => 100,
        "after" => 200,
        "replacement" => 300,
        _ => 400,
    }
}

fn config_value(label: &str) -> Value {
    json!({
        "label": label,
        "configVersion": 7,
        "httpServerEnabled": true,
        "httpServerHost": "127.0.0.1",
        "summaryCustomTemplates": [
            {"id": format!("{label}-summary-b"), "name": "Summary B", "instructions": "B"},
            {"id": format!("{label}-summary-a"), "name": "Summary A", "instructions": "A"}
        ],
        "polishCustomPresets": [
            {"id": format!("{label}-polish"), "name": "Polish", "context": label}
        ],
        "textReplacementSets": [{
            "id": format!("{label}-replace"),
            "name": "Replace",
            "enabled": true,
            "ignoreCase": true,
            "rules": [{"id": format!("{label}-replace-rule"), "from": "a", "to": "b"}]
        }],
        "hotwordSets": [{
            "id": format!("{label}-hotword"),
            "name": "Hotword",
            "enabled": true,
            "rules": [{"id": format!("{label}-hotword-rule"), "text": "sona"}]
        }],
        "polishKeywordSets": [{
            "id": format!("{label}-keyword"),
            "name": "Keyword",
            "enabled": true,
            "keywords": "alpha,beta"
        }],
        "speakerProfiles": []
    })
}

fn stored_config(config: &Value, updated_at: i64) -> AppConfigStoredState {
    app_config_stored_state_from_value(config, updated_at).unwrap()
}

fn tag(label: &str, suffix: &str) -> TagRecord {
    TagRecord {
        id: format!("{label}-tag-{suffix}"),
        name: format!("Tag {label} {suffix}"),
        description: format!("Description {suffix}"),
        icon: format!("icon-{suffix}"),
        color: if suffix == "a" { "#2563eb" } else { "#dc2626" }.to_string(),
        sort_order: usize::from(suffix != "a"),
        created_at: timestamp(label) as u64,
        updated_at: timestamp(label) as u64 + 1,
    }
}

fn tags(label: &str) -> Vec<TagRecord> {
    vec![tag(label, "b"), tag(label, "a")]
}

fn segment(label: &str, suffix: &str) -> Value {
    json!([{
        "id": format!("{label}-segment-{suffix}"),
        "text": format!("{label} transcript {suffix}"),
        "start": 0.0,
        "end": 1.0,
        "isFinal": true
    }])
}

fn seed_history(db: &Database, label: &str) {
    db.with_rw_transaction(|tx| {
        for suffix in ["b", "a"] {
            let id = format!("{label}-history-{suffix}");
            tx.execute(
                "INSERT INTO history_items (
                    id, timestamp, duration, audio_path, audio_status, transcript_path,
                    title, preview_text, icon, kind, search_content, status, draft_source
                 ) VALUES (?1, 500, ?2, ?3, 'removed', ?4, ?5, ?6, ?7, ?8, ?9, 'complete', NULL)",
                rusqlite::params![
                    id,
                    if suffix == "a" { 1.5 } else { 2.5 },
                    format!("{id}.wav"),
                    format!("{id}.json"),
                    format!("History {label} {suffix}"),
                    format!("Preview {suffix}"),
                    Some(format!("icon-{suffix}")),
                    if suffix == "a" { "recording" } else { "batch" },
                    format!("Search {label} {suffix}"),
                ],
            )?;
            tx.execute(
                "INSERT INTO history_item_tags (history_id, tag_id) VALUES (?1, ?2)",
                rusqlite::params![id, format!("{label}-tag-{suffix}")],
            )?;
            tx.execute(
                "INSERT INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![id, serde_json::to_string(&segment(label, suffix))?],
            )?;
            tx.execute(
                "INSERT INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
                rusqlite::params![
                    id,
                    json!({"summary": format!("{label}-{suffix}")}).to_string()
                ],
            )?;
        }

        let history_id = format!("{label}-history-b");
        for snapshot_suffix in ["b", "a"] {
            tx.execute(
                "INSERT INTO transcript_snapshots (
                    id, history_id, reason, created_at, segment_count, segments
                 ) VALUES (?1, ?2, ?3, 700, 1, ?4)",
                rusqlite::params![
                    format!("{label}-snapshot-{snapshot_suffix}"),
                    history_id,
                    if snapshot_suffix == "a" {
                        "polish"
                    } else {
                        "translate"
                    },
                    serde_json::to_string(&segment(label, snapshot_suffix))?,
                ],
            )?;
        }
        Ok(())
    })
    .unwrap();
}

fn automation_state(label: &str) -> AutomationRepositoryState {
    AutomationRepositoryState {
        profiles: vec![
            automation_profile(label, "b"),
            automation_profile(label, "a"),
        ],
        rules: vec![automation_rule(label, "b"), automation_rule(label, "a")],
        processed_entries: vec![
            automation_processed(label, "b"),
            automation_processed(label, "a"),
        ],
    }
}

fn automation_profile(label: &str, suffix: &str) -> AutomationProfileRecord {
    AutomationProfileRecord {
        id: format!("{label}-profile-{suffix}"),
        name: format!("Profile {suffix}"),
        translation_language: if suffix == "a" { "en" } else { "zh" }.to_string(),
        polish_preset_id: format!("{label}-polish"),
        summary_template_id: format!("{label}-summary-{suffix}"),
        enabled_text_replacement_set_ids: vec![
            format!("{label}-replace-{suffix}-2"),
            format!("{label}-replace-{suffix}-1"),
        ],
        enabled_hotword_set_ids: vec![format!("{label}-hotword-{suffix}")],
        enabled_polish_keyword_set_ids: vec![format!("{label}-keyword-{suffix}")],
        enabled_speaker_profile_ids: vec![
            format!("{label}-speaker-{suffix}-2"),
            format!("{label}-speaker-{suffix}-1"),
        ],
        created_at: timestamp(label),
        updated_at: timestamp(label) + 1,
    }
}

fn automation_rule(label: &str, suffix: &str) -> AutomationRuleRecord {
    AutomationRuleRecord {
        id: format!("{label}-rule-{suffix}"),
        name: format!("Rule {suffix}"),
        kind: "file".to_string(),
        priority: 0,
        profile_id: Some(format!("{label}-profile-{suffix}")),
        profile_source: "explicit".to_string(),
        save_history: true,
        tag_ids: vec![format!("{label}-tag-{suffix}")],
        preset_id: format!("preset-{suffix}"),
        watch_directory: format!("C:/{label}/{suffix}"),
        recursive: suffix == "a",
        enabled: true,
        actions: AutomationRuleInputActions {
            auto_polish: true,
            auto_translate: suffix == "b",
            auto_summary: false,
        },
        stage_config: AutomationRuleRecordStageConfig {
            auto_polish: true,
            polish_preset_id: format!("{label}-polish"),
            auto_translate: suffix == "b",
            translation_language: "en".to_string(),
            export_enabled: true,
        },
        export_config: AutomationRuleRecordExportConfig {
            directory: format!("C:/{label}/export/{suffix}"),
            format: "srt".to_string(),
            mode: "original".to_string(),
            prefix: format!("{suffix}-"),
        },
        created_at: timestamp(label),
        updated_at: timestamp(label) + 1,
        migration_notice: None,
    }
}

fn automation_processed(label: &str, suffix: &str) -> AutomationProcessedRecord {
    AutomationProcessedRecord {
        id: format!("{label}-processed-{suffix}"),
        rule_id: format!("{label}-rule-{suffix}"),
        kind: "file".to_string(),
        input_version: format!("fingerprint-{suffix}"),
        attempt: 1,
        file_path: format!("C:/{label}/{suffix}.wav"),
        source_fingerprint: format!("fingerprint-{suffix}"),
        size: if suffix == "a" { 11 } else { 22 },
        mtime_ms: timestamp(label),
        status: "complete".to_string(),
        processed_at: timestamp(label) + 2,
        history_id: Some(format!("{label}-history-{suffix}")),
        export_path: Some(format!("C:/{label}/{suffix}.srt")),
        error_message: None,
    }
}

fn analytics_content(label: &str) -> String {
    json!([
        {
            "occurredAt": "2026-07-13T00:00:00Z",
            "provider": format!("{label}-provider-b"),
            "category": "summary",
            "promptTokens": 20,
            "completionTokens": 2,
            "totalTokens": 22
        },
        {
            "occurredAt": "2026-07-13T00:00:00Z",
            "provider": format!("{label}-provider-a"),
            "category": "polish",
            "promptTokens": 10,
            "completionTokens": 1,
            "totalTokens": 11
        }
    ])
    .to_string()
}

fn manifest_for(dataset: &BackupDataset) -> sona_core::backup::BackupManifest {
    build_backup_manifest(
        0,
        "test".to_string(),
        dataset.tags.len(),
        dataset.history.items.len(),
        dataset.history.transcript_files.len(),
        dataset.history.summary_files.len(),
        dataset.automation.profiles.len(),
        dataset.automation.rules.len(),
        dataset.automation.processed_entries.len(),
    )
    .unwrap()
}

fn restore_dataset(dataset: BackupDataset, import_id: &str) -> BackupRestoreDataset {
    let manifest = manifest_for(&dataset);
    BackupRestoreDataset {
        import_id: import_id.to_string(),
        manifest,
        config_state: stored_config(&dataset.config, 999),
        tags: dataset.tags,
        history: dataset.history,
        automation: dataset.automation,
        analytics_content: dataset.analytics_content,
    }
}

fn assert_invalid_preflight(dataset: BackupRestoreDataset, expected_reason: &str) {
    let error = validate_backup_restore_dataset(&dataset).unwrap_err();
    assert!(
        matches!(error, BackupError::InvalidBackup(ref reason) if reason.contains(expected_reason)),
        "unexpected preflight error: {error:?}"
    );
}

#[test]
fn target_independent_restore_preflight_rejects_relationship_and_sql_constraints() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let source = fixture.repository.snapshot().unwrap();

    let mut duplicate_tags = source.clone();
    duplicate_tags.tags.push(source.tags[0].clone());
    assert_invalid_preflight(
        restore_dataset(duplicate_tags, "duplicate-tags"),
        "duplicate tag IDs",
    );

    let mut unknown_tag = source.clone();
    unknown_tag.history.items[0].tag_ids = vec!["missing-tag".to_string()];
    assert_invalid_preflight(restore_dataset(unknown_tag, "unknown-tag"), "unknown tag");

    let mut duplicate_automation = source;
    duplicate_automation
        .automation
        .rules
        .push(duplicate_automation.automation.rules[0].clone());
    let duplicate_automation = restore_dataset(duplicate_automation, "duplicate-automation");
    assert_invalid_preflight(duplicate_automation.clone(), "UNIQUE constraint failed");
    let error = fixture
        .repository
        .replace_all(duplicate_automation)
        .unwrap_err();
    assert!(
        matches!(error, BackupError::InvalidBackup(ref reason) if reason.contains("UNIQUE constraint failed")),
        "unexpected repository error: {error:?}"
    );
}

#[test]
fn lazy_repository_snapshot_requires_an_existing_directory_without_creating_it() {
    let parent = tempfile::tempdir().unwrap();
    let app_data_dir = parent.path().join("missing-app-data");
    let repository = LazySqliteBackupStateRepository::new(app_data_dir.clone());

    let error = repository.snapshot().unwrap_err();

    assert!(
        matches!(error, BackupError::State(ref reason) if reason == &format!(
            "Application data directory does not exist or is not a directory: {}",
            app_data_dir.display()
        )),
        "unexpected repository error: {error:?}"
    );
    assert!(!app_data_dir.exists());
}

#[test]
fn lazy_repository_validates_restore_before_checking_the_target_directory() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let mut duplicate_tags = fixture.repository.snapshot().unwrap();
    duplicate_tags.tags.push(duplicate_tags.tags[0].clone());
    let restore = restore_dataset(duplicate_tags, "duplicate-tags");

    let parent = tempfile::tempdir().unwrap();
    let app_data_dir = parent.path().join("missing-app-data");
    let repository = LazySqliteBackupStateRepository::new(app_data_dir.clone());

    let error = repository.replace_all(restore).unwrap_err();

    assert!(
        matches!(error, BackupError::InvalidBackup(ref reason) if reason.contains("duplicate tag IDs")),
        "unexpected repository error: {error:?}"
    );
    assert!(!app_data_dir.exists());
}

#[test]
fn lazy_repository_rejects_a_valid_restore_when_the_target_directory_is_missing() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let restore = restore_dataset(fixture.repository.snapshot().unwrap(), "valid-restore");

    let parent = tempfile::tempdir().unwrap();
    let app_data_dir = parent.path().join("missing-app-data");
    let repository = LazySqliteBackupStateRepository::new(app_data_dir.clone());

    let error = repository.replace_all(restore).unwrap_err();

    assert!(
        matches!(error, BackupError::State(ref reason) if reason == &format!(
            "Application data directory does not exist or is not a directory: {}",
            app_data_dir.display()
        )),
        "unexpected repository error: {error:?}"
    );
    assert!(!app_data_dir.exists());
}

fn dataset_value(dataset: &BackupDataset) -> Value {
    json!({
        "config": dataset.config,
        "tags": dataset.tags,
        "history": {
            "items": dataset.history.items,
            "transcriptFiles": dataset.history.transcript_files,
            "summaryFiles": dataset.history.summary_files,
            "snapshotFiles": dataset.history.snapshot_files,
        },
        "automation": dataset.automation,
        "analytics": serde_json::from_str::<Value>(&dataset.analytics_content).unwrap(),
    })
}

fn fresh_snapshot(root: &Path) -> BackupDataset {
    let db = Arc::new(Database::open(root).unwrap());
    SqliteBackupStateRepository::new(root.to_path_buf(), db)
        .snapshot()
        .unwrap()
}

#[test]
fn snapshot_and_replace_restore_all_scopes_and_clear_active_tag() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let before = fixture.repository.snapshot().unwrap();

    assert_eq!(
        before
            .tags
            .iter()
            .map(|tag| tag.id.as_str())
            .collect::<Vec<_>>(),
        ["before-tag-a", "before-tag-b"]
    );
    assert_eq!(
        before.automation.profiles[0].enabled_text_replacement_set_ids,
        ["before-replace-a-2", "before-replace-a-1"]
    );
    assert_eq!(
        before
            .history
            .summary_files
            .iter()
            .map(|(history_id, _)| history_id.as_str())
            .collect::<Vec<_>>(),
        ["before-history-a", "before-history-b"]
    );
    assert_eq!(
        serde_json::from_str::<Value>(&before.analytics_content).unwrap()[0]["provider"],
        "before-provider-b"
    );

    let expected = dataset_value(&before);
    let replacement = restore_dataset(before, "import-round-trip");
    let expected_manifest = replacement.manifest.clone();
    fixture.seed("after");
    let result = fixture.repository.replace_all(replacement).unwrap();

    assert_eq!(result.import_id, "import-round-trip");
    assert_eq!(result.manifest, expected_manifest);
    let config_store = SqliteConfigStore::new(Arc::clone(&fixture.db));
    assert_eq!(
        config_store.load_setting_json(ACTIVE_TAG_KEY).unwrap(),
        None
    );
    assert_eq!(
        dataset_value(&fresh_snapshot(fixture.root.path())),
        expected
    );
}

#[test]
fn every_replacement_phase_rolls_back_the_complete_pre_import_state() {
    let phases = [
        (
            "config",
            "CREATE TRIGGER fail_config BEFORE UPDATE ON app_config BEGIN SELECT RAISE(ABORT, 'forced config failure'); END;",
            "DROP TRIGGER fail_config;",
        ),
        (
            "tags",
            "CREATE TRIGGER fail_tags BEFORE INSERT ON tags BEGIN SELECT RAISE(ABORT, 'forced tags failure'); END;",
            "DROP TRIGGER fail_tags;",
        ),
        (
            "history",
            "CREATE TRIGGER fail_history BEFORE INSERT ON history_items BEGIN SELECT RAISE(ABORT, 'forced history failure'); END;",
            "DROP TRIGGER fail_history;",
        ),
        (
            "automation",
            "CREATE TRIGGER fail_automation BEFORE INSERT ON automation_rules BEGIN SELECT RAISE(ABORT, 'forced automation failure'); END;",
            "DROP TRIGGER fail_automation;",
        ),
        (
            "analytics",
            "CREATE TRIGGER analytics.fail_analytics BEFORE INSERT ON llm_usage BEGIN SELECT RAISE(ABORT, 'forced analytics failure'); END;",
            "DROP TRIGGER analytics.fail_analytics;",
        ),
    ];

    for (phase, install, remove) in phases {
        let fixture = Fixture::new();
        fixture.seed("replacement");
        let replacement = fixture.repository.snapshot().unwrap();
        fixture.seed("before");
        let before = dataset_value(&fixture.repository.snapshot().unwrap());
        let before_active_tag = fresh_active_tag_setting(fixture.root.path());
        fixture
            .db
            .with_write_connection(|connection| {
                connection.execute_batch(install)?;
                Ok(())
            })
            .unwrap();

        let error = fixture
            .repository
            .replace_all(restore_dataset(replacement, &format!("import-{phase}")))
            .unwrap_err();
        assert!(
            matches!(error, BackupError::State(ref reason) if reason.contains("forced")),
            "unexpected {phase} error: {error:?}"
        );
        fixture
            .db
            .with_write_connection(|connection| {
                connection.execute_batch(remove)?;
                Ok(())
            })
            .unwrap();

        assert_eq!(
            dataset_value(&fresh_snapshot(fixture.root.path())),
            before,
            "phase {phase} was not fully rolled back"
        );
        assert_eq!(
            fresh_active_tag_setting(fixture.root.path()),
            before_active_tag,
            "phase {phase} did not roll back the active-tag setting"
        );
    }
}

#[test]
fn malformed_history_is_rejected_before_any_replacement() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let before = dataset_value(&fixture.repository.snapshot().unwrap());
    fixture.seed("replacement");
    let mut replacement = fixture.repository.snapshot().unwrap();
    replacement.history.transcript_files.clear();
    fixture.seed("before");

    let error = fixture
        .repository
        .replace_all(restore_dataset(replacement, "invalid-history"))
        .unwrap_err();
    assert!(matches!(error, BackupError::InvalidBackup(_)));
    assert_eq!(dataset_value(&fresh_snapshot(fixture.root.path())), before);
}

#[test]
fn malformed_analytics_rows_are_rejected_before_any_replacement() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let before = dataset_value(&fixture.repository.snapshot().unwrap());
    let mut replacement = fixture.repository.snapshot().unwrap();
    replacement.analytics_content = "[null]".to_string();

    let error = fixture
        .repository
        .replace_all(restore_dataset(replacement, "invalid-analytics"))
        .unwrap_err();
    assert!(matches!(error, BackupError::InvalidBackup(_)));
    assert_eq!(dataset_value(&fresh_snapshot(fixture.root.path())), before);
}

#[test]
fn snapshot_rejects_corrupt_tag_and_history_storage() {
    let cases = [
        "UPDATE tags SET created_at = -1 WHERE id = 'before-tag-b'",
        "UPDATE tags SET updated_at = -1 WHERE id = 'before-tag-b'",
        "UPDATE history_items SET timestamp = -1 WHERE id = 'before-history-b'",
        "UPDATE history_items SET duration = -1 WHERE id = 'before-history-b'",
        "UPDATE history_items SET kind = 'unknown' WHERE id = 'before-history-b'",
        "UPDATE history_items SET audio_status = 'unknown' WHERE id = 'before-history-b'",
        "UPDATE history_items SET status = 'unknown' WHERE id = 'before-history-b'",
        "UPDATE history_items SET draft_source = 'unknown' WHERE id = 'before-history-b'",
        "UPDATE transcript_snapshots SET created_at = -1 WHERE history_id = 'before-history-b'",
        "UPDATE transcript_snapshots SET segment_count = -1 WHERE history_id = 'before-history-b'",
    ];

    for sql in cases {
        let fixture = Fixture::new();
        fixture.seed("before");
        fixture
            .db
            .with_write_connection(|connection| {
                connection.execute(sql, [])?;
                Ok(())
            })
            .unwrap();

        let error = fixture.repository.snapshot().unwrap_err();
        assert!(matches!(error, BackupError::State(_)), "{sql}: {error:?}");
    }
}

#[test]
fn snapshots_are_consistent_across_main_and_analytics_with_an_independent_writer() {
    let fixture = Fixture::new();
    fixture.seed("consistency");
    fixture
        .db
        .with_rw_transaction(|transaction| {
            transaction.execute(
                "UPDATE app_config SET config = ?1 WHERE id = 1",
                [json!({"marker": "A"}).to_string()],
            )?;
            transaction.execute("DELETE FROM analytics.llm_usage", [])?;
            transaction.execute(
                "INSERT INTO analytics.llm_usage (
                    occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens
                 ) VALUES ('2026-07-13T00:00:00Z', 'A', 'consistency', 1, 2, 3)",
                [],
            )?;
            Ok(())
        })
        .unwrap();

    let (transaction_started_tx, transaction_started_rx) = mpsc::channel();
    let (continue_transaction_tx, continue_transaction_rx) = mpsc::channel();
    let coordinated_db = Arc::new(CoordinatedDatabase {
        inner: Arc::clone(&fixture.db),
        transaction_started: Mutex::new(Some(transaction_started_tx)),
        continue_transaction: Mutex::new(continue_transaction_rx),
    });
    let repository = Arc::new(SqliteBackupStateRepository::new(
        fixture.root.path().to_path_buf(),
        coordinated_db,
    ));

    let (snapshot_tx, snapshot_rx) = mpsc::channel();
    let snapshot_worker = thread::spawn(move || snapshot_tx.send(repository.snapshot()).unwrap());

    let (attempt_tx, attempt_rx) = mpsc::channel();
    let (commit_tx, commit_rx) = mpsc::channel();
    let (writer_done_tx, writer_done_rx) = mpsc::channel();
    let writer_root = fixture.root.path().to_path_buf();
    let writer = thread::spawn(move || {
        let main_path = writer_root.join("sona.db");
        let analytics_path = writer_root.join("sona-analytics.db");
        let mut connection = Connection::open(main_path).unwrap();
        let mut analytics_connection = Connection::open(&analytics_path).unwrap();
        connection.busy_timeout(Duration::ZERO).unwrap();
        analytics_connection.busy_timeout(Duration::ZERO).unwrap();
        connection
            .execute_batch("PRAGMA journal_mode=WAL;")
            .unwrap();
        analytics_connection
            .execute_batch("PRAGMA journal_mode=WAL;")
            .unwrap();
        connection
            .execute_batch(&format!(
                "ATTACH DATABASE '{}' AS analytics; PRAGMA analytics.journal_mode=WAL;",
                analytics_path.to_string_lossy().replace('\'', "''")
            ))
            .unwrap();

        attempt_rx.recv().unwrap();
        let analytics_blocked = immediate_analytics_write_is_blocked(&mut analytics_connection);
        let main_blocked =
            match connection.transaction_with_behavior(TransactionBehavior::Immediate) {
                Ok(transaction) => {
                    drop(transaction);
                    false
                }
                Err(error) => matches!(
                    error,
                    rusqlite::Error::SqliteFailure(ref code, _)
                        if matches!(
                            code.code,
                            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                    )
                ),
            };
        writer_done_tx
            .send((main_blocked, analytics_blocked))
            .unwrap();

        commit_rx.recv().unwrap();
        connection.busy_timeout(Duration::from_secs(5)).unwrap();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        transaction
            .execute(
                "UPDATE app_config SET config = ?1 WHERE id = 1",
                [json!({"marker": "B"}).to_string()],
            )
            .unwrap();
        transaction
            .execute("DELETE FROM analytics.llm_usage", [])
            .unwrap();
        transaction
            .execute(
                "INSERT INTO analytics.llm_usage (
                    occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens
                 ) VALUES ('2026-07-13T00:00:00Z', 'B', 'consistency', 4, 5, 9)",
                [],
            )
            .unwrap();
        transaction.commit().unwrap();
    });

    transaction_started_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    attempt_tx.send(()).unwrap();
    let (main_writer_was_blocked, analytics_writer_was_blocked) =
        writer_done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    continue_transaction_tx.send(()).unwrap();
    let snapshot = snapshot_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert!(main_writer_was_blocked);
    assert!(analytics_writer_was_blocked);
    assert_eq!(snapshot.config["marker"], "A");
    let analytics: Value = serde_json::from_str(&snapshot.analytics_content).unwrap();
    assert_eq!(analytics[0]["provider"], "A");

    commit_tx.send(()).unwrap();
    writer.join().unwrap();
    snapshot_worker.join().unwrap();

    let after = fresh_snapshot(fixture.root.path());
    assert_eq!(after.config["marker"], "B");
    let analytics: Value = serde_json::from_str(&after.analytics_content).unwrap();
    assert_eq!(analytics[0]["provider"], "B");
}

fn immediate_analytics_write_is_blocked(connection: &mut Connection) -> bool {
    match connection.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(transaction) => {
            let blocked = match transaction.execute(
                "INSERT INTO llm_usage (
                    occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens
                 ) VALUES ('2026-07-13T00:00:00Z', 'unexpected', 'consistency', 0, 0, 0)",
                [],
            ) {
                Ok(_) => false,
                Err(error) => sqlite_is_busy_or_locked(&error),
            };
            drop(transaction);
            blocked
        }
        Err(error) => sqlite_is_busy_or_locked(&error),
    }
}

fn sqlite_is_busy_or_locked(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if matches!(
                code.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

#[test]
fn snapshot_waits_for_the_shared_history_file_lock() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let lock_file = independent_history_lock(fixture.root.path());
    let repository = Arc::clone(&fixture.repository);
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx.send(repository.snapshot()).unwrap();
    });

    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(
        done_rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    FileExt::unlock(&lock_file).unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
}

#[test]
fn replace_waits_for_the_shared_history_file_lock() {
    let fixture = Fixture::new();
    fixture.seed("replacement");
    let replacement = restore_dataset(fixture.repository.snapshot().unwrap(), "locked-replace");
    fixture.seed("before");
    let lock_file = independent_history_lock(fixture.root.path());
    let repository = Arc::clone(&fixture.repository);
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx.send(repository.replace_all(replacement)).unwrap();
    });

    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(
        done_rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    FileExt::unlock(&lock_file).unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
}

fn independent_history_lock(root: &Path) -> std::fs::File {
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".sona-history.lock"))
        .unwrap();
    lock_file.lock_exclusive().unwrap();
    lock_file
}

fn fresh_active_tag_setting(root: &Path) -> Option<String> {
    let db = Arc::new(Database::open(root).unwrap());
    SqliteConfigStore::new(db)
        .load_setting_json(ACTIVE_TAG_KEY)
        .unwrap()
}

#[test]
fn snapshot_lock_is_acquired_before_the_database_transaction() {
    let fixture = Fixture::new();
    fixture.seed("before");
    let lock_file = independent_history_lock(fixture.root.path());
    let repository = Arc::clone(&fixture.repository);
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || done_tx.send(repository.snapshot()).unwrap());
    thread::sleep(Duration::from_millis(100));

    let independent = Connection::open(fixture.root.path().join("sona.db")).unwrap();
    independent.busy_timeout(Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    independent
        .execute("UPDATE app_config SET updated_at = updated_at + 1", [])
        .unwrap();
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(matches!(
        done_rx.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));

    FileExt::unlock(&lock_file).unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
}

#[test]
fn backup_history_contains_only_complete_items() {
    let fixture = Fixture::new();
    fixture.seed("before");
    fixture
        .db
        .with_rw_transaction(|tx| {
            tx.execute(
                "INSERT INTO history_items (
                    id, timestamp, duration, audio_path, transcript_path, title, kind, status, draft_source
                 ) VALUES ('draft', 999, 0, '', 'draft.json', 'Draft', 'recording', 'draft', 'live_record')",
                [],
            )?;
            tx.execute(
                "INSERT INTO history_transcripts (history_id, segments) VALUES ('draft', '[]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

    let snapshot = fixture.repository.snapshot().unwrap();
    assert!(
        snapshot
            .history
            .items
            .iter()
            .all(|item| item.status == HistoryItemStatus::Complete)
    );
    assert!(!snapshot.history.items.iter().any(|item| item.id == "draft"));
}

fn _assert_history_shape_is_owned(_: HistoryBackupSnapshot) {}
