use serde_json::Value;
use serde_json::to_value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Runtime, State};

use crate::core::database::DatabaseError;
use crate::core::history_store::HistoryStore;
use crate::core::paths::{PathKind, PathProvider};
use crate::integrations::asr::TranscriptSegment;
use crate::repositories::history::SqliteHistoryStore;
use crate::repositories::history::backup::{
    apply_prepared_history_import_inner, export_backup_archive_inner, prepare_backup_import_inner,
};
use crate::repositories::history::fs_utils::remove_path_if_exists;
use crate::repositories::history::{
    BackupManifest, ExportBackupArchiveRequest, HISTORY_DIR_NAME, HistoryAudioCleanupReport,
    HistoryAudioCleanupRequest, HistoryCreateLiveDraftRequest, HistoryItemRecord,
    HistoryItemStatus, HistoryListOptions, HistoryRepositoryState, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult, HistoryWorkspaceScope,
    HistoryWorkspaceSortOrder, LiveRecordingDraftResult, PreparedBackupImport,
    PreparedBackupImportState, TranscriptDiffResult, TranscriptDiffRow, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};

async fn run_history_db_task_inner<T, F>(app_local_data_dir: PathBuf, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, DatabaseError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        task(SqliteHistoryStore::new(app_local_data_dir)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn run_history_file_task_inner<T, F>(
    app_local_data_dir: PathBuf,
    lock: Arc<Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, DatabaseError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        task(SqliteHistoryStore::new(app_local_data_dir)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn run_history_db_task<T, F>(provider: &dyn PathProvider, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, DatabaseError> + Send + 'static,
{
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    run_history_db_task_inner(app_local_data_dir, task).await
}

async fn run_history_file_task<T, F>(
    provider: &dyn PathProvider,
    state: State<'_, HistoryRepositoryState>,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, DatabaseError> + Send + 'static,
{
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    run_history_file_task_inner(app_local_data_dir, state.file_lock.clone(), task).await
}

async fn run_history_db_task_with_state<T, F>(
    provider: &dyn PathProvider,
    _state: HistoryRepositoryState,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, DatabaseError> + Send + 'static,
{
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    run_history_db_task_inner(app_local_data_dir, task).await
}

pub(crate) async fn history_create_llm_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    state: HistoryRepositoryState,
    history_id: String,
    reason: TranscriptSnapshotReason,
    segments: Vec<TranscriptSegment>,
) -> Result<Option<TranscriptSnapshotMetadata>, String> {
    run_history_db_task_with_state(&app as &dyn PathProvider, state, move |repository| {
        create_llm_transcript_snapshot_record(&repository, &history_id, reason, segments)
    })
    .await
}

pub(crate) async fn history_update_llm_transcript_segments<R: Runtime>(
    app: AppHandle<R>,
    state: HistoryRepositoryState,
    history_id: String,
    segments: Vec<TranscriptSegment>,
) -> Result<Option<HistoryItemRecord>, String> {
    if history_id.trim().is_empty() || history_id == "current" {
        return Ok(None);
    }

    run_history_db_task_with_state(&app as &dyn PathProvider, state, move |repository| {
        update_llm_transcript_segments_record(&repository, &history_id, segments)
    })
    .await
}

pub(crate) async fn history_save_llm_summary<R: Runtime>(
    app: AppHandle<R>,
    state: HistoryRepositoryState,
    history_id: String,
    summary_payload: Value,
) -> Result<(), String> {
    if history_id.trim().is_empty() || history_id == "current" {
        return Ok(());
    }

    run_history_db_task_with_state(&app as &dyn PathProvider, state, move |repository| {
        save_llm_summary_payload(&repository, &history_id, summary_payload)
    })
    .await
}

fn create_llm_transcript_snapshot_record(
    repository: &impl HistoryStore,
    history_id: &str,
    reason: TranscriptSnapshotReason,
    segments: Vec<TranscriptSegment>,
) -> Result<Option<TranscriptSnapshotMetadata>, DatabaseError> {
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

    repository
        .create_transcript_snapshot(history_id, reason, to_value(segments)?)
        .map(Some)
}

fn update_llm_transcript_segments_record(
    repository: &impl HistoryStore,
    history_id: &str,
    segments: Vec<TranscriptSegment>,
) -> Result<Option<HistoryItemRecord>, DatabaseError> {
    if history_id.trim().is_empty() || history_id == "current" {
        return Ok(None);
    }

    repository
        .update_transcript(history_id, to_value(segments)?)
        .map(Some)
}

fn save_llm_summary_payload(
    repository: &impl HistoryStore,
    history_id: &str,
    summary_payload: Value,
) -> Result<(), DatabaseError> {
    if history_id.trim().is_empty() || history_id == "current" {
        return Ok(());
    }

    repository.save_summary(history_id, summary_payload)
}

#[tauri::command]
pub async fn history_list_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<HistoryItemRecord>, String> {
    let opts = HistoryListOptions { limit, offset };
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.list_items_with_reconciled_live_drafts_paginated(opts)
    })
    .await
}

#[tauri::command]
pub async fn history_query_workspace<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    scope: HistoryWorkspaceScope,
    query: String,
    filter_type: HistoryWorkspaceFilterType,
    date_filter: HistoryWorkspaceDateFilter,
    sort_order: HistoryWorkspaceSortOrder,
) -> Result<HistoryWorkspaceQueryResult, String> {
    let request = HistoryWorkspaceQueryRequest {
        scope,
        query,
        filter_type,
        date_filter,
        sort_order,
    };
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.query_workspace(request)
    })
    .await
}

#[tauri::command]
pub async fn history_create_live_draft<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    id: Option<String>,
    audio_extension: String,
    project_id: Option<String>,
    icon: Option<String>,
) -> Result<LiveRecordingDraftResult, String> {
    let request = HistoryCreateLiveDraftRequest {
        id,
        audio_extension,
        project_id,
        icon,
    };
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.create_live_draft(request)
    })
    .await
}

