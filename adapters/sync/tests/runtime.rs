use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;
use sona_core::sync::{
    HybridLogicalClock, SyncCausalContext, SyncDeleteResult, SyncDeviceCursor, SyncEntityKey,
    SyncEntityKind, SyncError, SyncListPage, SyncLocalRepository, SyncLocalRuntimeState,
    SyncObject, SyncObjectKey, SyncObjectMetadata, SyncObjectPrefix, SyncObjectStore,
    SyncObjectStoreCapabilities, SyncOperation, SyncOperationKind, SyncPresetV1,
    SyncPublishedCheckpoint, SyncPublishedSegment, SyncPutResult, SyncRemoteApplyResult,
    SyncRemoteSegment, SyncVersion,
};
use sona_sync::{SyncRuntime, create_vault, load_remote_state_for_join};

#[derive(Clone, Default)]
struct MemoryStore {
    objects: Arc<Mutex<BTreeMap<String, (Vec<u8>, String)>>>,
    fail_after_create_once: Arc<Mutex<bool>>,
    omit_etags: Arc<AtomicBool>,
}

impl MemoryStore {
    fn object_count(&self) -> usize {
        self.objects.lock().unwrap().len()
    }

    fn fail_next_put_after_create(&self) {
        *self.fail_after_create_once.lock().unwrap() = true;
    }

    fn omit_etags(&self) {
        self.omit_etags.store(true, Ordering::SeqCst);
    }

    fn remove_segments_for_sequence(&self, sequence: u64) {
        let marker = format!("/segments/{sequence:020}-");
        self.objects
            .lock()
            .unwrap()
            .retain(|key, _| !key.contains(&marker));
    }

    fn object_with_path_fragment(&self, fragment: &str) -> (String, Vec<u8>) {
        self.objects
            .lock()
            .unwrap()
            .iter()
            .find(|(key, _)| key.contains(fragment))
            .map(|(key, (bytes, _))| (key.clone(), bytes.clone()))
            .expect("matching object should exist")
    }

