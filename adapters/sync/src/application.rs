use serde::{Deserialize, Serialize};
use sona_core::sync::{
    SyncError, SyncErrorSnapshot, SyncLifecycleState, SyncLocalRepository, SyncObjectStore,
    SyncPresetV1, SyncRunResult, SyncStatusSnapshot,
};

use crate::vault::{OpenedRemoteVault, update_remote_vault_preset};
use crate::{SyncBackoffPolicy, SyncRuntime};

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum SyncPresetChangeError {
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error("Local preset update failed ({local}); remote rollback also failed ({rollback}).")]
    LocalUpdateAndRollback {
        local: SyncError,
        rollback: SyncError,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncRetryState {
    pub last_success_at_ms: Option<u64>,
    pub consecutive_failures: u32,
    pub next_retry_at_ms: Option<u64>,
    pub last_error: Option<SyncErrorSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncStatusContext {
    pub provider_id: String,
    pub vault_id: String,
    pub preset: SyncPresetV1,
    pub paused: bool,
    pub unlocked: bool,
    pub syncing: bool,
    pub pending_operation_count: u64,
    pub conflict_count: u64,
}

pub fn disabled_sync_status() -> SyncStatusSnapshot {
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

pub fn build_sync_status(context: SyncStatusContext, retry: &SyncRetryState) -> SyncStatusSnapshot {
    let state = if context.syncing {
        SyncLifecycleState::Syncing
    } else if context.paused {
        SyncLifecycleState::Paused
    } else if !context.unlocked {
        SyncLifecycleState::Locked
    } else if retry.last_error.is_some() {
        SyncLifecycleState::Error
    } else {
        SyncLifecycleState::Idle
    };

    SyncStatusSnapshot {
        state,
        provider_id: Some(context.provider_id),
        vault_id: Some(context.vault_id),
        preset: Some(context.preset),
        last_success_at_ms: retry.last_success_at_ms,
        pending_operation_count: context.pending_operation_count,
        conflict_count: context.conflict_count,
        next_retry_at_ms: retry.next_retry_at_ms,
        last_error: retry.last_error.clone(),
    }
}

pub fn sync_error_code(error: &SyncError) -> &'static str {
    match error {
        SyncError::InvalidOperation(_) => "invalid_operation",
        SyncError::InvalidObjectKey(_) => "invalid_object_key",
        SyncError::ObjectStore(_) => "provider_error",
        SyncError::LocalRepository(_) => "local_repository_error",
        SyncError::SecretStore(_) => "secret_store_error",
        SyncError::Protocol(_) => "protocol_error",
        SyncError::Crypto(_) => "crypto_error",
    }
}

pub fn is_retryable_sync_error(error: &SyncError) -> bool {
    matches!(error, SyncError::ObjectStore(_))
}

pub fn apply_sync_run_result(
    retry: &mut SyncRetryState,
    now_ms: u64,
    jitter: u32,
    result: &Result<SyncRunResult, SyncError>,
) {
    match result {
        Ok(_) => {
            retry.last_success_at_ms = Some(now_ms);
            retry.consecutive_failures = 0;
            retry.next_retry_at_ms = None;
            retry.last_error = None;
        }
        Err(error) => {
            retry.consecutive_failures = retry.consecutive_failures.saturating_add(1);
            retry.next_retry_at_ms = Some(SyncBackoffPolicy::default().next_retry_at_ms(
                now_ms,
                retry.consecutive_failures,
                jitter,
            ));
            retry.last_error = Some(SyncErrorSnapshot {
                code: sync_error_code(error).to_string(),
                message: error.to_string(),
                retryable: is_retryable_sync_error(error),
            });
        }
    }
}

pub async fn run_sync_cycle(
    local: &dyn SyncLocalRepository,
    remote: &dyn SyncObjectStore,
    vault_key: &[u8],
    retry: &mut SyncRetryState,
    now_ms: u64,
    jitter: u32,
) -> Result<SyncRunResult, SyncError> {
    let result = SyncRuntime::new(local, remote, vault_key)
        .run_at(now_ms)
        .await;
    apply_sync_run_result(retry, now_ms, jitter, &result);
    result
}

pub async fn change_sync_preset(
    local: &dyn SyncLocalRepository,
    remote: &dyn SyncObjectStore,
    opened: &mut OpenedRemoteVault,
    preset: SyncPresetV1,
    confirm_shrink: bool,
) -> Result<(), SyncPresetChangeError> {
    local.validate_preset_change(preset, confirm_shrink)?;
    let previous = opened.header.preset;
    update_remote_vault_preset(remote, opened, preset).await?;

    if let Err(local_error) = local.change_preset(preset, confirm_shrink) {
        return match update_remote_vault_preset(remote, opened, previous).await {
            Ok(()) => Err(local_error.into()),
            Err(rollback_error) => Err(SyncPresetChangeError::LocalUpdateAndRollback {
                local: local_error,
                rollback: rollback_error,
            }),
        };
    }

    Ok(())
}
