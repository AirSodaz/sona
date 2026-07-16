use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::history::query_repository::HistoryQueryError;
use sona_core::history::query_service::HistoryQueryService;
use sona_core::history::{HistoryListOptions, HistoryWorkspaceQueryRequest};
use sona_sqlite::LazySqliteHistoryQueryRepository;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) async fn list_history_items_json(
    app_data_dir: String,
    limit: Option<u64>,
    offset: Option<u64>,
) -> SonaCoreBindingResult<String> {
    let opts = HistoryListOptions {
        limit: parse_pagination_value("limit", limit)?,
        offset: parse_pagination_value("offset", offset)?,
    };
    run_blocking(move || {
        with_service(&app_data_dir, |service| service.list_items(opts)).and_then(canonical_json)
    })
    .await
}

pub(crate) async fn query_history_workspace_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    run_blocking(move || {
        let request: HistoryWorkspaceQueryRequest =
            serde_json::from_str(&request_json).map_err(history_query_error)?;
        with_service(&app_data_dir, |service| service.query_workspace(request))
            .and_then(canonical_json)
    })
    .await
}

pub(crate) async fn load_history_transcript_json(
    app_data_dir: String,
    history_id: String,
) -> SonaCoreBindingResult<String> {
    run_blocking(move || {
        with_service(&app_data_dir, |service| {
            service.load_transcript(&history_id)
        })
        .and_then(canonical_json)
    })
    .await
}

pub(crate) async fn list_history_transcript_snapshots_json(
    app_data_dir: String,
    history_id: String,
) -> SonaCoreBindingResult<String> {
    run_blocking(move || {
        with_service(&app_data_dir, |service| {
            service.list_transcript_snapshots(&history_id)
        })
        .and_then(canonical_json)
    })
    .await
}

pub(crate) async fn load_history_transcript_snapshot_json(
    app_data_dir: String,
    history_id: String,
    snapshot_id: String,
) -> SonaCoreBindingResult<String> {
    run_blocking(move || {
        with_service(&app_data_dir, |service| {
            service.load_transcript_snapshot(&history_id, &snapshot_id)
        })
        .and_then(canonical_json)
    })
    .await
}

async fn run_blocking<T, F>(operation: F) -> SonaCoreBindingResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> SonaCoreBindingResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(history_query_error)?
}

fn with_service<T>(
    app_data_dir: &str,
    operation: impl FnOnce(HistoryQueryService) -> Result<T, HistoryQueryError>,
) -> SonaCoreBindingResult<T> {
    let app_data_dir =
        std::path::absolute(PathBuf::from(app_data_dir)).map_err(history_query_error)?;
    ensure_existing_directory(&app_data_dir)?;
    let repository = LazySqliteHistoryQueryRepository::new(app_data_dir);
    operation(HistoryQueryService::new(Arc::new(repository))).map_err(history_query_error)
}

fn ensure_existing_directory(path: &Path) -> SonaCoreBindingResult<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(history_query_error(format!(
            "History app data directory does not exist: {}",
            path.display()
        )))
    }
}

fn parse_pagination_value(label: &str, value: Option<u64>) -> SonaCoreBindingResult<Option<usize>> {
    value
        .map(|value| {
            if value > i64::MAX as u64 {
                return Err(history_query_error(format!(
                    "History {label} exceeds the supported range"
                )));
            }
            usize::try_from(value).map_err(history_query_error)
        })
        .transpose()
}

fn canonical_json(value: impl serde::Serialize) -> SonaCoreBindingResult<String> {
    let canonical = serde_json::to_value(value).map_err(history_query_error)?;
    serde_json::to_string(&canonical).map_err(history_query_error)
}

