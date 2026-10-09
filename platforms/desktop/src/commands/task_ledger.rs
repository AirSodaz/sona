use sona_core::task_ledger::types::{TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot};
use tauri::State;

use crate::services::DesktopServices;

#[tauri::command]
pub async fn task_ledger_load_snapshot(
    services: State<'_, DesktopServices>,
) -> Result<TaskLedgerSnapshot, String> {
    services.task_ledger.load_snapshot().await
}

#[tauri::command]
pub async fn task_ledger_upsert_task(
    services: State<'_, DesktopServices>,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    services.task_ledger.upsert_task(record).await
}

#[tauri::command]
pub async fn task_ledger_patch_task(
    services: State<'_, DesktopServices>,
    id: String,
    patch: TaskLedgerPatch,
) -> Result<TaskLedgerSnapshot, String> {
    services.task_ledger.patch_task(id, patch).await
}

#[tauri::command]
pub async fn task_ledger_remove_task(
    services: State<'_, DesktopServices>,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    services.task_ledger.remove_task(id).await
}

#[tauri::command]
pub async fn task_ledger_clear_resolved(
    services: State<'_, DesktopServices>,
) -> Result<TaskLedgerSnapshot, String> {
    services.task_ledger.clear_resolved().await
}
