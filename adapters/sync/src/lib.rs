//! Provider-neutral encrypted synchronization for Sona.

mod backoff;
mod crypto;
mod legacy_backup;
mod protocol;
mod runtime;
mod vault;

pub use backoff::SyncBackoffPolicy;
pub use crypto::{
    CreatedVault, PasswordKeySlotV1, RecoveryKeySlotV1, VaultHeaderV1, change_master_password,
    create_vault, open_json, seal_json, unlock_with_master_password, unlock_with_recovery_key,
};
pub use legacy_backup::{
    LegacyRemoteBackupEntry, LegacyRemoteBackupService, legacy_provider_credential_key,
};
pub use protocol::{
    CHECKPOINT_BYTE_THRESHOLD, CHECKPOINT_OPERATION_THRESHOLD, MAX_SEGMENT_ENCODED_BYTES,
    MAX_SEGMENT_OPERATIONS, MAX_SINGLE_OPERATION_BYTES, SyncCheckpointV1, SyncSegmentV1,
    checkpoint_object_key, segment_object_key, should_publish_checkpoint,
};
pub use runtime::{SyncRuntime, load_remote_state_for_join};
pub use vault::{
    CreatedRemoteVault, OpenedRemoteVault, change_remote_master_password, create_remote_vault,
    open_remote_vault_with_password, open_remote_vault_with_recovery_key,
    open_remote_vault_with_vault_key, regenerate_remote_recovery_key, update_remote_vault_preset,
    vault_header_object_key,
};
