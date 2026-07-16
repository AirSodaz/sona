use serde_json::json;
use sona_core::ports::time::UnixMillisClock;
use sona_core::task_ledger::repository::TaskLedgerStore;
use sona_core::task_ledger::service::TaskLedgerService;
use sona_core::task_ledger::types::{
    TaskLedgerKind, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerStatus,
};
use std::sync::Mutex;

#[derive(Default)]
struct MemoryStore {
    records: Mutex<Vec<TaskLedgerRecord>>,
}

impl MemoryStore {
    fn with_records(records: impl IntoIterator<Item = TaskLedgerRecord>) -> Self {
        Self {
            records: Mutex::new(records.into_iter().collect()),
        }
    }

    fn records(&self) -> Vec<TaskLedgerRecord> {
        self.records.lock().unwrap().clone()
    }
}

impl TaskLedgerStore for MemoryStore {
    fn load_records(&self) -> Result<Vec<TaskLedgerRecord>, String> {
        Ok(self.records())
    }

    fn upsert_record(&self, record: &TaskLedgerRecord) -> Result<(), String> {
        let mut records = self.records.lock().unwrap();
        records.retain(|item| item.id != record.id);
        records.push(record.clone());
        Ok(())
    }

    fn update_record(
        &self,
        id: &str,
        update: &mut dyn FnMut(TaskLedgerRecord) -> Result<TaskLedgerRecord, String>,
    ) -> Result<(), String> {
        let mut records = self.records.lock().unwrap();
        if let Some(index) = records.iter().position(|item| item.id == id) {
            records[index] = update(records[index].clone())?;
        }
        Ok(())
    }

    fn remove_record(&self, id: &str) -> Result<(), String> {
        self.records.lock().unwrap().retain(|item| item.id != id);
        Ok(())
    }

    fn remove_records_matching(
        &self,
        predicate: &mut dyn FnMut(&TaskLedgerRecord) -> bool,
    ) -> Result<(), String> {
        self.records.lock().unwrap().retain(|item| !predicate(item));
        Ok(())
    }
}

struct FailingStore;

impl TaskLedgerStore for FailingStore {
    fn load_records(&self) -> Result<Vec<TaskLedgerRecord>, String> {
        Err("store failure".into())
    }

    fn upsert_record(&self, _record: &TaskLedgerRecord) -> Result<(), String> {
        Err("store failure".into())
    }

    fn update_record(
        &self,
        _id: &str,
        _update: &mut dyn FnMut(TaskLedgerRecord) -> Result<TaskLedgerRecord, String>,
    ) -> Result<(), String> {
        Err("store failure".into())
    }

    fn remove_record(&self, _id: &str) -> Result<(), String> {
        Err("store failure".into())
    }

    fn remove_records_matching(
        &self,
        _predicate: &mut dyn FnMut(&TaskLedgerRecord) -> bool,
    ) -> Result<(), String> {
        Err("store failure".into())
    }
}

struct FixedClock(Result<u64, String>);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, String> {
        self.0.clone()
    }
}

static DEFAULT_CLOCK: FixedClock = FixedClock(Ok(0));

fn service<'a>(store: &'a dyn TaskLedgerStore) -> TaskLedgerService<'a> {
    TaskLedgerService::new(store, &DEFAULT_CLOCK)
}

fn record(id: &str, status: TaskLedgerStatus) -> TaskLedgerRecord {
    TaskLedgerRecord {
        id: id.into(),
        kind: TaskLedgerKind::LlmPolish,
        status,
        title: format!("Task {id}"),
        progress: 50.0,
        created_at: 1_000,
        updated_at: 1_000,
        retryable: false,
        cancelable: true,
        recoverable: false,
        stage: None,
        history_id: None,
        tag_ids: Vec::new(),
        file_path: None,
        automation_rule_id: None,
        source_fingerprint: None,
        error_message: None,
        template_id: None,
        target_language: None,
    }
}

#[test]
fn upsert_normalizes_identity_title_nonfinite_progress_and_zero_timestamps() {
    let store = MemoryStore::default();
    let service = service(&store);
    let mut task = record("  task-1  ", TaskLedgerStatus::Pending);
    task.title = "   ".into();
    task.progress = f64::NAN;
    task.created_at = 0;
    task.updated_at = 0;

    let snapshot = service.upsert_task_at(task, 5_000).unwrap();

    assert_eq!(snapshot.tasks[0].id, "task-1");
    assert_eq!(snapshot.tasks[0].title, "Task");
    assert_eq!(snapshot.tasks[0].progress, 0.0);
    assert_eq!(snapshot.tasks[0].created_at, 5_000);
    assert_eq!(snapshot.tasks[0].updated_at, 5_000);
}

