use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sona_core::sync::{
    SyncConflictResolution, SyncError, SyncPresetV1, SyncSecretStore, SyncStatusSnapshot,
};
use sona_sqlite::{Database, SqliteSyncRepositoryFactory};
use sona_sync::{
    JsonFileSyncConfigStore, SyncApplication, SyncProviderFactory, SyncProviderInput,
    SyncProviderRegistry, SystemSyncApplicationEnvironment,
};
use sona_sync_webdav::{WebDavObjectStoreConfig, WebDavSyncProviderFactory};

use crate::json_bridge::{parse_core_json, serialize_core_json};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};

const CONFIG_FILE: &str = "sync.json";

#[uniffi::export(foreign)]
#[async_trait]
pub trait FfiSyncSecretStore: Send + Sync {
    async fn get(&self, key: String) -> SonaCoreBindingResult<Option<Vec<u8>>>;
    async fn set(&self, key: String, value: Vec<u8>) -> SonaCoreBindingResult<()>;
    async fn delete(&self, key: String) -> SonaCoreBindingResult<()>;
}

pub(crate) fn register_sync_secret_store(store: Arc<dyn FfiSyncSecretStore>) {
    *secret_store_registration()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(store);
}

fn secret_store_registration() -> &'static RwLock<Option<Arc<dyn FfiSyncSecretStore>>> {
    static STORE: OnceLock<RwLock<Option<Arc<dyn FfiSyncSecretStore>>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(None))
}

#[derive(Clone, Copy, Debug, Default)]
struct ForeignSyncSecretStore;

#[async_trait]
impl SyncSecretStore for ForeignSyncSecretStore {
    async fn read_secret(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError> {
        let callback = registered_secret_store();
        match callback {
            Some(callback) => callback
                .get(key.to_string())
                .await
                .map_err(secret_store_error),
            None => Ok(None),
        }
    }

    async fn write_secret(&self, key: &str, value: &[u8]) -> Result<(), SyncError> {
        let callback = registered_secret_store();
        match callback {
            Some(callback) => callback
                .set(key.to_string(), value.to_vec())
                .await
                .map_err(secret_store_error),
            None => Ok(()),
        }
    }

    async fn delete_secret(&self, key: &str) -> Result<(), SyncError> {
        let callback = registered_secret_store();
        match callback {
            Some(callback) => callback
                .delete(key.to_string())
                .await
                .map_err(secret_store_error),
            None => Ok(()),
        }
    }
}

fn registered_secret_store() -> Option<Arc<dyn FfiSyncSecretStore>> {
    secret_store_registration()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn secret_store_error(error: SonaCoreBindingError) -> SyncError {
    SyncError::SecretStore(error.to_string())
}

fn applications() -> &'static RwLock<HashMap<PathBuf, Arc<SyncApplication>>> {
    static APPLICATIONS: OnceLock<RwLock<HashMap<PathBuf, Arc<SyncApplication>>>> = OnceLock::new();
    APPLICATIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn application(app_data_dir: &str) -> SonaCoreBindingResult<Arc<SyncApplication>> {
    let path = PathBuf::from(app_data_dir);
    if let Some(application) = applications()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&path)
        .cloned()
    {
        return Ok(application);
    }

    std::fs::create_dir_all(&path).map_err(sync_error)?;
    let database = Arc::new(Database::open(&path).map_err(sync_error)?);
    let application = Arc::new(SyncApplication::new(
        Arc::new(JsonFileSyncConfigStore::new(path.join(CONFIG_FILE))),
        Arc::new(SqliteSyncRepositoryFactory::new(database)),
        provider_registry(),
        Arc::new(ForeignSyncSecretStore),
        Arc::new(SystemSyncApplicationEnvironment),
    ));

    let mut cached = applications()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(Arc::clone(
        cached.entry(path).or_insert_with(|| application),
    ))
}

fn provider_registry() -> SyncProviderRegistry {
    SyncProviderRegistry::new([Arc::new(WebDavSyncProviderFactory) as Arc<dyn SyncProviderFactory>])
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateRequest {
    provider: WebDavObjectStoreConfig,
    preset: SyncPresetV1,
    master_password: String,
    create_recovery_key: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinRequest {
    provider: WebDavObjectStoreConfig,
    vault_id: String,
    master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnlockRequest {
    provider_password: String,
    master_password: Option<String>,
    recovery_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangePasswordRequest {
    current_master_password: String,
    next_master_password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateResult {
    vault_id: String,
    device_id: String,
    recovery_key: Option<String>,
    status: SyncStatusSnapshot,
}

pub(crate) async fn test_provider_json(config_json: String) -> SonaCoreBindingResult<String> {
    let config: WebDavObjectStoreConfig = parse_core_json(&config_json, "sync provider")?;
    let descriptor = provider_registry()
        .test_provider(provider_input(config)?)
        .await
        .map_err(sync_error)?;
    serialize_core_json(&descriptor, "sync provider descriptor")
}

pub(crate) async fn get_status_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let status = application(&app_data_dir)?
        .status()
        .await
        .map_err(sync_error)?;
    serialize_core_json(&status, "sync status")
}

pub(crate) async fn create_vault_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: CreateRequest = parse_core_json(&request_json, "sync create request")?;
    let result = application(&app_data_dir)?
        .create(
            provider_input(request.provider)?,
            request.preset,
            &request.master_password,
            request.create_recovery_key,
        )
        .await
        .map_err(sync_error)?;
    serialize_core_json(
        &CreateResult {
            vault_id: result.vault_id,
            device_id: result.device_id,
            recovery_key: result.recovery_key,
            status: result.status,
        },
        "sync create result",
    )
}

pub(crate) async fn preview_join_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: JoinRequest = parse_core_json(&request_json, "sync join preview request")?;
    let preview = application(&app_data_dir)?
        .preview_join(
            provider_input(request.provider)?,
            &request.vault_id,
            &request.master_password,
        )
        .await
        .map_err(sync_error)?;
    serialize_core_json(&preview, "sync join preview")
}

pub(crate) async fn join_vault_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    let request: JoinRequest = parse_core_json(&request_json, "sync join request")?;
    let result = application(&app_data_dir)?
        .join(
            provider_input(request.provider)?,
            &request.vault_id,
            &request.master_password,
        )
        .await
        .map_err(sync_error)?;
    serialize_core_json(&result, "sync run result")
}

pub(crate) async fn unlock_json(
    app_data_dir: String,
    request_json: String,
    recovery: bool,
) -> SonaCoreBindingResult<String> {
    let request: UnlockRequest = parse_core_json(&request_json, "sync unlock request")?;
    let application = application(&app_data_dir)?;
    let status = if recovery {
        application
            .unlock_with_recovery_key(
                request.provider_password.into_bytes(),
                request
                    .recovery_key
                    .as_deref()
                    .ok_or_else(|| sync_binding_error("Recovery key is required."))?,
            )
            .await
    } else {
        application
            .unlock_with_password(
                request.provider_password.into_bytes(),
                request
                    .master_password
                    .as_deref()
                    .ok_or_else(|| sync_binding_error("Master password is required."))?,
            )
            .await
    }
    .map_err(sync_error)?;
    serialize_core_json(&status, "sync status")
}

pub(crate) async fn lock(app_data_dir: String) -> SonaCoreBindingResult<()> {
    application(&app_data_dir)?
        .lock()
        .await
        .map(|_| ())
        .map_err(sync_error)
}

pub(crate) async fn set_paused_json(
    app_data_dir: String,
    paused: bool,
) -> SonaCoreBindingResult<String> {
    let status = application(&app_data_dir)?
        .set_paused(paused)
        .await
        .map_err(sync_error)?;
    serialize_core_json(&status, "sync status")
}

pub(crate) async fn disconnect_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let status = application(&app_data_dir)?
        .disconnect()
        .await
        .map_err(sync_error)?;
    serialize_core_json(&status, "sync status")
}

pub(crate) async fn run_now_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let result = application(&app_data_dir)?
        .run()
        .await
        .map_err(sync_error)?;
    serialize_core_json(&result, "sync run result")
}

