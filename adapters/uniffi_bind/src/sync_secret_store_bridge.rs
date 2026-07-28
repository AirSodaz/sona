use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use sona_core::sync::{SyncError, SyncSecretStore};

use crate::{SonaCoreBindingError, SonaCoreBindingResult};

/// Async keychain / secret-store bridge, implemented on the foreign side
/// (Kotlin / Swift) and called by the Rust sync engine for each vault
/// operation that needs a stored credential.
///
/// `#[async_trait]` is required here: native `async fn` in traits is not
/// dyn-compatible, so UniFFI cannot generate an `Arc<dyn FfiSecretStore>`
/// vtable from them. The macro desugars each method to return
/// `Pin<Box<dyn Future>>`, which is dyn-compatible. The generated Kotlin
/// bindings expose these as `suspend fun` via UniFFI's callback machinery.
#[uniffi::export(foreign)]
#[async_trait]
pub trait FfiSecretStore: Send + Sync {
    async fn get(&self, key: String) -> SonaCoreBindingResult<Option<Vec<u8>>>;
    async fn set(&self, key: String, value: Vec<u8>) -> SonaCoreBindingResult<()>;
    async fn delete(&self, key: String) -> SonaCoreBindingResult<()>;
}

#[derive(Default)]
pub(crate) struct HostSyncSecretStore {
    registration: RwLock<Option<Arc<dyn FfiSecretStore>>>,
}

impl HostSyncSecretStore {
    pub(crate) fn new(registration: Option<Arc<dyn FfiSecretStore>>) -> Self {
        Self {
            registration: RwLock::new(registration),
        }
    }

    pub(crate) fn register(&self, store: Arc<dyn FfiSecretStore>) {
        *self
            .registration
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(store);
    }

    fn registered(&self) -> Option<Arc<dyn FfiSecretStore>> {
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
