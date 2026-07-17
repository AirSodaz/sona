use serde_json::Value;

use crate::ports::time::UnixMillisClock;

use super::TaskLedgerError;
use super::repository::TaskLedgerStore;
use super::types::{
    TASK_LEDGER_VERSION, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};

const INTERRUPTED_MESSAGE: &str = "Task was interrupted before it finished.";

pub struct TaskLedgerService<'a> {
    store: &'a dyn TaskLedgerStore,
    clock: &'a dyn UnixMillisClock,
}

impl<'a> TaskLedgerService<'a> {
    pub fn new(store: &'a dyn TaskLedgerStore, clock: &'a dyn UnixMillisClock) -> Self {
        Self { store, clock }
    }

    pub fn load_snapshot(&self) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        self.load_snapshot_at(self.clock.now_ms()?)
    }

    pub fn load_snapshot_at(&self, now_ms: u64) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        let records = self.store.load_records()?;
        Ok(snapshot_from_records_at(records, now_ms))
    }

    pub fn upsert_task_at(
        &self,
        record: TaskLedgerRecord,
        now_ms: u64,
    ) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        let record = normalize_record_at(record, now_ms);
        self.store.upsert_record(&record)?;
        self.load_snapshot_at(now_ms)
    }

    pub fn upsert_task(
        &self,
        record: TaskLedgerRecord,
    ) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        self.upsert_task_at(record, self.clock.now_ms()?)
    }

    pub fn patch_task_at(
        &self,
        id: &str,
        patch: TaskLedgerPatch,
        now_ms: u64,
    ) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        let mut update = |record| Ok(apply_typed_patch_at(record, id, &patch, now_ms));
        self.store.update_record(id, &mut update)?;
        self.load_snapshot_at(now_ms)
    }

    pub fn patch_task(
        &self,
        id: &str,
        patch: TaskLedgerPatch,
    ) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        self.patch_task_at(id, patch, self.clock.now_ms()?)
    }

    pub fn patch_task_json_at(
        &self,
        id: &str,
        patch: Value,
        now_ms: u64,
    ) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        let mut update = |record| merge_json_patch_at(record, id, &patch, now_ms);
        self.store.update_record(id, &mut update)?;
        self.load_snapshot_at(now_ms)
    }

    pub fn patch_task_json(
        &self,
        id: &str,
        patch: Value,
    ) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        self.patch_task_json_at(id, patch, self.clock.now_ms()?)
    }

    pub fn remove_task_at(
        &self,
        id: &str,
        now_ms: u64,
    ) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        self.store.remove_record(id)?;
        self.load_snapshot_at(now_ms)
    }

    pub fn remove_task(&self, id: &str) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        self.remove_task_at(id, self.clock.now_ms()?)
    }

    pub fn clear_resolved_at(&self, now_ms: u64) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        let mut should_remove = |record: &TaskLedgerRecord| !is_retained_status(&record.status);
        self.store.remove_records_matching(&mut should_remove)?;
        self.load_snapshot_at(now_ms)
    }

    pub fn clear_resolved(&self) -> Result<TaskLedgerSnapshot, TaskLedgerError> {
        self.clear_resolved_at(self.clock.now_ms()?)
    }
}

fn apply_typed_patch_at(
    mut record: TaskLedgerRecord,
    id: &str,
    patch: &TaskLedgerPatch,
    now_ms: u64,
) -> TaskLedgerRecord {
    if let Some(value) = &patch.kind {
        record.kind = value.clone();
    }
    if let Some(value) = &patch.status {
        record.status = value.clone();
    }
    if let Some(value) = &patch.title {
        record.title.clone_from(value);
    }
    if let Some(value) = patch.progress {
        record.progress = value;
    }
    if let Some(value) = patch.created_at {
        record.created_at = value;
    }
    if let Some(value) = patch.retryable {
        record.retryable = value;
    }
    if let Some(value) = patch.cancelable {
        record.cancelable = value;
    }
    if let Some(value) = patch.recoverable {
        record.recoverable = value;
    }
    if let Some(value) = &patch.stage {
        record.stage.clone_from(value);
    }
    if let Some(value) = &patch.history_id {
        record.history_id.clone_from(value);
    }
    if let Some(value) = &patch.tag_ids {
        record.tag_ids.clone_from(value);
    }
    if let Some(value) = &patch.file_path {
        record.file_path.clone_from(value);
    }
    if let Some(value) = &patch.automation_rule_id {
        record.automation_rule_id.clone_from(value);
    }
    if let Some(value) = &patch.source_fingerprint {
        record.source_fingerprint.clone_from(value);
    }
    if let Some(value) = &patch.error_message {
        record.error_message.clone_from(value);
    }
    if let Some(value) = &patch.template_id {
        record.template_id.clone_from(value);
    }
    if let Some(value) = &patch.target_language {
        record.target_language.clone_from(value);
    }

    record.id = id.to_string();
    record.updated_at = now_ms;
    normalize_record_at(record, now_ms)
}

fn merge_json_patch_at(
    mut record: TaskLedgerRecord,
    id: &str,
    patch: &Value,
    now_ms: u64,
) -> Result<TaskLedgerRecord, TaskLedgerError> {
    if let Some(patch_object) = patch.as_object() {
        let mut current = serde_json::to_value(&record)?;
        if let Some(current_object) = current.as_object_mut() {
            for (key, value) in patch_object {
                current_object.insert(key.clone(), value.clone());
            }
        }
        record = serde_json::from_value(current)?;
    }

    record.id = id.to_string();
    record.updated_at = now_ms;
    Ok(normalize_record_at(record, now_ms))
}

fn normalize_record_at(mut record: TaskLedgerRecord, now_ms: u64) -> TaskLedgerRecord {
    record.id = record.id.trim().to_string();
    record.title = record.title.trim().to_string();
    if record.title.is_empty() {
        record.title = "Task".to_string();
    }
    record.progress = if record.progress.is_finite() {
        record.progress.clamp(0.0, 100.0)
    } else {
        0.0
    };
    if record.created_at == 0 {
        record.created_at = now_ms;
    }
    if record.updated_at == 0 {
        record.updated_at = record.created_at;
    }
    record
}

fn normalize_loaded_record_at(mut record: TaskLedgerRecord, now_ms: u64) -> TaskLedgerRecord {
    if record.created_at == 0 {
        record.created_at = now_ms;
    }
    if record.updated_at == 0 {
        record.updated_at = record.created_at;
    }
    if matches!(
        record.status,
        TaskLedgerStatus::Running | TaskLedgerStatus::CancelRequested
    ) {
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
    record
}

fn is_retained_status(status: &TaskLedgerStatus) -> bool {
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

fn snapshot_from_records_at(records: Vec<TaskLedgerRecord>, now_ms: u64) -> TaskLedgerSnapshot {
    let mut tasks = records
        .into_iter()
        .map(|record| normalize_loaded_record_at(record, now_ms))
        .filter(|record| is_retained_status(&record.status))
        .collect::<Vec<_>>();
    tasks.sort_by(|left, right| left.id.cmp(&right.id));
    let updated_at = tasks.iter().map(|record| record.updated_at).max();

    TaskLedgerSnapshot {
        version: TASK_LEDGER_VERSION,
        updated_at,
        tasks,
    }
}
