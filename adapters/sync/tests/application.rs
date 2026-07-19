use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::sync::{
    SyncDeleteResult, SyncError, SyncLifecycleState, SyncListPage, SyncLocalRepository,
    SyncLocalRuntimeState, SyncObject, SyncObjectKey, SyncObjectMetadata, SyncObjectPrefix,
    SyncObjectStore, SyncObjectStoreCapabilities, SyncOperation, SyncPresetV1,
    SyncPublishedCheckpoint, SyncPublishedSegment, SyncPutResult, SyncRemoteApplyResult,
    SyncRemoteSegment, SyncRepositoryFactory, SyncRunResult, SyncSecretStore,
};
use sona_sqlite::{Database, SqliteSyncRepositoryFactory as RealSqliteSyncRepositoryFactory};
use sona_sync::{
    JsonFileSyncConfigStore, SyncApplication, SyncApplicationConfig, SyncApplicationEnvironment,
    SyncApplicationError, SyncConfigStore, SyncPresetChangeError, SyncProvider,
    SyncProviderFactory, SyncProviderInput, SyncProviderRegistry, SyncRetryState,
    SyncStatusContext, apply_sync_run_result, build_sync_status, change_sync_preset,
    create_remote_vault, disabled_sync_status,
};
use tokio::sync::Notify;

struct RepositoryClock;

impl UnixMillisClock for RepositoryClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(1_000)
    }
}

struct SqliteSyncRepositoryFactory;

impl SqliteSyncRepositoryFactory {
    fn new(database: Arc<Database>) -> RealSqliteSyncRepositoryFactory {
        RealSqliteSyncRepositoryFactory::new(database, Arc::new(RepositoryClock))
    }
}

#[derive(Default)]
struct MemoryStore {
    objects: Mutex<BTreeMap<String, (Vec<u8>, String)>>,
    compare_calls: AtomicUsize,
    fail_compare_call: Mutex<Option<usize>>,
}

impl MemoryStore {
    fn fail_compare_call(&self, call: usize) {
        *self.fail_compare_call.lock().unwrap() = Some(call);
    }
}

#[async_trait]
impl SyncObjectStore for MemoryStore {
    async fn probe(&self) -> Result<SyncObjectStoreCapabilities, SyncError> {
        Ok(SyncObjectStoreCapabilities {
            conditional_create: true,
            compare_and_swap: true,
            delete: true,
        })
    }

