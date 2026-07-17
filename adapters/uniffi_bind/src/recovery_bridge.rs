use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde_json::Value;
use sona_core::recovery::types::{RecoveryItemInput, RecoverySnapshot};
use sona_recovery_fs::FsRecoveryAdapter;
use std::path::PathBuf;

pub(crate) fn load_recovery_snapshot_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let snapshot = FsRecoveryAdapter::new(PathBuf::from(app_data_dir))
        .load_snapshot()
        .map_err(recovery_error)?;
    serialize_snapshot(&snapshot)
}

pub(crate) fn save_recovery_snapshot_json(
    app_data_dir: String,
    items_json: String,
) -> SonaCoreBindingResult<String> {
    let items = parse_json_array("items", &items_json)?;
    let snapshot = FsRecoveryAdapter::new(PathBuf::from(app_data_dir))
        .save_snapshot(items)
        .map_err(recovery_error)?;
    serialize_snapshot(&snapshot)
}

pub(crate) fn persist_recovery_queue_snapshot_json(
    app_data_dir: String,
    queue_items_json: String,
    resolved_ids: Vec<String>,
) -> SonaCoreBindingResult<String> {
    let queue_items = parse_json_array("queue items", &queue_items_json)?;
    let snapshot = FsRecoveryAdapter::new(PathBuf::from(app_data_dir))
        .persist_queue_snapshot(queue_items, resolved_ids)
        .map_err(recovery_error)?;
    serialize_snapshot(&snapshot)
}

