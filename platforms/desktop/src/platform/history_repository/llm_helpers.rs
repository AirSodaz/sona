use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, to_value};
use tauri::{AppHandle, Runtime};

use super::{
    HistoryItemRecord, HistoryItemStatus, SqliteHistoryStore, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason,
};
use crate::integrations::asr::TranscriptSegment;
use crate::platform::paths::PathProvider;
use sona_core::history_store::{HistoryStore, HistoryStoreError};
use sona_sqlite::Database;

pub(crate) async fn run_llm_db_task<T, F>(
    app_local_data_dir: PathBuf,
    db: Arc<Database>,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        task(SqliteHistoryStore::new(app_local_data_dir, db)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub(crate) async fn run_llm_db_task_with_app<R, T, F>(
    app: &AppHandle<R>,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    let app_local_data_dir = crate::platform::paths::TauriPathProvider::from_app(app)
        .resolve_path(crate::platform::paths::PathKind::AppLocalData)?;
    let db = crate::platform::database::sqlite_database(app);
    run_llm_db_task(app_local_data_dir, db, task).await
}

pub(crate) fn create_llm_transcript_snapshot_record(
    repository: &impl HistoryStore,
    history_id: &str,
    reason: TranscriptSnapshotReason,
    segments: Vec<TranscriptSegment>,
) -> Result<Option<TranscriptSnapshotMetadata>, HistoryStoreError> {
    if history_id.trim().is_empty() || history_id == "current" || segments.is_empty() {
        return Ok(None);
    }

    let items = repository.list_items()?;
    let Some(item) = items.iter().find(|entry| entry.id == history_id) else {
        return Ok(None);
    };

    if item.status == HistoryItemStatus::Draft {
        return Ok(None);
    }

    let segments_value =
        to_value(segments).map_err(|e| HistoryStoreError::Internal(e.to_string()))?;
    repository
        .create_transcript_snapshot(history_id, reason, segments_value)
        .map(Some)
}

pub(crate) fn update_llm_transcript_segments_record(
    repository: &impl HistoryStore,
    history_id: &str,
    segments: Vec<TranscriptSegment>,
) -> Result<Option<HistoryItemRecord>, HistoryStoreError> {
    if history_id.trim().is_empty() || history_id == "current" {
        return Ok(None);
    }

    let segments_value =
        to_value(segments).map_err(|e| HistoryStoreError::Internal(e.to_string()))?;
    repository
        .update_transcript(history_id, segments_value)
        .map(Some)
}

pub(crate) fn save_llm_summary_payload(
    repository: &impl HistoryStore,
    history_id: &str,
    summary_payload: Value,
) -> Result<(), HistoryStoreError> {
    if history_id.trim().is_empty() || history_id == "current" {
        return Ok(());
    }

    repository.save_summary(history_id, summary_payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::history_repository::{
        HISTORY_DIR_NAME, HistoryCreateLiveDraftRequest, HistorySaveRecordingRequest,
    };
    use serde_json::json;
    use sona_core::history::query_repository::HistoryQueryRepository;
    use sona_sqlite::Database;
    use tempfile::tempdir;

    fn segment(id: &str, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: text.to_string(),
            start: 0.0,
            end: 1.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }
    }

    fn make_store() -> (tempfile::TempDir, SqliteHistoryStore) {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::new(root.path().to_path_buf(), Arc::new(db));
        store.ensure_ready().unwrap();
        (root, store)
    }

    #[test]
    fn llm_history_helpers_create_snapshot_then_update_transcript() {
        let (_root, repository) = make_store();

        let item = repository
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([{"id": "seg-1", "text": "before", "start": 0.0, "end": 1.0, "isFinal": true}]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![]),
                native_audio_path: None,
                audio_extension: None,
            })
            .unwrap();

        let snapshot = create_llm_transcript_snapshot_record(
            &repository,
            &item.id,
            TranscriptSnapshotReason::Translate,
            vec![segment("seg-1", "before")],
        )
        .unwrap()
        .unwrap();

        let mut translated = segment("seg-1", "before");
        translated.translation = Some("after".to_string());
        let updated =
            update_llm_transcript_segments_record(&repository, &item.id, vec![translated])
                .unwrap()
                .unwrap();

        assert_eq!(snapshot.reason, TranscriptSnapshotReason::Translate);
        assert_eq!(updated.preview_text, "before...");
        let snapshots = repository.list_transcript_snapshots(&item.id).unwrap();
        assert_eq!(snapshots.len(), 1);
        let snapshot_record = repository
            .load_transcript_snapshot(&item.id, &snapshot.id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot_record.segments[0].text, "before");
        let transcript = repository.load_transcript(&item.id).unwrap().unwrap();
        assert_eq!(transcript[0].translation.as_deref(), Some("after"));
    }

    #[test]
    fn llm_history_helpers_skip_current_jobs_without_writing() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let repository = SqliteHistoryStore::new(root.path().to_path_buf(), Arc::new(db));

        let snapshot = create_llm_transcript_snapshot_record(
            &repository,
            "current",
            TranscriptSnapshotReason::Polish,
            vec![segment("seg-1", "before")],
        )
        .unwrap();
        let updated = update_llm_transcript_segments_record(
            &repository,
            "current",
            vec![segment("seg-1", "after")],
        )
        .unwrap();
        save_llm_summary_payload(
            &repository,
            "current",
            json!({ "activeTemplateId": "general" }),
        )
        .unwrap();

        assert!(snapshot.is_none());
        assert!(updated.is_none());
        assert!(!root.path().join(HISTORY_DIR_NAME).exists());
    }

    #[test]
    fn llm_history_snapshot_helper_skips_drafts() {
        let (_root, repository) = make_store();

        let draft = repository
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: None,
                audio_extension: "wav".to_string(),
                project_id: None,
                icon: None,
            })
            .unwrap();

        let snapshot = create_llm_transcript_snapshot_record(
            &repository,
            &draft.item.id,
            TranscriptSnapshotReason::Polish,
            vec![segment("seg-1", "before")],
        )
        .unwrap();

        assert!(snapshot.is_none());
        assert!(
            repository
                .list_transcript_snapshots(&draft.item.id)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn llm_history_summary_helper_saves_sidecar_payload() {
        let (_root, repository) = make_store();

        let item = repository
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([{"id": "seg-1", "text": "test", "start": 0.0, "end": 1.0, "isFinal": true}]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![]),
                native_audio_path: None,
                audio_extension: None,
            })
            .unwrap();

        save_llm_summary_payload(
            &repository,
            &item.id,
            json!({
                "activeTemplateId": "meeting",
                "record": {
                    "templateId": "meeting",
                    "content": "Summary",
                    "generatedAt": "2026-05-04T00:00:00.000Z",
                    "sourceFingerprint": "fingerprint"
                }
            }),
        )
        .unwrap();

        assert_eq!(
            repository.load_summary(&item.id).unwrap(),
            Some(json!({
                "activeTemplateId": "meeting",
                "record": {
                    "templateId": "meeting",
                    "content": "Summary",
                    "generatedAt": "2026-05-04T00:00:00.000Z",
                    "sourceFingerprint": "fingerprint"
                }
            }))
        );
    }
}
