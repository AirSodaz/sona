use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sona_core::sync::{
    SyncConflictDetail, SyncConflictResolution, SyncConflictSummary, SyncJoinPreview,
    SyncObjectKey, SyncPresetV1, SyncProviderDescriptor, SyncRunResult, SyncSecretStore,
    SyncStatusSnapshot,
};
use sona_runtime_fs::SystemClock;
use sona_sync::{
    DiscoveredVaultSummary, JsonFileSyncConfigStore, LegacyRemoteBackupEntry,
    LegacyRemoteBackupService, SyncApplication, SyncCreateResult as ApplicationCreateResult,
    SyncPairingInfo, SyncProviderFactory, SyncProviderInput, SyncProviderRegistry,
    SystemSyncApplicationEnvironment, legacy_provider_credential_key,
};
use sona_sync_s3::S3SyncProviderFactory;
use sona_sync_webdav::{WebDavObjectStore, WebDavObjectStoreConfig, WebDavSyncProviderFactory};
use tokio::sync::Mutex;

use super::sync_secret_store::SystemSyncSecretStore;
use crate::platform::database::DesktopSqliteState;

pub const SYNC_CONFIG_FILE: &str = "sync.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCreateRequest {
    pub provider: SyncProviderInput,
    #[serde(default)]
    pub vault_id: Option<String>,
    pub preset: SyncPresetV1,
    pub master_password: String,
    pub create_recovery_key: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewJoinRequest {
    pub provider: SyncProviderInput,
    pub vault_id: String,
    pub master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncJoinRequest {
    pub provider: SyncProviderInput,
    pub vault_id: String,
    pub master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncUnlockRequest {
    pub provider_password: String,
    pub master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncUnlockRecoveryRequest {
    pub provider_password: String,
    pub recovery_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCreateResult {
    pub vault_id: String,
    pub device_id: String,
    pub recovery_key: Option<String>,
    pub status: SyncStatusSnapshot,
}

impl From<ApplicationCreateResult> for SyncCreateResult {
    fn from(value: ApplicationCreateResult) -> Self {
        Self {
            vault_id: value.vault_id,
            device_id: value.device_id,
            recovery_key: value.recovery_key,
            status: value.status,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncChangePasswordRequest {
    pub current_master_password: String,
    pub next_master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRemoteBackupListResult {
    pub entries: Vec<LegacyRemoteBackupEntry>,
    pub credentials_migrated: bool,
}

/// Desktop synchronization manager completely decoupled from Tauri runtime.
#[derive(Clone)]
pub struct DesktopSyncManager {
    inner: Arc<DesktopSyncManagerInner>,
}

struct DesktopSyncManagerInner {
    config_path: std::sync::RwLock<PathBuf>,
    sqlite: std::sync::RwLock<Option<DesktopSqliteState>>,
    application: Mutex<Option<Arc<SyncApplication>>>,
}

impl Default for DesktopSyncManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(DesktopSyncManagerInner {
                config_path: std::sync::RwLock::new(PathBuf::new()),
                sqlite: std::sync::RwLock::new(None),
                application: Mutex::new(None),
            }),
        }
    }
}

impl DesktopSyncManager {
    pub fn new(config_path: PathBuf, sqlite: DesktopSqliteState) -> Self {
        Self {
            inner: Arc::new(DesktopSyncManagerInner {
                config_path: std::sync::RwLock::new(config_path),
                sqlite: std::sync::RwLock::new(Some(sqlite)),
                application: Mutex::new(None),
            }),
        }
    }

    pub fn configure(&self, config_path: PathBuf, sqlite: DesktopSqliteState) {
        if let Ok(mut path_guard) = self.inner.config_path.write() {
            *path_guard = config_path;
        }
        if let Ok(mut sqlite_guard) = self.inner.sqlite.write() {
            *sqlite_guard = Some(sqlite);
        }
    }

    pub async fn reset(&self) {
        *self.inner.application.lock().await = None;
    }

    async fn application(&self) -> Result<Arc<SyncApplication>, String> {
        let mut shared = self.inner.application.lock().await;
        if let Some(application) = shared.as_ref() {
            return Ok(Arc::clone(application));
        }
        let sqlite = self
            .inner
            .sqlite
            .read()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or_else(|| "SQLite context not configured for DesktopSyncManager".to_string())?;
        let config_path = self
            .inner
            .config_path
            .read()
            .map_err(|e| e.to_string())?
            .clone();
        let context = sqlite.current_context()?;
        let application = Arc::new(SyncApplication::new(
            Arc::new(JsonFileSyncConfigStore::new(config_path)),
            Arc::new(context.sync_repository_factory(Arc::new(SystemClock))),
            SyncProviderRegistry::new([
                Arc::new(WebDavSyncProviderFactory) as Arc<dyn SyncProviderFactory>,
                Arc::new(S3SyncProviderFactory) as Arc<dyn SyncProviderFactory>,
            ]),
            Arc::new(SystemSyncSecretStore),
            Arc::new(SystemSyncApplicationEnvironment),
        ));
        *shared = Some(Arc::clone(&application));
        Ok(application)
    }

    pub async fn get_status(&self) -> Result<SyncStatusSnapshot, String> {
        self.application().await?.status().await.map_err(sync_error)
    }

    pub async fn test_provider(
        &self,
        provider: SyncProviderInput,
    ) -> Result<SyncProviderDescriptor, String> {
        self.application()
            .await?
            .test_provider(provider)
            .await
            .map_err(sync_error)
    }

    pub async fn create_vault(
        &self,
        request: SyncCreateRequest,
    ) -> Result<SyncCreateResult, String> {
        self.application()
            .await?
            .create_with_vault_id(
                request.provider,
                request.vault_id,
                request.preset,
                &request.master_password,
                request.create_recovery_key,
            )
            .await
            .map(Into::into)
            .map_err(sync_error)
    }

    pub async fn discover_vaults(
        &self,
        provider: SyncProviderInput,
    ) -> Result<Vec<DiscoveredVaultSummary>, String> {
        self.application()
            .await?
            .discover_vaults(provider)
            .await
            .map_err(sync_error)
    }

    pub async fn get_pairing_info(&self) -> Result<Option<SyncPairingInfo>, String> {
        self.application()
            .await?
            .get_pairing_info()
            .map_err(sync_error)
    }

    pub async fn preview_join(
        &self,
        request: SyncPreviewJoinRequest,
    ) -> Result<SyncJoinPreview, String> {
        self.application()
            .await?
            .preview_join(
                request.provider,
                &request.vault_id,
                &request.master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn join_vault(&self, request: SyncJoinRequest) -> Result<SyncRunResult, String> {
        self.application()
            .await?
            .join(
                request.provider,
                &request.vault_id,
                &request.master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn unlock(&self, request: SyncUnlockRequest) -> Result<SyncStatusSnapshot, String> {
        self.application()
            .await?
            .unlock_with_password(
                request.provider_password.into_bytes(),
                &request.master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn unlock_with_recovery(
        &self,
        request: SyncUnlockRecoveryRequest,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application()
            .await?
            .unlock_with_recovery_key(
                request.provider_password.into_bytes(),
                &request.recovery_key,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn lock(&self) -> Result<SyncStatusSnapshot, String> {
        self.application().await?.lock().await.map_err(sync_error)
    }

    pub async fn set_paused(&self, paused: bool) -> Result<SyncStatusSnapshot, String> {
        self.application()
            .await?
            .set_paused(paused)
            .await
            .map_err(sync_error)
    }

    pub async fn disconnect(&self) -> Result<SyncStatusSnapshot, String> {
        self.application()
            .await?
            .disconnect()
            .await
            .map_err(sync_error)
    }

    pub async fn run_now(&self) -> Result<SyncRunResult, String> {
        self.application().await?.run().await.map_err(sync_error)
    }

    pub async fn change_preset(
        &self,
        preset: SyncPresetV1,
        confirm_shrink: bool,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application()
            .await?
            .change_preset(preset, confirm_shrink)
            .await
            .map_err(sync_error)
    }

    pub async fn change_master_password(
        &self,
        request: SyncChangePasswordRequest,
    ) -> Result<(), String> {
        self.application()
            .await?
            .change_master_password(
                &request.current_master_password,
                &request.next_master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn generate_recovery_key(&self) -> Result<String, String> {
        self.application()
            .await?
            .generate_recovery_key()
            .await
            .map_err(sync_error)
    }

    pub async fn list_conflicts(&self) -> Result<Vec<SyncConflictSummary>, String> {
        self.application()
            .await?
            .list_conflicts()
            .map_err(sync_error)
    }

    pub async fn get_conflict(
        &self,
        conflict_id: &str,
    ) -> Result<Option<SyncConflictDetail>, String> {
        self.application()
            .await?
            .get_conflict(conflict_id)
            .map_err(sync_error)
    }

    pub async fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: SyncConflictResolution,
    ) -> Result<(), String> {
        self.application()
            .await?
            .resolve_conflict(conflict_id, resolution)
            .map_err(sync_error)
    }

    pub async fn list_legacy_backups(
        config: WebDavObjectStoreConfig,
    ) -> Result<LegacyRemoteBackupListResult, String> {
        let (config, password_from_store) = resolve_legacy_provider_config(config).await?;
        let credential_config = config.clone();
        let store = WebDavObjectStore::new(config).map_err(sync_error)?;
        let entries = LegacyRemoteBackupService::new(&store)
            .list()
            .await
            .map_err(sync_error)?;
        let credentials_migrated = if password_from_store {
            true
        } else {
            match persist_legacy_provider_password(&credential_config).await {
                Ok(()) => true,
                Err(error) => {
                    log::warn!(
                        "failed to migrate legacy WebDAV password to the system credential store: {error}"
                    );
                    false
                }
            }
        };
        Ok(LegacyRemoteBackupListResult {
            entries,
            credentials_migrated,
        })
    }

    pub async fn download_legacy_backup(
        config: WebDavObjectStoreConfig,
        key: String,
    ) -> Result<Vec<u8>, String> {
        let (config, _) = resolve_legacy_provider_config(config).await?;
        let store = WebDavObjectStore::new(config).map_err(sync_error)?;
        let key = SyncObjectKey::parse(key).map_err(sync_error)?;
        LegacyRemoteBackupService::new(&store)
            .download(&key)
            .await
            .map_err(sync_error)
    }
}

pub(crate) fn webdav_provider_input(
    config: WebDavObjectStoreConfig,
) -> Result<SyncProviderInput, String> {
    Ok(SyncProviderInput {
        provider_id: "webdav".to_string(),
        configuration: serde_json::to_value(config).map_err(sync_error)?,
    })
}

fn legacy_provider_password_secret_key(config: &WebDavObjectStoreConfig) -> String {
    legacy_provider_credential_key(
        "webdav",
        config.server_url.trim(),
        config.remote_root.trim(),
        config.username.trim(),
    )
}

async fn resolve_legacy_provider_config(
    mut config: WebDavObjectStoreConfig,
) -> Result<(WebDavObjectStoreConfig, bool), String> {
    if !config.password.is_empty() {
        return Ok((config, false));
    }
    let key = legacy_provider_password_secret_key(&config);
    let password = SystemSyncSecretStore
        .read_secret(&key)
        .await
        .map_err(sync_error)?
        .ok_or_else(|| "WebDAV password is required.".to_string())?;
    config.password = String::from_utf8(password)
        .map_err(|_| "Stored WebDAV password is not valid UTF-8.".to_string())?;
    Ok((config, true))
}

async fn persist_legacy_provider_password(
    config: &WebDavObjectStoreConfig,
) -> Result<(), sona_core::sync::SyncError> {
    SystemSyncSecretStore
        .write_secret(
            &legacy_provider_password_secret_key(config),
            config.password.as_bytes(),
        )
        .await
}

fn sync_error(error: impl ToString) -> String {
    let message = error.to_string();
    match message.as_str() {
        "Sync provider does not support required conditional object operations." => {
            "WebDAV server does not support required conditional operations.".to_string()
        }
        "Sync connection metadata exists but local sync state is missing." => {
            "Sync connection metadata exists but SQLite sync state is missing.".to_string()
        }
        "Local sync state is missing." => "SQLite sync state is missing.".to_string(),
        _ => message
            .strip_prefix("Sync configuration error: ")
            .map_or(message.clone(), str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_application_errors_keep_the_existing_desktop_text_contract() {
        assert_eq!(
            sync_error(sona_sync::SyncApplicationError::InvalidState(
                "Sync provider does not support required conditional object operations."
                    .to_string(),
            )),
            "WebDAV server does not support required conditional operations."
        );
        assert_eq!(
            sync_error(sona_sync::SyncApplicationError::InvalidState(
                "Sync connection metadata exists but local sync state is missing.".to_string(),
            )),
            "Sync connection metadata exists but SQLite sync state is missing."
        );
        assert_eq!(
            sync_error(sona_sync::SyncApplicationError::InvalidState(
                "Local sync state is missing.".to_string(),
            )),
            "SQLite sync state is missing."
        );
        assert_eq!(
            sync_error(sona_sync::SyncApplicationError::Config(
                "invalid sync.json".to_string(),
            )),
            "invalid sync.json"
        );
    }

    #[tokio::test]
    async fn test_desktop_sync_manager_without_tauri() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let sqlite = DesktopSqliteState::new(ctx);
        let config_path = temp.path().join("sync.json");
        let manager = DesktopSyncManager::new(config_path, sqlite);

        let status = manager.get_status().await.unwrap();
        assert_eq!(status.state, sona_core::sync::SyncLifecycleState::Disabled);
    }
}