#[tauri::command]
pub async fn history_complete_live_draft<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    segments: Value,
    duration: f64,
) -> Result<HistoryItemRecord, String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.complete_live_draft(&history_id, segments, duration)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_recording<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    segments: Value,
    duration: f64,
    project_id: Option<String>,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
    audio_extension: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveRecordingRequest {
        segments,
        duration,
        project_id,
        audio_bytes,
        native_audio_path,
        audio_extension,
    };
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.save_recording(request)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_imported_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    id: Option<String>,
    source_path: String,
    segments: Value,
    duration: f64,
    project_id: Option<String>,
    converted_source_path: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveImportedFileRequest {
        id,
        source_path,
        segments,
        duration,
        project_id,
        converted_source_path,
    };
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.save_imported_file(request)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
) -> Result<(), String> {
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.delete_items(&ids)
    })
    .await
}

#[tauri::command]
pub async fn history_load_transcript<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<Vec<TranscriptSegment>>, String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.load_transcript(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_update_transcript<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    segments: Value,
) -> Result<HistoryItemRecord, String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.update_transcript(&history_id, segments)
    })
    .await
}

#[tauri::command]
pub async fn history_create_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    reason: TranscriptSnapshotReason,
    segments: Value,
) -> Result<TranscriptSnapshotMetadata, String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.create_transcript_snapshot(&history_id, reason, segments)
    })
    .await
}

#[tauri::command]
pub async fn history_list_transcript_snapshots<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.list_transcript_snapshots(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    snapshot_id: String,
) -> Result<Option<TranscriptSnapshotRecord>, String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.load_transcript_snapshot(&history_id, &snapshot_id)
    })
    .await
}

#[tauri::command]
pub fn history_build_transcript_diff(
    snapshot_segments: Vec<TranscriptSegment>,
    current_segments: Vec<TranscriptSegment>,
) -> Result<TranscriptDiffResult, String> {
    Ok(
        crate::repositories::history::transcript_diff::build_transcript_diff(
            snapshot_segments,
            current_segments,
        ),
    )
}

