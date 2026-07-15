use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use sona_core::storage_usage::{StorageUsageError, StorageUsageRepository};
use sona_sqlite::{Database, LazySqliteStorageUsageRepository};

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

    assert!(matches!(error, StorageUsageError::Repository(_)));
    assert!(!app_local_data_dir.exists());
}

#[test]
fn valid_collection_reads_active_wal_and_analytics_without_modifying_source_files() {
    let dir = tempfile::tempdir().unwrap();
    let writer = Database::open(dir.path()).unwrap();
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

    let measurements = LazySqliteStorageUsageRepository::new(dir.path().to_path_buf())
        .collect_measurements()
        .unwrap();

    assert_eq!(measurements.history_audio.bytes, 9);
    assert!(measurements.database.sqlite.dbstat_available);
    assert!(
        measurements
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
    drop(writer);
}
