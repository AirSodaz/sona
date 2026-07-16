use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde_json::Value;
use sona_core::ports::time::UnixMillisClock;
use sona_core::task_ledger::types::{TaskLedgerRecord, TaskLedgerSnapshot};
use sona_runtime_fs::SystemClock;
use sona_sqlite::{Database, SqliteTaskLedgerAdapter};
use std::path::Path;
use std::sync::Arc;

pub(crate) fn load_task_ledger_snapshot_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    load_task_ledger_snapshot_json_with_clock(app_data_dir, Arc::new(SystemClock))
}

pub(crate) fn upsert_task_ledger_record_json(
    app_data_dir: String,
    record_json: String,
) -> SonaCoreBindingResult<String> {
    upsert_task_ledger_record_json_with_clock(app_data_dir, record_json, Arc::new(SystemClock))
}

pub(crate) fn patch_task_ledger_record_json(
    app_data_dir: String,
    id: String,
    patch_json: String,
) -> SonaCoreBindingResult<String> {
    patch_task_ledger_record_json_with_clock(app_data_dir, id, patch_json, Arc::new(SystemClock))
}

pub(crate) fn remove_task_ledger_record_json(
    app_data_dir: String,
    id: String,
) -> SonaCoreBindingResult<String> {
    remove_task_ledger_record_json_with_clock(app_data_dir, id, Arc::new(SystemClock))
}

pub(crate) fn clear_resolved_task_ledger_records_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    clear_resolved_task_ledger_records_json_with_clock(app_data_dir, Arc::new(SystemClock))
}

fn load_task_ledger_snapshot_json_with_clock(
    app_data_dir: String,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    with_task_ledger_adapter(&app_data_dir, clock, |adapter| adapter.load_snapshot())
        .and_then(serialize_snapshot)
}

fn upsert_task_ledger_record_json_with_clock(
    app_data_dir: String,
    record_json: String,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    let record = serde_json::from_str::<TaskLedgerRecord>(&record_json).map_err(|error| {
        SonaCoreBindingError::InvalidInput {
            reason: format!("Invalid task ledger record JSON: {error}"),
        }
    })?;
    with_task_ledger_adapter(&app_data_dir, clock, |adapter| adapter.upsert_task(record))
        .and_then(serialize_snapshot)
}

fn patch_task_ledger_record_json_with_clock(
    app_data_dir: String,
    id: String,
    patch_json: String,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    let patch = serde_json::from_str::<Value>(&patch_json).map_err(|error| {
        SonaCoreBindingError::InvalidInput {
            reason: format!("Invalid task ledger patch JSON: {error}"),
        }
    })?;
    with_task_ledger_adapter(&app_data_dir, clock, |adapter| {
        adapter.patch_task_json(&id, patch)
    })
    .and_then(serialize_snapshot)
}

fn remove_task_ledger_record_json_with_clock(
    app_data_dir: String,
    id: String,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    with_task_ledger_adapter(&app_data_dir, clock, |adapter| adapter.remove_task(&id))
        .and_then(serialize_snapshot)
}

fn clear_resolved_task_ledger_records_json_with_clock(
    app_data_dir: String,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    with_task_ledger_adapter(&app_data_dir, clock, |adapter| adapter.clear_resolved())
        .and_then(serialize_snapshot)
}

fn with_task_ledger_adapter<T>(
    app_data_dir: &str,
    clock: Arc<dyn UnixMillisClock>,
    operation: impl FnOnce(&SqliteTaskLedgerAdapter) -> Result<T, String>,
) -> SonaCoreBindingResult<T> {
    let database = Database::open(Path::new(app_data_dir)).map_err(task_ledger_error)?;
    let adapter = SqliteTaskLedgerAdapter::new(Arc::new(database), clock);
    operation(&adapter).map_err(task_ledger_error)
}

fn serialize_snapshot(snapshot: TaskLedgerSnapshot) -> SonaCoreBindingResult<String> {
    serde_json::to_string(&snapshot).map_err(|error| task_ledger_error(error.to_string()))
}

fn task_ledger_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::TaskLedger {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
struct FixedClock(u64);

#[cfg(test)]
impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, String> {
        Ok(self.0)
    }
}

#[cfg(test)]
fn load_task_ledger_snapshot_json_at(
    app_data_dir: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    load_task_ledger_snapshot_json_with_clock(app_data_dir, Arc::new(FixedClock(now_ms)))
}