#[tauri::command]
pub fn history_restore_transcript_diff_rows(
    rows: Vec<TranscriptDiffRow>,
    selected_row_ids: Vec<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    Ok(
        crate::repositories::history::transcript_diff::restore_transcript_diff_rows(
            rows,
            selected_row_ids,
        ),
    )
}

#[tauri::command]
pub async fn history_update_item_meta<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    updates: Value,
) -> Result<(), String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.update_item_meta(&history_id, updates)
    })
    .await
}

#[tauri::command]
pub async fn history_update_project_assignments<R: Runtime>(
    app: AppHandle<R>,
    ids: Vec<String>,
    project_id: Option<String>,
) -> Result<(), String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.update_project_assignments(&ids, project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_reassign_project<R: Runtime>(
    app: AppHandle<R>,
    current_project_id: String,
    next_project_id: Option<String>,
) -> Result<(), String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.reassign_project(current_project_id, next_project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<Value>, String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.load_summary(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_save_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    summary_payload: Value,
) -> Result<(), String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.save_summary(&history_id, summary_payload)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<(), String> {
    run_history_db_task(&app as &dyn PathProvider, move |repository| {
        repository.delete_summary(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_resolve_audio_path<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
) -> Result<Option<String>, String> {
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.resolve_audio_path(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_preview_audio_cleanup<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    retention_days: Option<u64>,
    exclude_history_id: Option<String>,
) -> Result<HistoryAudioCleanupReport, String> {
    let request = HistoryAudioCleanupRequest {
        retention_days,
        exclude_history_id,
    };
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.preview_audio_cleanup(request)
    })
    .await
}

#[tauri::command]
pub async fn history_cleanup_audio<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    retention_days: Option<u64>,
    exclude_history_id: Option<String>,
) -> Result<HistoryAudioCleanupReport, String> {
    let request = HistoryAudioCleanupRequest {
        retention_days,
        exclude_history_id,
    };
    run_history_file_task(&app as &dyn PathProvider, state, move |repository| {
        repository.cleanup_audio(request)
    })
    .await
}

#[tauri::command]
pub async fn history_open_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
) -> Result<(), String> {
    let app_local_data_dir = (&app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    {
        let _guard = state.file_lock.lock().map_err(|error| error.to_string())?;
        SqliteHistoryStore::new(app_local_data_dir.clone())
            .ensure_ready()
            .map_err(|e| e.to_string())?;
    }

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(
            app_local_data_dir.join(HISTORY_DIR_NAME).to_string_lossy(),
            None::<&str>,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn export_backup_archive<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    let app_local_data_dir = (&app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    let lock = history_state.file_lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        export_backup_archive_inner(&app_local_data_dir, request)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn prepare_backup_import(
    state: State<'_, PreparedBackupImportState>,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    let archive_path_buf = PathBuf::from(&archive_path);
    let (prepared, snapshot) = tauri::async_runtime::spawn_blocking(move || {
        prepare_backup_import_inner(&archive_path_buf)
    })
    .await
    .map_err(|error| error.to_string())??;

    state.insert(prepared.import_id.clone(), snapshot)?;
    Ok(prepared)
}

#[tauri::command]
pub async fn apply_prepared_history_import<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    prepared_state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = prepared_state.get(&import_id)? else {
        return Err(format!("Prepared backup import not found: {import_id}"));
    };

    let app_local_data_dir = (&app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    let lock = history_state.file_lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        apply_prepared_history_import_inner(
            &app_local_data_dir,
            &import_id,
            &snapshot.extraction_dir,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn dispose_prepared_backup_import(
    state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = state.remove(&import_id)? else {
        return Ok(());
    };

    tauri::async_runtime::spawn_blocking(move || {
        let _archive_path = snapshot.archive_path;
        remove_path_if_exists(&snapshot.extraction_dir)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;
    use crate::integrations::asr::TranscriptSegment;
    use crate::repositories::history::sqlite_store::SqliteHistoryStore;
    use serde_json::json;
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
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
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
        let repository = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);

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
