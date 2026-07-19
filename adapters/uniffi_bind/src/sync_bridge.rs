use std::sync::Arc;

#[cfg(test)]
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sona_core::sync::{SyncConflictResolution, SyncPresetV1, SyncSecretStore, SyncStatusSnapshot};
use sona_runtime_fs::SystemClock;
use sona_sync::{
    JsonFileSyncConfigStore, SyncApplication, SyncProviderFactory, SyncProviderInput,
    SyncProviderRegistry, SystemSyncApplicationEnvironment,
};
use sona_sync_webdav::{WebDavObjectStoreConfig, WebDavSyncProviderFactory};

use crate::application_context::{
    application_context, register_default_sync_secret_store,
    register_sync_secret_store_for_app_data_dir as register_context_sync_secret_store,
};
use crate::json_bridge::{parse_core_json, serialize_core_json};
use crate::sync_secret_store_bridge::FfiSyncSecretStore;
use crate::{SonaCoreBindingError, SonaCoreBindingResult};

const CONFIG_FILE: &str = "sync.json";

pub(crate) fn register_sync_secret_store(store: Arc<dyn FfiSyncSecretStore>) {
    register_default_sync_secret_store(store);
}

pub(crate) fn register_sync_secret_store_for_app_data_dir(
    app_data_dir: &str,
    store: Arc<dyn FfiSyncSecretStore>,
) -> SonaCoreBindingResult<()> {
    register_context_sync_secret_store(app_data_dir, store).map_err(sync_error)
}

fn application(app_data_dir: &str) -> SonaCoreBindingResult<Arc<SyncApplication>> {
    let context = application_context(app_data_dir).map_err(sync_error)?;
    let secret_store: Arc<dyn SyncSecretStore> = context.sync_secret_store();
    Ok(context.sync_application(|sqlite| {
        Arc::new(SyncApplication::new(
            Arc::new(JsonFileSyncConfigStore::new(
                sqlite.app_data_dir().join(CONFIG_FILE),
            )),
            Arc::new(sqlite.sync_repository_factory(Arc::new(SystemClock))),
            provider_registry(),
            secret_store,
            Arc::new(SystemSyncApplicationEnvironment),
        ))
    }))
}

