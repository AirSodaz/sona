use serde_json::Value;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime, State};

use super::backup::{
    apply_prepared_history_import_inner, export_backup_archive_inner, prepare_backup_import_inner,
};
use super::fs_utils::remove_path_if_exists;
use super::repository::HistoryRepository;
use super::state::{HistoryRepositoryState, PreparedBackupImportState};
use super::{
    BackupManifest, ExportBackupArchiveRequest, HistoryItemRecord, LiveRecordingDraftResult,
    PreparedBackupImport, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
    TranscriptSnapshotRecord, HISTORY_DIR_NAME,
};

fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

async fn run_history_task<R, T, F>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(HistoryRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    let lock = state.lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        task(HistoryRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn history_list_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
) -> Result<Vec<HistoryItemRecord>, String> {
    run_history_task(app, state, |repository| repository.list_items()).await
}

#[tauri::command]
pub async fn history_create_live_draft<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    item: Value,
) -> Result<LiveRecordingDraftResult, String> {
    run_history_task(app, state, move |repository| {
        repository.create_live_draft(item)
    })
    .await
}

#[tauri::command]
pub async fn history_complete_live_draft<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    segments: Value,
    preview_text: String,
    search_content: String,
    duration: f64,
) -> Result<HistoryItemRecord, String> {
    run_history_task(app, state, move |repository| {
        repository.complete_live_draft(
            &history_id,
            segments,
            preview_text,
            search_content,
            duration,
        )
    })
    .await
}

#[tauri::command]
pub async fn history_save_recording<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    item: Value,
    segments: Value,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
) -> Result<HistoryItemRecord, String> {
    run_history_task(app, state, move |repository| {
        repository.save_recording(item, segments, audio_bytes, native_audio_path)
    })
    .await
}

#[tauri::command]
pub async fn history_save_imported_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    item: Value,
    segments: Value,
    source_path: String,
) -> Result<HistoryItemRecord, String> {
    run_history_task(app, state, move |repository| {
        repository.save_imported_file(item, segments, source_path)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| repository.delete_items(&ids)).await
}

#[tauri::command]
pub async fn history_load_transcript<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    filename: String,
) -> Result<Option<Value>, String> {
    run_history_task(app, state, move |repository| {
        repository.load_transcript(&filename)
    })
    .await
}

#[tauri::command]
pub async fn history_update_transcript<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    segments: Value,
    preview_text: String,
    search_content: String,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.update_transcript(&history_id, segments, preview_text, search_content)
    })
    .await
}

#[tauri::command]
pub async fn history_create_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    reason: TranscriptSnapshotReason,
    segments: Value,
) -> Result<TranscriptSnapshotMetadata, String> {
    run_history_task(app, state, move |repository| {
        repository.create_transcript_snapshot(&history_id, reason, segments)
    })
    .await
}

#[tauri::command]
pub async fn history_list_transcript_snapshots<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
    run_history_task(app, state, move |repository| {
        repository.list_transcript_snapshots(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    snapshot_id: String,
) -> Result<Option<TranscriptSnapshotRecord>, String> {
    run_history_task(app, state, move |repository| {
        repository.load_transcript_snapshot(&history_id, &snapshot_id)
    })
    .await
}

#[tauri::command]
pub async fn history_update_item_meta<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    updates: Value,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.update_item_meta(&history_id, updates)
    })
    .await
}

#[tauri::command]
pub async fn history_update_project_assignments<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
    project_id: Option<String>,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.update_project_assignments(&ids, project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_reassign_project<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    current_project_id: String,
    next_project_id: Option<String>,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.reassign_project(current_project_id, next_project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_summary<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
) -> Result<Option<Value>, String> {
    run_history_task(app, state, move |repository| {
        repository.load_summary(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_save_summary<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    summary_payload: Value,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.save_summary(&history_id, summary_payload)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_summary<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.delete_summary(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_resolve_audio_path<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    filename: String,
) -> Result<Option<String>, String> {
    run_history_task(app, state, move |repository| {
        repository.resolve_audio_path(&filename)
    })
    .await
}

#[tauri::command]
pub async fn history_open_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
) -> Result<(), String> {
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    {
        let _guard = state.lock.lock().map_err(|error| error.to_string())?;
        HistoryRepository::new(app_local_data_dir.clone()).ensure_ready()?;
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
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    let lock = history_state.lock.clone();
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

    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    let lock = history_state.lock.clone();
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
