use sona_core::task_ledger::types::{TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot};
use tauri::AppHandle;

#[tauri::command]
pub async fn task_ledger_load_snapshot(app: AppHandle) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::load_snapshot(&app).await
}

#[tauri::command]
pub async fn task_ledger_upsert_task(
    app: AppHandle,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::upsert_task(&app, record).await
}

#[tauri::command]
pub async fn task_ledger_patch_task(
    app: AppHandle,
    id: String,
    patch: TaskLedgerPatch,
) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::patch_task(&app, id, patch).await
}

#[tauri::command]
pub async fn task_ledger_remove_task(
    app: AppHandle,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::remove_task(&app, id).await
}

#[tauri::command]
pub async fn task_ledger_clear_resolved(app: AppHandle) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::clear_resolved(&app).await
}
