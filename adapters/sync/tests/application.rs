use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use sona_core::sync::{
    SyncDeleteResult, SyncError, SyncLifecycleState, SyncListPage, SyncLocalRepository,
    SyncLocalRuntimeState, SyncObject, SyncObjectKey, SyncObjectMetadata, SyncObjectPrefix,
    SyncObjectStore, SyncObjectStoreCapabilities, SyncOperation, SyncPresetV1,
    SyncPublishedCheckpoint, SyncPublishedSegment, SyncPutResult, SyncRemoteApplyResult,
    SyncRemoteSegment, SyncRunResult,
};
use sona_sync::{
    SyncPresetChangeError, SyncRetryState, SyncStatusContext, apply_sync_run_result,
    build_sync_status, change_sync_preset, create_remote_vault, disabled_sync_status,
};

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