    async fn list(
        &self,
        _prefix: &SyncObjectPrefix,
        _continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError> {
        Ok(SyncListPage {
            objects: Vec::new(),
            continuation: None,
        })
    }

    async fn get(&self, key: &SyncObjectKey) -> Result<Option<SyncObject>, SyncError> {
        Ok(self
            .objects
            .lock()
            .unwrap()
            .get(key.as_str())
            .map(|(bytes, etag)| SyncObject {
                metadata: SyncObjectMetadata {
                    key: key.clone(),
                    etag: Some(etag.clone()),
                    size: bytes.len() as u64,
                    modified_at: None,
                },
                bytes: bytes.clone(),
            }))
    }

    async fn put_if_absent(
        &self,
        key: &SyncObjectKey,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        let mut objects = self.objects.lock().unwrap();
        if let Some((_, etag)) = objects.get(key.as_str()) {
            return Ok(SyncPutResult::AlreadyExists {
                etag: Some(etag.clone()),
            });
        }
        let etag = "etag-1".to_string();
        objects.insert(key.to_string(), (bytes, etag.clone()));
        Ok(SyncPutResult::Created { etag: Some(etag) })
    }

    async fn compare_and_swap(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        let call = self.compare_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self
            .fail_compare_call
            .lock()
            .unwrap()
            .is_some_and(|target| target == call)
        {
            return Err(SyncError::ObjectStore(
                "simulated compare-and-swap failure".to_string(),
            ));
        }

        let mut objects = self.objects.lock().unwrap();
        let Some((_, current_etag)) = objects.get(key.as_str()) else {
            return Ok(SyncPutResult::Conflict { current_etag: None });
        };
        if expected_etag != Some(current_etag.as_str()) {
            return Ok(SyncPutResult::Conflict {
                current_etag: Some(current_etag.clone()),
            });
        }
        let next_etag = format!("etag-{}", call + 1);
        objects.insert(key.to_string(), (bytes, next_etag.clone()));
        Ok(SyncPutResult::Created {
            etag: Some(next_etag),
        })
    }

    async fn delete(
        &self,
        _key: &SyncObjectKey,
        _expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError> {
        Ok(SyncDeleteResult::NotFound)
    }
}

struct PresetRepository {
    preset: Mutex<SyncPresetV1>,
    fail_change: bool,
}

impl PresetRepository {
    fn new(fail_change: bool) -> Self {
        Self {
            preset: Mutex::new(SyncPresetV1::Standard),
            fail_change,
        }
    }
}

impl SyncLocalRepository for PresetRepository {
    fn load_runtime_state(&self) -> Result<SyncLocalRuntimeState, SyncError> {
        Ok(SyncLocalRuntimeState {
            vault_id: "vault-a".to_string(),
            device_id: "device-a".to_string(),
            preset: *self.preset.lock().unwrap(),
            next_sequence: 1,
            previous_cipher_hash: None,
            remote_cursors: BTreeMap::new(),
            operations_since_checkpoint: 0,
            bytes_since_checkpoint: 0,
            checkpoint_required: false,
        })
    }

    fn load_pending_operations(
        &self,
        _preset: SyncPresetV1,
        _maximum_operations: usize,
        _maximum_bytes: usize,
    ) -> Result<Vec<SyncOperation>, SyncError> {
        Ok(Vec::new())
    }

    fn mark_segment_published(&self, _published: &SyncPublishedSegment) -> Result<(), SyncError> {
        Ok(())
    }

    fn load_checkpoint_operations(&self) -> Result<Vec<SyncOperation>, SyncError> {
        Ok(Vec::new())
    }

    fn mark_checkpoint_published(
        &self,
        _checkpoint: &SyncPublishedCheckpoint,
    ) -> Result<(), SyncError> {
        Ok(())
    }

    fn apply_remote_segment(
        &self,
        _segment: &SyncRemoteSegment,
    ) -> Result<SyncRemoteApplyResult, SyncError> {
        Ok(SyncRemoteApplyResult::default())
    }

    fn validate_preset_change(
        &self,
        _preset: SyncPresetV1,
        _confirm_shrink: bool,
    ) -> Result<(), SyncError> {
        Ok(())
    }

    fn change_preset(&self, preset: SyncPresetV1, _confirm_shrink: bool) -> Result<(), SyncError> {
        if self.fail_change {
            return Err(SyncError::LocalRepository(
                "simulated local preset failure".to_string(),
            ));
        }
        *self.preset.lock().unwrap() = preset;
        Ok(())
    }
}

fn status_context() -> SyncStatusContext {
    SyncStatusContext {
        provider_id: "webdav".to_string(),
        vault_id: "vault-a".to_string(),
        preset: SyncPresetV1::Standard,
        paused: false,
        unlocked: true,
        syncing: false,
        pending_operation_count: 3,
        conflict_count: 2,
    }
}

#[test]
fn disabled_status_uses_the_shared_contract() {
    let status = disabled_sync_status();

    assert_eq!(status.state, SyncLifecycleState::Disabled);
    assert_eq!(status.provider_id, None);
    assert_eq!(status.vault_id, None);
    assert_eq!(status.preset, None);
    assert_eq!(status.pending_operation_count, 0);
    assert_eq!(status.conflict_count, 0);
}

#[test]
fn status_precedence_is_syncing_paused_locked_error_idle() {
    let mut context = status_context();
    let mut retry = SyncRetryState::default();

    assert_eq!(
        build_sync_status(context.clone(), &retry).state,
        SyncLifecycleState::Idle
    );

    retry.last_error = Some(sona_core::sync::SyncErrorSnapshot {
        code: "provider_error".to_string(),
        message: "offline".to_string(),
        retryable: true,
    });
    assert_eq!(
        build_sync_status(context.clone(), &retry).state,
        SyncLifecycleState::Error
    );

    context.unlocked = false;
    assert_eq!(
        build_sync_status(context.clone(), &retry).state,
        SyncLifecycleState::Locked
    );

    context.paused = true;
    assert_eq!(
        build_sync_status(context.clone(), &retry).state,
        SyncLifecycleState::Paused
    );

    context.syncing = true;
    assert_eq!(
        build_sync_status(context, &retry).state,
        SyncLifecycleState::Syncing
    );
}

#[test]
fn failed_run_updates_backoff_and_typed_error_snapshot() {
    let mut retry = SyncRetryState::default();
    let result = Err(SyncError::ObjectStore("offline".to_string()));

    apply_sync_run_result(&mut retry, 1_000, 500_000, &result);

    assert_eq!(retry.last_success_at_ms, None);
    assert_eq!(retry.consecutive_failures, 1);
    assert_eq!(retry.next_retry_at_ms, Some(31_000));
    assert_eq!(
        retry.last_error,
        Some(sona_core::sync::SyncErrorSnapshot {
            code: "provider_error".to_string(),
            message: "Sync object store error: offline".to_string(),
            retryable: true,
        })
    );
}

#[test]
fn successful_run_clears_retry_state() {
    let mut retry = SyncRetryState {
        last_success_at_ms: Some(100),
        consecutive_failures: 4,
        next_retry_at_ms: Some(900),
        last_error: Some(sona_core::sync::SyncErrorSnapshot {
            code: "provider_error".to_string(),
            message: "offline".to_string(),
            retryable: true,
        }),
    };
    let result = Ok(SyncRunResult::default());

    apply_sync_run_result(&mut retry, 2_000, 500_000, &result);

    assert_eq!(
        retry,
        SyncRetryState {
            last_success_at_ms: Some(2_000),
            consecutive_failures: 0,
            next_retry_at_ms: None,
            last_error: None,
        }
    );
}

#[tokio::test]
async fn preset_change_updates_remote_and_local_state() {
    let store = MemoryStore::default();
    let mut opened = create_remote_vault(
        &store,
        "vault-a",
        SyncPresetV1::Standard,
        "master-password",
        false,
    )
    .await
    .unwrap()
    .opened;
    let repository = PresetRepository::new(false);

    change_sync_preset(&repository, &store, &mut opened, SyncPresetV1::Full, true)
        .await
        .unwrap();

    assert_eq!(opened.header.preset, SyncPresetV1::Full);
    assert_eq!(*repository.preset.lock().unwrap(), SyncPresetV1::Full);
}

#[tokio::test]
async fn preset_change_rolls_remote_state_back_after_local_failure() {
    let store = MemoryStore::default();
    let mut opened = create_remote_vault(
        &store,
        "vault-a",
        SyncPresetV1::Standard,
        "master-password",
        false,
    )
    .await
    .unwrap()
    .opened;
    let repository = PresetRepository::new(true);

    let error = change_sync_preset(&repository, &store, &mut opened, SyncPresetV1::Full, true)
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        SyncPresetChangeError::Sync(SyncError::LocalRepository(_))
    ));
    assert_eq!(opened.header.preset, SyncPresetV1::Standard);
}