#[test]
fn upsert_clamps_finite_progress_to_percent_bounds() {
    let store = MemoryStore::default();
    let service = service(&store);
    let mut lower = record("lower", TaskLedgerStatus::Pending);
    lower.progress = -1.0;
    let mut upper = record("upper", TaskLedgerStatus::Pending);
    upper.progress = 101.0;

    service.upsert_task_at(lower, 5_000).unwrap();
    let snapshot = service.upsert_task_at(upper, 5_000).unwrap();

    assert_eq!(snapshot.tasks[0].progress, 0.0);
    assert_eq!(snapshot.tasks[1].progress, 100.0);
}

#[test]
fn load_recovers_running_task_with_default_interruption_error() {
    let store = MemoryStore::with_records([record("running", TaskLedgerStatus::Running)]);

    let snapshot = service(&store).load_snapshot_at(6_000).unwrap();

    assert_eq!(snapshot.tasks[0].status, TaskLedgerStatus::Interrupted);
    assert!(!snapshot.tasks[0].cancelable);
    assert_eq!(
        snapshot.tasks[0].error_message.as_deref(),
        Some("Task was interrupted before it finished.")
    );
}

#[test]
fn load_preserves_nonempty_interruption_error() {
    let mut task = record("cancel", TaskLedgerStatus::CancelRequested);
    task.error_message = Some("Cancellation was pending".into());
    let store = MemoryStore::with_records([task]);

    let snapshot = service(&store).load_snapshot_at(6_000).unwrap();

    assert_eq!(
        snapshot.tasks[0].error_message.as_deref(),
        Some("Cancellation was pending")
    );
}

#[test]
fn load_supplies_zero_timestamps_from_host_time() {
    let mut task = record("zero-time", TaskLedgerStatus::Pending);
    task.created_at = 0;
    task.updated_at = 0;
    let store = MemoryStore::with_records([task]);

    let snapshot = service(&store).load_snapshot_at(6_000).unwrap();

    assert_eq!(snapshot.tasks[0].created_at, 6_000);
    assert_eq!(snapshot.tasks[0].updated_at, 6_000);
    assert_eq!(snapshot.updated_at, Some(6_000));
}

#[test]
fn load_filters_resolved_tasks_and_sorts_retained_ids() {
    let store = MemoryStore::with_records([
        record("z-pending", TaskLedgerStatus::Pending),
        record("done", TaskLedgerStatus::Succeeded),
        record("a-failed", TaskLedgerStatus::Failed),
        record("cancelled", TaskLedgerStatus::Cancelled),
    ]);

    let snapshot = service(&store).load_snapshot_at(6_000).unwrap();

    assert_eq!(
        snapshot
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a-failed", "z-pending"]
    );
}

#[test]
fn typed_patch_updates_fields_clears_nullable_values_and_supplies_update_time() {
    let mut task = record("task-2", TaskLedgerStatus::Pending);
    task.error_message = Some("previous failure".into());
    let store = MemoryStore::with_records([task]);

    let snapshot = service(&store)
        .patch_task_at(
            "task-2",
            TaskLedgerPatch {
                status: Some(TaskLedgerStatus::Running),
                progress: Some(75.0),
                updated_at: Some(123),
                error_message: Some(None),
                ..Default::default()
            },
            7_000,
        )
        .unwrap();

    assert_eq!(snapshot.tasks[0].status, TaskLedgerStatus::Interrupted);
    assert_eq!(snapshot.tasks[0].progress, 75.0);
    assert_eq!(
        snapshot.tasks[0].error_message.as_deref(),
        Some("Task was interrupted before it finished.")
    );
    assert_eq!(snapshot.tasks[0].updated_at, 7_000);
    assert_eq!(store.records()[0].status, TaskLedgerStatus::Running);
    assert_eq!(store.records()[0].error_message, None);
}

#[test]
fn typed_patch_deserialization_distinguishes_missing_null_and_string_fields() {
    let missing: TaskLedgerPatch = serde_json::from_value(json!({})).unwrap();
    let cleared: TaskLedgerPatch = serde_json::from_value(json!({"errorMessage": null})).unwrap();
    let replaced: TaskLedgerPatch =
        serde_json::from_value(json!({"errorMessage": "retry failed"})).unwrap();

    assert_eq!(missing.error_message, None);
    assert_eq!(cleared.error_message, Some(None));
    assert_eq!(replaced.error_message, Some(Some("retry failed".into())));
}

#[test]
fn json_patch_compatibility_merges_object_forces_id_and_supplies_update_time() {
    let store = MemoryStore::with_records([record("task-2", TaskLedgerStatus::Pending)]);

    let snapshot = service(&store)
        .patch_task_json_at("task-2", json!({"id":"changed","progress":75.0}), 7_000)
        .unwrap();

    assert_eq!(snapshot.tasks[0].id, "task-2");
    assert_eq!(snapshot.tasks[0].progress, 75.0);
    assert_eq!(snapshot.tasks[0].updated_at, 7_000);
}

