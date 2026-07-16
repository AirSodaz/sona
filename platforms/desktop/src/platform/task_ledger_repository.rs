use sona_core::task_ledger::types::{
    TASK_LEDGER_UPDATED_EVENT, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot,
};
use sona_runtime_fs::SystemClock;
use sona_sqlite::SqliteTaskLedgerAdapter;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};

use crate::platform::event::{EventEmitter, TauriEventEmitter};

async fn run_task_ledger_adapter_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&SqliteTaskLedgerAdapter) -> Result<T, String> + Send + 'static,
{
    let db = crate::platform::database::sqlite_database(app);
    tauri::async_runtime::spawn_blocking(move || {
        let adapter = SqliteTaskLedgerAdapter::new(db, Arc::new(SystemClock));
        task(&adapter)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn emit_task_ledger_snapshot(
    emitter: &dyn EventEmitter,
    snapshot: &TaskLedgerSnapshot,
) -> Result<(), String> {
    sona_ts_bind::validate_task_ledger_snapshot_for_typescript(snapshot)?;
    emitter.emit(
        TASK_LEDGER_UPDATED_EVENT,
        serde_json::to_value(snapshot).map_err(|error| error.to_string())?,
    )
}

pub async fn load_snapshot<R: Runtime>(app: &AppHandle<R>) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_task_ledger_adapter_task(app, |adapter| adapter.load_snapshot()).await?;
    sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)?;
    Ok(snapshot)
}

pub async fn upsert_task<R: Runtime>(
    app: &AppHandle<R>,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    sona_ts_bind::validate_task_ledger_record_for_typescript(&record)?;
    let snapshot =
        run_task_ledger_adapter_task(app, move |adapter| adapter.upsert_task(record)).await?;
    sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)?;
    let emitter = TauriEventEmitter(app.clone());
    let _ = emit_task_ledger_snapshot(&emitter, &snapshot);
    Ok(snapshot)
}

pub async fn patch_task<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    patch: TaskLedgerPatch,
) -> Result<TaskLedgerSnapshot, String> {
    sona_ts_bind::validate_task_ledger_patch_for_typescript(&patch)?;
    let snapshot =
        run_task_ledger_adapter_task(app, move |adapter| adapter.patch_task(&id, patch)).await?;
    sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)?;
    let emitter = TauriEventEmitter(app.clone());
    let _ = emit_task_ledger_snapshot(&emitter, &snapshot);
    Ok(snapshot)
}

pub async fn remove_task<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot =
        run_task_ledger_adapter_task(app, move |adapter| adapter.remove_task(&id)).await?;
    sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)?;
    let emitter = TauriEventEmitter(app.clone());
    let _ = emit_task_ledger_snapshot(&emitter, &snapshot);
    Ok(snapshot)
}

pub async fn clear_resolved<R: Runtime>(app: &AppHandle<R>) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_task_ledger_adapter_task(app, |adapter| adapter.clear_resolved()).await?;
    sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)?;
    let emitter = TauriEventEmitter(app.clone());
    let _ = emit_task_ledger_snapshot(&emitter, &snapshot);
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::event::MockEventEmitter;
    use sona_core::task_ledger::types::{TaskLedgerKind, TaskLedgerStatus};

    #[test]
    fn emits_canonical_task_ledger_snapshot_through_event_port() {
        let emitter = MockEventEmitter::new();
        let snapshot = TaskLedgerSnapshot {
            version: 1,
            updated_at: Some(2_000),
            tasks: vec![TaskLedgerRecord {
                id: "task-1".to_string(),
                kind: TaskLedgerKind::LlmPolish,
                status: TaskLedgerStatus::Running,
                title: "Polish transcript".to_string(),
                progress: 25.0,
                created_at: 1_000,
                updated_at: 2_000,
                retryable: false,
                cancelable: true,
                recoverable: false,
                stage: None,
                history_id: None,
                project_id: None,
                file_path: None,
                automation_rule_id: None,
                source_fingerprint: None,
                error_message: None,
                template_id: None,
                target_language: None,
            }],
        };

        emit_task_ledger_snapshot(&emitter, &snapshot).unwrap();

        let emitted = emitter.emitted.lock().unwrap();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].0, TASK_LEDGER_UPDATED_EVENT);
        assert_eq!(emitted[0].1["updatedAt"], 2_000);
        assert_eq!(emitted[0].1["tasks"][0]["createdAt"], 1_000);
        assert_eq!(emitted[0].1["tasks"][0]["kind"], "llmPolish");
    }

    #[test]
    fn rejects_invalid_task_ledger_snapshot_before_emitting() {
        let emitter = MockEventEmitter::new();
        let snapshot = TaskLedgerSnapshot {
            version: 1,
            updated_at: Some(sona_ts_bind::TYPESCRIPT_MAX_SAFE_INTEGER + 1),
            tasks: Vec::new(),
        };

        let error = emit_task_ledger_snapshot(&emitter, &snapshot).unwrap_err();

        assert!(error.contains("$.updatedAt"), "{error}");
        assert!(emitter.emitted.lock().unwrap().is_empty());
    }
}