    fn keys(&self) -> Vec<String> {
        self.objects.lock().unwrap().keys().cloned().collect()
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
        prefix: &SyncObjectPrefix,
        _continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError> {
        let objects = self
            .objects
            .lock()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.starts_with(prefix.as_str()))
            .map(|(key, (bytes, etag))| SyncObjectMetadata {
                key: SyncObjectKey::parse(key.clone()).unwrap(),
                etag: (!self.omit_etags.load(Ordering::SeqCst)).then(|| etag.clone()),
                size: bytes.len() as u64,
                modified_at: None,
            })
            .collect();
        Ok(SyncListPage {
            objects,
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
                    etag: (!self.omit_etags.load(Ordering::SeqCst)).then(|| etag.clone()),
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
        let etag = format!("etag-{}", objects.len() + 1);
        objects.insert(key.to_string(), (bytes, etag.clone()));
        if std::mem::take(&mut *self.fail_after_create_once.lock().unwrap()) {
            return Err(SyncError::ObjectStore(
                "simulated response loss after create".to_string(),
            ));
        }
        Ok(SyncPutResult::Created { etag: Some(etag) })
    }

    async fn compare_and_swap(
        &self,
        _key: &SyncObjectKey,
        _expected_etag: Option<&str>,
        _bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        unreachable!("runtime segment flow only uses immutable writes")
    }

    async fn delete(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError> {
        let mut objects = self.objects.lock().unwrap();
        let Some((_, current_etag)) = objects.get(key.as_str()) else {
            return Ok(SyncDeleteResult::NotFound);
        };
        if expected_etag.is_some_and(|expected| expected != current_etag) {
            return Ok(SyncDeleteResult::Conflict {
                current_etag: Some(current_etag.clone()),
            });
        }
        objects.remove(key.as_str());
        Ok(SyncDeleteResult::Deleted)
    }
}

struct FakeLocalRepository {
    inner: Mutex<FakeLocalState>,
}

struct FakeLocalState {
    runtime: SyncLocalRuntimeState,
    pending: Vec<SyncOperation>,
    checkpoint_operations: Vec<SyncOperation>,
    published_checkpoints: Vec<SyncPublishedCheckpoint>,
    applied: Vec<SyncOperation>,
}

impl FakeLocalRepository {
    fn new(device_id: &str, pending: Vec<SyncOperation>) -> Self {
        Self {
            inner: Mutex::new(FakeLocalState {
                runtime: SyncLocalRuntimeState {
                    vault_id: "vault-a".to_string(),
                    device_id: device_id.to_string(),
                    preset: SyncPresetV1::Standard,
                    next_sequence: 1,
                    previous_cipher_hash: None,
                    remote_cursors: BTreeMap::new(),
                    operations_since_checkpoint: 0,
                    bytes_since_checkpoint: 0,
                    checkpoint_required: false,
                },
                checkpoint_operations: pending.clone(),
                pending,
                published_checkpoints: Vec::new(),
                applied: Vec::new(),
            }),
        }
    }

    fn applied_operations(&self) -> Vec<SyncOperation> {
        self.inner.lock().unwrap().applied.clone()
    }

    fn set_checkpoint_progress(&self, operations: u64, bytes: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.runtime.operations_since_checkpoint = operations;
        inner.runtime.bytes_since_checkpoint = bytes;
    }

    fn published_checkpoint_count(&self) -> usize {
        self.inner.lock().unwrap().published_checkpoints.len()
    }

    fn require_checkpoint(&self) {
        self.inner.lock().unwrap().runtime.checkpoint_required = true;
    }

    fn add_pending(&self, operation: SyncOperation) {
        let mut inner = self.inner.lock().unwrap();
        inner.pending.push(operation.clone());
        inner.checkpoint_operations.push(operation);
    }
}

impl SyncLocalRepository for FakeLocalRepository {
    fn load_runtime_state(&self) -> Result<SyncLocalRuntimeState, SyncError> {
        Ok(self.inner.lock().unwrap().runtime.clone())
    }

    fn load_pending_operations(
        &self,
        _preset: SyncPresetV1,
        maximum_operations: usize,
        _maximum_bytes: usize,
    ) -> Result<Vec<SyncOperation>, SyncError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .pending
            .iter()
            .take(maximum_operations)
            .cloned()
            .collect())
    }

    fn mark_segment_published(&self, published: &SyncPublishedSegment) -> Result<(), SyncError> {
        let mut inner = self.inner.lock().unwrap();
        let operation_ids = published
            .operations
            .iter()
            .map(|operation| operation.operation_id.as_str())
            .collect::<Vec<_>>();
        inner
            .pending
            .retain(|operation| !operation_ids.contains(&operation.operation_id.as_str()));
        for published_operation in &published.operations {
            if let Some(checkpoint_operation) = inner
                .checkpoint_operations
                .iter_mut()
                .find(|operation| operation.operation_id == published_operation.operation_id)
            {
                *checkpoint_operation = published_operation.clone();
            }
        }
        inner.runtime.next_sequence = published.sequence + 1;
        inner.runtime.previous_cipher_hash = Some(published.cipher_hash.clone());
        inner.runtime.operations_since_checkpoint += published.operations.len() as u64;
        inner.runtime.bytes_since_checkpoint += published.encrypted_bytes;
        Ok(())
    }

    fn load_checkpoint_operations(&self) -> Result<Vec<SyncOperation>, SyncError> {
        Ok(self.inner.lock().unwrap().checkpoint_operations.clone())
    }

    fn mark_checkpoint_published(
        &self,
        checkpoint: &SyncPublishedCheckpoint,
    ) -> Result<(), SyncError> {
        let mut inner = self.inner.lock().unwrap();
        inner.published_checkpoints.push(checkpoint.clone());
        inner.runtime.operations_since_checkpoint = 0;
        inner.runtime.bytes_since_checkpoint = 0;
        Ok(())
    }

    fn apply_remote_segment(
        &self,
        segment: &SyncRemoteSegment,
    ) -> Result<SyncRemoteApplyResult, SyncError> {
        let mut inner = self.inner.lock().unwrap();
        inner.applied.extend(segment.operations.iter().cloned());
        inner.runtime.remote_cursors.insert(
            segment.device_id.clone(),
            SyncDeviceCursor {
                sequence: segment.sequence,
                cipher_hash: segment.cipher_hash.clone(),
            },
        );
        Ok(SyncRemoteApplyResult {
            applied_operation_count: segment.operations.len() as u64,
            conflict_count: 0,
        })
    }
}

fn operation() -> SyncOperation {
    operation_with("op-a", "device-a", 1, "Shared project")
}

fn operation_with(
    operation_id: &str,
    device_id: &str,
    source_sequence: u64,
    value: &str,
) -> SyncOperation {
    SyncOperation {
        operation_id: operation_id.to_string(),
        source_device_id: device_id.to_string(),
        source_sequence,
        causal_context: SyncCausalContext::default(),
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms: 10,
                logical: 0,
            },
            device_id: device_id.to_string(),
            operation_id: operation_id.to_string(),
        },
        entity: SyncEntityKey {
            kind: SyncEntityKind::Project,
            id: "project-a".to_string(),
        },
        kind: SyncOperationKind::SetField {
            field: "name".to_string(),
            value: json!(value),
        },
    }
}