#[tokio::test]
async fn preset_change_reports_local_and_remote_rollback_failures() {
    let store = MemoryStore::default();
    store.fail_compare_call(2);
    let mut opened = create_remote_vault(
        &store,
        "vault-a",
        SyncPresetV1::Standard,
        "master-password",
        false,
    )
    .await
    .unwrap()
    .opened;
    let repository = PresetRepository::new(true);

    let error = change_sync_preset(&repository, &store, &mut opened, SyncPresetV1::Full, true)
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        SyncPresetChangeError::LocalUpdateAndRollback { .. }
    ));
    assert!(error.to_string().contains("Local preset update failed"));
    assert!(error.to_string().contains("remote rollback also failed"));
    assert_eq!(opened.header.preset, SyncPresetV1::Full);
}

#[test]
fn json_config_store_reads_legacy_paused_but_does_not_write_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sync.json");
    std::fs::write(
        &path,
        r#"{
  "providerId": "webdav",
  "vaultId": "vault-a",
  "deviceId": "device-a",
  "preset": "standard",
  "webdav": {
    "serverUrl": "https://dav.example.com",
    "remoteRoot": "sona",
    "username": "alice"
  },
  "paused": true,
  "consecutiveFailures": 2,
  "nextRetryAtMs": 3000
}"#,
    )
    .unwrap();
    let store = JsonFileSyncConfigStore::new(path.clone());

    let config = store.load().unwrap().unwrap();

    assert_eq!(config.provider_id, "webdav");
    assert_eq!(config.retry.consecutive_failures, 2);
    assert_eq!(
        config.provider_configuration,
        serde_json::json!({
            "serverUrl": "https://dav.example.com",
            "remoteRoot": "sona",
            "username": "alice"
        })
    );

    store.save(&config).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(saved.get("paused").is_none());
    assert_eq!(saved["consecutiveFailures"], 2);
    assert_eq!(saved["webdav"]["username"], "alice");
}