pub(crate) async fn change_preset_json(
    app_data_dir: String,
    preset_json: String,
    confirm_shrink: bool,
) -> SonaCoreBindingResult<String> {
    let preset: SyncPresetV1 = parse_core_json(&preset_json, "sync preset")?;
    let status = application(&app_data_dir)?
        .change_preset(preset, confirm_shrink)
        .await
        .map_err(sync_error)?;
    serialize_core_json(&status, "sync status")
}

pub(crate) async fn change_master_password_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<()> {
    let request: ChangePasswordRequest =
        parse_core_json(&request_json, "sync password change request")?;
    application(&app_data_dir)?
        .change_master_password(
            &request.current_master_password,
            &request.next_master_password,
        )
        .await
        .map_err(sync_error)
}

pub(crate) async fn generate_recovery_key(app_data_dir: String) -> SonaCoreBindingResult<String> {
    application(&app_data_dir)?
        .generate_recovery_key()
        .await
        .map_err(sync_error)
}

pub(crate) fn list_conflicts_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let conflicts = application(&app_data_dir)?
        .list_conflicts()
        .map_err(sync_error)?;
    serialize_core_json(&conflicts, "sync conflicts")
}

pub(crate) fn get_conflict_json(
    app_data_dir: String,
    conflict_id: String,
) -> SonaCoreBindingResult<String> {
    let conflict = application(&app_data_dir)?
        .get_conflict(&conflict_id)
        .map_err(sync_error)?;
    serialize_core_json(&conflict, "sync conflict")
}

pub(crate) fn resolve_conflict_json(
    app_data_dir: String,
    conflict_id: String,
    resolution_json: String,
) -> SonaCoreBindingResult<()> {
    let resolution: SyncConflictResolution =
        parse_core_json(&resolution_json, "sync conflict resolution")?;
    application(&app_data_dir)?
        .resolve_conflict(&conflict_id, resolution)
        .map_err(sync_error)
}

