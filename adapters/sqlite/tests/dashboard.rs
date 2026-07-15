use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDate;
use serde_json::json;
use sha2::{Digest, Sha256};
use sona_core::dashboard::{DashboardServiceError, DashboardSnapshotTime};
use sona_core::history::HistorySaveRecordingRequest;
use sona_core::history::mutation_repository::HistoryMutationRepository;
use sona_core::history_store::HistoryStore;
use sona_core::llm::usage::{LlmUsageCategory, TokenUsage, UsageRecord};
use sona_core::project::{
    DEFAULT_POLISH_PRESET_ID, DEFAULT_SUMMARY_TEMPLATE_ID, DEFAULT_TRANSLATION_LANGUAGE,
    ProjectDefaults, ProjectRecord, ProjectStore,
};
use sona_sqlite::llm_usage::{read_stats, record_usage};
use sona_sqlite::{
    Database, SqliteHistoryStore, SqliteProjectRepository, create_dashboard_service,
    load_dashboard_snapshot,
};

fn project() -> ProjectRecord {
    ProjectRecord {
        id: "project-dashboard".to_string(),
        name: "Dashboard".to_string(),
        description: String::new(),
        icon: String::new(),
        created_at: 1,
        updated_at: 1,
        defaults: ProjectDefaults {
            summary_template_id: DEFAULT_SUMMARY_TEMPLATE_ID.to_string(),
            translation_language: DEFAULT_TRANSLATION_LANGUAGE.to_string(),
            polish_preset_id: DEFAULT_POLISH_PRESET_ID.to_string(),
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: String::new(),
            enabled_text_replacement_set_ids: Vec::new(),
            enabled_hotword_set_ids: Vec::new(),
            enabled_polish_keyword_set_ids: Vec::new(),
            enabled_speaker_profile_ids: Vec::new(),
        },
    }
}

fn snapshot_time() -> DashboardSnapshotTime {
    DashboardSnapshotTime {
        generated_at: "2026-07-13T08:00:00.000Z".to_string(),
        today: NaiveDate::from_ymd_opt(2026, 7, 13).unwrap(),
        local_utc_offset_seconds: 0,
    }
}

fn file_hashes(root: &Path) -> BTreeMap<PathBuf, String> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, String>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                let digest = Sha256::digest(fs::read(&path).unwrap());
                files.insert(relative, format!("{digest:x}"));
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
}

#[test]
fn read_only_snapshot_includes_analytics_usage() {
    let dir = tempfile::tempdir().unwrap();
    let writer = Database::open(dir.path()).unwrap();
    record_usage(
        &writer,
        &UsageRecord {
            occurred_at: "2026-07-13T07:00:00Z".to_string(),
            provider: "test-provider".to_string(),
            category: LlmUsageCategory::Summary,
            usage: Some(TokenUsage {
                prompt_tokens: 20,
                completion_tokens: 5,
                total_tokens: 25,
                ..TokenUsage::default()
            }),
        },
    )
    .unwrap();

    let read_only = Database::open_read_only_with_analytics(dir.path()).unwrap();
    let stats = read_stats(&read_only).unwrap();

    assert_eq!(stats.totals.total_tokens, 25);
    drop(writer);
}

