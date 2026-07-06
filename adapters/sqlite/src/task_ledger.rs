use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use serde::Serialize;
use serde_json::Value;
use sona_core::task_ledger::types::{
    TASK_LEDGER_VERSION, TaskLedgerKind, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};
use std::sync::Arc;

const INTERRUPTED_MESSAGE: &str = "Task was interrupted before it finished.";
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

impl<D> SqliteLedgerRepository<D>
where
    D: DatabasePort,
{
    pub fn load_snapshot(&self) -> Result<TaskLedgerSnapshot, DatabaseError> {
        let records = self.get_db()?.with_connection(|conn| {
            let sql = format!("SELECT {TASK_LEDGER_COLUMNS} FROM task_ledger ORDER BY id");
            let mut stmt = conn.prepare_cached(&sql)?;
            let mut rows = stmt.query([])?;
            let mut records = Vec::new();
            while let Some(row) = rows.next()? {
                records.push(normalize_loaded_record(map_row_to_record(row)?));
            }
            records.retain(|r| is_persisted_status(&r.status));
            Ok(records)
        })?;

        let updated_at = records.iter().map(|r| r.updated_at).max();
        Ok(TaskLedgerSnapshot {
            version: TASK_LEDGER_VERSION,
            updated_at: if records.is_empty() { None } else { updated_at },
            tasks: records,
        })
    }

    pub fn upsert_task(&self, record: TaskLedgerRecord) -> Result<(), DatabaseError> {
        let normalized = normalize_record(record);

        self.get_db()?.with_write_connection(|conn| {
            let mut stmt = conn.prepare_cached(UPSERT_TASK_SQL)?;
            execute_upsert_task(&mut stmt, &normalized)?;
            Ok(())
        })
    }

    pub fn patch_task(&self, id: &str, patch: Value) -> Result<(), DatabaseError> {
        self.get_db()?.with_rw_transaction(|tx| {
            let existing = {
                let sql = format!("SELECT {TASK_LEDGER_COLUMNS} FROM task_ledger WHERE id = ?1");
                let mut stmt = tx.prepare_cached(&sql)?;
                let mut rows = stmt.query([id])?;
                if let Some(row) = rows.next()? {
                    Some(map_row_to_record(row)?)
                } else {
                    None
                }
            };

            let Some(mut record) = existing else {
                return Ok(());
            };

            if let Some(patch_obj) = patch.as_object() {
                let mut current = serde_json::to_value(&record)?;
                if let Some(current_obj) = current.as_object_mut() {
                    for (key, val) in patch_obj {
                        current_obj.insert(key.clone(), val.clone());
                    }
                }
                record = serde_json::from_value(current)?;
            }

            record.id = id.to_string();
            record.updated_at = now_ms();
            let normalized = normalize_record(record);
            let mut stmt = tx.prepare_cached(UPSERT_TASK_SQL)?;
            execute_upsert_task(&mut stmt, &normalized)?;
            Ok(())
        })
    }

    pub fn remove_task(&self, id: &str) -> Result<(), DatabaseError> {
        self.get_db()?.with_write_connection(|conn| {
            conn.execute("DELETE FROM task_ledger WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    pub fn clear_resolved(&self) -> Result<(), DatabaseError> {
        self.get_db()?.with_rw_transaction(|tx| {
            let mut stmt = tx.prepare_cached("SELECT id, status FROM task_ledger")?;
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let status: String = row.get(1)?;
                Ok((id, status))
            })?;
            let mut to_delete = Vec::new();
            for row in rows {
                let (id, status) = row?;
                if let Ok(status) = task_status_from_storage(status)
                    && !is_persisted_status(&status)
                {
                    to_delete.push(id);
                }
            }
            let mut stmt = tx.prepare_cached("DELETE FROM task_ledger WHERE id = ?1")?;
            for id in &to_delete {
                stmt.execute([id])?;
            }
            Ok(())
        })?;

        Ok(())
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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

fn normalize_record(mut record: TaskLedgerRecord) -> TaskLedgerRecord {
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
    let now = now_ms();
    if record.created_at == 0 {
        record.created_at = now;
    }
    if record.updated_at == 0 {
        record.updated_at = record.created_at;
    }
    record
}

fn normalize_loaded_record(mut record: TaskLedgerRecord) -> TaskLedgerRecord {
    if record.created_at == 0 {
        record.created_at = now_ms();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;

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
    fn test_ledger_upsert_and_load() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let repo = SqliteLedgerRepository::new(Arc::clone(&db));

        let record = make_record("task-1", TaskLedgerStatus::Running);
        repo.upsert_task(record).unwrap();

        let snapshot = repo.load_snapshot().unwrap();
        assert_eq!(snapshot.tasks.len(), 1);
        // Running tasks should be interrupted on load
        assert_eq!(snapshot.tasks[0].status, TaskLedgerStatus::Interrupted);
        assert_eq!(snapshot.tasks[0].id, "task-1");
    }

    #[test]
    fn test_ledger_patch_task() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);

        let record = make_record("task-2", TaskLedgerStatus::Pending);
        repo.upsert_task(record).unwrap();

        repo.patch_task("task-2", json!({"progress": 75.0}))
            .unwrap();

        let snapshot = repo.load_snapshot().unwrap();
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].progress, 75.0);
    }

    #[test]
    fn test_ledger_remove_task() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);

        let record = make_record("task-3", TaskLedgerStatus::Pending);
        repo.upsert_task(record).unwrap();
        repo.remove_task("task-3").unwrap();

        let snapshot = repo.load_snapshot().unwrap();
        assert!(snapshot.tasks.is_empty());
    }

    #[test]
    fn test_ledger_clear_resolved() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);

        repo.upsert_task(make_record("t1", TaskLedgerStatus::Pending))
            .unwrap();
        repo.upsert_task(make_record("t2", TaskLedgerStatus::Running))
            .unwrap();
        repo.upsert_task(make_record("t3", TaskLedgerStatus::Succeeded))
            .unwrap();
        repo.upsert_task(make_record("t4", TaskLedgerStatus::Cancelled))
            .unwrap();

        repo.clear_resolved().unwrap();

        let snapshot = repo.load_snapshot().unwrap();
        assert_eq!(snapshot.tasks.len(), 2);
        for task in &snapshot.tasks {
            assert!(is_persisted_status(&task.status));
        }
    }

    #[test]
    fn test_ledger_upsert_round_trips_typed_columns() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);

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

        repo.upsert_task(record.clone()).unwrap();

        let snapshot = repo.load_snapshot().unwrap();
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

        let snapshot = repo.load_snapshot().unwrap();
        assert!(snapshot.tasks.is_empty());
    }
}
