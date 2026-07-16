use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sona_core::sync::{SyncError, SyncObjectKey, SyncObjectPrefix, SyncObjectStore};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRemoteBackupEntry {
    pub key: SyncObjectKey,
    pub file_name: String,
    pub size: u64,
    pub modified_at: Option<String>,
}

pub struct LegacyRemoteBackupService<'a> {
    store: &'a dyn SyncObjectStore,
}

pub fn legacy_provider_credential_key(
    provider_id: &str,
    endpoint: &str,
    remote_root: &str,
    username: &str,
) -> String {
    let mut digest = Sha256::new();
    for component in [provider_id, endpoint, remote_root, username] {
        digest.update((component.len() as u64).to_be_bytes());
        digest.update(component.as_bytes());
    }
    format!(
        "legacy-provider-password:{}",
        hex::encode(digest.finalize())
    )
}

impl<'a> LegacyRemoteBackupService<'a> {
    pub fn new(store: &'a dyn SyncObjectStore) -> Self {
        Self { store }
    }

    pub async fn list(&self) -> Result<Vec<LegacyRemoteBackupEntry>, SyncError> {
        let mut continuation = None;
        let mut entries = Vec::new();
        loop {
            let page = self
                .store
                .list(&SyncObjectPrefix::root(), continuation.as_deref())
                .await?;
            for object in page.objects {
                let Some(file_name) = object.key.as_str().rsplit('/').next() else {
                    continue;
                };
                if !file_name.ends_with(".tar.bz2") {
                    continue;
                }
                let file_name = file_name.to_string();
                entries.push(LegacyRemoteBackupEntry {
                    key: object.key,
                    file_name,
                    size: object.size,
                    modified_at: object.modified_at,
                });
            }
            match page.continuation {
                Some(next) if !next.is_empty() => continuation = Some(next),
                _ => break,
            }
        }
        entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
        Ok(entries)
    }

    pub async fn download(&self, key: &SyncObjectKey) -> Result<Vec<u8>, SyncError> {
        self.store
            .get(key)
            .await?
            .map(|object| object.bytes)
            .ok_or_else(|| SyncError::ObjectStore(format!("Legacy backup does not exist: {key}.")))
    }
}