fn provider_registry() -> SyncProviderRegistry {
    SyncProviderRegistry::new([Arc::new(WebDavSyncProviderFactory) as Arc<dyn SyncProviderFactory>])
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum ProviderInputWire {
    Canonical(SyncProviderInput),
    LegacyWebDav(WebDavObjectStoreConfig),
}

impl ProviderInputWire {
    fn into_provider_input(self) -> SonaCoreBindingResult<SyncProviderInput> {
        match self {
            Self::Canonical(provider) => Ok(provider),
            Self::LegacyWebDav(config) => webdav_provider_input(config),
        }
    }
}

fn parse_provider_input_json(
    input: &str,
    context: &str,
) -> SonaCoreBindingResult<SyncProviderInput> {
    parse_core_json::<ProviderInputWire>(input, context)?.into_provider_input()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateRequest {
    provider: ProviderInputWire,
    preset: SyncPresetV1,
    master_password: String,
    create_recovery_key: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinRequest {
    provider: ProviderInputWire,
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
    let provider = parse_provider_input_json(&config_json, "sync provider")?;
    let descriptor = provider_registry()
        .test_provider(provider)
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
            request.provider.into_provider_input()?,
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
            request.provider.into_provider_input()?,
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
            request.provider.into_provider_input()?,
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

fn webdav_provider_input(
    config: WebDavObjectStoreConfig,
) -> SonaCoreBindingResult<SyncProviderInput> {
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
mod tests {
    use super::*;
    use crate::application_context::ApplicationContextRegistry;
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

    #[test]
    fn sync_provider_input_accepts_canonical_and_legacy_webdav_json() {
        let webdav = serde_json::json!({
            "serverUrl": "https://dav.example.com",
            "remoteRoot": "sona",
            "username": "alice",
            "password": "secret"
        });
        let canonical = parse_provider_input_json(
            &serde_json::json!({
                "providerId": "webdav",
                "configuration": webdav.clone()
            })
            .to_string(),
            "sync provider",
        )
        .unwrap();
        let legacy = parse_provider_input_json(&webdav.to_string(), "sync provider").unwrap();

        assert_eq!(canonical, legacy);
        assert_eq!(canonical.provider_id, "webdav");
        assert_eq!(canonical.configuration, webdav);
    }

    #[test]
    fn sync_lifecycle_requests_accept_both_provider_wire_shapes() {
        let webdav = serde_json::json!({
            "serverUrl": "https://dav.example.com",
            "remoteRoot": "sona",
            "username": "alice",
            "password": "secret"
        });
        for provider in [
            serde_json::json!({
                "providerId": "webdav",
                "configuration": webdav.clone()
            }),
            webdav.clone(),
        ] {
            let request: CreateRequest = serde_json::from_value(serde_json::json!({
                "provider": provider,
                "preset": "standard",
                "masterPassword": "master-password",
                "createRecoveryKey": true
            }))
            .unwrap();
            assert_eq!(
                request.provider.into_provider_input().unwrap().provider_id,
                "webdav"
            );
        }
    }

    #[tokio::test]
    async fn unconfigured_status_uses_the_shared_disabled_contract() {
        let directory = tempfile::tempdir().unwrap();
        let app_data_dir = directory.path().to_string_lossy().into_owned();
        let json = get_status_json(app_data_dir.clone()).await.unwrap();
        let status: SyncStatusSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(status.state, SyncLifecycleState::Disabled);
        assert_eq!(status.pending_operation_count, 0);
        let _ = crate::application_context::release_application_context(&app_data_dir).unwrap();
    }

    #[tokio::test]
    async fn secret_store_proxy_is_noop_until_a_foreign_store_is_registered() {
        let directory = tempfile::tempdir().unwrap();
        let mut registry = ApplicationContextRegistry::with_capacity(2);
        let context = registry.get_or_open(directory.path()).unwrap();
        let proxy = context.sync_secret_store();

        assert_eq!(proxy.read_secret("missing").await.unwrap(), None);
        proxy.write_secret("ephemeral", b"ignored").await.unwrap();
        proxy.delete_secret("ephemeral").await.unwrap();

        let store = Arc::new(MemoryFfiSyncSecretStore::default());
        registry.register_default_sync_secret_store(store.clone());
        proxy.write_secret("provider", b"password").await.unwrap();
        assert_eq!(
            proxy.read_secret("provider").await.unwrap(),
            Some(b"password".to_vec())
        );
        proxy.delete_secret("provider").await.unwrap();
        assert_eq!(proxy.read_secret("provider").await.unwrap(), None);
    }

    #[tokio::test]
    async fn secret_store_registration_is_isolated_per_application_context() {
        let first_directory = tempfile::tempdir().unwrap();
        let second_directory = tempfile::tempdir().unwrap();
        let first_path = first_directory.path().to_string_lossy().into_owned();
        let second_path = second_directory.path().to_string_lossy().into_owned();
        let first_store = Arc::new(MemoryFfiSyncSecretStore::default());
        let second_store = Arc::new(MemoryFfiSyncSecretStore::default());

        register_sync_secret_store_for_app_data_dir(&first_path, first_store.clone()).unwrap();
        register_sync_secret_store_for_app_data_dir(&second_path, second_store.clone()).unwrap();

        let first_proxy = application_context(&first_path)
            .unwrap()
            .sync_secret_store();
        let second_proxy = application_context(&second_path)
            .unwrap()
            .sync_secret_store();
        first_proxy
            .write_secret("provider", b"first")
            .await
            .unwrap();
        second_proxy
            .write_secret("provider", b"second")
            .await
            .unwrap();

        assert_eq!(
            first_store.get("provider".to_string()).await.unwrap(),
            Some(b"first".to_vec())
        );
        assert_eq!(
            second_store.get("provider".to_string()).await.unwrap(),
            Some(b"second".to_vec())
        );
        let _ = crate::application_context::release_application_context(&first_path).unwrap();
        let _ = crate::application_context::release_application_context(&second_path).unwrap();
    }

    #[tokio::test]
    async fn path_registration_updates_an_existing_application_context() {
        let directory = tempfile::tempdir().unwrap();
        let app_data_dir = directory.path().to_string_lossy().into_owned();
        let _application = application(&app_data_dir).unwrap();
        let store = Arc::new(MemoryFfiSyncSecretStore::default());

        register_sync_secret_store_for_app_data_dir(&app_data_dir, store.clone()).unwrap();
        application_context(&app_data_dir)
            .unwrap()
            .sync_secret_store()
            .write_secret("vault", b"key")
            .await
            .unwrap();

        assert_eq!(
            store.get("vault".to_string()).await.unwrap(),
            Some(b"key".to_vec())
        );
        let _ = crate::application_context::release_application_context(&app_data_dir).unwrap();
    }

    #[tokio::test]
    async fn application_is_cached_per_data_directory_and_can_be_recreated() {
        let directory = tempfile::tempdir().unwrap();
        let app_data_dir = directory.path().to_string_lossy().into_owned();

        let first = application(&app_data_dir).unwrap();
        let second = application(&app_data_dir).unwrap();
        assert!(Arc::ptr_eq(&first, &second));

        assert!(crate::application_context::release_application_context(&app_data_dir).unwrap());
        assert!(!crate::application_context::release_application_context(&app_data_dir).unwrap());
        let restarted = application(&app_data_dir).unwrap();
        assert!(!Arc::ptr_eq(&first, &restarted));
        assert_eq!(
            restarted.status().await.unwrap().state,
            SyncLifecycleState::Disabled
        );
        let _ = crate::application_context::release_application_context(&app_data_dir).unwrap();
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
