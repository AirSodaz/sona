use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use serde::Serialize;
use serde_json::Value;
use sona_core::ports::time::UnixMillisClock;
use sona_core::task_ledger::repository::TaskLedgerStore;
use sona_core::task_ledger::service::TaskLedgerService;
use sona_core::task_ledger::types::{
    TASK_LEDGER_VERSION, TaskLedgerKind, TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot,
    TaskLedgerStatus,
};
use std::sync::Arc;

const TASK_LEDGER_COLUMNS: &str = "id, kind, status, title, progress, created_at, updated_at,
    retryable, cancelable, recoverable, stage, history_id, project_id, file_path,
    automation_rule_id, source_fingerprint, error_message, template_id, target_language, version";
const UPSERT_TASK_SQL: &str = "INSERT OR REPLACE INTO task_ledger (
    id, kind, status, title, progress, created_at, updated_at,
    retryable, cancelable, recoverable, stage, history_id, project_id, file_path,
    automation_rule_id, source_fingerprint, error_message, template_id, target_language, version
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)";

#[derive(Clone)]
pub struct SqliteLedgerRepository<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteLedgerRepository);

pub struct SqliteTaskLedgerAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    repository: SqliteLedgerRepository<D>,
    clock: Arc<dyn UnixMillisClock>,
}

impl<D> SqliteTaskLedgerAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(db: Arc<D>, clock: Arc<dyn UnixMillisClock>) -> Self {
        Self {
            repository: SqliteLedgerRepository::new(db),
            clock,
        }
    }

    pub fn load_snapshot(&self) -> Result<TaskLedgerSnapshot, String> {
        self.service().load_snapshot()
    }

    pub fn upsert_task(&self, record: TaskLedgerRecord) -> Result<TaskLedgerSnapshot, String> {
        self.service().upsert_task(record)
    }

    pub fn patch_task(
        &self,
        id: &str,
        patch: TaskLedgerPatch,
    ) -> Result<TaskLedgerSnapshot, String> {
        self.service().patch_task(id, patch)
    }

    pub fn patch_task_json(&self, id: &str, patch: Value) -> Result<TaskLedgerSnapshot, String> {
        self.service().patch_task_json(id, patch)
    }

    pub fn remove_task(&self, id: &str) -> Result<TaskLedgerSnapshot, String> {
        self.service().remove_task(id)
    }

    pub fn clear_resolved(&self) -> Result<TaskLedgerSnapshot, String> {
        self.service().clear_resolved()
    }

    fn service(&self) -> TaskLedgerService<'_> {
        TaskLedgerService::new(&self.repository, self.clock.as_ref())
    }
}

impl<D> TaskLedgerStore for SqliteLedgerRepository<D>
where
    D: DatabasePort,
{
    fn load_records(&self) -> Result<Vec<TaskLedgerRecord>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(|conn| {
                let sql = format!("SELECT {TASK_LEDGER_COLUMNS} FROM task_ledger ORDER BY id");
                let mut stmt = conn.prepare_cached(&sql)?;
                let mut rows = stmt.query([])?;
                let mut records = Vec::new();
                while let Some(row) = rows.next()? {
                    records.push(map_row_to_record(row)?);
                }
                Ok(records)
            })
            .map_err(|error| error.to_string())
    }

    fn upsert_record(&self, record: &TaskLedgerRecord) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_write_connection(|conn| {
                let mut stmt = conn.prepare_cached(UPSERT_TASK_SQL)?;
                execute_upsert_task(&mut stmt, record)
            })
            .map_err(|error| error.to_string())
    }

    fn update_record(
        &self,
        id: &str,
        update: &mut dyn FnMut(TaskLedgerRecord) -> Result<TaskLedgerRecord, String>,
    ) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_rw_transaction(|tx| {
                let record = {
                    let sql =
                        format!("SELECT {TASK_LEDGER_COLUMNS} FROM task_ledger WHERE id = ?1");
                    let mut stmt = tx.prepare_cached(&sql)?;
                    let mut rows = stmt.query([id])?;
                    rows.next()?.map(map_row_to_record).transpose()?
                };
                let Some(record) = record else {
                    return Ok(());
                };
                let record = update(record).map_err(DatabaseError::Internal)?;
                if record.id != id {
                    tx.execute("DELETE FROM task_ledger WHERE id = ?1", [id])?;
                }
                let mut stmt = tx.prepare_cached(UPSERT_TASK_SQL)?;
                execute_upsert_task(&mut stmt, &record)
            })
            .map_err(|error| error.to_string())
    }

    fn remove_record(&self, id: &str) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_write_connection(|conn| {
                conn.execute("DELETE FROM task_ledger WHERE id = ?1", [id])?;
                Ok(())
            })
            .map_err(|error| error.to_string())
    }

    fn remove_records_matching(
        &self,
        predicate: &mut dyn FnMut(&TaskLedgerRecord) -> bool,
    ) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_rw_transaction(|tx| {
                let ids = {
                    let sql = format!("SELECT {TASK_LEDGER_COLUMNS} FROM task_ledger ORDER BY id");
                    let mut stmt = tx.prepare_cached(&sql)?;
                    let mut rows = stmt.query([])?;
                    let mut ids = Vec::new();
                    while let Some(row) = rows.next()? {
                        let record = map_row_to_record(row)?;
                        if predicate(&record) {
                            ids.push(record.id);
                        }
                    }
                    ids
                };
                let mut stmt = tx.prepare_cached("DELETE FROM task_ledger WHERE id = ?1")?;
                for id in ids {
                    stmt.execute([id])?;
                }
                Ok(())
            })
            .map_err(|error| error.to_string())
    }
}