#[tokio::test]
async fn two_devices_exchange_incremental_segments_without_provider_specific_logic() {
    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device_a = FakeLocalRepository::new("device-a", vec![operation()]);
    let device_b = FakeLocalRepository::new("device-b", Vec::new());

    let pushed = SyncRuntime::new(&device_a, &store, created.vault_key.as_slice())
        .run_at(100)
        .await
        .unwrap();
    assert_eq!(pushed.pushed_segment_count, 1);
    assert_eq!(pushed.published_operation_count, 1);
    assert_eq!(store.object_count(), 1);

    let pulled = SyncRuntime::new(&device_b, &store, created.vault_key.as_slice())
        .run_at(200)
        .await
        .unwrap();
    assert_eq!(pulled.pulled_segment_count, 1);
    assert_eq!(pulled.applied_operation_count, 1);
    assert_eq!(device_b.applied_operations(), vec![operation()]);

    let idempotent = SyncRuntime::new(&device_b, &store, created.vault_key.as_slice())
        .run_at(300)
        .await
        .unwrap();
    assert_eq!(idempotent.pulled_segment_count, 0);
    assert_eq!(device_b.applied_operations(), vec![operation()]);
}

#[tokio::test]
async fn runtime_publishes_a_checkpoint_when_the_operation_threshold_is_reached() {
    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device = FakeLocalRepository::new("device-a", vec![operation()]);
    device.set_checkpoint_progress(999, 0);

    let result = SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(100)
        .await
        .unwrap();

    assert!(result.checkpoint_published);
    assert_eq!(device.published_checkpoint_count(), 1);
    assert_eq!(store.object_count(), 2);
}

#[tokio::test]
async fn runtime_publishes_the_initial_checkpoint_below_the_regular_threshold() {
    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device = FakeLocalRepository::new("device-a", vec![operation()]);
    device.require_checkpoint();

    let result = SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(100)
        .await
        .unwrap();

    assert!(result.checkpoint_published);
    assert_eq!(device.published_checkpoint_count(), 1);
}

#[tokio::test]
async fn retry_after_an_ambiguous_upload_reuses_the_same_immutable_object() {
    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    store.fail_next_put_after_create();
    let device = FakeLocalRepository::new(
        "device-a",
        vec![operation_with("op-retry", "device-a", 0, "retry")],
    );
    let runtime = SyncRuntime::new(&device, &store, created.vault_key.as_slice());

    assert!(runtime.run_at(100).await.is_err());
    assert_eq!(store.object_count(), 1);

    let retried = runtime.run_at(200).await.unwrap();
    assert_eq!(retried.pushed_segment_count, 1);
    assert_eq!(store.object_count(), 1);
}

#[tokio::test]
async fn published_operations_use_their_segment_sequence() {
    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device = FakeLocalRepository::new(
        "device-a",
        vec![operation_with("op-sequence", "device-a", 0, "sequenced")],
    );

    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(100)
        .await
        .unwrap();

    let (_, bytes) = store.object_with_path_fragment("/segments/");
    let segment: sona_sync::SyncSegmentV1 = sona_sync::open_json(
        created.vault_key.as_slice(),
        b"sona-sync/v1/vault-a/devices/device-a/segments/00000000000000000001",
        &bytes,
    )
    .unwrap();
    assert_eq!(segment.operations[0].source_sequence, 1);
}

