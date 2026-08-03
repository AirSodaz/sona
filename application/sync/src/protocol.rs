use serde::{Deserialize, Serialize};
use sona_core::sync::{
    SYNC_PROTOCOL_VERSION, SyncCausalContext, SyncError, SyncObjectKey, SyncOperation,
};

pub const MAX_SEGMENT_OPERATIONS: usize = 256;
pub const MAX_SEGMENT_ENCODED_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SINGLE_OPERATION_BYTES: usize = 64 * 1024 * 1024;
pub const CHECKPOINT_OPERATION_THRESHOLD: u64 = 1_000;
pub const CHECKPOINT_BYTE_THRESHOLD: u64 = 32 * 1024 * 1024;

pub fn should_publish_checkpoint(operations: u64, bytes: u64) -> bool {
    operations >= CHECKPOINT_OPERATION_THRESHOLD || bytes >= CHECKPOINT_BYTE_THRESHOLD
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncSegmentV1 {
    pub protocol_version: u64,
    pub vault_id: String,
    pub device_id: String,
    pub sequence: u64,
    pub previous_cipher_hash: Option<String>,
    pub created_at_ms: u64,
    pub operations: Vec<SyncOperation>,
}

impl SyncSegmentV1 {
    pub fn validate(&self) -> Result<(), SyncError> {
        validate_protocol_version(self.protocol_version)?;
        validate_component(&self.vault_id, "vault ID")?;
        validate_component(&self.device_id, "device ID")?;
        if self.sequence == 0 {
            return Err(protocol_error(
                "Segment sequence must be greater than zero.",
            ));
        }
        if self.operations.is_empty() {
            return Err(protocol_error(
                "Segment must contain at least one operation.",
            ));
        }
        if self.operations.len() > MAX_SEGMENT_OPERATIONS {
            return Err(protocol_error(format!(
                "Segment operation limit exceeded: {} > {MAX_SEGMENT_OPERATIONS}.",
                self.operations.len()
            )));
        }
        for operation in &self.operations {
            if operation.source_device_id != self.device_id {
                return Err(protocol_error(
                    "Segment operation source device does not match the segment device.",
                ));
            }
        }
        let encoded = serde_json::to_vec(self)
            .map_err(|error| protocol_error(format!("Segment serialization failed: {error}")))?;
        if encoded.len() > MAX_SEGMENT_ENCODED_BYTES && self.operations.len() > 1 {
            return Err(protocol_error(format!(
                "Segment encoded byte limit exceeded: {} > {MAX_SEGMENT_ENCODED_BYTES}.",
                encoded.len()
            )));
        }
        if encoded.len() > MAX_SINGLE_OPERATION_BYTES {
            return Err(protocol_error(format!(
                "Segment single-operation byte limit exceeded: {} > {MAX_SINGLE_OPERATION_BYTES}.",
                encoded.len()
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncCheckpointV1 {
    pub protocol_version: u64,
    pub vault_id: String,
    pub device_id: String,
    pub sequence: u64,
    pub covered_segment_cipher_hash: String,
    pub created_at_ms: u64,
    pub causal_context: SyncCausalContext,
    pub operations: Vec<SyncOperation>,
}

impl SyncCheckpointV1 {
    pub fn validate(&self) -> Result<(), SyncError> {
        validate_protocol_version(self.protocol_version)?;
        validate_component(&self.vault_id, "vault ID")?;
        validate_component(&self.device_id, "device ID")?;
        if self.sequence == 0 {
            return Err(protocol_error(
                "Checkpoint sequence must be greater than zero.",
            ));
        }
        validate_component(
            &self.covered_segment_cipher_hash,
            "covered segment cipher hash",
        )?;
        for operation in &self.operations {
            let observed = if operation.source_device_id == self.device_id {
                operation.source_sequence <= self.sequence
            } else {
                self.causal_context
                    .observes(&operation.source_device_id, operation.source_sequence)
            };
            if !observed {
                return Err(protocol_error(
                    "Checkpoint contains an operation outside its causal context.",
                ));
            }
        }
        Ok(())
    }
}

pub fn segment_object_key(
    vault_id: &str,
    device_id: &str,
    sequence: u64,
    cipher_hash: &str,
) -> Result<SyncObjectKey, SyncError> {
    object_key(vault_id, device_id, "segments", sequence, cipher_hash)
}

pub fn checkpoint_object_key(
    vault_id: &str,
    device_id: &str,
    sequence: u64,
    cipher_hash: &str,
) -> Result<SyncObjectKey, SyncError> {
    object_key(vault_id, device_id, "checkpoints", sequence, cipher_hash)
}

fn object_key(
    vault_id: &str,
    device_id: &str,
    collection: &str,
    sequence: u64,
    cipher_hash: &str,
) -> Result<SyncObjectKey, SyncError> {
    validate_component(vault_id, "vault ID")?;
    validate_component(device_id, "device ID")?;
    validate_component(cipher_hash, "cipher hash")?;
    if sequence == 0 {
        return Err(protocol_error("Object sequence must be greater than zero."));
    }
    SyncObjectKey::parse(format!(
        "sona-sync/v2/{vault_id}/devices/{device_id}/{collection}/{sequence:020}-{cipher_hash}.sync"
    ))
}

fn validate_protocol_version(version: u64) -> Result<(), SyncError> {
    if version == SYNC_PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(protocol_error(format!(
            "Sync protocol version {version} is incompatible with this client (version {SYNC_PROTOCOL_VERSION}). Upgrade every connected client before resuming sync."
        )))
    }
}

fn validate_component(value: &str, label: &str) -> Result<(), SyncError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(protocol_error(format!("Invalid {label}.")));
    }
    Ok(())
}

fn protocol_error(message: impl Into<String>) -> SyncError {
    SyncError::Protocol(message.into())
}
