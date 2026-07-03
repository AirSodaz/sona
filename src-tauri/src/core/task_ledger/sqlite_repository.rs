use crate::core::database::DatabaseError;
use serde_json::Value;
use std::path::PathBuf;

use crate::core::task_ledger::types::{
    TASK_LEDGER_VERSION, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};

const INTERRUPTED_MESSAGE: &str = "Task was interrupted before it finished.";

#[derive(Clone)]
pub struct SqliteLedgerRepository {
    #[allow(dead_code)]
    app_local_data_dir: PathBuf,
    db: crate::core::database::DbProvider,
}

crate::impl_db_repository!(SqliteLedgerRepository);

impl SqliteLedgerRepository {
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn load_snapshot(&self) -> Result<TaskLedgerSnapshot, DatabaseError> {
        let records = self.get_db()?.with_connection(|conn| {
            let mut stmt =
                conn.prepare_cached("SELECT data, version FROM task_ledger ORDER BY id")?;
            let mut rows = stmt.query([])?;
            let mut records = Vec::new();
            while let Some(row) = rows.next()? {
                let data_str: String = row.get(0)?;
                let _version: i64 = row.get(1)?;
                let mut record: TaskLedgerRecord = serde_json::from_str(&data_str)?;
                // Auto-assign timestamps if missing
                if record.created_at == 0 {
                    record.created_at = Self::now_ms();
                }
                if record.updated_at == 0 {
                    record.updated_at = record.created_at;
                }
                // Interrupt running tasks
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
                records.push(record);
            }
            // Filter to only persisted statuses
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
        let data_str = serde_json::to_string(&normalized)?;
        let now = Self::now_ms();

        self.get_db()?.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO task_ledger (id, data, version, updated_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![normalized.id, data_str, TASK_LEDGER_VERSION, now.to_string()],
            )?;
            Ok(())
        })
    }

    pub fn patch_task(&self, id: &str, patch: Value) -> Result<(), DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            let existing: Option<String> = {
                let mut stmt = tx.prepare_cached("SELECT data FROM task_ledger WHERE id = ?1")?;
                let mut rows = stmt.query([id])?;
                if let Some(row) = rows.next()? {
                    Some(row.get(0)?)
                } else {
                    None
                }
            };

            let Some(data_str) = existing else {
                return Ok(());
            };

            let mut record: TaskLedgerRecord = serde_json::from_str(&data_str)?;

            if let Some(patch_obj) = patch.as_object() {
                let mut current = serde_json::to_value(&record)?;
                if let Some(current_obj) = current.as_object_mut() {
                    for (key, val) in patch_obj {
                        current_obj.insert(key.clone(), val.clone());
                    }
                }
                record = serde_json::from_value(current)?;
            }

            record.updated_at = Self::now_ms();
            let new_data_str = serde_json::to_string(&record)?;
            let now = record.updated_at.to_string();

            tx.execute(
                "UPDATE task_ledger SET data = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![new_data_str, now, id],
            )?;
            Ok(())
        })
    }

    pub fn remove_task(&self, id: &str) -> Result<(), DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            conn.execute("DELETE FROM task_ledger WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    pub fn clear_resolved(&self) -> Result<(), DatabaseError> {
        let deleted_count = self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached("SELECT id, data FROM task_ledger")?;
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let data_str: String = row.get(1)?;
                Ok((id, data_str))
            })?;
            let mut to_delete = Vec::new();
            for row in rows {
                let (id, data_str) = row?;
                if let Ok(record) = serde_json::from_str::<TaskLedgerRecord>(&data_str)
                    && !is_persisted_status(&record.status)
                {
                    to_delete.push(id);
                }
            }
            let mut stmt = conn.prepare_cached("DELETE FROM task_ledger WHERE id = ?1")?;
            for id in &to_delete {
                stmt.execute([id])?;
            }
            Ok(to_delete.len())
        })?;

        if deleted_count > 0 {
            self.get_db()?.vacuum()?;
        }

        Ok(())
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
    let now = SqliteLedgerRepository::now_ms();
    if record.created_at == 0 {
        record.created_at = now;
    }
    if record.updated_at == 0 {
        record.updated_at = record.created_at;
    }
    record
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;
    use crate::core::task_ledger::types::TaskLedgerKind;
    use serde_json::json;
    use std::path::PathBuf;

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
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);

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
    fn test_ledger_empty_snapshot() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteLedgerRepository::with_db(PathBuf::new(), db);

        let snapshot = repo.load_snapshot().unwrap();
        assert!(snapshot.tasks.is_empty());
    }
}