struct TestProviderFactory {
    store: Arc<dyn SyncObjectStore>,
}

impl Default for TestProviderFactory {
    fn default() -> Self {
        Self {
            store: Arc::new(MemoryStore::default()),
        }
    }
}

#[async_trait]
impl SyncProviderFactory for TestProviderFactory {
    fn provider_id(&self) -> &str {
        "test"
    }

    fn credential_secret_key(&self, vault_id: &str) -> String {
        format!("test-password:{vault_id}")
    }

    async fn prepare(&self, configuration: serde_json::Value) -> Result<SyncProvider, SyncError> {
        Ok(SyncProvider {
            descriptor: sona_core::sync::SyncProviderDescriptor {
                id: "test".to_string(),
                display_name: "Test".to_string(),
            },
            store: self.store.clone(),
            persisted_configuration: serde_json::json!({ "account": configuration["account"] }),
            credential: configuration["password"]
                .as_str()
                .unwrap()
                .as_bytes()
                .to_vec(),
        })
    }

    async fn restore(
        &self,
        persisted_configuration: serde_json::Value,
        credential: Vec<u8>,
    ) -> Result<SyncProvider, SyncError> {
        Ok(SyncProvider {
            descriptor: sona_core::sync::SyncProviderDescriptor {
                id: "test".to_string(),
                display_name: "Test".to_string(),
            },
            store: self.store.clone(),
            persisted_configuration,
            credential,
        })
    }
}

#[test]
fn provider_input_uses_the_provider_neutral_host_wire_shape() {
    let value = serde_json::json!({
        "providerId": "test",
        "configuration": {
            "account": "alice"
        }
    });

    let input: SyncProviderInput = serde_json::from_value(value.clone()).unwrap();

    assert_eq!(input.provider_id, "test");
    assert_eq!(
        input.configuration,
        serde_json::json!({ "account": "alice" })
    );
    assert_eq!(serde_json::to_value(input).unwrap(), value);
}

#[tokio::test]
async fn provider_registry_separates_persisted_settings_from_credentials() {
    let registry = SyncProviderRegistry::new([Arc::new(TestProviderFactory::default()) as Arc<_>]);

    let descriptor = registry
        .test_provider(SyncProviderInput {
            provider_id: "test".to_string(),
            configuration: serde_json::json!({
                "account": "alice",
                "password": "secret"
            }),
        })
        .await
        .unwrap();
    assert_eq!(descriptor.id, "test");

    let provider = registry
        .prepare(SyncProviderInput {
            provider_id: "test".to_string(),
            configuration: serde_json::json!({
                "account": "alice",
                "password": "secret"
            }),
        })
        .await
        .unwrap();

    assert_eq!(
        provider.persisted_configuration,
        serde_json::json!({ "account": "alice" })
    );
    assert_eq!(provider.credential, b"secret");
    assert_eq!(
        registry.credential_secret_key("test", "vault-a").unwrap(),
        "test-password:vault-a"
    );

    let restored = registry
        .restore(
            "test",
            provider.persisted_configuration.clone(),
            provider.credential.clone(),
        )
        .await
        .unwrap();
    assert_eq!(restored.credential, b"secret");

    assert!(matches!(
        registry
            .prepare(SyncProviderInput {
                provider_id: "missing".to_string(),
                configuration: serde_json::Value::Null,
            })
            .await,
        Err(SyncApplicationError::UnknownProvider(provider)) if provider == "missing"
    ));
}