#[tokio::test]
async fn a_new_device_recovers_from_a_checkpoint_after_covered_segments_are_removed() {
    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device_a = FakeLocalRepository::new("device-a", vec![operation()]);
    device_a.set_checkpoint_progress(999, 0);

    SyncRuntime::new(&device_a, &store, created.vault_key.as_slice())
        .run_at(100)
        .await
        .unwrap();
    store.remove_segments_for_sequence(1);
    device_a.add_pending(operation_with("op-tail", "device-a", 2, "tail"));
    SyncRuntime::new(&device_a, &store, created.vault_key.as_slice())
        .run_at(200)
        .await
        .unwrap();

    let device_b = FakeLocalRepository::new("device-b", Vec::new());
    let joined = SyncRuntime::new(&device_b, &store, created.vault_key.as_slice())
        .run_at(300)
        .await
        .unwrap();

    assert_eq!(joined.pulled_checkpoint_count, 1);
    assert_eq!(joined.pulled_segment_count, 1);
    assert_eq!(joined.applied_operation_count, 2);
    assert_eq!(device_b.applied_operations().len(), 2);
}

#[tokio::test]
async fn join_preview_loader_returns_checkpoint_and_tail_without_applying_them() {
    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device = FakeLocalRepository::new("device-a", vec![operation()]);
    device.set_checkpoint_progress(999, 0);

    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(100)
        .await
        .unwrap();
    store.remove_segments_for_sequence(1);
    device.add_pending(operation_with("op-tail", "device-a", 0, "tail"));
    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(200)
        .await
        .unwrap();

    let batches = load_remote_state_for_join(&store, "vault-a", created.vault_key.as_slice())
        .await
        .unwrap();

    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].sequence, 1);
    assert_eq!(batches[1].sequence, 2);
    assert_eq!(
        batches
            .iter()
            .map(|batch| batch.operations.len())
            .sum::<usize>(),
        2
    );
    assert_eq!(device.applied_operations(), Vec::<SyncOperation>::new());
}

#[tokio::test]
async fn garbage_collection_keeps_two_checkpoints_and_only_recent_or_uncovered_segments() {
    const DAY_MS: u64 = 24 * 60 * 60 * 1_000;

    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device = FakeLocalRepository::new("device-a", vec![operation()]);
    device.set_checkpoint_progress(999, 0);
    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(DAY_MS)
        .await
        .unwrap();

    device.add_pending(operation_with("op-2", "device-a", 0, "second"));
    device.set_checkpoint_progress(999, 0);
    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(2 * DAY_MS)
        .await
        .unwrap();

    device.add_pending(operation_with("op-3", "device-a", 0, "third"));
    device.set_checkpoint_progress(999, 0);
    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(33 * DAY_MS)
        .await
        .unwrap();

    let keys = store.keys();
    assert_eq!(
        keys.iter()
            .filter(|key| key.contains("/checkpoints/"))
            .count(),
        2
    );
    assert_eq!(
        keys.iter().filter(|key| key.contains("/segments/")).count(),
        1
    );
    assert!(
        keys.iter()
            .any(|key| key.contains("/segments/00000000000000000003-"))
    );
}

#[tokio::test]
async fn garbage_collection_skips_objects_without_etags() {
    const DAY_MS: u64 = 24 * 60 * 60 * 1_000;

    let created = create_vault(
        "vault-a",
        SyncPresetV1::Standard,
        "correct horse battery staple",
        false,
    )
    .unwrap();
    let store = MemoryStore::default();
    let device = FakeLocalRepository::new("device-a", vec![operation()]);
    device.set_checkpoint_progress(999, 0);
    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(DAY_MS)
        .await
        .unwrap();

    device.add_pending(operation_with("op-2", "device-a", 0, "second"));
    device.set_checkpoint_progress(999, 0);
    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(2 * DAY_MS)
        .await
        .unwrap();

    store.omit_etags();
    device.add_pending(operation_with("op-3", "device-a", 0, "third"));
    device.set_checkpoint_progress(999, 0);
    SyncRuntime::new(&device, &store, created.vault_key.as_slice())
        .run_at(33 * DAY_MS)
        .await
        .unwrap();

    let keys = store.keys();
    assert_eq!(
        keys.iter()
            .filter(|key| key.contains("/checkpoints/"))
            .count(),
        3
    );
    assert_eq!(
        keys.iter().filter(|key| key.contains("/segments/")).count(),
        3
    );
}
