use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sona_core::sync::{
    SyncDeleteResult, SyncError, SyncListPage, SyncObject, SyncObjectKey, SyncObjectMetadata,
    SyncObjectPrefix, SyncObjectStore, SyncObjectStoreCapabilities, SyncPresetV1, SyncPutResult,
};
use sona_sync::{
    change_remote_master_password, create_remote_vault, open_remote_vault_with_password,
    open_remote_vault_with_recovery_key, open_remote_vault_with_vault_key,
    regenerate_remote_recovery_key,
};

#[derive(Clone, Default)]
struct MemoryStore {
    objects: Arc<Mutex<BTreeMap<String, (Vec<u8>, String)>>>,
}

#[async_trait]
impl SyncObjectStore for MemoryStore {
    async fn probe(&self) -> Result<SyncObjectStoreCapabilities, SyncError> {
        Ok(SyncObjectStoreCapabilities {
            conditional_create: true,
            compare_and_swap: true,
            delete: true,
        })
    }

    async fn list(
        &self,
        prefix: &SyncObjectPrefix,
        _continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError> {
        let objects = self
            .objects
            .lock()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.starts_with(prefix.as_str()))
            .map(|(key, (bytes, etag))| SyncObjectMetadata {
                key: SyncObjectKey::parse(key.clone()).unwrap(),
                etag: Some(etag.clone()),
                size: bytes.len() as u64,
                modified_at: None,
            })
            .collect();
        Ok(SyncListPage {
            objects,
            continuation: None,
        })
    }

    async fn get(&self, key: &SyncObjectKey) -> Result<Option<SyncObject>, SyncError> {
        Ok(self
            .objects
            .lock()
            .unwrap()
            .get(key.as_str())
            .map(|(bytes, etag)| SyncObject {
                metadata: SyncObjectMetadata {
                    key: key.clone(),
                    etag: Some(etag.clone()),
                    size: bytes.len() as u64,
                    modified_at: None,
                },
                bytes: bytes.clone(),
            }))
    }

    async fn put_if_absent(
        &self,
        key: &SyncObjectKey,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        let mut objects = self.objects.lock().unwrap();
        if let Some((_, etag)) = objects.get(key.as_str()) {
            return Ok(SyncPutResult::AlreadyExists {
                etag: Some(etag.clone()),
            });
        }
        let etag = "etag-1".to_string();
        objects.insert(key.to_string(), (bytes, etag.clone()));
        Ok(SyncPutResult::Created { etag: Some(etag) })
    }

    async fn compare_and_swap(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        let mut objects = self.objects.lock().unwrap();
        let Some((_, current_etag)) = objects.get(key.as_str()) else {
            return Ok(SyncPutResult::Conflict { current_etag: None });
        };
        if expected_etag != Some(current_etag.as_str()) {
            return Ok(SyncPutResult::Conflict {
                current_etag: Some(current_etag.clone()),
            });
        }
        let next_etag = format!("etag-{}", current_etag.len() + bytes.len());
        objects.insert(key.to_string(), (bytes, next_etag.clone()));
        Ok(SyncPutResult::Created {
            etag: Some(next_etag),
        })
    }

    async fn delete(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError> {
        let mut objects = self.objects.lock().unwrap();
        let Some((_, current_etag)) = objects.get(key.as_str()) else {
            return Ok(SyncDeleteResult::NotFound);
        };
        if expected_etag.is_some_and(|expected| expected != current_etag) {
            return Ok(SyncDeleteResult::Conflict {
                current_etag: Some(current_etag.clone()),
            });
        }
        objects.remove(key.as_str());
        Ok(SyncDeleteResult::Deleted)
    }
}

#[tokio::test]
async fn remote_vault_supports_password_rotation_recovery_rotation_and_secure_key_restore() {
    let store = MemoryStore::default();
    let created = create_remote_vault(
        &store,
        "vault-a",
        SyncPresetV1::Standard,
        "initial master password",
        true,
    )
    .await
    .unwrap();
    let old_recovery_key = created.recovery_key.unwrap();
    let mut opened = created.opened;
    let vault_key = opened.vault_key.to_vec();

    change_remote_master_password(
        &store,
        &mut opened,
        "initial master password",
        "replacement master password",
    )
    .await
    .unwrap();
    assert!(
        open_remote_vault_with_password(&store, "vault-a", "initial master password")
            .await
            .is_err()
    );
    let reopened =
        open_remote_vault_with_password(&store, "vault-a", "replacement master password")
            .await
            .unwrap();
    assert_eq!(reopened.vault_key.as_slice(), vault_key.as_slice());

    let new_recovery_key = regenerate_remote_recovery_key(&store, &mut opened)
        .await
        .unwrap();
    assert!(
        open_remote_vault_with_recovery_key(&store, "vault-a", &old_recovery_key)
            .await
            .is_err()
    );
    let recovered = open_remote_vault_with_recovery_key(&store, "vault-a", &new_recovery_key)
        .await
        .unwrap();
    assert_eq!(recovered.vault_key.as_slice(), vault_key.as_slice());

    let restored = open_remote_vault_with_vault_key(&store, "vault-a", &vault_key)
        .await
        .unwrap();
    assert_eq!(restored.header.vault_id, "vault-a");
    assert_eq!(restored.vault_key.as_slice(), vault_key.as_slice());
}
