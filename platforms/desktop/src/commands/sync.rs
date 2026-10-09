use tauri::State;

use sona_core::sync::{
    SyncConflictDetail, SyncConflictResolution, SyncConflictSummary, SyncJoinPreview, SyncPresetV1,
    SyncProviderDescriptor, SyncRunResult, SyncStatusSnapshot,
};
use sona_sync::{DiscoveredVaultSummary, SyncPairingInfo, SyncProviderInput};
use sona_sync_webdav::WebDavObjectStoreConfig;

use crate::platform::history_repository::PreparedBackupImport;
use crate::platform::sync::{
    DesktopSyncManager, LegacyRemoteBackupListResult, SyncChangePasswordRequest, SyncCreateRequest,
    SyncCreateResult, SyncJoinRequest, SyncPreviewJoinRequest, SyncUnlockRecoveryRequest,
    SyncUnlockRequest, webdav_provider_input,
};
use crate::services::DesktopServices;

#[tauri::command]
pub async fn sync_get_status(
    services: State<'_, DesktopServices>,
) -> Result<SyncStatusSnapshot, String> {
    services.sync.get_status().await
}

#[tauri::command]
pub async fn sync_test_provider(
    services: State<'_, DesktopServices>,
    provider: SyncProviderInput,
) -> Result<SyncProviderDescriptor, String> {
    services.sync.test_provider(provider).await
}

#[tauri::command]
pub async fn sync_test_webdav_provider(
    services: State<'_, DesktopServices>,
    config: WebDavObjectStoreConfig,
) -> Result<SyncProviderDescriptor, String> {
    services
        .sync
        .test_provider(webdav_provider_input(config)?)
        .await
}

#[tauri::command]
pub async fn sync_discover_vaults(
    services: State<'_, DesktopServices>,
    provider: SyncProviderInput,
) -> Result<Vec<DiscoveredVaultSummary>, String> {
    services.sync.discover_vaults(provider).await
}

#[tauri::command]
pub async fn sync_discover_webdav_vaults(
    services: State<'_, DesktopServices>,
    config: WebDavObjectStoreConfig,
) -> Result<Vec<DiscoveredVaultSummary>, String> {
    services
        .sync
        .discover_vaults(webdav_provider_input(config)?)
        .await
}

#[tauri::command]
pub async fn sync_get_pairing_info(
    services: State<'_, DesktopServices>,
) -> Result<Option<SyncPairingInfo>, String> {
    services.sync.get_pairing_info().await
}

#[tauri::command]
pub async fn sync_list_legacy_backups(
    config: WebDavObjectStoreConfig,
) -> Result<LegacyRemoteBackupListResult, String> {
    DesktopSyncManager::list_legacy_backups(config).await
}

#[tauri::command]
pub async fn sync_prepare_legacy_backup_import(
    services: State<'_, DesktopServices>,
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
    let result = services
        .history
        .prepare_backup_import(archive_path.to_string_lossy().into_owned())
        .await;
    let _ = std::fs::remove_dir_all(&temporary_dir);
    result
}

#[tauri::command]
pub async fn sync_create_vault(
    services: State<'_, DesktopServices>,
    request: SyncCreateRequest,
) -> Result<SyncCreateResult, String> {
    services.sync.create_vault(request).await
}

#[tauri::command]
pub async fn sync_preview_join(
    services: State<'_, DesktopServices>,
    request: SyncPreviewJoinRequest,
) -> Result<SyncJoinPreview, String> {
    services.sync.preview_join(request).await
}

#[tauri::command]
pub async fn sync_join_vault(
    services: State<'_, DesktopServices>,
    request: SyncJoinRequest,
) -> Result<SyncRunResult, String> {
    services.sync.join_vault(request).await
}

#[tauri::command]
pub async fn sync_unlock(
    services: State<'_, DesktopServices>,
    request: SyncUnlockRequest,
) -> Result<SyncStatusSnapshot, String> {
    services.sync.unlock(request).await
}

#[tauri::command]
pub async fn sync_unlock_with_recovery(
    services: State<'_, DesktopServices>,
    request: SyncUnlockRecoveryRequest,
) -> Result<SyncStatusSnapshot, String> {
    services.sync.unlock_with_recovery(request).await
}

#[tauri::command]
pub async fn sync_lock(services: State<'_, DesktopServices>) -> Result<SyncStatusSnapshot, String> {
    services.sync.lock().await
}

#[tauri::command]
pub async fn sync_set_paused(
    services: State<'_, DesktopServices>,
    paused: bool,
) -> Result<SyncStatusSnapshot, String> {
    services.sync.set_paused(paused).await
}

#[tauri::command]
pub async fn sync_disconnect(
    services: State<'_, DesktopServices>,
) -> Result<SyncStatusSnapshot, String> {
    services.sync.disconnect().await
}

#[tauri::command]
pub async fn sync_run_now(services: State<'_, DesktopServices>) -> Result<SyncRunResult, String> {
    services.sync.run_now().await
}

#[tauri::command]
pub async fn sync_change_preset(
    services: State<'_, DesktopServices>,
    preset: SyncPresetV1,
    confirm_shrink: bool,
) -> Result<SyncStatusSnapshot, String> {
    services.sync.change_preset(preset, confirm_shrink).await
}

#[tauri::command]
pub async fn sync_change_master_password(
    services: State<'_, DesktopServices>,
    request: SyncChangePasswordRequest,
) -> Result<(), String> {
    services.sync.change_master_password(request).await
}

#[tauri::command]
pub async fn sync_generate_recovery_key(
    services: State<'_, DesktopServices>,
) -> Result<String, String> {
    services.sync.generate_recovery_key().await
}

#[tauri::command]
pub async fn sync_list_conflicts(
    services: State<'_, DesktopServices>,
) -> Result<Vec<SyncConflictSummary>, String> {
    services.sync.list_conflicts().await
}

#[tauri::command]
pub async fn sync_get_conflict(
    services: State<'_, DesktopServices>,
    conflict_id: String,
) -> Result<Option<SyncConflictDetail>, String> {
    services.sync.get_conflict(&conflict_id).await
}

#[tauri::command]
pub async fn sync_resolve_conflict(
    services: State<'_, DesktopServices>,
    conflict_id: String,
    resolution: SyncConflictResolution,
) -> Result<(), String> {
    services
        .sync
        .resolve_conflict(&conflict_id, resolution)
        .await
}
