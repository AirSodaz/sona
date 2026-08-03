use std::io::{Read, Write};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use hkdf::Hkdf;
use rand::RngCore;
use rand::rngs::OsRng;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::Sha256;
use sona_core::sync::{SYNC_PROTOCOL_VERSION, SyncError, SyncPresetV1};
use zeroize::{Zeroize, Zeroizing};

use crate::MAX_SINGLE_OPERATION_BYTES;

const VAULT_KEY_BYTES: usize = 32;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const ENVELOPE_MAGIC: &[u8] = b"SONASYNC1";

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordKeySlotV1 {
    pub memory_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub salt: String,
    pub nonce: String,
    pub wrapped_vault_key: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryKeySlotV1 {
    pub nonce: String,
    pub wrapped_vault_key: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VaultHeaderV1 {
    pub protocol_version: u64,
    pub vault_id: String,
    pub preset: SyncPresetV1,
    pub password_slot: PasswordKeySlotV1,
    pub recovery_slot: Option<RecoveryKeySlotV1>,
}

#[derive(Debug)]
pub struct CreatedVault {
    pub header: VaultHeaderV1,
    pub vault_key: Zeroizing<Vec<u8>>,
    pub recovery_key: Option<String>,
}

pub fn create_vault(
    vault_id: &str,
    preset: SyncPresetV1,
    master_password: &str,
    create_recovery_key: bool,
) -> Result<CreatedVault, SyncError> {
    require_master_password(master_password)?;
    require_identifier(vault_id, "vault ID")?;

    let mut vault_key = Zeroizing::new(vec![0_u8; VAULT_KEY_BYTES]);
    OsRng.fill_bytes(&mut vault_key);
    let password_slot = wrap_with_master_password(vault_id, &vault_key, master_password)?;
    let (recovery_slot, recovery_key) = if create_recovery_key {
        let mut recovery_secret = Zeroizing::new(vec![0_u8; VAULT_KEY_BYTES]);
        OsRng.fill_bytes(&mut recovery_secret);
        let slot = wrap_with_recovery_secret(vault_id, &vault_key, &recovery_secret)?;
        let encoded = URL_SAFE_NO_PAD.encode(&*recovery_secret);
        (Some(slot), Some(encoded))
    } else {
        (None, None)
    };

    Ok(CreatedVault {
        header: VaultHeaderV1 {
            protocol_version: SYNC_PROTOCOL_VERSION,
            vault_id: vault_id.to_string(),
            preset,
            password_slot,
            recovery_slot,
        },
        vault_key,
        recovery_key,
    })
}

pub fn unlock_with_master_password(
    header: &VaultHeaderV1,
    master_password: &str,
) -> Result<Zeroizing<Vec<u8>>, SyncError> {
    validate_header(header)?;
    let slot = &header.password_slot;
    validate_argon2_parameters(slot)?;
    let salt = decode_exact(&slot.salt, SALT_BYTES, "password salt")?;
    let nonce = decode_exact(&slot.nonce, NONCE_BYTES, "password nonce")?;
    let ciphertext = decode(&slot.wrapped_vault_key, "wrapped vault key")?;
    let mut key = derive_master_key(master_password, &salt, slot)?;
    let plaintext = decrypt_key(&key, &nonce, &ciphertext, vault_key_aad(&header.vault_id))?;
    key.zeroize();
    validate_vault_key(plaintext)
}

pub fn unlock_with_recovery_key(
    header: &VaultHeaderV1,
    recovery_key: &str,
) -> Result<Zeroizing<Vec<u8>>, SyncError> {
    validate_header(header)?;
    let slot = header
        .recovery_slot
        .as_ref()
        .ok_or_else(|| crypto_error("Vault does not have a recovery key slot."))?;
    let recovery_secret = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(recovery_key.trim())
            .map_err(|_| crypto_error("Recovery key is invalid."))?,
    );
    if recovery_secret.len() != VAULT_KEY_BYTES {
        return Err(crypto_error("Recovery key is invalid."));
    }
    let key = derive_recovery_key(&header.vault_id, &recovery_secret)?;
    let nonce = decode_exact(&slot.nonce, NONCE_BYTES, "recovery nonce")?;
    let ciphertext = decode(&slot.wrapped_vault_key, "wrapped recovery vault key")?;
    let plaintext = decrypt_key(&key, &nonce, &ciphertext, vault_key_aad(&header.vault_id))?;
    validate_vault_key(plaintext)
}

pub fn change_master_password(
    header: &VaultHeaderV1,
    current_master_password: &str,
    next_master_password: &str,
) -> Result<VaultHeaderV1, SyncError> {
    require_master_password(next_master_password)?;
    let vault_key = unlock_with_master_password(header, current_master_password)?;
    let mut changed = header.clone();
    changed.password_slot =
        wrap_with_master_password(&header.vault_id, &vault_key, next_master_password)?;
    Ok(changed)
}

pub fn seal_json<T: Serialize>(
    vault_key: &[u8],
    aad: &[u8],
    value: &T,
) -> Result<Vec<u8>, SyncError> {
    require_vault_key(vault_key)?;
    let json = serde_json::to_vec(value)
        .map_err(|error| crypto_error(format!("Sync JSON serialization failed: {error}")))?;
    if json.len() > MAX_SINGLE_OPERATION_BYTES {
        return Err(crypto_error("Sync plaintext exceeds the supported limit."));
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&json)
        .map_err(|error| crypto_error(format!("Sync compression failed: {error}")))?;
    let compressed = encoder
        .finish()
        .map_err(|error| crypto_error(format!("Sync compression failed: {error}")))?;
    let mut nonce = [0_u8; NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new_from_slice(vault_key)
        .map_err(|_| crypto_error("Vault key has an invalid size."))?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &compressed,
                aad,
            },
        )
        .map_err(|_| crypto_error("Sync encryption failed."))?;
    let mut envelope = Vec::with_capacity(ENVELOPE_MAGIC.len() + NONCE_BYTES + ciphertext.len());
    envelope.extend_from_slice(ENVELOPE_MAGIC);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

pub fn open_json<T: DeserializeOwned>(
    vault_key: &[u8],
    aad: &[u8],
    envelope: &[u8],
) -> Result<T, SyncError> {
    require_vault_key(vault_key)?;
    let header_len = ENVELOPE_MAGIC.len() + NONCE_BYTES;
    if envelope.len() <= header_len || &envelope[..ENVELOPE_MAGIC.len()] != ENVELOPE_MAGIC {
        return Err(crypto_error("Sync envelope is invalid."));
    }
    let nonce = &envelope[ENVELOPE_MAGIC.len()..header_len];
    let ciphertext = &envelope[header_len..];
    let cipher = XChaCha20Poly1305::new_from_slice(vault_key)
        .map_err(|_| crypto_error("Vault key has an invalid size."))?;
    let compressed = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| crypto_error("Sync envelope authentication failed."))?;
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut json = Vec::new();
    decoder
        .by_ref()
        .take((MAX_SINGLE_OPERATION_BYTES + 1) as u64)
        .read_to_end(&mut json)
        .map_err(|error| crypto_error(format!("Sync decompression failed: {error}")))?;
    if json.len() > MAX_SINGLE_OPERATION_BYTES {
        return Err(crypto_error(
            "Sync decompressed plaintext exceeds the supported limit.",
        ));
    }
    serde_json::from_slice(&json)
        .map_err(|error| crypto_error(format!("Sync JSON decoding failed: {error}")))
}

