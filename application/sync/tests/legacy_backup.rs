use async_trait::async_trait;
use sona_core::sync::{
    SyncDeleteResult, SyncError, SyncListPage, SyncObject, SyncObjectKey, SyncObjectMetadata,
    SyncObjectPrefix, SyncObjectStore, SyncObjectStoreCapabilities, SyncPutResult,
};
use sona_sync::{LegacyRemoteBackupService, legacy_provider_credential_key};

struct LegacyStore;

#[async_trait]
impl SyncObjectStore for LegacyStore {
    async fn probe(&self) -> Result<SyncObjectStoreCapabilities, SyncError> {
        unreachable!()
    }

    async fn list(
        &self,
        prefix: &SyncObjectPrefix,
        _continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError> {
        assert_eq!(prefix.as_str(), "");
        Ok(SyncListPage {
            objects: vec![
                metadata("notes.txt", 4),
                metadata("sona-backup-2026-01-01.tar.bz2", 10),
                metadata("nested/sona-backup-2026-02-01.tar.bz2", 20),
            ],
            continuation: None,
        })
    }

    async fn get(&self, key: &SyncObjectKey) -> Result<Option<SyncObject>, SyncError> {
        Ok(Some(SyncObject {
            metadata: metadata(key.as_str(), 3),
            bytes: vec![1, 2, 3],
        }))
    }

    async fn put_if_absent(
        &self,
        _key: &SyncObjectKey,
        _bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        unreachable!()
    }

    async fn compare_and_swap(
        &self,
        _key: &SyncObjectKey,
        _expected_etag: Option<&str>,
        _bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        unreachable!()
    }

    async fn delete(
        &self,
        _key: &SyncObjectKey,
        _expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError> {
        unreachable!()
    }
}

fn metadata(key: &str, size: u64) -> SyncObjectMetadata {
    SyncObjectMetadata {
        key: SyncObjectKey::parse(key).unwrap(),
        etag: None,
        size,
        modified_at: None,
    }
}

#[tokio::test]
async fn legacy_service_filters_archives_above_the_provider_boundary() {
    let service = LegacyRemoteBackupService::new(&LegacyStore);

    let entries = service.list().await.unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].file_name, "sona-backup-2026-01-01.tar.bz2");
    assert_eq!(entries[1].file_name, "sona-backup-2026-02-01.tar.bz2");
}

#[tokio::test]
async fn legacy_service_downloads_opaque_archive_bytes() {
    let service = LegacyRemoteBackupService::new(&LegacyStore);
    let key = SyncObjectKey::parse("sona-backup-2026-01-01.tar.bz2").unwrap();

    let bytes = service.download(&key).await.unwrap();

    assert_eq!(bytes, vec![1, 2, 3]);
}

#[test]
fn legacy_credential_keys_are_deterministic_and_provider_scoped() {
    let key = legacy_provider_credential_key("webdav", "https://dav.example.com", "sona", "alice");

    assert_eq!(
        key,
        legacy_provider_credential_key("webdav", "https://dav.example.com", "sona", "alice")
    );
    assert_ne!(
        key,
        legacy_provider_credential_key("filesystem", "https://dav.example.com", "sona", "alice")
    );
}
