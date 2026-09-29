use sona_core::recovery::types::{RecoveryItemInput, RecoverySnapshot};
use tauri::AppHandle;

#[tauri::command]
pub async fn recovery_load_snapshot(app: AppHandle) -> Result<RecoverySnapshot, String> {
    crate::platform::recovery_repository::load_snapshot_for_app(&app).await
}

#[tauri::command]
pub async fn recovery_save_snapshot(
    app: AppHandle,
    items: Vec<RecoveryItemInput>,
) -> Result<RecoverySnapshot, String> {
    crate::platform::recovery_repository::save_snapshot_for_app(&app, items).await
}

#[tauri::command]
pub async fn recovery_persist_queue_snapshot(
    app: AppHandle,
    queue_items: Vec<RecoveryItemInput>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    crate::platform::recovery_repository::persist_queue_snapshot_for_app(
        &app,
        queue_items,
        resolved_ids,
    )
    .await
}