fn map_row_to_record(row: &rusqlite::Row) -> Result<TaskLedgerRecord, DatabaseError> {
    Ok(TaskLedgerRecord {
        id: row.get("id")?,
        kind: task_kind_from_storage(row.get("kind")?)?,
        status: task_status_from_storage(row.get("status")?)?,
        title: row.get("title")?,
        progress: row.get("progress")?,
        created_at: row.get::<_, i64>("created_at")?.max(0) as u64,
        updated_at: row.get::<_, i64>("updated_at")?.max(0) as u64,
        retryable: row.get::<_, i64>("retryable")? != 0,
        cancelable: row.get::<_, i64>("cancelable")? != 0,
        recoverable: row.get::<_, i64>("recoverable")? != 0,
        stage: row.get("stage")?,
        history_id: row.get("history_id")?,
        project_id: row.get("project_id")?,
        file_path: row.get("file_path")?,
        automation_rule_id: row.get("automation_rule_id")?,
        source_fingerprint: row.get("source_fingerprint")?,
        error_message: row.get("error_message")?,
        template_id: row.get("template_id")?,
        target_language: row.get("target_language")?,
    })
}

fn execute_upsert_task(
    stmt: &mut rusqlite::Statement<'_>,
    record: &TaskLedgerRecord,
) -> Result<(), DatabaseError> {
    stmt.execute(rusqlite::params![
        record.id,
        enum_to_storage(&record.kind)?,
        enum_to_storage(&record.status)?,
        record.title,
        record.progress,
        record.created_at as i64,
        record.updated_at as i64,
        record.retryable as i64,
        record.cancelable as i64,
        record.recoverable as i64,
        record.stage.as_deref(),
        record.history_id.as_deref(),
        record.project_id.as_deref(),
        record.file_path.as_deref(),
        record.automation_rule_id.as_deref(),
        record.source_fingerprint.as_deref(),
        record.error_message.as_deref(),
        record.template_id.as_deref(),
        record.target_language.as_deref(),
        TASK_LEDGER_VERSION as i64,
    ])?;
    Ok(())
}

fn enum_to_storage<T: Serialize>(value: &T) -> Result<String, DatabaseError> {
    match serde_json::to_value(value)? {
        Value::String(value) => Ok(value),
        _ => Err(DatabaseError::Internal(
            "task ledger enum did not serialize as a string".to_string(),
        )),
    }
}

fn task_kind_from_storage(value: String) -> Result<TaskLedgerKind, DatabaseError> {
    let stored = if value.trim().is_empty() {
        "llmPolish".to_string()
    } else {
        value
    };
    serde_json::from_value(Value::String(stored)).map_err(DatabaseError::SerializationError)
}

