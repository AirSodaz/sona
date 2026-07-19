use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sona_core::sync::SyncSecretStore;
use sona_runtime_fs::SystemClock;
use sona_sync::{
    JsonFileSyncConfigStore, SyncApplication, SyncProviderRegistry,
    SystemSyncApplicationEnvironment,
};

use crate::application_context::ApplicationContextRegistry;
use crate::{FfiSyncSecretStore, SonaCoreBindingResult};

#[derive(Default)]
struct RecordingSecretStore {
    writes: Mutex<Vec<(String, Vec<u8>)>>,
}

impl RecordingSecretStore {
    fn written_values(&self) -> Vec<(String, Vec<u8>)> {
        self.writes.lock().unwrap().clone()
    }
}

#[async_trait]
impl FfiSyncSecretStore for RecordingSecretStore {
    async fn get(&self, _key: String) -> SonaCoreBindingResult<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn set(&self, key: String, value: Vec<u8>) -> SonaCoreBindingResult<()> {
        self.writes.lock().unwrap().push((key, value));
        Ok(())
    }

    async fn delete(&self, _key: String) -> SonaCoreBindingResult<()> {
        Ok(())
    }
}

#[test]
fn application_context_cache_reuses_canonical_path_aliases() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    std::fs::create_dir_all(app_data_dir.join("child")).unwrap();
    let alias = app_data_dir.join("child").join("..");
    let mut registry = ApplicationContextRegistry::with_capacity(4);

    let first = registry.get_or_open(&app_data_dir).unwrap();
    let second = registry.get_or_open(&alias).unwrap();

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(registry.len(), 1);
}

#[test]
fn application_context_cache_evicts_the_least_recently_used_entry() {
    let root = tempfile::tempdir().unwrap();
    let first_path = root.path().join("first");
    let second_path = root.path().join("second");
    let third_path = root.path().join("third");
    let mut registry = ApplicationContextRegistry::with_capacity(2);

    let first = registry.get_or_open(&first_path).unwrap();
    registry.get_or_open(&second_path).unwrap();
    registry.get_or_open(&first_path).unwrap();
    registry.get_or_open(&third_path).unwrap();

    assert_eq!(registry.len(), 2);
    assert!(registry.contains(first.sqlite().app_data_dir()));
    assert!(!registry.contains(&second_path));
    assert!(registry.contains(&third_path));
}

#[test]
fn failed_application_context_open_is_not_cached_and_can_recover() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    std::fs::write(&app_data_dir, "not a directory").unwrap();
    let mut registry = ApplicationContextRegistry::with_capacity(2);

    assert!(registry.get_or_open(&app_data_dir).is_err());
    assert_eq!(registry.len(), 0);

    std::fs::remove_file(&app_data_dir).unwrap();
    let context = registry.get_or_open(&app_data_dir).unwrap();
    assert!(context.sqlite().app_data_dir().is_dir());
    assert_eq!(registry.len(), 1);
}

#[test]
fn application_context_cache_releases_only_the_requested_path() {
    let root = tempfile::tempdir().unwrap();
    let first_path = root.path().join("first");
    let second_path = root.path().join("second");
    let mut registry = ApplicationContextRegistry::with_capacity(2);

    let first = registry.get_or_open(&first_path).unwrap();
    let first_secret_store = first.sync_secret_store();
    let second = registry.get_or_open(&second_path).unwrap();
    drop(first);

    assert!(registry.release(&first_path).unwrap());
    assert!(!registry.release(&first_path).unwrap());
    assert!(!registry.contains(&first_path));
    assert!(registry.contains(&second_path));
    assert_eq!(registry.len(), 1);

    let recreated = registry.get_or_open(&first_path).unwrap();
    assert!(!Arc::ptr_eq(&recreated, &second));
    assert!(!Arc::ptr_eq(
        &recreated.sync_secret_store(),
        &first_secret_store,
    ));
}

#[test]
fn application_context_cache_release_uses_the_canonical_path_key() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    std::fs::create_dir_all(app_data_dir.join("child")).unwrap();
    let alias = app_data_dir.join("child").join("..");
    let mut registry = ApplicationContextRegistry::with_capacity(2);

    registry.get_or_open(&app_data_dir).unwrap();

    assert!(registry.release(&alias).unwrap());
    assert_eq!(registry.len(), 0);
}

#[tokio::test]
async fn path_secret_store_override_survives_default_registration_and_lru_reopen() {
    let root = tempfile::tempdir().unwrap();
    let first_path = root.path().join("first");
    let second_path = root.path().join("second");
    let explicit = Arc::new(RecordingSecretStore::default());
    let compatibility_default = Arc::new(RecordingSecretStore::default());
    let mut registry = ApplicationContextRegistry::with_capacity(1);

    registry
        .register_sync_secret_store(&first_path, explicit.clone())
        .unwrap();
    registry.register_default_sync_secret_store(compatibility_default.clone());
    registry.get_or_open(&second_path).unwrap();
    assert!(!registry.contains(&first_path));

    registry
        .get_or_open(&first_path)
        .unwrap()
        .sync_secret_store()
        .write_secret("vault", b"explicit")
        .await
        .unwrap();

    assert_eq!(
        explicit.written_values(),
        vec![("vault".to_string(), b"explicit".to_vec())]
    );
    assert!(compatibility_default.written_values().is_empty());
}

#[tokio::test]
async fn explicit_release_drops_the_path_secret_store_registration() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    let explicit = Arc::new(RecordingSecretStore::default());
    let mut registry = ApplicationContextRegistry::with_capacity(1);

    registry
        .register_sync_secret_store(&app_data_dir, explicit.clone())
        .unwrap();
    assert!(registry.release(&app_data_dir).unwrap());
    registry
        .get_or_open(&app_data_dir)
        .unwrap()
        .sync_secret_store()
        .write_secret("vault", b"ignored")
        .await
        .unwrap();

    assert!(explicit.written_values().is_empty());
}

#[test]
fn application_context_cache_protects_active_sync_handles_from_lru_eviction() {
    let root = tempfile::tempdir().unwrap();
    let first_path = root.path().join("first");
    let second_path = root.path().join("second");
    let third_path = root.path().join("third");
    let fourth_path = root.path().join("fourth");
    let mut registry = ApplicationContextRegistry::with_capacity(2);

    let first = registry.get_or_open(&first_path).unwrap();
    let secret_store: Arc<dyn SyncSecretStore> = first.sync_secret_store();
    let active_application = first.sync_application(|sqlite| {
        Arc::new(SyncApplication::new(
            Arc::new(JsonFileSyncConfigStore::new(
                sqlite.app_data_dir().join("sync-active-test.json"),
            )),
            Arc::new(sqlite.sync_repository_factory(Arc::new(SystemClock))),
            SyncProviderRegistry::default(),
            secret_store,
            Arc::new(SystemSyncApplicationEnvironment),
        ))
    });
    drop(first);

    registry.get_or_open(&second_path).unwrap();
    registry.get_or_open(&third_path).unwrap();
    assert!(registry.contains(&first_path));
    assert!(!registry.contains(&second_path));
    assert!(registry.contains(&third_path));

    drop(active_application);
    registry.get_or_open(&fourth_path).unwrap();
    assert!(!registry.contains(&first_path));
    assert!(registry.contains(&third_path));
    assert!(registry.contains(&fourth_path));
}
