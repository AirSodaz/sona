use tauri::{AppHandle, Runtime, State};

use sona_core::sync::{
    SyncConflictDetail, SyncConflictResolution, SyncConflictSummary, SyncJoinPreview, SyncPresetV1,
    SyncProviderDescriptor, SyncRunResult, SyncStatusSnapshot,
};
use sona_sync_webdav::WebDavObjectStoreConfig;

use crate::platform::history_repository::{PreparedBackupImport, PreparedBackupImportState};
use crate::platform::sync::{
    DesktopSyncManager, LegacyRemoteBackupListResult, SyncChangePasswordRequest, SyncCreateRequest,
    SyncCreateResult, SyncJoinRequest, SyncPreviewJoinRequest, SyncUnlockRecoveryRequest,
    SyncUnlockRequest,
};

#[tauri::command]
pub async fn sync_get_status<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
) -> Result<SyncStatusSnapshot, String> {
    manager.get_status(&app).await
}

#[tauri::command]
pub async fn sync_test_webdav_provider<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    config: WebDavObjectStoreConfig,
) -> Result<SyncProviderDescriptor, String> {
    manager.test_webdav_provider(&app, config).await
}

#[tauri::command]
pub async fn sync_list_legacy_backups(
    config: WebDavObjectStoreConfig,
) -> Result<LegacyRemoteBackupListResult, String> {
    DesktopSyncManager::list_legacy_backups(config).await
}

#[tauri::command]
pub async fn sync_prepare_legacy_backup_import<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, PreparedBackupImportState>,
    config: WebDavObjectStoreConfig,
    key: String,
) -> Result<PreparedBackupImport, String> {
    let bytes = DesktopSyncManager::download_legacy_backup(config, key).await?;
    let temporary_dir =
        std::env::temp_dir().join(format!("sona-legacy-backup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temporary_dir).map_err(|error| error.to_string())?;
    let archive_path = temporary_dir.join("legacy-backup.tar.bz2");
    if let Err(error) = std::fs::write(&archive_path, bytes) {
        let _ = std::fs::remove_dir_all(&temporary_dir);
        return Err(error.to_string());
    }
    let result = crate::platform::history_repository::prepare_backup_import(
        &app,
        state.inner(),
        archive_path.to_string_lossy().into_owned(),
    )
    .await;
    let _ = std::fs::remove_dir_all(&temporary_dir);
    result
}

#[tauri::command]
pub async fn sync_create_vault<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    request: SyncCreateRequest,
) -> Result<SyncCreateResult, String> {
    manager.create_vault(&app, request).await
}

#[tauri::command]
pub async fn sync_preview_join<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    request: SyncPreviewJoinRequest,
) -> Result<SyncJoinPreview, String> {
    manager.preview_join(&app, request).await
}

#[tauri::command]
pub async fn sync_join_vault<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    request: SyncJoinRequest,
) -> Result<SyncRunResult, String> {
    manager.join_vault(&app, request).await
}

#[tauri::command]
pub async fn sync_unlock<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    request: SyncUnlockRequest,
) -> Result<SyncStatusSnapshot, String> {
    manager.unlock(&app, request).await
}

#[tauri::command]
pub async fn sync_unlock_with_recovery<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    request: SyncUnlockRecoveryRequest,
) -> Result<SyncStatusSnapshot, String> {
    manager.unlock_with_recovery(&app, request).await
}

#[tauri::command]
pub async fn sync_lock<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
) -> Result<SyncStatusSnapshot, String> {
    manager.lock(&app).await
}

#[tauri::command]
pub async fn sync_set_paused<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    paused: bool,
) -> Result<SyncStatusSnapshot, String> {
    manager.set_paused(&app, paused).await
}

#[tauri::command]
pub async fn sync_disconnect<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
) -> Result<SyncStatusSnapshot, String> {
    manager.disconnect(&app).await
}

#[tauri::command]
pub async fn sync_run_now<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
) -> Result<SyncRunResult, String> {
    manager.run_now(&app).await
}

#[tauri::command]
pub async fn sync_change_preset<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    preset: SyncPresetV1,
    confirm_shrink: bool,
) -> Result<SyncStatusSnapshot, String> {
    manager.change_preset(&app, preset, confirm_shrink).await
}

#[tauri::command]
pub async fn sync_change_master_password<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    request: SyncChangePasswordRequest,
) -> Result<(), String> {
    manager.change_master_password(&app, request).await
}

#[tauri::command]
pub async fn sync_generate_recovery_key<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
) -> Result<String, String> {
    manager.generate_recovery_key(&app).await
}

#[tauri::command]
pub async fn sync_list_conflicts<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
) -> Result<Vec<SyncConflictSummary>, String> {
    manager.list_conflicts(&app).await
}

#[tauri::command]
pub async fn sync_get_conflict<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    conflict_id: String,
) -> Result<Option<SyncConflictDetail>, String> {
    manager.get_conflict(&app, &conflict_id).await
}

#[tauri::command]
pub async fn sync_resolve_conflict<R: Runtime>(
    app: AppHandle<R>,
    manager: State<'_, DesktopSyncManager>,
    conflict_id: String,
    resolution: SyncConflictResolution,
) -> Result<(), String> {
    manager
        .resolve_conflict(&app, &conflict_id, resolution)
        .await
}
