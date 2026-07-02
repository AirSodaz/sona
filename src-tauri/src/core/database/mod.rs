pub mod legacy_migration;
pub mod schema;

use rusqlite::{Connection, Transaction};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static GLOBAL_DB: OnceLock<Database> = OnceLock::new();

/// Central database handle for all SQLite operations.
///
/// Wraps a single `Connection` behind a `Mutex` for thread-safe access.
/// The main database (`sona.db`) stores all structured data. An analytics
/// database (`sona-analytics.db`) is ATTACHed as `analytics` for LLM usage
/// tracking.
pub struct Database {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish()
    }
}

impl Database {
    pub fn global() -> &'static Database {
        GLOBAL_DB.get().expect("Database not initialized")
    }

    pub fn set_global(db: Database) -> Result<(), String> {
        GLOBAL_DB
            .set(db)
            .map_err(|_| "Database already initialized".to_string())
    }

    /// Opens (or creates) the main database at `{app_local_data_dir}/sona.db`,
    /// enables WAL mode and foreign keys, ATTACHes the analytics database,
    /// and runs any pending schema migrations.
    pub fn open(app_local_data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(app_local_data_dir).map_err(|e| e.to_string())?;

        let db_path = app_local_data_dir.join("sona.db");
        let analytics_path = app_local_data_dir.join("sona-analytics.db");

        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")
            .map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;

        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS analytics",
            analytics_path.to_string_lossy().replace('\'', "''")
        );
        conn.execute_batch(&attach_sql).map_err(|e| e.to_string())?;

        // Request SQLite to perform internal maintenance/optimization at startup
        conn.execute_batch("PRAGMA optimize;")
            .map_err(|e| format!("startup optimize: {e}"))?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        schema::run_migrations(&db)?;

        Ok(db)
    }

    /// Opens an in-memory database for testing. WAL is not applicable to
    /// in-memory databases but foreign keys are enabled and migrations run.
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        conn.execute_batch("ATTACH DATABASE ':memory:' AS analytics;")
            .map_err(|e| e.to_string())?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        schema::run_migrations(&db)?;

        Ok(db)
    }

    /// Executes a closure with a shared reference to the connection.
    /// The Mutex is held for the duration of the closure.
    pub fn with_connection<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        f(&conn).map_err(|e| e.to_string())
    }

    /// Runs `PRAGMA optimize;` to let SQLite perform internal maintenance
    /// (analyze, query planner stats, etc.). Safe to call periodically.
    pub fn run_optimize(&self) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("lock optimize: {e}"))?;
        conn.execute_batch("PRAGMA optimize;")
            .map_err(|e| format!("optimize: {e}"))
    }

    /// Runs `VACUUM` on both the main and analytics databases to reclaim
    /// unused space and defragment. Should be called after large bulk deletes
    /// (e.g., clearing resolved tasks).
    pub fn vacuum(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock vacuum: {e}"))?;
        conn.execute_batch("VACUUM;")
            .map_err(|e| format!("vacuum main: {e}"))?;
        conn.execute_batch("VACUUM analytics;")
            .map_err(|e| format!("vacuum analytics: {e}"))?;
        Ok(())
    }

    /// Executes a closure inside a transaction. The transaction is committed
    /// if the closure returns `Ok`, rolled back on `Err`.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Transaction) -> Result<T, rusqlite::Error>,
    {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let result = f(&tx).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DbProvider {
    db: Option<std::sync::Arc<Database>>,
}

impl DbProvider {
    pub fn new(db: Option<std::sync::Arc<Database>>) -> Self {
        Self { db }
    }

    pub fn get(&self) -> &Database {
        if let Some(ref db) = self.db {
            db
        } else {
            Database::global()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_opens_in_memory() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute_batch("SELECT 1")?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_database_foreign_keys_enabled() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let fk: i32 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
            assert_eq!(fk, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_schema_version_table_exists() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))?;
            assert!(count >= 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_history_items_table_exists() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM history_items", [], |row| row.get(0))?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_transaction_commits() {
        let db = Database::open_in_memory().unwrap();
        db.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)",
                ["test_key", "test_value"],
            )?;
            Ok(())
        })
        .unwrap();

        db.with_connection(|conn| {
            let val: String = conn.query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                ["test_key"],
                |row| row.get(0),
            )?;
            assert_eq!(val, "test_value");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_transaction_rolls_back_on_error() {
        let db = Database::open_in_memory().unwrap();

        let result: Result<(), String> = db.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)",
                ["rollback_key", "rollback_value"],
            )?;
            // Force an error
            Err(rusqlite::Error::QueryReturnedNoRows)
        });
        assert!(result.is_err());

        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key = ?1",
                ["rollback_key"],
                |row| row.get(0),
            )?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_cascade_delete() {
        let db = Database::open_in_memory().unwrap();
        db.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO history_items (id, timestamp, duration, kind, status)
                 VALUES ('test-1', 1000, 5.0, 'recording', 'complete')",
                [],
            )?;
            tx.execute(
                "INSERT INTO history_transcripts (history_id, segments) VALUES ('test-1', '[]')",
                [],
            )?;
            tx.execute(
                "INSERT INTO history_summaries (history_id, payload) VALUES ('test-1', '{}')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        // Delete the parent row
        db.with_connection(|conn| {
            conn.execute("DELETE FROM history_items WHERE id = 'test-1'", [])?;
            Ok(())
        })
        .unwrap();

        // Verify children are cascaded
        db.with_connection(|conn| {
            let t_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_transcripts WHERE history_id = 'test-1'",
                [],
                |row| row.get(0),
            )?;
            let s_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_summaries WHERE history_id = 'test-1'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(t_count, 0);
            assert_eq!(s_count, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_open_on_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(tmp.path()).unwrap();
        db.with_connection(|conn| {
            conn.execute_batch("SELECT 1")?;
            Ok(())
        })
        .unwrap();

        // Verify WAL mode
        db.with_connection(|conn| {
            let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
            assert_eq!(mode, "wal");
            Ok(())
        })
        .unwrap();

        // Verify db file exists
        assert!(tmp.path().join("sona.db").exists());
    }

    #[test]
    fn test_analytics_database_is_attached() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute_batch("CREATE TABLE analytics.test_table (id INTEGER PRIMARY KEY);")?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_run_optimize() {
        let db = Database::open_in_memory().unwrap();
        db.run_optimize().unwrap();
    }

    #[test]
    fn test_vacuum() {
        let db = Database::open_in_memory().unwrap();
        // Insert some data then delete it to create free pages
        db.with_connection(|conn| {
            conn.execute_batch(
                "INSERT INTO app_settings (key, value) VALUES ('k1', 'v1'), ('k2', 'v2'), ('k3', 'v3')",
            )?;
            conn.execute_batch("DELETE FROM app_settings")?;
            Ok(())
        })
        .unwrap();
        // VACUUM should succeed and reclaim the free space
        db.vacuum().unwrap();
    }
}

#[cfg(test)]
mod tests_provider {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_db_provider_fallback() {
        let provider = DbProvider::default();
        // This should panic since Database::global() is not initialized in this unit test context
        let result = std::panic::catch_unwind(|| {
            provider.get();
        });
        assert!(result.is_err());

        let local_db = Database::open_in_memory().unwrap();
        let provider_with_db = DbProvider::new(Some(Arc::new(local_db)));
        assert!(
            std::panic::catch_unwind(|| {
                provider_with_db.get();
            })
            .is_ok()
        );
    }
}