#[derive(Default)]
struct MemoryConfigStore(Mutex<Option<SyncApplicationConfig>>);

impl SyncConfigStore for MemoryConfigStore {
    fn load(&self) -> Result<Option<SyncApplicationConfig>, SyncApplicationError> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, config: &SyncApplicationConfig) -> Result<(), SyncApplicationError> {
        *self.0.lock().unwrap() = Some(config.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), SyncApplicationError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[derive(Default)]
struct MemorySecretStore(Mutex<BTreeMap<String, Vec<u8>>>);

#[async_trait]
impl SyncSecretStore for MemorySecretStore {
    async fn read_secret(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }

    async fn write_secret(&self, key: &str, value: &[u8]) -> Result<(), SyncError> {
        self.0
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn delete_secret(&self, key: &str) -> Result<(), SyncError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

struct FixedEnvironment {
    ids: Mutex<VecDeque<String>>,
}

impl FixedEnvironment {
    fn new(ids: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            ids: Mutex::new(ids.into_iter().map(str::to_string).collect()),
        }
    }
}

impl SyncApplicationEnvironment for FixedEnvironment {
    fn now_ms(&self) -> u64 {
        1_000
    }

    fn next_id(&self) -> String {
        self.ids.lock().unwrap().pop_front().unwrap()
    }

    fn jitter(&self) -> u32 {
        500_000
    }
}

#[tokio::test]
async fn application_manual_lock_suppresses_restore_until_restart() {
    let config = Arc::new(MemoryConfigStore::default());
    let secrets = Arc::new(MemorySecretStore::default());
    let factory = Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
        Database::open_in_memory().unwrap(),
    )));
    let providers = SyncProviderRegistry::new([
        Arc::new(TestProviderFactory::default()) as Arc<dyn SyncProviderFactory>
    ]);
    let application = SyncApplication::new(
        config.clone(),
        factory.clone(),
        providers.clone(),
        secrets.clone(),
        Arc::new(FixedEnvironment::new(["vault-a", "device-a"])),
    );

    let created = application
        .create(
            SyncProviderInput {
                provider_id: "test".to_string(),
                configuration: serde_json::json!({
                    "account": "alice",
                    "password": "provider-secret"
                }),
            },
            SyncPresetV1::Standard,
            "master-password",
            true,
        )
        .await
        .unwrap();
    assert_eq!(created.vault_id, "vault-a");
    assert_eq!(created.device_id, "device-a");
    assert_eq!(
        created.status.state,
        SyncLifecycleState::Error,
        "create keeps the connection when the initial sync fails"
    );
    assert_eq!(
        secrets.read_secret("test-password:vault-a").await.unwrap(),
        Some(b"provider-secret".to_vec())
    );
    assert!(
        secrets
            .read_secret("vault-key:vault-a")
            .await
            .unwrap()
            .is_some()
    );

    assert_eq!(
        application.lock().await.unwrap().state,
        SyncLifecycleState::Locked
    );
    assert_eq!(
        application.status().await.unwrap().state,
        SyncLifecycleState::Locked
    );

    let restarted = SyncApplication::new(
        config,
        factory,
        providers,
        secrets,
        Arc::new(FixedEnvironment::new([])),
    );
    assert_eq!(
        restarted.status().await.unwrap().state,
        SyncLifecycleState::Error
    );
}