#[test]
fn read_only_dashboard_composes_all_ports_without_mutating_active_wal() {
    let dir = tempfile::tempdir().unwrap();
    let writer = Arc::new(Database::open(dir.path()).unwrap());
    let history = SqliteHistoryStore::new(dir.path().to_path_buf(), Arc::clone(&writer));
    history.ensure_ready().unwrap();
    let projects = SqliteProjectRepository::new(Arc::clone(&writer));
    ProjectStore::insert_project(&projects, project()).unwrap();

    let unicode_text = "你好, dashboard 🌍";
    history
        .save_recording(HistorySaveRecordingRequest {
            segments: json!([{
                "id": "segment-identified",
                "text": unicode_text,
                "start": 0.0,
                "end": 2.5,
                "isFinal": true,
                "speaker": {
                    "id": "speaker-alice",
                    "label": "Alice",
                    "kind": "identified",
                    "score": 0.95
                }
            }]),
            duration: 2.5,
            project_id: Some("project-dashboard".to_string()),
            audio_bytes: Some(vec![1, 2, 3]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        })
        .unwrap();
    history
        .save_recording(HistorySaveRecordingRequest {
            segments: json!([{
                "id": "segment-anonymous",
                "text": "plain",
                "start": 0.0,
                "end": 1.5,
                "isFinal": true,
                "speaker": {
                    "id": "speaker-1",
                    "label": "Speaker 1",
                    "kind": "anonymous",
                    "score": null
                }
            }]),
            duration: 1.5,
            project_id: None,
            audio_bytes: Some(vec![4, 5]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        })
        .unwrap();
    record_usage(
        writer.as_ref(),
        &UsageRecord {
            occurred_at: "2026-07-13T07:00:00Z".to_string(),
            provider: "test-provider".to_string(),
            category: LlmUsageCategory::Summary,
            usage: Some(TokenUsage {
                prompt_tokens: 20,
                completion_tokens: 5,
                total_tokens: 25,
                ..TokenUsage::default()
            }),
        },
    )
    .unwrap();

    assert!(dir.path().join("sona.db-wal").is_file());
    assert!(dir.path().join("sona.db-shm").is_file());
    assert!(dir.path().join("sona-analytics.db-wal").is_file());
    assert!(dir.path().join("sona-analytics.db-shm").is_file());
    let before = file_hashes(dir.path());

    let read_only = Arc::new(Database::open_read_only_with_analytics(dir.path()).unwrap());
    let service = create_dashboard_service(dir.path().to_path_buf(), read_only);
    let runtime = runtime();
    let shallow = runtime
        .block_on(service.build_snapshot_at(false, snapshot_time()))
        .unwrap();
    let deep = runtime
        .block_on(load_dashboard_snapshot(
            dir.path().to_path_buf(),
            true,
            snapshot_time(),
        ))
        .unwrap();

    assert_eq!(shallow.generated_at, "2026-07-13T08:00:00.000Z");
    assert_eq!(shallow.content.overview.item_count, 2);
    assert_eq!(shallow.content.overview.project_count, 1);
    assert_eq!(shallow.content.overview.total_duration_seconds, 4.0);
    assert_eq!(shallow.content.overview.inbox_count, 1);
    assert_eq!(shallow.llm_usage.totals.total_tokens, 25);
    assert!(!shallow.content.overview.is_deep_loaded);
    assert!(shallow.content.speakers.is_none());

    assert!(deep.content.overview.is_deep_loaded);
    assert_eq!(
        deep.content.overview.transcript_character_count,
        Some((unicode_text.encode_utf16().count() + "plain".len()) as u64)
    );
    let speakers = deep.content.speakers.unwrap();
    assert_eq!(speakers.identified_speaker_count, 1);
    assert_eq!(speakers.anonymous_speaker_slot_count, 1);
    assert_eq!(speakers.speaker_tagged_segment_count, 2);
    assert_eq!(speakers.top_identified_speakers[0].label, "Alice");

    assert_eq!(file_hashes(dir.path()), before);
    drop(writer);
}

#[test]
fn lazy_dashboard_entrypoint_rejects_future_schema() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).unwrap();
    db.with_write_connection(|connection| {
        connection.execute("INSERT INTO schema_version (version) VALUES (99)", [])?;
        Ok(())
    })
    .unwrap();
    drop(db);

    let error = runtime()
        .block_on(load_dashboard_snapshot(
            dir.path().to_path_buf(),
            false,
            snapshot_time(),
        ))
        .unwrap_err();

    assert!(matches!(
        error,
        DashboardServiceError::Internal(reason) if reason.contains("99")
    ));
}

#[test]
fn lazy_dashboard_entrypoint_rejects_missing_directory_without_creating_it() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");

    let error = runtime()
        .block_on(load_dashboard_snapshot(
            missing.clone(),
            false,
            snapshot_time(),
        ))
        .unwrap_err();

    assert!(matches!(error, DashboardServiceError::Internal(_)));
    assert!(!missing.exists());
}
