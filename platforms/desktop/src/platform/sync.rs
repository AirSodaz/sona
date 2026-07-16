use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use sona_core::sync::{
    SyncConflictDetail, SyncConflictResolution, SyncConflictSummary, SyncErrorSnapshot,
    SyncJoinPreview, SyncLifecycleState, SyncLocalRepository, SyncObjectKey, SyncObjectStore,
    SyncPresetV1, SyncProviderDescriptor, SyncRunResult, SyncSecretStore, SyncStatusSnapshot,
};
use sona_sqlite::SqliteSyncRepository;
use sona_sync::{
    LegacyRemoteBackupEntry, LegacyRemoteBackupService, OpenedRemoteVault, SyncBackoffPolicy,
    SyncRuntime, change_remote_master_password, create_remote_vault,
    legacy_provider_credential_key, load_remote_state_for_join, open_remote_vault_with_password,
    open_remote_vault_with_recovery_key, open_remote_vault_with_vault_key,
    regenerate_remote_recovery_key, update_remote_vault_preset,
};
use sona_sync_webdav::{WebDavObjectStore, WebDavObjectStoreConfig};
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

use super::sync_secret_store::SystemSyncSecretStore;

const SYNC_CONFIG_FILE: &str = "sync.json";
const PROVIDER_PASSWORD_SECRET_PREFIX: &str = "webdav-password";
const VAULT_KEY_SECRET_PREFIX: &str = "vault-key";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCreateRequest {
    pub provider: WebDavObjectStoreConfig,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreviewJoinRequest {
    pub provider: WebDavObjectStoreConfig,
    pub vault_id: String,
    pub master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncJoinRequest {
    pub provider: WebDavObjectStoreConfig,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedWebDavConfig {
    server_url: String,
    remote_root: String,
    username: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSyncConfig {
    provider_id: String,
    vault_id: String,
    device_id: String,
    preset: SyncPresetV1,
    webdav: PersistedWebDavConfig,
    #[serde(default)]
    paused: bool,
    last_success_at_ms: Option<u64>,
    consecutive_failures: u32,
    next_retry_at_ms: Option<u64>,
    last_error: Option<SyncErrorSnapshot>,
}

struct UnlockedSession {
    store: WebDavObjectStore,
    opened: OpenedRemoteVault,
}

struct SyncRunGuard<'a>(&'a AtomicBool);

impl Drop for SyncRunGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub struct DesktopSyncManager {
    session: Mutex<Option<UnlockedSession>>,
    syncing: AtomicBool,
    manually_locked: AtomicBool,
    secret_store: SystemSyncSecretStore,
}

impl DesktopSyncManager {
    pub async fn test_webdav_provider(
        config: WebDavObjectStoreConfig,
    ) -> Result<SyncProviderDescriptor, String> {
        let store = WebDavObjectStore::new(config).map_err(sync_error)?;
        let capabilities = store.probe().await.map_err(sync_error)?;
        if !capabilities.conditional_create
            || !capabilities.compare_and_swap
            || !capabilities.delete
        {
            return Err("WebDAV server does not support required conditional operations.".into());
        }
        Ok(SyncProviderDescriptor {
            id: "webdav".to_string(),
            display_name: "WebDAV".to_string(),
        })
    }

    pub async fn list_legacy_backups(
        config: WebDavObjectStoreConfig,
    ) -> Result<LegacyRemoteBackupListResult, String> {
        let (config, password_from_store) = resolve_legacy_provider_config(config)?;
        let credential_config = config.clone();
        let store = WebDavObjectStore::new(config).map_err(sync_error)?;
        let entries = LegacyRemoteBackupService::new(&store)
            .list()
            .await
            .map_err(sync_error)?;
        let credentials_migrated = if password_from_store {
            true
        } else {
            match persist_legacy_provider_password(&credential_config) {
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
        let (config, _) = resolve_legacy_provider_config(config)?;
        let store = WebDavObjectStore::new(config).map_err(sync_error)?;
        let key = SyncObjectKey::parse(key).map_err(sync_error)?;
        LegacyRemoteBackupService::new(&store)
            .download(&key)
            .await
            .map_err(sync_error)
    }

    pub async fn get_status<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<SyncStatusSnapshot, String> {
        let Some(config) = load_config(app)? else {
            return Ok(disabled_status());
        };
        if !self.manually_locked.load(Ordering::SeqCst)
            && let Err(error) = self.restore_session_from_secret_store(&config).await
        {
            log::warn!("failed to restore sync session from the system credential store: {error}");
        }
        let repository = repository(app)?.ok_or_else(|| {
            "Sync connection metadata exists but SQLite sync state is missing.".to_string()
        })?;
        let runtime = repository.load_runtime_state().map_err(sync_error)?;
        let unlocked = self.session.lock().await.is_some();
        let state = if self.syncing.load(Ordering::SeqCst) {
            SyncLifecycleState::Syncing
        } else if repository.is_paused().map_err(sync_error)? {
            SyncLifecycleState::Paused
        } else if !unlocked {
            SyncLifecycleState::Locked
        } else if config.last_error.is_some() {
            SyncLifecycleState::Error
        } else {
            SyncLifecycleState::Idle
        };
        Ok(SyncStatusSnapshot {
            state,
            provider_id: Some(config.provider_id),
            vault_id: Some(config.vault_id),
            preset: Some(runtime.preset),
            last_success_at_ms: config.last_success_at_ms,
            pending_operation_count: repository.pending_operation_count().map_err(sync_error)?,
            conflict_count: repository.unresolved_conflict_count().map_err(sync_error)?,
            next_retry_at_ms: config.next_retry_at_ms,
            last_error: config.last_error,
        })
    }

    pub async fn create_vault<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncCreateRequest,
    ) -> Result<SyncCreateResult, String> {
        if load_config(app)?.is_some() || repository(app)?.is_some() {
            return Err("This Sona data directory is already connected to a sync vault.".into());
        }
        let store = WebDavObjectStore::new(request.provider.clone()).map_err(sync_error)?;
        store.probe().await.map_err(sync_error)?;
        let vault_id = uuid::Uuid::new_v4().to_string();
        let device_id = uuid::Uuid::new_v4().to_string();
        let created = create_remote_vault(
            &store,
            &vault_id,
            request.preset,
            &request.master_password,
            request.create_recovery_key,
        )
        .await
        .map_err(sync_error)?;
        SqliteSyncRepository::initialize(
            crate::platform::database::sqlite_database(app),
            &vault_id,
            &device_id,
            request.preset,
        )
        .map_err(sync_error)?;
        let config = persisted_config(
            &request.provider,
            vault_id.clone(),
            device_id.clone(),
            request.preset,
        );
        save_config(app, &config)?;
        if let Err(error) = self.persist_session_secrets(
            &vault_id,
            request.provider.password.as_bytes(),
            created.opened.vault_key.as_slice(),
        ) {
            log::warn!("failed to persist sync secrets in the system credential store: {error}");
        }
        self.manually_locked.store(false, Ordering::SeqCst);
        *self.session.lock().await = Some(UnlockedSession {
            store,
            opened: created.opened,
        });
        let recovery_key = created.recovery_key;
        let _ = self.run_now(app).await;
        Ok(SyncCreateResult {
            vault_id,
            device_id,
            recovery_key,
            status: self.get_status(app).await?,
        })
    }

    pub async fn preview_join<R: Runtime>(
        app: &AppHandle<R>,
        request: SyncPreviewJoinRequest,
    ) -> Result<SyncJoinPreview, String> {
        if load_config(app)?.is_some() || repository(app)?.is_some() {
            return Err("This Sona data directory is already connected to a sync vault.".into());
        }
        let store = WebDavObjectStore::new(request.provider).map_err(sync_error)?;
        let opened =
            open_remote_vault_with_password(&store, &request.vault_id, &request.master_password)
                .await
                .map_err(sync_error)?;
        let remote_segments =
            load_remote_state_for_join(&store, &request.vault_id, opened.vault_key.as_slice())
                .await
                .map_err(sync_error)?;
        SqliteSyncRepository::preview_join(
            crate::platform::database::sqlite_database(app),
            &request.vault_id,
            &format!("preview-{}", uuid::Uuid::new_v4()),
            opened.header.preset,
            &remote_segments,
        )
        .map_err(sync_error)
    }

    pub async fn join_vault<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncJoinRequest,
    ) -> Result<SyncRunResult, String> {
        if load_config(app)?.is_some() || repository(app)?.is_some() {
            return Err("This Sona data directory is already connected to a sync vault.".into());
        }
        let store = WebDavObjectStore::new(request.provider.clone()).map_err(sync_error)?;
        let opened =
            open_remote_vault_with_password(&store, &request.vault_id, &request.master_password)
                .await
                .map_err(sync_error)?;
        let device_id = uuid::Uuid::new_v4().to_string();
        SqliteSyncRepository::initialize(
            crate::platform::database::sqlite_database(app),
            &request.vault_id,
            &device_id,
            opened.header.preset,
        )
        .map_err(sync_error)?;
        save_config(
            app,
            &persisted_config(
                &request.provider,
                request.vault_id.clone(),
                device_id,
                opened.header.preset,
            ),
        )?;
        if let Err(error) = self.persist_session_secrets(
            &request.vault_id,
            request.provider.password.as_bytes(),
            opened.vault_key.as_slice(),
        ) {
            log::warn!("failed to persist sync secrets in the system credential store: {error}");
        }
        self.manually_locked.store(false, Ordering::SeqCst);
        *self.session.lock().await = Some(UnlockedSession { store, opened });
        self.run_now(app).await
    }

    pub async fn unlock<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncUnlockRequest,
    ) -> Result<SyncStatusSnapshot, String> {
        let config = load_config(app)?.ok_or_else(|| "Sync is not configured.".to_string())?;
        let provider = runtime_provider_config(&config, &request.provider_password)?;
        let store = WebDavObjectStore::new(provider).map_err(sync_error)?;
        let opened =
            open_remote_vault_with_password(&store, &config.vault_id, &request.master_password)
                .await
                .map_err(sync_error)?;
        if let Err(error) = self.persist_session_secrets(
            &config.vault_id,
            request.provider_password.as_bytes(),
            opened.vault_key.as_slice(),
        ) {
            log::warn!("failed to persist sync secrets in the system credential store: {error}");
        }
        self.manually_locked.store(false, Ordering::SeqCst);
        *self.session.lock().await = Some(UnlockedSession { store, opened });
        self.get_status(app).await
    }

    pub async fn unlock_with_recovery<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        request: SyncUnlockRecoveryRequest,
    ) -> Result<SyncStatusSnapshot, String> {
        let config = load_config(app)?.ok_or_else(|| "Sync is not configured.".to_string())?;
        let provider = runtime_provider_config(&config, &request.provider_password)?;
        let store = WebDavObjectStore::new(provider).map_err(sync_error)?;
        let opened =
            open_remote_vault_with_recovery_key(&store, &config.vault_id, &request.recovery_key)
                .await
                .map_err(sync_error)?;
        if let Err(error) = self.persist_session_secrets(
            &config.vault_id,
            request.provider_password.as_bytes(),
            opened.vault_key.as_slice(),
        ) {
            log::warn!("failed to persist sync secrets in the system credential store: {error}");
        }
        self.manually_locked.store(false, Ordering::SeqCst);
        *self.session.lock().await = Some(UnlockedSession { store, opened });
        self.get_status(app).await
    }

    pub async fn lock<R: Runtime>(&self, app: &AppHandle<R>) -> Result<SyncStatusSnapshot, String> {
        self.manually_locked.store(true, Ordering::SeqCst);
        *self.session.lock().await = None;
        self.get_status(app).await
    }

    pub async fn set_paused<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        paused: bool,
    ) -> Result<SyncStatusSnapshot, String> {
        let mut config = load_config(app)?.ok_or_else(|| "Sync is not configured.".to_string())?;
        config.paused = paused;
        repository(app)?
            .ok_or_else(|| "SQLite sync state is missing.".to_string())?
            .set_paused(paused)
            .map_err(sync_error)?;
        save_config(app, &config)?;
        self.get_status(app).await
    }

    pub async fn disconnect<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<SyncStatusSnapshot, String> {
        if let Some(config) = load_config(app)? {
            self.delete_session_secrets(&config.vault_id)
                .map_err(sync_error)?;
        }
        if let Some(repository) = repository(app)? {
            repository.disconnect().map_err(sync_error)?;
        }
        *self.session.lock().await = None;
        self.manually_locked.store(false, Ordering::SeqCst);
        let path = config_path(app)?;
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        Ok(disabled_status())
    }

    pub async fn run_now<R: Runtime>(&self, app: &AppHandle<R>) -> Result<SyncRunResult, String> {
        let mut config = load_config(app)?.ok_or_else(|| "Sync is not configured.".to_string())?;
        if repository(app)?
            .ok_or_else(|| "SQLite sync state is missing.".to_string())?
            .is_paused()
            .map_err(sync_error)?
        {
            return Err("Sync is paused.".to_string());
        }
        let repository =
            repository(app)?.ok_or_else(|| "SQLite sync state is missing.".to_string())?;
        self.restore_session_from_secret_store(&config).await?;
        self.syncing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "A sync run is already in progress.".to_string())?;
        let _run_guard = SyncRunGuard(&self.syncing);
        let session = self.session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| "Sync vault is locked.".to_string())?;
        let now_ms = now_ms();
        let result = SyncRuntime::new(
            &repository,
            &session.store,
            session.opened.vault_key.as_slice(),
        )
        .run_at(now_ms)
        .await;
        match result {
            Ok(result) => {
                config.last_success_at_ms = Some(now_ms);
                config.consecutive_failures = 0;
                config.next_retry_at_ms = None;
                config.last_error = None;
                save_config(app, &config)?;
                Ok(result)
            }
            Err(error) => {
                config.consecutive_failures = config.consecutive_failures.saturating_add(1);
                let jitter = (uuid::Uuid::new_v4().as_u128() % 1_000_001) as u32;
                config.next_retry_at_ms = Some(SyncBackoffPolicy::default().next_retry_at_ms(
                    now_ms,
                    config.consecutive_failures,
                    jitter,
                ));
                config.last_error = Some(SyncErrorSnapshot {
                    code: sync_error_code(&error).to_string(),
                    message: error.to_string(),
                    retryable: is_retryable(&error),
                });
                save_config(app, &config)?;
                Err(error.to_string())
            }
        }
    }

    pub async fn change_preset<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        preset: SyncPresetV1,
        confirm_shrink: bool,
    ) -> Result<SyncStatusSnapshot, String> {
        let repository = repository(app)?.ok_or_else(|| "Sync is not configured.".to_string())?;
        repository
            .validate_preset_change(preset, confirm_shrink)
            .map_err(sync_error)?;
        {
            let mut session = self.session.lock().await;
            let session = session
                .as_mut()
                .ok_or_else(|| "Sync vault is locked.".to_string())?;
            let previous = session.opened.header.preset;
            update_remote_vault_preset(&session.store, &mut session.opened, preset)
                .await
                .map_err(sync_error)?;
            if let Err(error) = repository.change_preset(preset, confirm_shrink) {
                let rollback =
                    update_remote_vault_preset(&session.store, &mut session.opened, previous).await;
                return match rollback {
                    Ok(()) => Err(sync_error(error)),
                    Err(rollback_error) => Err(format!(
                        "Local preset update failed ({error}); remote rollback also failed ({rollback_error})."
                    )),
                };
            }
        }
        let mut config = load_config(app)?.ok_or_else(|| "Sync is not configured.".to_string())?;
        config.preset = preset;
        save_config(app, &config)?;
        self.get_status(app).await
    }

    pub async fn change_master_password<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
        request: SyncChangePasswordRequest,
    ) -> Result<(), String> {
        let mut session = self.session.lock().await;
        let session = session
            .as_mut()
            .ok_or_else(|| "Sync vault is locked.".to_string())?;
        change_remote_master_password(
            &session.store,
            &mut session.opened,
            &request.current_master_password,
            &request.next_master_password,
        )
        .await
        .map_err(sync_error)
    }

    pub async fn generate_recovery_key<R: Runtime>(
        &self,
        _app: &AppHandle<R>,
    ) -> Result<String, String> {
        let mut session = self.session.lock().await;
        let session = session
            .as_mut()
            .ok_or_else(|| "Sync vault is locked.".to_string())?;
        regenerate_remote_recovery_key(&session.store, &mut session.opened)
            .await
            .map_err(sync_error)
    }

    async fn restore_session_from_secret_store(
        &self,
        config: &PersistedSyncConfig,
    ) -> Result<(), String> {
        if self.manually_locked.load(Ordering::SeqCst) {
            return Ok(());
        }
        let mut session = self.session.lock().await;
        if session.is_some() {
            return Ok(());
        }
        let Some(provider_password) = self
            .secret_store
            .read_secret(&provider_password_secret_key(&config.vault_id))
            .map_err(sync_error)?
        else {
            return Ok(());
        };
        let Some(vault_key) = self
            .secret_store
            .read_secret(&vault_key_secret_key(&config.vault_id))
            .map_err(sync_error)?
        else {
            return Ok(());
        };
        let provider_password = String::from_utf8(provider_password)
            .map_err(|_| "Stored WebDAV password is not valid UTF-8.".to_string())?;
        let provider = runtime_provider_config(config, &provider_password)?;
        let store = WebDavObjectStore::new(provider).map_err(sync_error)?;
        let opened = open_remote_vault_with_vault_key(&store, &config.vault_id, &vault_key)
            .await
            .map_err(sync_error)?;
        *session = Some(UnlockedSession { store, opened });
        Ok(())
    }

    fn persist_session_secrets(
        &self,
        vault_id: &str,
        provider_password: &[u8],
        vault_key: &[u8],
    ) -> Result<(), sona_core::sync::SyncError> {
        let provider_key = provider_password_secret_key(vault_id);
        self.secret_store
            .write_secret(&provider_key, provider_password)?;
        if let Err(error) = self
            .secret_store
            .write_secret(&vault_key_secret_key(vault_id), vault_key)
        {
            let _ = self.secret_store.delete_secret(&provider_key);
            return Err(error);
        }
        Ok(())
    }

    fn delete_session_secrets(&self, vault_id: &str) -> Result<(), sona_core::sync::SyncError> {
        self.secret_store
            .delete_secret(&provider_password_secret_key(vault_id))?;
        self.secret_store
            .delete_secret(&vault_key_secret_key(vault_id))
    }

    pub fn list_conflicts<R: Runtime>(
        app: &AppHandle<R>,
    ) -> Result<Vec<SyncConflictSummary>, String> {
        repository(app)?
            .ok_or_else(|| "Sync is not configured.".to_string())?
            .list_conflict_summaries()
            .map_err(sync_error)
    }

    pub fn get_conflict<R: Runtime>(
        app: &AppHandle<R>,
        conflict_id: &str,
    ) -> Result<Option<SyncConflictDetail>, String> {
        repository(app)?
            .ok_or_else(|| "Sync is not configured.".to_string())?
            .get_conflict_detail(conflict_id)
            .map_err(sync_error)
    }

    pub fn resolve_conflict<R: Runtime>(
        app: &AppHandle<R>,
        conflict_id: &str,
        resolution: SyncConflictResolution,
    ) -> Result<(), String> {
        repository(app)?
            .ok_or_else(|| "Sync is not configured.".to_string())?
            .resolve_conflict(conflict_id, resolution, now_ms())
            .map_err(sync_error)
    }
}