#[tokio::test]
async fn application_owns_join_unlock_pause_and_disconnect_lifecycle() {
    let provider_factory = Arc::new(TestProviderFactory::default());
    let providers =
        SyncProviderRegistry::new([provider_factory.clone() as Arc<dyn SyncProviderFactory>]);
    let source = SyncApplication::new(
        Arc::new(MemoryConfigStore::default()),
        Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
            Database::open_in_memory().unwrap(),
        ))),
        providers.clone(),
        Arc::new(MemorySecretStore::default()),
        Arc::new(FixedEnvironment::new(["vault-shared", "device-source"])),
    );
    source
        .create(
            SyncProviderInput {
                provider_id: "test".to_string(),
                configuration: serde_json::json!({
                    "account": "alice",
                    "password": "provider-secret"
                }),
            },
            SyncPresetV1::Standard,
            "old-password",
            false,
        )
        .await
        .unwrap();

    let config = Arc::new(MemoryConfigStore::default());
    let secrets = Arc::new(MemorySecretStore::default());
    let repository_factory = Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
        Database::open_in_memory().unwrap(),
    )));
    let joining = SyncApplication::new(
        config.clone(),
        repository_factory.clone(),
        providers,
        secrets.clone(),
        Arc::new(FixedEnvironment::new(["preview-device", "device-joining"])),
    );
    let provider_input = || SyncProviderInput {
        provider_id: "test".to_string(),
        configuration: serde_json::json!({
            "account": "alice",
            "password": "provider-secret"
        }),
    };

    let preview = joining
        .preview_join(provider_input(), "vault-shared", "old-password")
        .await
        .unwrap();
    assert_eq!(preview.remote_operation_count, 0);

    let join_error = joining
        .join(provider_input(), "vault-shared", "old-password")
        .await
        .unwrap_err();
    assert!(matches!(join_error, SyncApplicationError::Sync(_)));
    assert_eq!(
        joining.status().await.unwrap().state,
        SyncLifecycleState::Error
    );

    assert_eq!(
        joining.set_paused(true).await.unwrap().state,
        SyncLifecycleState::Paused
    );
    assert_eq!(
        joining.set_paused(false).await.unwrap().state,
        SyncLifecycleState::Error
    );
    assert_eq!(
        joining.lock().await.unwrap().state,
        SyncLifecycleState::Locked
    );
    assert_eq!(
        joining
            .unlock_with_password(b"provider-secret".to_vec(), "old-password")
            .await
            .unwrap()
            .state,
        SyncLifecycleState::Error
    );

    joining
        .change_master_password("old-password", "new-password")
        .await
        .unwrap();
    let recovery_key = joining.generate_recovery_key().await.unwrap();
    joining.lock().await.unwrap();
    assert!(
        joining
            .unlock_with_password(b"provider-secret".to_vec(), "old-password")
            .await
            .is_err()
    );
    assert_eq!(
        joining
            .unlock_with_recovery_key(b"provider-secret".to_vec(), &recovery_key)
            .await
            .unwrap()
            .state,
        SyncLifecycleState::Error
    );

    assert_eq!(
        joining.disconnect().await.unwrap().state,
        SyncLifecycleState::Disabled
    );
    assert!(config.load().unwrap().is_none());
    assert!(repository_factory.open().unwrap().is_none());
    assert!(secrets.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn application_changes_preset_and_delegates_conflicts() {
    let application = SyncApplication::new(
        Arc::new(MemoryConfigStore::default()),
        Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
            Database::open_in_memory().unwrap(),
        ))),
        SyncProviderRegistry::new([
            Arc::new(TestProviderFactory::default()) as Arc<dyn SyncProviderFactory>
        ]),
        Arc::new(MemorySecretStore::default()),
        Arc::new(FixedEnvironment::new(["vault-a", "device-a"])),
    );
    application
        .create(
            SyncProviderInput {
                provider_id: "test".to_string(),
                configuration: serde_json::json!({
                    "account": "alice",
                    "password": "provider-secret"
                }),
            },
            SyncPresetV1::Standard,
            "master-password",
            false,
        )
        .await
        .unwrap();

    let status = application
        .change_preset(SyncPresetV1::Full, true)
        .await
        .unwrap();
    assert_eq!(status.preset, Some(SyncPresetV1::Full));
    assert!(application.list_conflicts().unwrap().is_empty());
    assert!(application.get_conflict("missing").unwrap().is_none());
    assert!(
        application
            .resolve_conflict(
                "missing",
                sona_core::sync::SyncConflictResolution::KeepCurrent,
            )
            .is_err()
    );
}