fn history_query_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::HistoryQuery {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        list_history_items_json, list_history_transcript_snapshots_json,
        load_history_transcript_json, load_history_transcript_snapshot_json,
        query_history_workspace_json,
    };
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use sona_core::history::mutation_repository::{
        HistoryCreateTranscriptSnapshotRequest, HistoryMutationRepository,
    };
    use sona_core::history::{HistorySaveRecordingRequest, TranscriptSnapshotReason};
    use sona_core::history_store::HistoryStore;
    use sona_sqlite::{Database, SqliteHistoryStore};
    use std::path::Path;
    use std::sync::Arc;

    fn create_fixture(app_data_dir: &Path) -> (String, String) {
        let database = Arc::new(Database::open(app_data_dir).unwrap());
        let store = SqliteHistoryStore::new(app_data_dir.to_path_buf(), database);
        store.ensure_ready().unwrap();
        let segments: Vec<sona_core::transcription::transcript::TranscriptSegment> =
            serde_json::from_value(json!([{
            "id": "segment-1",
            "text": "Hello mobile history",
            "start": 0.0,
            "end": 1.5,
            "isFinal": true
            }]))
            .unwrap();
        let item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: segments.clone(),
                duration: 1.5,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        let snapshot = store
            .create_transcript_snapshot(HistoryCreateTranscriptSnapshotRequest {
                history_id: item.id.clone(),
                reason: TranscriptSnapshotReason::Polish,
                segments,
            })
            .unwrap();
        (item.id, snapshot.id)
    }

    fn app_data_dir(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    fn parse_canonical(output: &str) -> Value {
        let value: Value = serde_json::from_str(output).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), output);
        value
    }

    #[tokio::test]
    async fn all_history_query_bindings_return_canonical_persisted_state() {
        let dir = tempfile::tempdir().unwrap();
        let (history_id, snapshot_id) = create_fixture(dir.path());
        let app_data = app_data_dir(&dir);
        let request = json!({
            "scope": {"kind": "all"},
            "query": "mobile",
            "filterType": "all",
            "dateFilter": "all",
            "sortOrder": "newest",
            "limit": 20,
            "offset": 0
        })
        .to_string();

        let items = parse_canonical(
            &list_history_items_json(app_data.clone(), Some(20), Some(0))
                .await
                .unwrap(),
        );
        let workspace = parse_canonical(
            &query_history_workspace_json(app_data.clone(), request)
                .await
                .unwrap(),
        );
        let transcript = parse_canonical(
            &load_history_transcript_json(app_data.clone(), history_id.clone())
                .await
                .unwrap(),
        );
        let snapshots = parse_canonical(
            &list_history_transcript_snapshots_json(app_data.clone(), history_id.clone())
                .await
                .unwrap(),
        );
        let snapshot = parse_canonical(
            &load_history_transcript_snapshot_json(
                app_data,
                history_id.clone(),
                snapshot_id.clone(),
            )
            .await
            .unwrap(),
        );

        assert_eq!(items[0]["id"], history_id);
        assert_eq!(workspace["filteredItemCount"], 1);
        assert_eq!(transcript[0]["text"], "Hello mobile history");
        assert_eq!(snapshots[0]["id"], snapshot_id);
        assert_eq!(snapshot["metadata"]["id"], snapshot_id);
    }

    #[tokio::test]
    async fn invalid_inputs_use_history_query_error_without_creating_directories() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");

        let errors = [
            list_history_items_json(missing.to_string_lossy().into_owned(), None, None)
                .await
                .unwrap_err(),
            query_history_workspace_json(app_data_dir(&root), "{".to_string())
                .await
                .unwrap_err(),
            list_history_items_json(app_data_dir(&root), Some(u64::MAX), None)
                .await
                .unwrap_err(),
        ];

        for error in errors {
            assert!(matches!(error, SonaCoreBindingError::HistoryQuery { .. }));
        }
        assert!(!missing.exists());
        assert!(!root.path().join("sona.db").exists());
    }

    #[tokio::test]
    async fn relative_unicode_app_data_paths_are_supported() {
        let current = std::env::current_dir().unwrap();
        let parent = tempfile::tempdir_in(&current).unwrap();
        let app_data_dir = parent.path().join("历史-移动端-🌍");
        std::fs::create_dir(&app_data_dir).unwrap();
        let (history_id, _) = create_fixture(&app_data_dir);
        let relative = app_data_dir.strip_prefix(&current).unwrap();

        let output =
            list_history_items_json(relative.to_string_lossy().into_owned(), Some(20), Some(0))
                .await
                .unwrap();
        let items = parse_canonical(&output);

        assert_eq!(items[0]["id"], history_id);
    }
}
