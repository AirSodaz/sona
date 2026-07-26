use crate::application_context::cached_application_context;
use crate::{FfiDashboardSnapshotV1, SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::dashboard::models::DashboardSnapshotDomainModel;
use sona_sqlite::load_dashboard_snapshot;
use std::path::PathBuf;

pub(crate) async fn load_dashboard_snapshot_json(
    app_data_dir: String,
    deep: bool,
) -> SonaCoreBindingResult<String> {
    let snapshot = load_snapshot(app_data_dir, deep).await?;
    let canonical = serde_json::to_value(snapshot).map_err(dashboard_error)?;
    serde_json::to_string(&canonical).map_err(dashboard_error)
}

pub(crate) async fn load_dashboard_snapshot_v1(
    app_data_dir: String,
    deep: bool,
) -> SonaCoreBindingResult<FfiDashboardSnapshotV1> {
    load_snapshot(app_data_dir, deep).await.map(Into::into)
}

async fn load_snapshot(
    app_data_dir: String,
    deep: bool,
) -> SonaCoreBindingResult<DashboardSnapshotDomainModel> {
    tokio::task::spawn_blocking(move || build_dashboard_snapshot(app_data_dir, deep))
        .await
        .map_err(dashboard_error)?
}

fn build_dashboard_snapshot(
    app_data_dir: String,
    deep: bool,
) -> SonaCoreBindingResult<DashboardSnapshotDomainModel> {
    let app_data_dir = std::path::absolute(PathBuf::from(app_data_dir)).map_err(dashboard_error)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(dashboard_error)?;
    let time = sona_runtime_fs::dashboard_snapshot_time_now();
    match cached_application_context(&app_data_dir).map_err(dashboard_error)? {
        Some(context) => runtime.block_on(
            context
                .sqlite()
                .dashboard_service()
                .build_snapshot_at(deep, time),
        ),
        None => runtime.block_on(load_dashboard_snapshot(app_data_dir, deep, time)),
    }
    .map_err(dashboard_error)
}

fn dashboard_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Dashboard {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{load_dashboard_snapshot_json, load_dashboard_snapshot_v1};
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use sona_core::history::HistorySaveRecordingRequest;
    use sona_core::history::mutation_repository::HistoryMutationRepository;
    use sona_core::history_store::HistoryStore;
    use sona_core::llm::usage::{LlmUsageCategory, TokenUsage, UsageRecord};
    use sona_runtime_fs::{SystemClock, UuidGenerator};
    use sona_sqlite::llm_usage::record_usage;
    use sona_sqlite::{Database, SqliteHistoryStore};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

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
                    let digest = Sha256::digest(fs::read(&path).unwrap());
                    files.insert(
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        format!("{digest:x}"),
                    );
                }
            }
        }

        let mut files = BTreeMap::new();
        visit(root, root, &mut files);
        files
    }

    #[tokio::test]
    async fn missing_directory_is_rejected_without_creation() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");

        let error = load_dashboard_snapshot_json(missing.to_string_lossy().into_owned(), false)
            .await
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Dashboard { .. }));
        assert!(!missing.exists());
    }

    #[tokio::test]
    async fn relative_directory_returns_canonical_empty_snapshot() {
        let current = std::env::current_dir().unwrap();
        let dir = tempfile::tempdir_in(&current).unwrap();
        drop(Database::open(dir.path()).unwrap());
        let relative = dir.path().strip_prefix(&current).unwrap();

        let output = load_dashboard_snapshot_json(relative.to_string_lossy().into_owned(), false)
            .await
            .unwrap();
        let snapshot: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(serde_json::to_string(&snapshot).unwrap(), output);
        assert_eq!(snapshot["content"]["overview"]["itemCount"], 0);
        assert_eq!(snapshot["content"]["overview"]["tagCount"], 0);
        assert_eq!(snapshot["content"]["overview"]["isDeepLoaded"], false);
        assert!(snapshot["content"].get("speakers").unwrap().is_null());
        assert!(!output.contains('\n'));
    }

    #[tokio::test]
    async fn deep_snapshot_reads_unicode_active_wal_without_source_changes() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Arc::new(Database::open(dir.path()).unwrap());
        let history = SqliteHistoryStore::with_environment(
            dir.path().to_path_buf(),
            Arc::clone(&writer),
            Arc::new(SystemClock),
            Arc::new(UuidGenerator),
        );
        history.ensure_ready().unwrap();
        let text = "你好 UniFFI 🌍";
        history
            .save_recording(HistorySaveRecordingRequest {
                segments: serde_json::from_value(json!([{
                    "id": "segment-unicode",
                    "text": text,
                    "start": 0.0,
                    "end": 2.0,
                    "isFinal": true
                }]))
                .unwrap(),
                duration: 2.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        record_usage(
            writer.as_ref(),
            &UsageRecord {
                occurred_at: "2026-07-13T07:00:00Z".to_string(),
                provider: "uniffi-test".to_string(),
                category: LlmUsageCategory::Summary,
                usage: Some(TokenUsage {
                    prompt_tokens: 8,
                    completion_tokens: 5,
                    total_tokens: 13,
                    ..TokenUsage::default()
                }),
            },
        )
        .unwrap();
        for sidecar in [
            "sona.db-wal",
            "sona.db-shm",
            "sona-analytics.db-wal",
            "sona-analytics.db-shm",
        ] {
            assert!(dir.path().join(sidecar).is_file(), "missing {sidecar}");
        }
        let before = file_hashes(dir.path());

        let output = load_dashboard_snapshot_json(dir.path().to_string_lossy().into_owned(), true)
            .await
            .unwrap();
        let snapshot: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(snapshot["content"]["overview"]["itemCount"], 1);
        assert_eq!(
            snapshot["content"]["overview"]["transcriptCharacterCount"],
            text.encode_utf16().count() as u64
        );
        assert_eq!(snapshot["content"]["overview"]["isDeepLoaded"], true);
        assert_eq!(snapshot["llmUsage"]["totals"]["totalTokens"], 13);
        assert_eq!(file_hashes(dir.path()), before);
        drop(writer);
    }

    #[tokio::test]
    async fn typed_snapshot_matches_the_legacy_json_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Arc::new(Database::open(dir.path()).unwrap());
        let history = SqliteHistoryStore::with_environment(
            dir.path().to_path_buf(),
            Arc::clone(&writer),
            Arc::new(SystemClock),
            Arc::new(UuidGenerator),
        );
        history.ensure_ready().unwrap();
        history
            .save_recording(HistorySaveRecordingRequest {
                segments: serde_json::from_value(json!([{
                    "id": "segment-typed",
                    "text": "typed dashboard",
                    "start": 0.0,
                    "end": 3.0,
                    "isFinal": true
                }]))
                .unwrap(),
                duration: 3.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![4, 5, 6]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        record_usage(
            writer.as_ref(),
            &UsageRecord {
                occurred_at: "2026-07-13T07:00:00Z".to_string(),
                provider: "typed-provider".to_string(),
                category: LlmUsageCategory::Summary,
                usage: Some(TokenUsage {
                    prompt_tokens: 11,
                    completion_tokens: 7,
                    total_tokens: 18,
                    ..TokenUsage::default()
                }),
            },
        )
        .unwrap();
        drop(writer);
        let app_data_dir = dir.path().to_string_lossy().into_owned();

        let typed = load_dashboard_snapshot_v1(app_data_dir.clone(), true)
            .await
            .unwrap();
        let json: Value = serde_json::from_str(
            &load_dashboard_snapshot_json(app_data_dir, true)
                .await
                .unwrap(),
        )
        .unwrap();

        // Both surfaces project the same snapshot, including the preformatted
        // display strings Core renders next to every number.
        let overview = &json["content"]["overview"];
        assert_eq!(typed.content.overview.item_count, 1);
        assert_eq!(typed.content.overview.item_count, overview["itemCount"]);
        assert_eq!(
            typed.content.overview.item_count_display,
            overview["itemCountDisplay"].as_str().unwrap()
        );
        assert_eq!(
            typed.content.overview.total_duration_seconds,
            overview["totalDurationSeconds"].as_f64().unwrap()
        );
        assert!(typed.content.overview.is_deep_loaded);
        assert_eq!(typed.llm_usage.totals.total_tokens, 18);
        assert_eq!(
            typed.llm_usage.totals.total_tokens,
            json["llmUsage"]["totals"]["totalTokens"]
        );
        assert_eq!(
            typed.llm_usage.by_provider.len(),
            json["llmUsage"]["byProvider"].as_array().unwrap().len()
        );
        assert_eq!(typed.llm_usage.by_provider[0].key, "typed-provider");
        // A deep load populates speakers; a shallow one leaves it absent.
        assert!(typed.content.speakers.is_some());
    }

    #[tokio::test]
    async fn typed_shallow_snapshot_omits_speakers_and_reports_errors() {
        let dir = tempfile::tempdir().unwrap();
        drop(Database::open(dir.path()).unwrap());

        let shallow = load_dashboard_snapshot_v1(dir.path().to_string_lossy().into_owned(), false)
            .await
            .unwrap();
        assert!(shallow.content.speakers.is_none());
        assert!(!shallow.content.overview.is_deep_loaded);

        let missing = dir.path().join("missing");
        let error = load_dashboard_snapshot_v1(missing.to_string_lossy().into_owned(), false)
            .await
            .unwrap_err();
        assert!(matches!(error, SonaCoreBindingError::Dashboard { .. }));
        assert!(!missing.exists());
    }

    #[tokio::test]
    async fn future_schema_uses_dashboard_error() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        db.with_write_connection(|connection| {
            connection.execute("INSERT INTO schema_version (version) VALUES (99)", [])?;
            Ok(())
        })
        .unwrap();
        drop(db);

        let error = load_dashboard_snapshot_json(dir.path().to_string_lossy().into_owned(), false)
            .await
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Dashboard { .. }));
        assert!(error.to_string().contains("99"));
    }
}
