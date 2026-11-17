use crate::services::DesktopServices;
use sona_core::recovery::types::{RecoveryItemInput, RecoverySnapshot};
use tauri::State;

#[tauri::command]
pub async fn recovery_load_snapshot(
    services: State<'_, DesktopServices>,
) -> Result<RecoverySnapshot, String> {
    crate::platform::recovery_repository::load_snapshot_for_dir(services.active_data_dir())
        .await
        .map_err(|err| {
            log::error!("[RecoveryCommand] Failed to load recovery snapshot: {err}");
            err
        })
}

#[tauri::command]
pub async fn recovery_save_snapshot(
    services: State<'_, DesktopServices>,
    items: Vec<RecoveryItemInput>,
) -> Result<RecoverySnapshot, String> {
    crate::platform::recovery_repository::save_snapshot_for_dir(services.active_data_dir(), items)
        .await
        .map_err(|err| {
            log::error!("[RecoveryCommand] Failed to save recovery snapshot: {err}");
            err
        })
}

#[tauri::command]
pub async fn recovery_persist_queue_snapshot(
    services: State<'_, DesktopServices>,
    queue_items: Vec<RecoveryItemInput>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    crate::platform::recovery_repository::persist_queue_snapshot_for_dir(
        services.active_data_dir(),
        queue_items,
        resolved_ids,
    )
    .await
    .map_err(|err| {
        log::error!("[RecoveryCommand] Failed to persist queue snapshot: {err}");
        err
    })
}
