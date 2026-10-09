use sona_core::task_ledger::TaskLedgerError;
use sona_core::task_ledger::types::{
    TASK_LEDGER_UPDATED_EVENT, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot,
};
use sona_runtime_fs::SystemClock;
use sona_sqlite::SqliteTaskLedgerAdapter;
use std::sync::Arc;

use crate::platform::database::DesktopSqliteState;
use crate::platform::event::EventEmitterPort;
use crate::services::db_runner::{map_err_string, run_sqlite_task};

fn emit_task_ledger_snapshot(
    emitter: &dyn EventEmitterPort,
    snapshot: &TaskLedgerSnapshot,
) -> Result<(), String> {
    sona_ts_bind::validate_task_ledger_snapshot_for_typescript(snapshot).map_err(map_err_string)?;
    emitter
        .emit(
            TASK_LEDGER_UPDATED_EVENT,
            serde_json::to_value(snapshot).map_err(map_err_string)?,
        )
        .map_err(map_err_string)
}

/// Pure Rust Task Ledger domain service without any Tauri dependency.
#[derive(Clone)]
pub struct TaskLedgerService {
    sqlite: DesktopSqliteState,
    emitter: Arc<dyn EventEmitterPort>,
}

impl TaskLedgerService {
    pub fn new(sqlite: DesktopSqliteState, emitter: Arc<dyn EventEmitterPort>) -> Self {
        Self { sqlite, emitter }
    }

    pub fn sqlite(&self) -> &DesktopSqliteState {
        &self.sqlite
    }

    pub fn emitter(&self) -> &Arc<dyn EventEmitterPort> {
        &self.emitter
    }

    async fn run_adapter<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&SqliteTaskLedgerAdapter) -> Result<T, TaskLedgerError> + Send + 'static,
    {
        run_sqlite_task(&self.sqlite, move |context| {
            let adapter = context.task_ledger_adapter(Arc::new(SystemClock));
            task(&adapter)
        })
        .await
    }

    pub async fn load_snapshot(&self) -> Result<TaskLedgerSnapshot, String> {
        let snapshot = self.run_adapter(|adapter| adapter.load_snapshot()).await?;
        sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)
            .map_err(map_err_string)?;
        Ok(snapshot)
    }

    pub async fn upsert_task(
        &self,
        record: TaskLedgerRecord,
    ) -> Result<TaskLedgerSnapshot, String> {
        sona_ts_bind::validate_task_ledger_record_for_typescript(&record)
            .map_err(map_err_string)?;
        let snapshot = self
            .run_adapter(move |adapter| adapter.upsert_task(record))
            .await?;
        sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)
            .map_err(map_err_string)?;
        let _ = emit_task_ledger_snapshot(self.emitter.as_ref(), &snapshot);
        Ok(snapshot)
    }

    pub async fn patch_task(
        &self,
        id: String,
        patch: TaskLedgerPatch,
    ) -> Result<TaskLedgerSnapshot, String> {
        sona_ts_bind::validate_task_ledger_patch_for_typescript(&patch).map_err(map_err_string)?;
        let snapshot = self
            .run_adapter(move |adapter| adapter.patch_task(&id, patch))
            .await?;
        sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)
            .map_err(map_err_string)?;
        let _ = emit_task_ledger_snapshot(self.emitter.as_ref(), &snapshot);
        Ok(snapshot)
    }

    pub async fn remove_task(&self, id: String) -> Result<TaskLedgerSnapshot, String> {
        let snapshot = self
            .run_adapter(move |adapter| adapter.remove_task(&id))
            .await?;
        sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)
            .map_err(map_err_string)?;
        let _ = emit_task_ledger_snapshot(self.emitter.as_ref(), &snapshot);
        Ok(snapshot)
    }

    pub async fn clear_resolved(&self) -> Result<TaskLedgerSnapshot, String> {
        let snapshot = self.run_adapter(|adapter| adapter.clear_resolved()).await?;
        sona_ts_bind::validate_task_ledger_snapshot_for_typescript(&snapshot)
            .map_err(map_err_string)?;
        let _ = emit_task_ledger_snapshot(self.emitter.as_ref(), &snapshot);
        Ok(snapshot)
    }
}

// Standalone functions for convenience
pub async fn load_snapshot(service: &TaskLedgerService) -> Result<TaskLedgerSnapshot, String> {
    service.load_snapshot().await
}

pub async fn upsert_task(
    service: &TaskLedgerService,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    service.upsert_task(record).await
}

pub async fn patch_task(
    service: &TaskLedgerService,
    id: String,
    patch: TaskLedgerPatch,
) -> Result<TaskLedgerSnapshot, String> {
    service.patch_task(id, patch).await
}

pub async fn remove_task(
    service: &TaskLedgerService,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    service.remove_task(id).await
}

pub async fn clear_resolved(service: &TaskLedgerService) -> Result<TaskLedgerSnapshot, String> {
    service.clear_resolved().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::event::MockEventEmitter;
    use sona_core::task_ledger::types::{TaskLedgerKind, TaskLedgerStatus};

    #[tokio::test]
    async fn test_task_ledger_service_without_tauri() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let state = DesktopSqliteState::new(ctx);
        let emitter = Arc::new(MockEventEmitter::new());
        let service = TaskLedgerService::new(state, emitter.clone());

        let snapshot = service.load_snapshot().await.unwrap();
        assert_eq!(snapshot.tasks.len(), 0);

        let record = TaskLedgerRecord {
            id: "task-test".to_string(),
            kind: TaskLedgerKind::LlmPolish,
            status: TaskLedgerStatus::Running,
            title: "Test Task".to_string(),
            progress: 10.0,
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
            tag_automation_rule_id: None,
            automation_profile_id: None,
            automation_profile_source: None,
            source_fingerprint: None,
            error_message: None,
            template_id: None,
            target_language: None,
        };

        let updated = service.upsert_task(record).await.unwrap();
        assert_eq!(updated.tasks.len(), 1);
        assert_eq!(emitter.emitted.lock().unwrap().len(), 1);

        let cleared = service.clear_resolved().await.unwrap();
        assert_eq!(cleared.tasks.len(), 1); // Still running, not resolved
    }

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
                tag_ids: Vec::new(),
                file_path: None,
                automation_rule_id: None,
                tag_automation_rule_id: None,
                automation_profile_id: None,
                automation_profile_source: None,
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
