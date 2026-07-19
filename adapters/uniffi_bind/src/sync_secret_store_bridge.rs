use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use sona_core::sync::{SyncError, SyncSecretStore};

use crate::{SonaCoreBindingError, SonaCoreBindingResult};

#[uniffi::export(foreign)]
#[async_trait]
pub trait FfiSyncSecretStore: Send + Sync {
    async fn get(&self, key: String) -> SonaCoreBindingResult<Option<Vec<u8>>>;
    async fn set(&self, key: String, value: Vec<u8>) -> SonaCoreBindingResult<()>;
    async fn delete(&self, key: String) -> SonaCoreBindingResult<()>;
}

#[derive(Default)]
pub(crate) struct HostSyncSecretStore {
    registration: RwLock<Option<Arc<dyn FfiSyncSecretStore>>>,
}

impl HostSyncSecretStore {
    pub(crate) fn new(registration: Option<Arc<dyn FfiSyncSecretStore>>) -> Self {
        Self {
            registration: RwLock::new(registration),
        }
    }

    pub(crate) fn register(&self, store: Arc<dyn FfiSyncSecretStore>) {
        *self
            .registration
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(store);
    }

    fn registered(&self) -> Option<Arc<dyn FfiSyncSecretStore>> {
        self.registration
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[async_trait]
impl SyncSecretStore for HostSyncSecretStore {
    async fn read_secret(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError> {
        match self.registered() {
            Some(callback) => callback
                .get(key.to_string())
                .await
                .map_err(secret_store_error),
            None => Ok(None),
        }
    }

    async fn write_secret(&self, key: &str, value: &[u8]) -> Result<(), SyncError> {
        match self.registered() {
            Some(callback) => callback
                .set(key.to_string(), value.to_vec())
                .await
                .map_err(secret_store_error),
            None => Ok(()),
        }
    }

    async fn delete_secret(&self, key: &str) -> Result<(), SyncError> {
        match self.registered() {
            Some(callback) => callback
                .delete(key.to_string())
                .await
                .map_err(secret_store_error),
            None => Ok(()),
        }
    }
}

fn secret_store_error(error: SonaCoreBindingError) -> SyncError {
    SyncError::SecretStore(error.to_string())
}
