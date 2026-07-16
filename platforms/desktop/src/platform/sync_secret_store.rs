use keyring::{Entry, Error as KeyringError};
use sona_core::sync::{SyncError, SyncSecretStore};

const SYNC_KEYRING_SERVICE: &str = "com.asoda.sona.sync";

#[derive(Default)]
pub struct SystemSyncSecretStore;

impl SystemSyncSecretStore {
    fn entry(key: &str) -> Result<Entry, SyncError> {
        Entry::new(SYNC_KEYRING_SERVICE, key)
            .map_err(|error| SyncError::SecretStore(error.to_string()))
    }
}

impl SyncSecretStore for SystemSyncSecretStore {
    fn read_secret(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError> {
        match Self::entry(key)?.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(SyncError::SecretStore(error.to_string())),
        }
    }

    fn write_secret(&self, key: &str, value: &[u8]) -> Result<(), SyncError> {
        Self::entry(key)?
            .set_secret(value)
            .map_err(|error| SyncError::SecretStore(error.to_string()))
    }

    fn delete_secret(&self, key: &str) -> Result<(), SyncError> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(SyncError::SecretStore(error.to_string())),
        }
    }
}
