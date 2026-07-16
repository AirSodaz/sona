use rand::RngCore;
use rand::rngs::OsRng;
use sona_core::sync::{SyncError, SyncObjectKey, SyncObjectStore, SyncPresetV1, SyncPutResult};
use zeroize::{Zeroize, Zeroizing};

use crate::crypto::{
    CreatedVault, VaultHeaderV1, change_master_password, create_vault, unlock_with_master_password,
    unlock_with_recovery_key, wrap_with_recovery_secret,
};

const RECOVERY_KEY_BYTES: usize = 32;

#[derive(Debug)]
pub struct OpenedRemoteVault {
    pub header: VaultHeaderV1,
    pub vault_key: Zeroizing<Vec<u8>>,
    pub etag: Option<String>,
}

#[derive(Debug)]
pub struct CreatedRemoteVault {
    pub opened: OpenedRemoteVault,
    pub recovery_key: Option<String>,
}

pub async fn create_remote_vault(
    store: &dyn SyncObjectStore,
    vault_id: &str,
    preset: SyncPresetV1,
    master_password: &str,
    create_recovery_key: bool,
) -> Result<CreatedRemoteVault, SyncError> {
    let CreatedVault {
        header,
        vault_key,
        recovery_key,
    } = create_vault(vault_id, preset, master_password, create_recovery_key)?;
    let key = vault_header_object_key(vault_id)?;
    let bytes = serde_json::to_vec(&header)
        .map_err(|error| protocol_error(format!("Vault header encoding failed: {error}")))?;
    let etag = match store.put_if_absent(&key, bytes).await? {
        SyncPutResult::Created { etag } => etag,
        SyncPutResult::AlreadyExists { .. } | SyncPutResult::Conflict { .. } => {
            return Err(protocol_error("Sync vault already exists."));
        }
    };
    Ok(CreatedRemoteVault {
        opened: OpenedRemoteVault {
            header,
            vault_key,
            etag,
        },
        recovery_key,
    })
}

pub async fn open_remote_vault_with_password(
    store: &dyn SyncObjectStore,
    vault_id: &str,
    master_password: &str,
) -> Result<OpenedRemoteVault, SyncError> {
    let (header, etag) = load_remote_header(store, vault_id).await?;
    let vault_key = unlock_with_master_password(&header, master_password)?;
    Ok(OpenedRemoteVault {
        header,
        vault_key,
        etag,
    })
}

pub async fn open_remote_vault_with_recovery_key(
    store: &dyn SyncObjectStore,
    vault_id: &str,
    recovery_key: &str,
) -> Result<OpenedRemoteVault, SyncError> {
    let (header, etag) = load_remote_header(store, vault_id).await?;
    let vault_key = unlock_with_recovery_key(&header, recovery_key)?;
    Ok(OpenedRemoteVault {
        header,
        vault_key,
        etag,
    })
}

pub async fn open_remote_vault_with_vault_key(
    store: &dyn SyncObjectStore,
    vault_id: &str,
    vault_key: &[u8],
) -> Result<OpenedRemoteVault, SyncError> {
    if vault_key.len() != 32 {
        return Err(SyncError::Crypto(
            "Stored sync vault key has an invalid length.".to_string(),
        ));
    }
    let (header, etag) = load_remote_header(store, vault_id).await?;
    Ok(OpenedRemoteVault {
        header,
        vault_key: Zeroizing::new(vault_key.to_vec()),
        etag,
    })
}

pub async fn change_remote_master_password(
    store: &dyn SyncObjectStore,
    opened: &mut OpenedRemoteVault,
    current_master_password: &str,
    next_master_password: &str,
) -> Result<(), SyncError> {
    let changed = change_master_password(
        &opened.header,
        current_master_password,
        next_master_password,
    )?;
    replace_remote_header(store, opened, changed).await
}

pub async fn regenerate_remote_recovery_key(
    store: &dyn SyncObjectStore,
    opened: &mut OpenedRemoteVault,
) -> Result<String, SyncError> {
    let mut recovery_secret = Zeroizing::new(vec![0_u8; RECOVERY_KEY_BYTES]);
    OsRng.fill_bytes(&mut recovery_secret);
    let mut changed = opened.header.clone();
    changed.recovery_slot = Some(wrap_with_recovery_secret(
        &changed.vault_id,
        &opened.vault_key,
        &recovery_secret,
    )?);
    let recovery_key = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &*recovery_secret,
    );
    recovery_secret.zeroize();
    replace_remote_header(store, opened, changed).await?;
    Ok(recovery_key)
}

pub async fn update_remote_vault_preset(
    store: &dyn SyncObjectStore,
    opened: &mut OpenedRemoteVault,
    preset: SyncPresetV1,
) -> Result<(), SyncError> {
    let mut changed = opened.header.clone();
    changed.preset = preset;
    replace_remote_header(store, opened, changed).await
}

pub fn vault_header_object_key(vault_id: &str) -> Result<SyncObjectKey, SyncError> {
    SyncObjectKey::parse(format!("sona-sync/v1/{vault_id}/vault.json"))
}

async fn load_remote_header(
    store: &dyn SyncObjectStore,
    vault_id: &str,
) -> Result<(VaultHeaderV1, Option<String>), SyncError> {
    let key = vault_header_object_key(vault_id)?;
    let object = store
        .get(&key)
        .await?
        .ok_or_else(|| protocol_error("Sync vault was not found."))?;
    let header: VaultHeaderV1 = serde_json::from_slice(&object.bytes)
        .map_err(|error| protocol_error(format!("Vault header decoding failed: {error}")))?;
    if header.vault_id != vault_id {
        return Err(protocol_error(
            "Vault header ID does not match its object path.",
        ));
    }
    unlock_header_version(&header)?;
    Ok((header, object.metadata.etag))
}

async fn replace_remote_header(
    store: &dyn SyncObjectStore,
    opened: &mut OpenedRemoteVault,
    changed: VaultHeaderV1,
) -> Result<(), SyncError> {
    let key = vault_header_object_key(&changed.vault_id)?;
    let bytes = serde_json::to_vec(&changed)
        .map_err(|error| protocol_error(format!("Vault header encoding failed: {error}")))?;
    let etag = match store
        .compare_and_swap(&key, opened.etag.as_deref(), bytes)
        .await?
    {
        SyncPutResult::Created { etag } => etag,
        SyncPutResult::AlreadyExists { .. } | SyncPutResult::Conflict { .. } => {
            return Err(protocol_error(
                "Vault header changed on another device; reload before retrying.",
            ));
        }
    };
    opened.header = changed;
    opened.etag = etag;
    Ok(())
}

fn unlock_header_version(header: &VaultHeaderV1) -> Result<(), SyncError> {
    if header.protocol_version == sona_core::sync::SYNC_PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(protocol_error(format!(
            "Unsupported sync protocol version: {}.",
            header.protocol_version
        )))
    }
}

fn protocol_error(message: impl Into<String>) -> SyncError {
    SyncError::Protocol(message.into())
}