fn task_status_from_storage(value: String) -> Result<TaskLedgerStatus, DatabaseError> {
    let stored = if value.trim().is_empty() {
        "pending".to_string()
    } else {
        value
    };
    serde_json::from_value(Value::String(stored)).map_err(DatabaseError::SerializationError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use serde_json::json;
    use sona_core::task_ledger::repository::TaskLedgerStore;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TestClock;

    impl UnixMillisClock for TestClock {
        fn now_ms(&self) -> Result<u64, String> {
            Ok(0)
        }
    }

    static TEST_CLOCK: TestClock = TestClock;

    struct FixedClock(u64);

    impl UnixMillisClock for FixedClock {
        fn now_ms(&self) -> Result<u64, String> {
            Ok(self.0)
        }
    }

    fn service(repository: &SqliteLedgerRepository) -> TaskLedgerService<'_> {
        TaskLedgerService::new(repository, &TEST_CLOCK)
    }

    fn make_record(id: &str, status: TaskLedgerStatus) -> TaskLedgerRecord {
        TaskLedgerRecord {
            id: id.to_string(),
            kind: TaskLedgerKind::LlmPolish,
            status,
            title: format!("Task {}", id),
            progress: 50.0,
            created_at: 1000,
            updated_at: 1000,
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
        }
    }

    #[test]
    fn task_ledger_adapter_composes_repository_and_clock() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let adapter = SqliteTaskLedgerAdapter::new(db, Arc::new(FixedClock(2_000)));
        let mut record = make_record("adapter-task", TaskLedgerStatus::Pending);
        record.created_at = 0;
        record.updated_at = 0;

        let snapshot = adapter.upsert_task(record).unwrap();

        assert_eq!(snapshot.updated_at, Some(2_000));
        assert_eq!(snapshot.tasks[0].id, "adapter-task");
        assert_eq!(snapshot.tasks[0].created_at, 2_000);
        assert_eq!(snapshot.tasks[0].updated_at, 2_000);
    }

    #[test]
    fn store_port_updates_existing_record_atomically() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        let task = make_record("atomic-update", TaskLedgerStatus::Pending);
        TaskLedgerStore::upsert_record(&repo, &task).unwrap();
        let mut update = |mut record: TaskLedgerRecord| {
            record.progress = 75.0;
            Ok(record)
        };

        TaskLedgerStore::update_record(&repo, "atomic-update", &mut update).unwrap();

        let records = TaskLedgerStore::load_records(&repo).unwrap();
        assert_eq!(records[0].progress, 75.0);
    }

    #[test]
    fn service_patch_rekeys_legacy_record_to_normalized_identity() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        let task = make_record("  legacy-id  ", TaskLedgerStatus::Pending);
        TaskLedgerStore::upsert_record(&repo, &task).unwrap();

        let snapshot = service(&repo)
            .patch_task_json_at("  legacy-id  ", json!({"progress": 75.0}), 2_000)
            .unwrap();

        let records = TaskLedgerStore::load_records(&repo).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "legacy-id");
        assert_eq!(snapshot.tasks, records);
    }

    #[test]
    fn store_port_update_rolls_back_when_callback_fails() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        let task = make_record("rollback-id", TaskLedgerStatus::Pending);
        TaskLedgerStore::upsert_record(&repo, &task).unwrap();
        let mut update = |_record: TaskLedgerRecord| Err("sentinel update failure".to_string());

        let error = TaskLedgerStore::update_record(&repo, "rollback-id", &mut update).unwrap_err();

        assert_eq!(error, "sentinel update failure");
        assert_eq!(TaskLedgerStore::load_records(&repo).unwrap(), vec![task]);
    }

    #[test]
    fn store_port_removes_records_matching_core_predicate() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        TaskLedgerStore::upsert_record(&repo, &make_record("pending", TaskLedgerStatus::Pending))
            .unwrap();
        TaskLedgerStore::upsert_record(
            &repo,
            &make_record("succeeded", TaskLedgerStatus::Succeeded),
        )
        .unwrap();
        let mut is_succeeded =
            |record: &TaskLedgerRecord| matches!(record.status, TaskLedgerStatus::Succeeded);

        TaskLedgerStore::remove_records_matching(&repo, &mut is_succeeded).unwrap();

        let records = TaskLedgerStore::load_records(&repo).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "pending");
    }

    #[test]
    fn test_ledger_upsert_and_load() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let repo = SqliteLedgerRepository::new(Arc::clone(&db));
        let service = service(&repo);

        let record = make_record("task-1", TaskLedgerStatus::Running);
        let snapshot = service.upsert_task_at(record, 2_000).unwrap();
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].status, TaskLedgerStatus::Interrupted);
        assert_eq!(snapshot.tasks[0].id, "task-1");
    }

    #[test]
    fn test_ledger_patch_task() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        let service = service(&repo);

        let record = make_record("task-2", TaskLedgerStatus::Pending);
        service.upsert_task_at(record, 2_000).unwrap();
        let snapshot = service
            .patch_task_at(
                "task-2",
                TaskLedgerPatch {
                    progress: Some(75.0),
                    ..Default::default()
                },
                3_000,
            )
            .unwrap();
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].progress, 75.0);
        assert_eq!(snapshot.tasks[0].updated_at, 3_000);
    }

    #[test]
    fn test_ledger_remove_task() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        let service = service(&repo);

        let record = make_record("task-3", TaskLedgerStatus::Pending);
        service.upsert_task_at(record, 2_000).unwrap();
        let snapshot = service.remove_task_at("task-3", 3_000).unwrap();
        assert!(snapshot.tasks.is_empty());
    }

    #[test]
    fn test_ledger_clear_resolved() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        let service = service(&repo);

        service
            .upsert_task_at(make_record("t1", TaskLedgerStatus::Pending), 2_000)
            .unwrap();
        service
            .upsert_task_at(make_record("t2", TaskLedgerStatus::Running), 2_000)
            .unwrap();
        service
            .upsert_task_at(make_record("t3", TaskLedgerStatus::Succeeded), 2_000)
            .unwrap();
        service
            .upsert_task_at(make_record("t4", TaskLedgerStatus::Cancelled), 2_000)
            .unwrap();

        let snapshot = service.clear_resolved_at(3_000).unwrap();
        assert_eq!(snapshot.tasks.len(), 2);
        assert_eq!(TaskLedgerStore::load_records(&repo).unwrap().len(), 2);
    }

    #[test]
    fn test_ledger_upsert_round_trips_typed_columns() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);
        let service = service(&repo);

        let mut record = make_record("typed-task", TaskLedgerStatus::Failed);
        record.kind = TaskLedgerKind::Automation;
        record.progress = 42.5;
        record.retryable = true;
        record.cancelable = false;
        record.recoverable = true;
        record.stage = Some("export".to_string());
        record.history_id = Some("history-1".to_string());
        record.project_id = Some("project-1".to_string());
        record.file_path = Some("C:\\audio.wav".to_string());
        record.automation_rule_id = Some("rule-1".to_string());
        record.source_fingerprint = Some("fingerprint-1".to_string());
        record.error_message = Some("failed once".to_string());
        record.template_id = Some("summary-general".to_string());
        record.target_language = Some("ja".to_string());

        let snapshot = service.upsert_task_at(record.clone(), 2_000).unwrap();
        assert_eq!(snapshot.tasks, vec![record.clone()]);

        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let stored = conn.query_row(
                    "SELECT kind, status, retryable, cancelable, recoverable, stage, history_id,
                            project_id, file_path, automation_rule_id, source_fingerprint,
                            error_message, template_id, target_language, version
                     FROM task_ledger WHERE id = 'typed-task'",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, Option<String>>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, Option<String>>(10)?,
                            row.get::<_, Option<String>>(11)?,
                            row.get::<_, Option<String>>(12)?,
                            row.get::<_, Option<String>>(13)?,
                            row.get::<_, i64>(14)?,
                        ))
                    },
                )?;

                assert_eq!(stored.0, "automation");
                assert_eq!(stored.1, "failed");
                assert_eq!(stored.2, 1);
                assert_eq!(stored.3, 0);
                assert_eq!(stored.4, 1);
                assert_eq!(stored.5.as_deref(), Some("export"));
                assert_eq!(stored.6.as_deref(), Some("history-1"));
                assert_eq!(stored.7.as_deref(), Some("project-1"));
                assert_eq!(stored.8.as_deref(), Some("C:\\audio.wav"));
                assert_eq!(stored.9.as_deref(), Some("rule-1"));
                assert_eq!(stored.10.as_deref(), Some("fingerprint-1"));
                assert_eq!(stored.11.as_deref(), Some("failed once"));
                assert_eq!(stored.12.as_deref(), Some("summary-general"));
                assert_eq!(stored.13.as_deref(), Some("ja"));
                assert_eq!(stored.14, TASK_LEDGER_VERSION as i64);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_ledger_empty_snapshot() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);

        let snapshot = service(&repo).load_snapshot_at(2_000).unwrap();
        assert!(snapshot.tasks.is_empty());
    }
}