fn parse_json_array(label: &str, input: &str) -> SonaCoreBindingResult<Vec<RecoveryItemInput>> {
    let value = serde_json::from_str::<Value>(input).map_err(|error| {
        SonaCoreBindingError::InvalidInput {
            reason: format!("Invalid {label} JSON: {error}"),
        }
    })?;
    let values = value
        .as_array()
        .cloned()
        .ok_or_else(|| SonaCoreBindingError::InvalidInput {
            reason: format!("{label} JSON must be an array"),
        })?;
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

fn serialize_snapshot(snapshot: &RecoverySnapshot) -> SonaCoreBindingResult<String> {
    serde_json::to_string(snapshot).map_err(|error| recovery_error(error.to_string()))
}

fn recovery_error(error: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Recovery {
        reason: error.to_string(),
    }
}

#[cfg(test)]
fn load_recovery_snapshot_json_at(
    app_data_dir: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    let snapshot = FsRecoveryAdapter::new(PathBuf::from(app_data_dir))
        .load_snapshot_at(now_ms)
        .map_err(recovery_error)?;
    serialize_snapshot(&snapshot)
}

#[cfg(test)]
fn save_recovery_snapshot_json_at(
    app_data_dir: String,
    items_json: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    let items = parse_json_array("items", &items_json)?;
    let snapshot = FsRecoveryAdapter::new(PathBuf::from(app_data_dir))
        .save_snapshot_at(items, now_ms)
        .map_err(recovery_error)?;
    serialize_snapshot(&snapshot)
}

#[cfg(test)]
fn persist_recovery_queue_snapshot_json_at(
    app_data_dir: String,
    queue_items_json: String,
    resolved_ids: Vec<String>,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    let queue_items = parse_json_array("queue items", &queue_items_json)?;
    let snapshot = FsRecoveryAdapter::new(PathBuf::from(app_data_dir))
        .persist_queue_snapshot_at(queue_items, resolved_ids, now_ms)
        .map_err(recovery_error)?;
    serialize_snapshot(&snapshot)
}

#[cfg(test)]
mod tests {
    use super::{
        load_recovery_snapshot_json_at, persist_recovery_queue_snapshot_json_at,
        save_recovery_snapshot_json_at,
    };
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "sona-uniffi-recovery-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn app_data_dir(&self) -> String {
            self.0.to_string_lossy().into_owned()
        }

        fn snapshot_path(&self) -> PathBuf {
            self.0.join("recovery").join("queue-recovery.json")
        }

        fn write_snapshot(&self, snapshot: Value) {
            fs::create_dir_all(self.0.join("recovery")).unwrap();
            fs::write(self.snapshot_path(), serde_json::to_vec(&snapshot).unwrap()).unwrap();
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn load_returns_canonical_camel_case_json() {
        let dir = TestDir::new();
        let source_path = dir.path().join("recording.wav");
        fs::write(&source_path, b"audio").unwrap();
        dir.write_snapshot(json!({
            "version": 1,
            "updatedAt": 123,
            "items": [{
                "id": "saved-1",
                "filename": "recording.wav",
                "filePath": source_path,
                "resolution": "pending",
                "segments": {"legacy": true}
            }]
        }));

        let output = load_recovery_snapshot_json_at(dir.app_data_dir(), 5_000).unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["updatedAt"], 123);
        assert!(value.get("updated_at").is_none());
        assert_eq!(value["items"][0]["filePath"], json!(source_path));
        assert!(value["items"][0].get("file_path").is_none());
        assert_eq!(value["items"][0]["updatedAt"], 5_000);
        assert_eq!(value["items"][0]["hasSourceFile"], true);
        assert_eq!(value["items"][0]["canResume"], true);
        assert_eq!(value["items"][0]["segments"], json!([]));
    }

    #[test]
    fn save_rejects_non_array_json() {
        let dir = TestDir::new();

        let error = save_recovery_snapshot_json_at(dir.app_data_dir(), "{}".to_string(), 6_000)
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
        assert_eq!(error.to_string(), "items JSON must be an array");
    }

    #[test]
    fn save_persists_and_returns_canonical_snapshot_json() {
        let dir = TestDir::new();
        let source_path = dir.path().join("saved.wav");
        fs::write(&source_path, b"audio").unwrap();
        let items = json!([{
            "id": "saved-1",
            "filename": "saved.wav",
            "filePath": source_path,
            "resolution": "pending",
            "segments": []
        }]);

        let output = save_recovery_snapshot_json_at(
            dir.app_data_dir(),
            serde_json::to_string(&items).unwrap(),
            6_500,
        )
        .unwrap();
        let output_value: Value = serde_json::from_str(&output).unwrap();
        let stored_value: Value =
            serde_json::from_slice(&fs::read(dir.snapshot_path()).unwrap()).unwrap();

        assert_eq!(stored_value, output_value);
        assert_eq!(output_value["updatedAt"], 6_500);
        assert_eq!(output_value["items"][0]["filePath"], json!(source_path));
        assert_eq!(output_value["items"][0]["updatedAt"], 6_500);
        assert!(output_value.get("updated_at").is_none());
        assert!(output_value["items"][0].get("file_path").is_none());
    }

    #[test]
    fn save_keeps_valid_items_when_legacy_json_contains_invalid_siblings() {
        let dir = TestDir::new();
        let items = json!([
            {
                "id": "first",
                "filename": "first.wav",
                "filePath": "",
                "resolution": "pending",
                "segments": []
            },
            "locally-corrupt-item",
            {
                "id": "second",
                "filename": "second.wav",
                "filePath": "",
                "resolution": "pending",
                "segments": []
            }
        ]);

        let output = save_recovery_snapshot_json_at(
            dir.app_data_dir(),
            serde_json::to_string(&items).unwrap(),
            6_550,
        )
        .unwrap();
        let output: Value = serde_json::from_str(&output).unwrap();
        let ids = output["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["first", "second"]);
    }

    #[test]
    fn malformed_items_json_is_invalid_input_and_preserves_recovery_snapshot() {
        let dir = TestDir::new();
        dir.write_snapshot(json!({
            "version": 1,
            "updatedAt": 55,
            "items": [{
                "id": "retained",
                "filename": "retained.wav",
                "filePath": "",
                "resolution": "pending",
                "segments": []
            }]
        }));

        let error =
            save_recovery_snapshot_json_at(dir.app_data_dir(), "[".to_string(), 6_600).unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
        assert_eq!(
            error.to_string(),
            "Invalid items JSON: EOF while parsing a list at line 1 column 1"
        );
        let recovered = load_recovery_snapshot_json_at(dir.app_data_dir(), 6_601).unwrap();
        let recovered: Value = serde_json::from_str(&recovered).unwrap();
        assert_eq!(recovered["items"][0]["id"], "retained");
        assert_eq!(recovered["updatedAt"], 55);
    }

    #[test]
    fn malformed_queue_items_json_is_invalid_input_and_preserves_recovery_snapshot() {
        let dir = TestDir::new();
        dir.write_snapshot(json!({
            "version": 1,
            "updatedAt": 56,
            "items": [{
                "id": "retained",
                "filename": "retained.wav",
                "filePath": "",
                "resolution": "pending",
                "segments": []
            }]
        }));

        let error = persist_recovery_queue_snapshot_json_at(
            dir.app_data_dir(),
            "[".to_string(),
            Vec::new(),
            6_700,
        )
        .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
        assert_eq!(
            error.to_string(),
            "Invalid queue items JSON: EOF while parsing a list at line 1 column 1"
        );
        let recovered = load_recovery_snapshot_json_at(dir.app_data_dir(), 6_701).unwrap();
        let recovered: Value = serde_json::from_str(&recovered).unwrap();
        assert_eq!(recovered["items"][0]["id"], "retained");
        assert_eq!(recovered["updatedAt"], 56);
    }

    #[test]
    fn persist_removes_resolved_id_and_retains_unobserved_pending_item() {
        let dir = TestDir::new();
        dir.write_snapshot(json!({
            "version": 1,
            "items": [
                {
                    "id": "resolved",
                    "filename": "resolved.wav",
                    "filePath": "",
                    "resolution": "pending",
                    "segments": []
                },
                {
                    "id": "retained",
                    "filename": "retained.wav",
                    "filePath": "",
                    "resolution": "pending",
                    "segments": []
                }
            ]
        }));

        let output = persist_recovery_queue_snapshot_json_at(
            dir.app_data_dir(),
            "[]".to_string(),
            vec!["resolved".to_string()],
            7_000,
        )
        .unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();
        let ids = value["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["retained"]);
    }

    #[test]
    fn invalid_filesystem_root_maps_to_recovery_error() {
        let dir = TestDir::new();
        let blocked = dir.path().join("blocked");
        File::create(&blocked).unwrap();

        let error = load_recovery_snapshot_json_at(blocked.to_string_lossy().into_owned(), 8_000)
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Recovery { .. }));
    }
}