fn wrap_with_master_password(
    vault_id: &str,
    vault_key: &[u8],
    master_password: &str,
) -> Result<PasswordKeySlotV1, SyncError> {
    require_master_password(master_password)?;
    let mut salt = [0_u8; SALT_BYTES];
    OsRng.fill_bytes(&mut salt);
    let mut nonce = [0_u8; NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    let template = PasswordKeySlotV1 {
        memory_kib: ARGON2_MEMORY_KIB,
        time_cost: ARGON2_TIME_COST,
        parallelism: ARGON2_PARALLELISM,
        salt: URL_SAFE_NO_PAD.encode(salt),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        wrapped_vault_key: String::new(),
    };
    let mut key = derive_master_key(master_password, &salt, &template)?;
    let ciphertext = encrypt_key(&key, &nonce, vault_key, vault_key_aad(vault_id))?;
    key.zeroize();
    Ok(PasswordKeySlotV1 {
        wrapped_vault_key: URL_SAFE_NO_PAD.encode(ciphertext),
        ..template
    })
}

pub(crate) fn wrap_with_recovery_secret(
    vault_id: &str,
    vault_key: &[u8],
    recovery_secret: &[u8],
) -> Result<RecoveryKeySlotV1, SyncError> {
    let key = derive_recovery_key(vault_id, recovery_secret)?;
    let mut nonce = [0_u8; NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = encrypt_key(&key, &nonce, vault_key, vault_key_aad(vault_id))?;
    Ok(RecoveryKeySlotV1 {
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        wrapped_vault_key: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

fn derive_master_key(
    master_password: &str,
    salt: &[u8],
    slot: &PasswordKeySlotV1,
) -> Result<Zeroizing<Vec<u8>>, SyncError> {
    let params = Params::new(
        slot.memory_kib,
        slot.time_cost,
        slot.parallelism,
        Some(VAULT_KEY_BYTES),
    )
    .map_err(|error| crypto_error(format!("Argon2 parameters are invalid: {error}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = Zeroizing::new(vec![0_u8; VAULT_KEY_BYTES]);
    argon2
        .hash_password_into(master_password.as_bytes(), salt, &mut output)
        .map_err(|error| crypto_error(format!("Master password derivation failed: {error}")))?;
    Ok(output)
}

fn derive_recovery_key(
    vault_id: &str,
    recovery_secret: &[u8],
) -> Result<Zeroizing<Vec<u8>>, SyncError> {
    let hkdf = Hkdf::<Sha256>::new(Some(b"sona-sync-recovery-v1"), recovery_secret);
    let mut output = Zeroizing::new(vec![0_u8; VAULT_KEY_BYTES]);
    hkdf.expand(vault_id.as_bytes(), &mut output)
        .map_err(|_| crypto_error("Recovery key derivation failed."))?;
    Ok(output)
}

fn encrypt_key(
    key: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
    aad: Vec<u8>,
) -> Result<Vec<u8>, SyncError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| crypto_error("Wrapping key has an invalid size."))?;
    cipher
        .encrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| crypto_error("Vault key wrapping failed."))
}

fn decrypt_key(
    key: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
    aad: Vec<u8>,
) -> Result<Vec<u8>, SyncError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| crypto_error("Wrapping key has an invalid size."))?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| crypto_error("Vault key could not be unlocked."))
}

fn validate_header(header: &VaultHeaderV1) -> Result<(), SyncError> {
    if header.protocol_version != SYNC_PROTOCOL_VERSION {
        return Err(SyncError::Protocol(format!(
            "Unsupported sync protocol version: {}.",
            header.protocol_version
        )));
    }
    require_identifier(&header.vault_id, "vault ID")
}

fn validate_argon2_parameters(slot: &PasswordKeySlotV1) -> Result<(), SyncError> {
    if slot.memory_kib != ARGON2_MEMORY_KIB
        || slot.time_cost != ARGON2_TIME_COST
        || slot.parallelism != ARGON2_PARALLELISM
    {
        return Err(crypto_error("Vault Argon2 parameters are unsupported."));
    }
    Ok(())
}

fn require_master_password(password: &str) -> Result<(), SyncError> {
    if password.is_empty() {
        return Err(crypto_error("Master password must not be empty."));
    }
    Ok(())
}

fn require_identifier(value: &str, label: &str) -> Result<(), SyncError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(crypto_error(format!("Invalid {label}.")));
    }
    Ok(())
}

fn require_vault_key(vault_key: &[u8]) -> Result<(), SyncError> {
    if vault_key.len() == VAULT_KEY_BYTES {
        Ok(())
    } else {
        Err(crypto_error("Vault key has an invalid size."))
    }
}

fn validate_vault_key(key: Vec<u8>) -> Result<Zeroizing<Vec<u8>>, SyncError> {
    require_vault_key(&key)?;
    Ok(Zeroizing::new(key))
}

fn decode(value: &str, label: &str) -> Result<Vec<u8>, SyncError> {
    URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| crypto_error(format!("Vault {label} is invalid.")))
}

fn decode_exact(value: &str, length: usize, label: &str) -> Result<Vec<u8>, SyncError> {
    let decoded = decode(value, label)?;
    if decoded.len() != length {
        return Err(crypto_error(format!("Vault {label} has an invalid size.")));
    }
    Ok(decoded)
}

fn vault_key_aad(vault_id: &str) -> Vec<u8> {
    format!("sona-sync/vault-key/v1/{vault_id}").into_bytes()
}

fn crypto_error(message: impl Into<String>) -> SyncError {
    SyncError::Crypto(message.into())
}