#[test]
fn nonobject_patch_still_updates_timestamp() {
    let store = MemoryStore::with_records([record("task-2", TaskLedgerStatus::Pending)]);

    let snapshot = service(&store)
        .patch_task_json_at("task-2", json!(null), 7_000)
        .unwrap();

    assert_eq!(snapshot.tasks[0].progress, 50.0);
    assert_eq!(snapshot.tasks[0].updated_at, 7_000);
}

#[test]
fn invalid_patch_returns_serialization_error() {
    let store = MemoryStore::with_records([record("task-2", TaskLedgerStatus::Pending)]);

    let error = service(&store)
        .patch_task_json_at("task-2", json!({"status":"not-a-status"}), 7_000)
        .unwrap_err();

    assert!(error.starts_with("Serialization error: "));
}

#[test]
fn patch_missing_record_is_noop() {
    let store = MemoryStore::default();

    let snapshot = service(&store)
        .patch_task_at(
            "missing",
            TaskLedgerPatch {
                progress: Some(75.0),
                ..Default::default()
            },
            7_000,
        )
        .unwrap();

    assert!(snapshot.tasks.is_empty());
}

#[test]
fn remove_returns_snapshot_without_removed_record() {
    let store = MemoryStore::with_records([
        record("keep", TaskLedgerStatus::Pending),
        record("remove", TaskLedgerStatus::Failed),
    ]);

    let snapshot = service(&store).remove_task_at("remove", 8_000).unwrap();

    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].id, "keep");
}

#[test]
fn clear_resolved_removes_only_terminal_records() {
    let store = MemoryStore::with_records([
        record("pending", TaskLedgerStatus::Pending),
        record("failed", TaskLedgerStatus::Failed),
        record("done", TaskLedgerStatus::Succeeded),
        record("cancelled", TaskLedgerStatus::Cancelled),
    ]);

    let snapshot = service(&store).clear_resolved_at(8_000).unwrap();

    assert_eq!(
        snapshot
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        vec!["failed", "pending"]
    );
    assert_eq!(store.records().len(), 2);
}

#[test]
fn empty_snapshot_has_no_update_time() {
    let snapshot = service(&MemoryStore::default())
        .load_snapshot_at(9_000)
        .unwrap();

    assert_eq!(snapshot.version, 2);
    assert_eq!(snapshot.updated_at, None);
    assert!(snapshot.tasks.is_empty());
}

#[test]
fn store_errors_are_returned_unchanged() {
    let service = service(&FailingStore);

    assert_eq!(
        service.load_snapshot_at(9_000).unwrap_err(),
        "store failure"
    );
    assert_eq!(
        service
            .upsert_task_at(record("task", TaskLedgerStatus::Pending), 9_000)
            .unwrap_err(),
        "store failure"
    );
    assert_eq!(
        service
            .patch_task_at("task", TaskLedgerPatch::default(), 9_000)
            .unwrap_err(),
        "store failure"
    );
    assert_eq!(
        service.remove_task_at("task", 9_000).unwrap_err(),
        "store failure"
    );
    assert_eq!(
        service.clear_resolved_at(9_000).unwrap_err(),
        "store failure"
    );
}

#[test]
fn task_ledger_runtime_methods_use_the_injected_clock() {
    let store = MemoryStore::default();
    let clock = FixedClock(Ok(10_000));
    let service = TaskLedgerService::new(&store, &clock);
    let mut pending = record("runtime", TaskLedgerStatus::Pending);
    pending.created_at = 0;
    pending.updated_at = 0;

    let upserted = service.upsert_task(pending).unwrap();
    let patched = service
        .patch_task(
            "runtime",
            TaskLedgerPatch {
                progress: Some(75.0),
                ..Default::default()
            },
        )
        .unwrap();
    let loaded = service.load_snapshot().unwrap();
    let removed = service.remove_task("runtime").unwrap();
    let cleared = service.clear_resolved().unwrap();

    assert_eq!(upserted.tasks[0].created_at, 10_000);
    assert_eq!(patched.tasks[0].updated_at, 10_000);
    assert_eq!(loaded.tasks[0].updated_at, 10_000);
    assert!(removed.tasks.is_empty());
    assert!(cleared.tasks.is_empty());
}

#[test]
fn task_ledger_runtime_methods_propagate_clock_errors_before_writes() {
    let store = MemoryStore::default();
    let clock = FixedClock(Err("task ledger clock unavailable".to_string()));
    let service = TaskLedgerService::new(&store, &clock);

    let error = service
        .upsert_task(record("runtime", TaskLedgerStatus::Pending))
        .unwrap_err();

    assert_eq!(error, "task ledger clock unavailable");
    assert!(store.records().is_empty());
}
