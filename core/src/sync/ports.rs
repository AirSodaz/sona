use std::collections::BTreeMap;

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

pub trait SyncSecretStore: Send + Sync {
    fn read_secret(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError>;
    fn write_secret(&self, key: &str, value: &[u8]) -> Result<(), SyncError>;
    fn delete_secret(&self, key: &str) -> Result<(), SyncError>;
}
