use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_sqlite::load_storage_usage_snapshot;
use std::path::PathBuf;

pub(crate) async fn load_storage_usage_snapshot_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    tokio::task::spawn_blocking(move || build_storage_usage_snapshot_json(app_data_dir))
        .await
        .map_err(storage_usage_error)?
}

fn build_storage_usage_snapshot_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let app_data_dir =
        std::path::absolute(PathBuf::from(app_data_dir)).map_err(storage_usage_error)?;
    let snapshot = load_storage_usage_snapshot(
        app_data_dir,
        sona_runtime_fs::storage_usage_generated_at_now(),
    )
    .map_err(storage_usage_error)?;
    let canonical = serde_json::to_value(snapshot).map_err(storage_usage_error)?;
    serde_json::to_string(&canonical).map_err(storage_usage_error)
}

fn storage_usage_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::StorageUsage {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::load_storage_usage_snapshot_json;
    use crate::SonaCoreBindingError;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use sona_sqlite::Database;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

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

    #[tokio::test]
    async fn missing_directory_is_rejected_without_creation() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");

        let error = load_storage_usage_snapshot_json(missing.to_string_lossy().into_owned())
            .await
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::StorageUsage { .. }));
        assert!(!missing.exists());
    }

    #[tokio::test]
    async fn snapshot_is_canonical_json_and_does_not_modify_active_databases() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Database::open(dir.path()).unwrap();
        writer
            .with_write_connection(|connection| {
                connection.execute_batch(
                    "CREATE TABLE storage_probe (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
                     CREATE INDEX idx_storage_probe_value ON storage_probe(value);
                     INSERT INTO storage_probe (value) VALUES ('active wal');
                     CREATE TABLE analytics.storage_probe_analytics (
                         id INTEGER PRIMARY KEY,
                         value TEXT NOT NULL
                     );
                     CREATE INDEX analytics.idx_storage_probe_analytics_value
                         ON storage_probe_analytics(value);
                     INSERT INTO analytics.storage_probe_analytics (value)
                         VALUES ('analytics active wal');",
                )?;
                Ok(())
            })
            .unwrap();
        fs::create_dir_all(dir.path().join("history")).unwrap();
        fs::write(dir.path().join("history").join("recording.wav"), [1_u8; 9]).unwrap();
        let before = file_hashes(dir.path());

        let output = load_storage_usage_snapshot_json(dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        let snapshot: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(serde_json::to_string(&snapshot).unwrap(), output);
        assert_eq!(snapshot["categories"]["audio"]["historyAudioBytes"], 9);
        assert_eq!(
            snapshot["categories"]["database"]["sqlite"]["dbstatAvailable"],
            true
        );
        assert!(
            snapshot["categories"]["database"]["sqlite"]["indexBytes"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            snapshot["categories"]["database"]["sqlite"]["indexEntries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| {
                    entry["schema"] == "analytics"
                        && entry["name"] == "idx_storage_probe_analytics_value"
                        && entry["bytes"].as_u64().unwrap() > 0
                })
        );
        assert!(snapshot["generatedAt"].as_str().unwrap().contains('T'));
        assert!(snapshot["totalBytes"].as_u64().unwrap() >= 9);
        assert_eq!(file_hashes(dir.path()), before);
        drop(writer);
    }

    #[tokio::test]
    async fn future_schema_is_rejected_without_modifying_source_files() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Database::open(dir.path()).unwrap();
        writer
            .with_write_connection(|connection| {
                connection.execute("INSERT INTO schema_version (version) VALUES (99)", [])?;
                Ok(())
            })
            .unwrap();
        drop(writer);
        let before = file_hashes(dir.path());

        let error = load_storage_usage_snapshot_json(dir.path().to_string_lossy().into_owned())
            .await
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::StorageUsage { .. }));
        assert!(error.to_string().contains("99"));
        assert_eq!(file_hashes(dir.path()), before);
    }
}
