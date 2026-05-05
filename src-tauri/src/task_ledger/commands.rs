use serde_json::Value;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::repository::TaskLedgerRepository;
use super::types::{TaskLedgerRecord, TaskLedgerSnapshot, TASK_LEDGER_UPDATED_EVENT};

fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

async fn run_repository_task<R, T, F>(app: AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(TaskLedgerRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        task(TaskLedgerRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn emit_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &TaskLedgerSnapshot,
) -> Result<(), String> {
    app.emit(TASK_LEDGER_UPDATED_EVENT, snapshot)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn task_ledger_load_snapshot<R: Runtime>(
    app: AppHandle<R>,
) -> Result<TaskLedgerSnapshot, String> {
    run_repository_task(app, |repository| repository.load_snapshot()).await
}

#[tauri::command]
pub async fn task_ledger_upsert_task<R: Runtime>(
    app: AppHandle<R>,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_repository_task(app.clone(), move |repository| {
        repository.upsert_task(record)
    })
    .await?;
    emit_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn task_ledger_patch_task<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    patch: Value,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_repository_task(app.clone(), move |repository| {
        repository.patch_task(&id, patch)
    })
    .await?;
    emit_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn task_ledger_remove_task<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot =
        run_repository_task(app.clone(), move |repository| repository.remove_task(&id)).await?;
    emit_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn task_ledger_clear_resolved<R: Runtime>(
    app: AppHandle<R>,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot =
        run_repository_task(app.clone(), |repository| repository.clear_resolved()).await?;
    emit_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}