fn repository<R: Runtime>(app: &AppHandle<R>) -> Result<Option<SqliteSyncRepository>, String> {
    SqliteSyncRepository::open_existing(crate::platform::database::sqlite_database(app))
        .map_err(sync_error)
}

fn provider_password_secret_key(vault_id: &str) -> String {
    format!("{PROVIDER_PASSWORD_SECRET_PREFIX}:{vault_id}")
}

fn vault_key_secret_key(vault_id: &str) -> String {
    format!("{VAULT_KEY_SECRET_PREFIX}:{vault_id}")
}

fn legacy_provider_password_secret_key(config: &WebDavObjectStoreConfig) -> String {
    legacy_provider_credential_key(
        "webdav",
        config.server_url.trim(),
        config.remote_root.trim(),
        config.username.trim(),
    )
}

fn resolve_legacy_provider_config(
    mut config: WebDavObjectStoreConfig,
) -> Result<(WebDavObjectStoreConfig, bool), String> {
    if !config.password.is_empty() {
        return Ok((config, false));
    }
    let key = legacy_provider_password_secret_key(&config);
    let password = SystemSyncSecretStore
        .read_secret(&key)
        .map_err(sync_error)?
        .ok_or_else(|| "WebDAV password is required.".to_string())?;
    config.password = String::from_utf8(password)
        .map_err(|_| "Stored WebDAV password is not valid UTF-8.".to_string())?;
    Ok((config, true))
}