struct FailingSecretStore;

#[async_trait]
impl SyncSecretStore for FailingSecretStore {
    async fn read_secret(&self, _key: &str) -> Result<Option<Vec<u8>>, SyncError> {
        Ok(None)
    }

    async fn write_secret(&self, _key: &str, _value: &[u8]) -> Result<(), SyncError> {
        Err(SyncError::SecretStore("write failed".to_string()))
    }

    async fn delete_secret(&self, _key: &str) -> Result<(), SyncError> {
        Err(SyncError::SecretStore("delete failed".to_string()))
    }
}

#[tokio::test]
async fn secret_write_is_best_effort_but_delete_failure_stops_disconnect() {
    let config = Arc::new(MemoryConfigStore::default());
    let repository_factory = Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
        Database::open_in_memory().unwrap(),
    )));
    let application = SyncApplication::new(
        config.clone(),
        repository_factory.clone(),
        SyncProviderRegistry::new([
            Arc::new(TestProviderFactory::default()) as Arc<dyn SyncProviderFactory>
        ]),
        Arc::new(FailingSecretStore),
        Arc::new(FixedEnvironment::new(["vault-a", "device-a"])),
    );

    application
        .create(
            SyncProviderInput {
                provider_id: "test".to_string(),
                configuration: serde_json::json!({
                    "account": "alice",
                    "password": "provider-secret"
                }),
            },
            SyncPresetV1::Standard,
            "master-password",
            false,
        )
        .await
        .unwrap();

    assert!(matches!(
        application.disconnect().await,
        Err(SyncApplicationError::Sync(SyncError::SecretStore(_)))
    ));
    assert!(config.load().unwrap().is_some());
    assert!(repository_factory.open().unwrap().is_some());
}

struct FailingSaveConfigStore;

impl SyncConfigStore for FailingSaveConfigStore {
    fn load(&self) -> Result<Option<SyncApplicationConfig>, SyncApplicationError> {
        Ok(None)
    }

    fn save(&self, _config: &SyncApplicationConfig) -> Result<(), SyncApplicationError> {
        Err(SyncApplicationError::Config("save failed".to_string()))
    }

    fn delete(&self) -> Result<(), SyncApplicationError> {
        Ok(())
    }
}

#[tokio::test]
async fn config_failure_does_not_roll_back_remote_or_local_vault_creation() {
    let repository_factory = Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
        Database::open_in_memory().unwrap(),
    )));
    let application = SyncApplication::new(
        Arc::new(FailingSaveConfigStore),
        repository_factory.clone(),
        SyncProviderRegistry::new([
            Arc::new(TestProviderFactory::default()) as Arc<dyn SyncProviderFactory>
        ]),
        Arc::new(MemorySecretStore::default()),
        Arc::new(FixedEnvironment::new(["vault-a", "device-a"])),
    );

    assert!(matches!(
        application
            .create(
                SyncProviderInput {
                    provider_id: "test".to_string(),
                    configuration: serde_json::json!({
                        "account": "alice",
                        "password": "provider-secret"
                    }),
                },
                SyncPresetV1::Standard,
                "master-password",
                false,
            )
            .await,
        Err(SyncApplicationError::Config(message)) if message == "save failed"
    ));
    assert!(repository_factory.open().unwrap().is_some());
}

#[derive(Default)]
struct BlockingStore {
    inner: MemoryStore,
    block_next_list: AtomicBool,
    entered: Notify,
    release: Notify,
}

