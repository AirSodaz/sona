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
    JsonFileSyncConfigStore, LegacyRemoteBackupEntry, LegacyRemoteBackupService, SyncApplication,
    SyncCreateResult as ApplicationCreateResult, SyncProviderFactory, SyncProviderInput,
    SyncProviderRegistry, SystemSyncApplicationEnvironment, legacy_provider_credential_key,
};
use sona_sync_webdav::{WebDavObjectStore, WebDavObjectStoreConfig, WebDavSyncProviderFactory};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use super::sync_secret_store::SystemSyncSecretStore;

const SYNC_CONFIG_FILE: &str = "sync.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCreateRequest {
    pub provider: SyncProviderInput,
    pub preset: SyncPresetV1,
    pub master_password: String,
    pub create_recovery_key: bool,
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

#[derive(Default)]
pub struct DesktopSyncManager {
    application: Mutex<Option<Arc<SyncApplication>>>,
}

impl DesktopSyncManager {
    async fn application<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Arc<SyncApplication>, String> {
        let mut shared = self.application.lock().await;
        if let Some(application) = shared.as_ref() {
            return Ok(Arc::clone(application));
        }
        let application = Arc::new(SyncApplication::new(
            Arc::new(JsonFileSyncConfigStore::new(config_path(app)?)),
            Arc::new(
                crate::platform::database::sqlite_application_context(app)
                    .sync_repository_factory(Arc::new(SystemClock)),
            ),
            SyncProviderRegistry::new([
                Arc::new(WebDavSyncProviderFactory) as Arc<dyn SyncProviderFactory>
            ]),
            Arc::new(SystemSyncSecretStore),
            Arc::new(SystemSyncApplicationEnvironment),
        ));
        *shared = Some(Arc::clone(&application));
        Ok(application)
    }

    pub async fn get_status<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application(app)
            .await?
            .status()
            .await
            .map_err(sync_error)
    }

    pub async fn test_provider<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        provider: SyncProviderInput,
    ) -> Result<SyncProviderDescriptor, String> {
        self.application(app)
            .await?
            .test_provider(provider)
            .await
            .map_err(sync_error)
    }

    pub async fn create_vault<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncCreateRequest,
    ) -> Result<SyncCreateResult, String> {
        self.application(app)
            .await?
            .create(
                request.provider,
                request.preset,
                &request.master_password,
                request.create_recovery_key,
            )
            .await
            .map(Into::into)
            .map_err(sync_error)
    }

    pub async fn preview_join<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncPreviewJoinRequest,
    ) -> Result<SyncJoinPreview, String> {
        self.application(app)
            .await?
            .preview_join(
                request.provider,
                &request.vault_id,
                &request.master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn join_vault<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncJoinRequest,
    ) -> Result<SyncRunResult, String> {
        self.application(app)
            .await?
            .join(
                request.provider,
                &request.vault_id,
                &request.master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn unlock<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncUnlockRequest,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application(app)
            .await?
            .unlock_with_password(
                request.provider_password.into_bytes(),
                &request.master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn unlock_with_recovery<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncUnlockRecoveryRequest,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application(app)
            .await?
            .unlock_with_recovery_key(
                request.provider_password.into_bytes(),
                &request.recovery_key,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn lock<R: Runtime>(&self, app: &AppHandle<R>) -> Result<SyncStatusSnapshot, String> {
        self.application(app)
            .await?
            .lock()
            .await
            .map_err(sync_error)
    }

    pub async fn set_paused<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        paused: bool,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application(app)
            .await?
            .set_paused(paused)
            .await
            .map_err(sync_error)
    }

    pub async fn disconnect<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application(app)
            .await?
            .disconnect()
            .await
            .map_err(sync_error)
    }

    pub async fn run_now<R: Runtime>(&self, app: &AppHandle<R>) -> Result<SyncRunResult, String> {
        self.application(app).await?.run().await.map_err(sync_error)
    }

    pub async fn change_preset<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        preset: SyncPresetV1,
        confirm_shrink: bool,
    ) -> Result<SyncStatusSnapshot, String> {
        self.application(app)
            .await?
            .change_preset(preset, confirm_shrink)
            .await
            .map_err(sync_error)
    }

    pub async fn change_master_password<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncChangePasswordRequest,
    ) -> Result<(), String> {
        self.application(app)
            .await?
            .change_master_password(
                &request.current_master_password,
                &request.next_master_password,
            )
            .await
            .map_err(sync_error)
    }

    pub async fn generate_recovery_key<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<String, String> {
        self.application(app)
            .await?
            .generate_recovery_key()
            .await
            .map_err(sync_error)
    }

    pub async fn list_conflicts<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Vec<SyncConflictSummary>, String> {
        self.application(app)
            .await?
            .list_conflicts()
            .map_err(sync_error)
    }

    pub async fn get_conflict<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        conflict_id: &str,
    ) -> Result<Option<SyncConflictDetail>, String> {
        self.application(app)
            .await?
            .get_conflict(conflict_id)
            .map_err(sync_error)
    }

    pub async fn resolve_conflict<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        conflict_id: &str,
        resolution: SyncConflictResolution,
    ) -> Result<(), String> {
        self.application(app)
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

fn config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join(SYNC_CONFIG_FILE))
        .map_err(sync_error)
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
}