#[cfg(test)]
fn upsert_task_ledger_record_json_at(
    app_data_dir: String,
    record_json: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    upsert_task_ledger_record_json_with_clock(
        app_data_dir,
        record_json,
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
fn patch_task_ledger_record_json_at(
    app_data_dir: String,
    id: String,
    patch_json: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    patch_task_ledger_record_json_with_clock(
        app_data_dir,
        id,
        patch_json,
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
fn remove_task_ledger_record_json_at(
    app_data_dir: String,
    id: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    remove_task_ledger_record_json_with_clock(app_data_dir, id, Arc::new(FixedClock(now_ms)))
}

#[cfg(test)]
fn clear_resolved_task_ledger_records_json_at(
    app_data_dir: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    clear_resolved_task_ledger_records_json_with_clock(app_data_dir, Arc::new(FixedClock(now_ms)))
}

#[cfg(test)]
mod tests {
    use super::{
        clear_resolved_task_ledger_records_json_at, load_task_ledger_snapshot_json_at,
        patch_task_ledger_record_json_at, remove_task_ledger_record_json_at,
        upsert_task_ledger_record_json_at,
    };
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use sona_core::task_ledger::repository::TaskLedgerStore;
    use sona_sqlite::{Database, SqliteLedgerRepository};
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    struct TestDir(tempfile::TempDir);

    impl TestDir {
        fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }

        fn path(&self) -> &Path {
            self.0.path()
        }

        fn app_data_dir(&self) -> String {
            self.path().to_string_lossy().into_owned()
        }
    }

    fn record_json(id: &str, status: &str) -> String {
        json!({
            "id": id,
            "kind": "llmPolish",
            "status": status,
            "title": "  Polish transcript  ",
            "progress": 25.0,
            "createdAt": 0,
            "updatedAt": 0,
            "retryable": false,
            "cancelable": true,
            "recoverable": false
        })
        .to_string()
    }

    #[test]
    fn load_returns_empty_canonical_snapshot_json() {
        let dir = TestDir::new();

        let output = load_task_ledger_snapshot_json_at(dir.app_data_dir(), 1_000).unwrap();

        assert_eq!(output, r#"{"version":1,"updatedAt":null,"tasks":[]}"#);
    }

    #[test]
    fn upsert_normalizes_persists_and_returns_canonical_json() {
        let dir = TestDir::new();

        let output = upsert_task_ledger_record_json_at(
            dir.app_data_dir(),
            record_json("  task-1  ", "pending"),
            2_000,
        )
        .unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["tasks"][0]["id"], "task-1");
        assert_eq!(value["tasks"][0]["title"], "Polish transcript");
        assert_eq!(value["tasks"][0]["createdAt"], 2_000);
        assert!(value["tasks"][0].get("created_at").is_none());

        let reloaded = load_task_ledger_snapshot_json_at(dir.app_data_dir(), 2_001).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&reloaded).unwrap(), value);
    }

    #[test]
    fn patch_updates_progress_with_supplied_timestamp() {
        let dir = TestDir::new();
        upsert_task_ledger_record_json_at(
            dir.app_data_dir(),
            record_json("task-2", "pending"),
            3_000,
        )
        .unwrap();

        let output = patch_task_ledger_record_json_at(
            dir.app_data_dir(),
            "task-2".to_string(),
            json!({"progress": 75.0}).to_string(),
            3_500,
        )
        .unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["tasks"][0]["progress"], 75.0);
        assert_eq!(value["tasks"][0]["updatedAt"], 3_500);
    }

    #[test]
    fn missing_patch_and_remove_are_noops() {
        let dir = TestDir::new();

        let patched = patch_task_ledger_record_json_at(
            dir.app_data_dir(),
            "missing".to_string(),
            "{}".to_string(),
            4_000,
        )
        .unwrap();
        let removed =
            remove_task_ledger_record_json_at(dir.app_data_dir(), "missing".to_string(), 4_001)
                .unwrap();

        assert_eq!(patched, r#"{"version":1,"updatedAt":null,"tasks":[]}"#);
        assert_eq!(removed, r#"{"version":1,"updatedAt":null,"tasks":[]}"#);
    }

    #[test]
    fn clear_resolved_removes_terminal_records_from_storage() {
        let dir = TestDir::new();
        upsert_task_ledger_record_json_at(
            dir.app_data_dir(),
            record_json("done", "succeeded"),
            5_000,
        )
        .unwrap();

        let output = clear_resolved_task_ledger_records_json_at(dir.app_data_dir(), 5_001).unwrap();
        assert_eq!(output, r#"{"version":1,"updatedAt":null,"tasks":[]}"#);

        let db = Arc::new(Database::open(dir.path()).unwrap());
        let repository = SqliteLedgerRepository::new(db);
        assert!(
            TaskLedgerStore::load_records(&repository)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn malformed_record_and_patch_json_are_invalid_input() {
        let dir = TestDir::new();

        let record_error =
            upsert_task_ledger_record_json_at(dir.app_data_dir(), "{".to_string(), 6_000)
                .unwrap_err();
        let patch_error = patch_task_ledger_record_json_at(
            dir.app_data_dir(),
            "task-1".to_string(),
            "{".to_string(),
            6_001,
        )
        .unwrap_err();

        assert!(matches!(
            record_error,
            SonaCoreBindingError::InvalidInput { .. }
        ));
        assert!(matches!(
            patch_error,
            SonaCoreBindingError::InvalidInput { .. }
        ));
    }

    #[test]
    fn app_data_path_that_is_a_file_maps_to_task_ledger_error() {
        let dir = TestDir::new();
        let blocked = dir.path().join("blocked");
        fs::write(&blocked, b"not a directory").unwrap();

        let error =
            load_task_ledger_snapshot_json_at(blocked.to_string_lossy().into_owned(), 7_000)
                .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::TaskLedger { .. }));
    }
}