fn provider_input(config: WebDavObjectStoreConfig) -> SonaCoreBindingResult<SyncProviderInput> {
    Ok(SyncProviderInput {
        provider_id: "webdav".to_string(),
        configuration: serde_json::to_value(config).map_err(sync_error)?,
    })
}

fn sync_error(error: impl ToString) -> SonaCoreBindingError {
    let message = error.to_string();
    let reason = match message.as_str() {
        "Sync provider does not support required conditional object operations." => {
            "WebDAV server lacks required conditional object operations.".to_string()
        }
        "Sync connection metadata exists but local sync state is missing." => {
            "SQLite sync state is missing.".to_string()
        }
        "Local sync state is missing." => "Sync is not configured.".to_string(),
        _ => message
            .strip_prefix("Sync configuration error: ")
            .map_or(message.clone(), str::to_string),
    };
    sync_binding_error(reason)
}

fn sync_binding_error(reason: impl Into<String>) -> SonaCoreBindingError {
    SonaCoreBindingError::Sync {
        reason: reason.into(),
    }
}

#[cfg(test)]
fn clear_applications_for_tests() {
    applications()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

#[cfg(test)]
fn clear_sync_runtime_for_tests() {
    clear_applications_for_tests();
    *secret_store_registration()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::sync::SyncLifecycleState;
    use sona_sync::SyncApplicationError;
    use std::collections::BTreeMap;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct MemoryFfiSyncSecretStore {
        values: StdMutex<BTreeMap<String, Vec<u8>>>,
    }

    #[async_trait]
    impl FfiSyncSecretStore for MemoryFfiSyncSecretStore {
        async fn get(&self, key: String) -> SonaCoreBindingResult<Option<Vec<u8>>> {
            Ok(self.values.lock().unwrap().get(&key).cloned())
        }

        async fn set(&self, key: String, value: Vec<u8>) -> SonaCoreBindingResult<()> {
            self.values.lock().unwrap().insert(key, value);
            Ok(())
        }

        async fn delete(&self, key: String) -> SonaCoreBindingResult<()> {
            self.values.lock().unwrap().remove(&key);
            Ok(())
        }
    }

    #[tokio::test]
    async fn unconfigured_status_uses_the_shared_disabled_contract() {
        let directory = tempfile::tempdir().unwrap();
        let json = get_status_json(directory.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        let status: SyncStatusSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(status.state, SyncLifecycleState::Disabled);
        assert_eq!(status.pending_operation_count, 0);
    }

    #[tokio::test]
    async fn secret_store_proxy_is_noop_until_a_foreign_store_is_registered() {
        clear_sync_runtime_for_tests();
        let proxy = ForeignSyncSecretStore;

        assert_eq!(proxy.read_secret("missing").await.unwrap(), None);
        proxy.write_secret("ephemeral", b"ignored").await.unwrap();
        proxy.delete_secret("ephemeral").await.unwrap();

        let store = Arc::new(MemoryFfiSyncSecretStore::default());
        register_sync_secret_store(store.clone());
        proxy.write_secret("provider", b"password").await.unwrap();
        assert_eq!(
            proxy.read_secret("provider").await.unwrap(),
            Some(b"password".to_vec())
        );
        proxy.delete_secret("provider").await.unwrap();
        assert_eq!(proxy.read_secret("provider").await.unwrap(), None);
        clear_sync_runtime_for_tests();
    }

    #[tokio::test]
    async fn application_is_cached_per_data_directory_and_can_be_recreated() {
        clear_sync_runtime_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let app_data_dir = directory.path().to_string_lossy().into_owned();

        let first = application(&app_data_dir).unwrap();
        let second = application(&app_data_dir).unwrap();
        assert!(Arc::ptr_eq(&first, &second));

        clear_applications_for_tests();
        let restarted = application(&app_data_dir).unwrap();
        assert!(!Arc::ptr_eq(&first, &restarted));
        assert_eq!(
            restarted.status().await.unwrap().state,
            SyncLifecycleState::Disabled
        );
        clear_sync_runtime_for_tests();
    }

    #[test]
    fn shared_application_errors_keep_the_existing_uniffi_text_contract() {
        let cases = [
            (
                SyncApplicationError::InvalidState(
                    "Sync provider does not support required conditional object operations."
                        .to_string(),
                ),
                "WebDAV server lacks required conditional object operations.",
            ),
            (
                SyncApplicationError::InvalidState(
                    "Sync connection metadata exists but local sync state is missing.".to_string(),
                ),
                "SQLite sync state is missing.",
            ),
            (
                SyncApplicationError::InvalidState("Local sync state is missing.".to_string()),
                "Sync is not configured.",
            ),
            (
                SyncApplicationError::Config("invalid sync.json".to_string()),
                "invalid sync.json",
            ),
        ];

        for (error, expected) in cases {
            assert!(matches!(
                sync_error(error),
                SonaCoreBindingError::Sync { reason } if reason == expected
            ));
        }
    }
}
