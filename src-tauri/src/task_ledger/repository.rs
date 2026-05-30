use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

use crate::storage::write_json_pretty_atomic;

use super::types::{
    TASK_LEDGER_DIR_NAME, TASK_LEDGER_FILE_NAME, TASK_LEDGER_VERSION, TaskLedgerRecord,
    TaskLedgerSnapshot, TaskLedgerStatus,
};

const INTERRUPTED_MESSAGE: &str = "Task was interrupted before it finished.";

#[derive(Clone, Debug)]
pub struct TaskLedgerRepository {
    app_local_data_dir: PathBuf,
}

impl TaskLedgerRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn ledger_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(TASK_LEDGER_DIR_NAME)
    }

    fn ledger_path(&self) -> PathBuf {
        self.ledger_dir().join(TASK_LEDGER_FILE_NAME)
    }

    pub fn ensure_ready(&self) -> Result<(), String> {
        fs::create_dir_all(self.ledger_dir()).map_err(|error| error.to_string())?;
        let ledger_path = self.ledger_path();
        if !ledger_path.exists() {
            write_json_pretty_atomic(&ledger_path, &empty_snapshot())?;
        }
        Ok(())
    }

    pub fn load_snapshot(&self) -> Result<TaskLedgerSnapshot, String> {
        self.ensure_ready()?;
        let content = fs::read_to_string(self.ledger_path()).map_err(|error| error.to_string())?;
        let value = match serde_json::from_str::<Value>(&content) {
            Ok(value) => value,
            Err(error) => {
                log::error!("[TaskLedger] Failed to parse task ledger: {}", error);
                let snapshot = empty_snapshot();
                self.write_snapshot(&snapshot)?;
                return Ok(snapshot);
            }
        };

        let snapshot = snapshot_from_value(value, true);
        self.write_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn upsert_task(&self, record: TaskLedgerRecord) -> Result<TaskLedgerSnapshot, String> {
        self.ensure_ready()?;
        let mut snapshot = self.read_snapshot_without_interrupting()?;
        let normalized_record = normalize_record(record, false)?;
        snapshot
            .tasks
            .retain(|task| task.id != normalized_record.id);
        if is_persisted_status(&normalized_record.status) {
            snapshot.tasks.push(normalized_record);
        }
        snapshot.updated_at = Some(now_ms());
        self.write_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn patch_task(&self, id: &str, patch: Value) -> Result<TaskLedgerSnapshot, String> {
        self.ensure_ready()?;
        let mut snapshot = self.read_snapshot_without_interrupting()?;
        let Some(index) = snapshot.tasks.iter().position(|task| task.id == id) else {
            return Ok(snapshot);
        };

        let mut value =
            serde_json::to_value(&snapshot.tasks[index]).map_err(|error| error.to_string())?;
        apply_object_patch(&mut value, patch);
        let next_record = record_from_value(value, false)?;
        snapshot.tasks.remove(index);
        if is_persisted_status(&next_record.status) {
            snapshot.tasks.insert(index, next_record);
        }
        snapshot.updated_at = Some(now_ms());
        self.write_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn remove_task(&self, id: &str) -> Result<TaskLedgerSnapshot, String> {
        self.ensure_ready()?;
        let mut snapshot = self.read_snapshot_without_interrupting()?;
        snapshot.tasks.retain(|task| task.id != id);
        snapshot.updated_at = Some(now_ms());
        self.write_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn clear_resolved(&self) -> Result<TaskLedgerSnapshot, String> {
        self.ensure_ready()?;
        let mut snapshot = self.read_snapshot_without_interrupting()?;
        snapshot
            .tasks
            .retain(|task| is_persisted_status(&task.status));
        snapshot.updated_at = Some(now_ms());
        self.write_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn read_snapshot_without_interrupting(&self) -> Result<TaskLedgerSnapshot, String> {
        let content = fs::read_to_string(self.ledger_path()).map_err(|error| error.to_string())?;
        let value = serde_json::from_str::<Value>(&content).unwrap_or_else(|error| {
            log::error!("[TaskLedger] Failed to parse task ledger: {}", error);
            serde_json::json!({})
        });
        Ok(snapshot_from_value(value, false))
    }

    fn write_snapshot(&self, snapshot: &TaskLedgerSnapshot) -> Result<(), String> {
        let mut persisted = snapshot.clone();
        persisted
            .tasks
            .retain(|task| is_persisted_status(&task.status));
        if persisted.tasks.is_empty() {
            persisted.updated_at = None;
        }
        write_json_pretty_atomic(&self.ledger_path(), &persisted)
    }
}

fn empty_snapshot() -> TaskLedgerSnapshot {
    TaskLedgerSnapshot {
        version: TASK_LEDGER_VERSION,
        updated_at: None,
        tasks: Vec::new(),
    }
}

fn snapshot_from_value(value: Value, interrupt_running: bool) -> TaskLedgerSnapshot {
    let updated_at = value.get("updatedAt").and_then(Value::as_u64);
    let tasks = value
        .get("tasks")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| record_from_value(item.clone(), interrupt_running).ok())
                .filter(|record| is_persisted_status(&record.status))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    TaskLedgerSnapshot {
        version: TASK_LEDGER_VERSION,
        updated_at: if tasks.is_empty() { None } else { updated_at },
        tasks,
    }
}

fn record_from_value(value: Value, interrupt_running: bool) -> Result<TaskLedgerRecord, String> {
    let record =
        serde_json::from_value::<TaskLedgerRecord>(value).map_err(|error| error.to_string())?;
    normalize_record(record, interrupt_running)
}

fn normalize_record(
    mut record: TaskLedgerRecord,
    interrupt_running: bool,
) -> Result<TaskLedgerRecord, String> {
    record.id = record.id.trim().to_string();
    record.title = record.title.trim().to_string();
    if record.id.is_empty() {
        return Err("Task id is required.".to_string());
    }
    if record.title.is_empty() {
        record.title = "Task".to_string();
    }

    record.progress = if record.progress.is_finite() {
        record.progress.clamp(0.0, 100.0)
    } else {
        0.0
    };

    if record.created_at == 0 {
        record.created_at = now_ms();
    }
    if record.updated_at == 0 {
        record.updated_at = record.created_at;
    }

    if interrupt_running
        && matches!(
            record.status,
            TaskLedgerStatus::Running | TaskLedgerStatus::CancelRequested
        )
    {
        record.status = TaskLedgerStatus::Interrupted;
        record.cancelable = false;
        if record
            .error_message
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            record.error_message = Some(INTERRUPTED_MESSAGE.to_string());
        }
    }

    Ok(record)
}

fn apply_object_patch(value: &mut Value, patch: Value) {
    let (Value::Object(target), Value::Object(patch)) = (value, patch) else {
        return;
    };

    merge_object(target, patch);
}

fn merge_object(target: &mut Map<String, Value>, patch: Map<String, Value>) {
    for (key, value) in patch {
        target.insert(key, value);
    }
}

fn is_persisted_status(status: &TaskLedgerStatus) -> bool {
    matches!(
        status,
        TaskLedgerStatus::Pending
            | TaskLedgerStatus::Running
            | TaskLedgerStatus::CancelRequested
            | TaskLedgerStatus::Failed
            | TaskLedgerStatus::Recoverable
            | TaskLedgerStatus::Interrupted
    )
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