fn persist_legacy_provider_password(
    config: &WebDavObjectStoreConfig,
) -> Result<(), sona_core::sync::SyncError> {
    SystemSyncSecretStore.write_secret(
        &legacy_provider_password_secret_key(config),
        config.password.as_bytes(),
    )
}

fn persisted_config(
    provider: &WebDavObjectStoreConfig,
    vault_id: String,
    device_id: String,
    preset: SyncPresetV1,
) -> PersistedSyncConfig {
    PersistedSyncConfig {
        provider_id: "webdav".to_string(),
        vault_id,
        device_id,
        preset,
        webdav: PersistedWebDavConfig {
            server_url: provider.server_url.clone(),
            remote_root: provider.remote_root.clone(),
            username: provider.username.clone(),
        },
        paused: false,
        last_success_at_ms: None,
        consecutive_failures: 0,
        next_retry_at_ms: None,
        last_error: None,
    }
}

fn runtime_provider_config(
    config: &PersistedSyncConfig,
    password: &str,
) -> Result<WebDavObjectStoreConfig, String> {
    WebDavObjectStoreConfig::new(
        &config.webdav.server_url,
        &config.webdav.remote_root,
        &config.webdav.username,
        password,
    )
    .map_err(sync_error)
}

fn load_config<R: Runtime>(app: &AppHandle<R>) -> Result<Option<PersistedSyncConfig>, String> {
    let path = config_path(app)?;
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn save_config<R: Runtime>(app: &AppHandle<R>, config: &PersistedSyncConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

fn config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join(SYNC_CONFIG_FILE))
        .map_err(|error| error.to_string())
}

fn disabled_status() -> SyncStatusSnapshot {
    SyncStatusSnapshot {
        state: SyncLifecycleState::Disabled,
        provider_id: None,
        vault_id: None,
        preset: None,
        last_success_at_ms: None,
        pending_operation_count: 0,
        conflict_count: 0,
        next_retry_at_ms: None,
        last_error: None,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn sync_error(error: impl ToString) -> String {
    error.to_string()
}

fn sync_error_code(error: &sona_core::sync::SyncError) -> &'static str {
    match error {
        sona_core::sync::SyncError::InvalidOperation(_) => "invalid_operation",
        sona_core::sync::SyncError::InvalidObjectKey(_) => "invalid_object_key",
        sona_core::sync::SyncError::ObjectStore(_) => "provider_error",
        sona_core::sync::SyncError::LocalRepository(_) => "local_repository_error",
        sona_core::sync::SyncError::SecretStore(_) => "secret_store_error",
        sona_core::sync::SyncError::Protocol(_) => "protocol_error",
        sona_core::sync::SyncError::Crypto(_) => "crypto_error",
    }
}

fn is_retryable(error: &sona_core::sync::SyncError) -> bool {
    matches!(error, sona_core::sync::SyncError::ObjectStore(_))
}
