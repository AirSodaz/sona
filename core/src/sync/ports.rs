use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;

use super::{SyncError, SyncObjectKey, SyncObjectPrefix, SyncOperation, SyncPresetV1};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncObjectMetadata {
    pub key: SyncObjectKey,
    pub etag: Option<String>,
    pub size: u64,
    pub modified_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncObject {
    pub metadata: SyncObjectMetadata,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncObjectStoreCapabilities {
    pub conditional_create: bool,
    pub compare_and_swap: bool,
    pub delete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncListPage {
    pub objects: Vec<SyncObjectMetadata>,
    pub continuation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncPutResult {
    Created { etag: Option<String> },
    AlreadyExists { etag: Option<String> },
    Conflict { current_etag: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncDeleteResult {
    Deleted,
    NotFound,
    Conflict { current_etag: Option<String> },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncDeviceCursor {
    pub sequence: u64,
    pub cipher_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncLocalRuntimeState {
    pub vault_id: String,
    pub device_id: String,
    pub preset: SyncPresetV1,
    pub next_sequence: u64,
    pub previous_cipher_hash: Option<String>,
    pub remote_cursors: BTreeMap<String, SyncDeviceCursor>,
    pub operations_since_checkpoint: u64,
    pub bytes_since_checkpoint: u64,
    pub checkpoint_required: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncPublishedSegment {
    pub sequence: u64,
    pub cipher_hash: String,
    pub operations: Vec<SyncOperation>,
    pub encrypted_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPublishedCheckpoint {
    pub sequence: u64,
    pub cipher_hash: String,
    pub encrypted_bytes: u64,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncRemoteSegment {
    pub device_id: String,
    pub sequence: u64,
    pub cipher_hash: String,
    pub operations: Vec<SyncOperation>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncRemoteApplyResult {
    pub applied_operation_count: u64,
    pub conflict_count: u64,
}

#[async_trait]
pub trait SyncObjectStore: Send + Sync {
    async fn probe(&self) -> Result<SyncObjectStoreCapabilities, SyncError>;
    async fn list(
        &self,
        prefix: &SyncObjectPrefix,
        continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError>;
    async fn get(&self, key: &SyncObjectKey) -> Result<Option<SyncObject>, SyncError>;
    async fn put_if_absent(
        &self,
        key: &SyncObjectKey,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError>;
    async fn compare_and_swap(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError>;
    async fn delete(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError>;
}

pub trait SyncLocalRepository: Send + Sync {
    fn load_runtime_state(&self) -> Result<SyncLocalRuntimeState, SyncError>;

    fn load_pending_operations(
        &self,
        preset: SyncPresetV1,
        maximum_operations: usize,
        maximum_bytes: usize,
    ) -> Result<Vec<SyncOperation>, SyncError>;

    fn mark_segment_published(&self, published: &SyncPublishedSegment) -> Result<(), SyncError>;

    fn load_checkpoint_operations(&self) -> Result<Vec<SyncOperation>, SyncError>;

    fn mark_checkpoint_published(
        &self,
        checkpoint: &SyncPublishedCheckpoint,
    ) -> Result<(), SyncError>;

    fn apply_remote_segment(
        &self,
        segment: &SyncRemoteSegment,
    ) -> Result<SyncRemoteApplyResult, SyncError>;

    fn validate_preset_change(
        &self,
        preset: SyncPresetV1,
        confirm_shrink: bool,
    ) -> Result<(), SyncError>;

    fn change_preset(&self, preset: SyncPresetV1, confirm_shrink: bool) -> Result<(), SyncError>;
}

/// Application-facing repository capabilities that sit alongside the runtime
/// repository consumed by the sync engine.
pub trait SyncApplicationRepository: Send + Sync {
    fn runtime_repository(&self) -> &dyn SyncLocalRepository;

    fn is_paused(&self) -> Result<bool, SyncError>;
    fn set_paused(&self, paused: bool) -> Result<(), SyncError>;
    fn pending_operation_count(&self) -> Result<u64, SyncError>;
    fn unresolved_conflict_count(&self) -> Result<u64, SyncError>;
    fn disconnect(&self) -> Result<(), SyncError>;
    fn list_conflict_summaries(&self) -> Result<Vec<super::SyncConflictSummary>, SyncError>;
    fn get_conflict_detail(
        &self,
        conflict_id: &str,
    ) -> Result<Option<super::SyncConflictDetail>, SyncError>;
    fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: super::SyncConflictResolution,
        resolved_at_ms: u64,
    ) -> Result<(), SyncError>;
}

/// Opens the local sync repository and owns initialization/preview operations
/// that require the adapter's underlying storage handle.
pub trait SyncRepositoryFactory: Send + Sync {
    fn open(&self) -> Result<Option<Arc<dyn SyncApplicationRepository>>, SyncError>;

    fn initialize(
        &self,
        vault_id: &str,
        device_id: &str,
        preset: SyncPresetV1,
    ) -> Result<Arc<dyn SyncApplicationRepository>, SyncError>;

    fn preview(
        &self,
        vault_id: &str,
        preview_device_id: &str,
        preset: SyncPresetV1,
        remote_segments: &[SyncRemoteSegment],
    ) -> Result<super::SyncJoinPreview, SyncError>;
}

#[async_trait]
pub trait SyncSecretStore: Send + Sync {
    async fn read_secret(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError>;
    async fn write_secret(&self, key: &str, value: &[u8]) -> Result<(), SyncError>;
    async fn delete_secret(&self, key: &str) -> Result<(), SyncError>;
}