#[async_trait]
impl SyncObjectStore for BlockingStore {
    async fn probe(&self) -> Result<SyncObjectStoreCapabilities, SyncError> {
        self.inner.probe().await
    }

    async fn list(
        &self,
        prefix: &SyncObjectPrefix,
        continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError> {
        if self.block_next_list.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.inner.list(prefix, continuation).await
    }

    async fn get(&self, key: &SyncObjectKey) -> Result<Option<SyncObject>, SyncError> {
        self.inner.get(key).await
    }

    async fn put_if_absent(
        &self,
        key: &SyncObjectKey,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        self.inner.put_if_absent(key, bytes).await
    }

    async fn compare_and_swap(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        self.inner.compare_and_swap(key, expected_etag, bytes).await
    }

    async fn delete(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError> {
        self.inner.delete(key, expected_etag).await
    }
}

#[tokio::test]
async fn concurrent_run_reports_syncing_and_rejects_the_second_run() {
    let store = Arc::new(BlockingStore::default());
    let application = Arc::new(SyncApplication::new(
        Arc::new(MemoryConfigStore::default()),
        Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
            Database::open_in_memory().unwrap(),
        ))),
        SyncProviderRegistry::new([Arc::new(TestProviderFactory {
            store: store.clone(),
        }) as Arc<dyn SyncProviderFactory>]),
        Arc::new(MemorySecretStore::default()),
        Arc::new(FixedEnvironment::new(["vault-a", "device-a"])),
    ));
    application
        .create(
            SyncProviderInput {
                provider_id: "test".to_string(),
                configuration: serde_json::json!({
                    "account": "alice",
                    "password": "provider-secret"
                }),
            },
            SyncPresetV1::Standard,
            "master-password",
            false,
        )
        .await
        .unwrap();
    store.block_next_list.store(true, Ordering::SeqCst);

    let running_application = application.clone();
    let running = tokio::spawn(async move { running_application.run().await });
    store.entered.notified().await;

    assert_eq!(
        application.status().await.unwrap().state,
        SyncLifecycleState::Syncing
    );
    assert!(matches!(
        application.run().await,
        Err(SyncApplicationError::InvalidState(message))
            if message == "A sync run is already in progress."
    ));

    store.release.notify_one();
    let _ = running.await.unwrap();
}

#[tokio::test]
async fn disconnect_waits_for_an_active_run_and_cannot_be_undone_by_retry_persistence() {
    let store = Arc::new(BlockingStore::default());
    let config = Arc::new(MemoryConfigStore::default());
    let application = Arc::new(SyncApplication::new(
        config.clone(),
        Arc::new(SqliteSyncRepositoryFactory::new(Arc::new(
            Database::open_in_memory().unwrap(),
        ))),
        SyncProviderRegistry::new([Arc::new(TestProviderFactory {
            store: store.clone(),
        }) as Arc<dyn SyncProviderFactory>]),
        Arc::new(MemorySecretStore::default()),
        Arc::new(FixedEnvironment::new(["vault-a", "device-a"])),
    ));
    application
        .create(
            SyncProviderInput {
                provider_id: "test".to_string(),
                configuration: serde_json::json!({
                    "account": "alice",
                    "password": "provider-secret"
                }),
            },
            SyncPresetV1::Standard,
            "master-password",
            false,
        )
        .await
        .unwrap();
    store.block_next_list.store(true, Ordering::SeqCst);

    let running_application = application.clone();
    let running = tokio::spawn(async move { running_application.run().await });
    store.entered.notified().await;

    let disconnecting_application = application.clone();
    let disconnecting = tokio::spawn(async move { disconnecting_application.disconnect().await });
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;
    assert!(
        !disconnecting.is_finished(),
        "disconnect must wait for the active run to finish"
    );

    store.release.notify_one();
    let _ = running.await.unwrap();
    assert_eq!(
        disconnecting.await.unwrap().unwrap().state,
        SyncLifecycleState::Disabled
    );
    assert!(config.load().unwrap().is_none());
    assert_eq!(
        application.status().await.unwrap().state,
        SyncLifecycleState::Disabled
    );
}
