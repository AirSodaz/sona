use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use sona_core::storage_usage::{StorageUsageError, StorageUsageRepository};
use sona_sqlite::{
    Database, LazySqliteStorageUsageRepository, load_storage_usage_snapshot,
    load_storage_usage_snapshot_with_database,
};

fn file_hashes(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            files.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
            );
        }
    }
    files
}

#[test]
fn lazy_repository_implements_the_core_storage_usage_port() {
    fn assert_port<T: StorageUsageRepository>() {}

    assert_port::<LazySqliteStorageUsageRepository>();
}

#[test]
fn missing_directory_is_rejected_without_creation() {
    let parent = tempfile::tempdir().unwrap();
    let app_local_data_dir = parent.path().join("missing-app-data");
    let repository = LazySqliteStorageUsageRepository::new(app_local_data_dir.clone());

    let error = repository.collect_measurements().unwrap_err();

    assert!(matches!(error, StorageUsageError::Database(_)));
    assert!(!app_local_data_dir.exists());
}

#[test]
fn valid_collection_reads_active_wal_and_analytics_without_modifying_source_files() {
    let dir = tempfile::tempdir().unwrap();
    let writer = Arc::new(Database::open(dir.path()).unwrap());
    writer
        .with_write_connection(|connection| {
            connection.execute_batch(
                "CREATE TABLE storage_adapter_probe (
                     id INTEGER PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 CREATE INDEX idx_storage_adapter_probe_value
                     ON storage_adapter_probe(value);
                 INSERT INTO storage_adapter_probe (value) VALUES ('active wal');
                 CREATE TABLE analytics.storage_adapter_probe (
                     id INTEGER PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 CREATE INDEX analytics.idx_storage_adapter_probe_value
                     ON storage_adapter_probe(value);
                 INSERT INTO analytics.storage_adapter_probe (value)
                     VALUES ('analytics active wal');",
            )?;
            Ok(())
        })
        .unwrap();
    fs::create_dir_all(dir.path().join("history")).unwrap();
    fs::write(dir.path().join("history").join("recording.wav"), [1_u8; 9]).unwrap();
    let before = file_hashes(dir.path());

    let snapshot = load_storage_usage_snapshot(
        dir.path().to_path_buf(),
        "2026-07-15T20:00:00.000Z".to_string(),
    )
    .unwrap();

    assert_eq!(snapshot.generated_at, "2026-07-15T20:00:00.000Z");
    assert_eq!(snapshot.categories.audio.history_audio_bytes, 9);
    assert!(snapshot.categories.database.sqlite.dbstat_available);
    assert!(
        snapshot
            .categories
            .database
            .sqlite
            .index_entries
            .iter()
            .any(|entry| {
                entry.schema == "analytics"
                    && entry.name == "idx_storage_adapter_probe_value"
                    && entry.bytes > 0
            })
    );
    assert_eq!(file_hashes(dir.path()), before);

    let before_shared = file_hashes(dir.path());

    let shared_snapshot = load_storage_usage_snapshot_with_database(
        dir.path().to_path_buf(),
        Arc::clone(&writer),
        "2026-07-15T20:01:00.000Z".to_string(),
    )
    .unwrap();

    assert_eq!(shared_snapshot.categories.audio.history_audio_bytes, 9);
    assert!(shared_snapshot.categories.database.sqlite.dbstat_available);
    let after_shared = file_hashes(dir.path());
    for durable_file in [
        "sona.db",
        "sona.db-wal",
        "sona-analytics.db",
        "sona-analytics.db-wal",
    ] {
        assert_eq!(
            after_shared.get(Path::new(durable_file)),
            before_shared.get(Path::new(durable_file))
        );
    }
    drop(writer);
}
