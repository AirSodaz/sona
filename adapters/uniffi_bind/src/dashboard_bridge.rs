use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::dashboard::DashboardService;
use sona_sqlite::analytics::SqliteAnalyticsRepository;
use sona_sqlite::{Database, SqliteHistoryStore, SqliteProjectRepository};
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) async fn load_dashboard_snapshot_json(
    app_data_dir: String,
    deep: bool,
) -> SonaCoreBindingResult<String> {
    tokio::task::spawn_blocking(move || build_dashboard_snapshot_json(app_data_dir, deep))
        .await
        .map_err(dashboard_error)?
}

fn build_dashboard_snapshot_json(
    app_data_dir: String,
    deep: bool,
) -> SonaCoreBindingResult<String> {
    let app_data_dir = std::path::absolute(PathBuf::from(app_data_dir)).map_err(dashboard_error)?;
    let database =
        Arc::new(Database::open_read_only_with_analytics(&app_data_dir).map_err(dashboard_error)?);
    let service = DashboardService::new(
        Arc::new(SqliteHistoryStore::new(app_data_dir, Arc::clone(&database))),
        Arc::new(SqliteProjectRepository::new(Arc::clone(&database))),
        Arc::new(SqliteAnalyticsRepository::new(database)),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(dashboard_error)?;
    let snapshot = runtime
        .block_on(service.build_snapshot_at(deep, sona_runtime_fs::dashboard_snapshot_time_now()))
        .map_err(dashboard_error)?;
    let canonical = serde_json::to_value(snapshot).map_err(dashboard_error)?;
    serde_json::to_string(&canonical).map_err(dashboard_error)
}

fn dashboard_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Dashboard {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::load_dashboard_snapshot_json;
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use sona_core::history::HistorySaveRecordingRequest;
    use sona_core::history::mutation_repository::HistoryMutationRepository;
    use sona_core::history_store::HistoryStore;
    use sona_core::llm::usage::{LlmUsageCategory, TokenUsage, UsageRecord};
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
        assert_eq!(snapshot["content"]["overview"]["projectCount"], 0);
        assert_eq!(snapshot["content"]["overview"]["isDeepLoaded"], false);
        assert!(snapshot["content"].get("speakers").unwrap().is_null());
        assert!(!output.contains('\n'));
    }

    #[tokio::test]
    async fn deep_snapshot_reads_unicode_active_wal_without_source_changes() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Arc::new(Database::open(dir.path()).unwrap());
        let history = SqliteHistoryStore::new(dir.path().to_path_buf(), Arc::clone(&writer));
        history.ensure_ready().unwrap();
        let text = "你好 UniFFI 🌍";
        history
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([{
                    "id": "segment-unicode",
                    "text": text,
                    "start": 0.0,
                    "end": 2.0,
                    "isFinal": true
                }]),
                duration: 2.0,
                project_id: None,
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
