pub mod error;
pub mod legacy_migration;
pub mod schema;

pub use error::DatabaseError;

use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::path::Path;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, TryLockError};
use std::time::{Duration, Instant};

/// Number of connections in the pool. With WAL mode, multiple readers can
/// proceed concurrently through separate connections, and writers are
/// serialized by SQLite's internal locking (handled via `busy_timeout`).
const POOL_SIZE: usize = 4;
const POOL_ACQUIRE_TIMEOUT_MS: u64 = 5_000;
const POOL_ACQUIRE_RETRY_DELAY_MS: u64 = 5;

static GLOBAL_DB: OnceLock<Database> = OnceLock::new();

/// Central database handle backed by a pool of SQLite connections.
///
/// WAL mode combined with a multi-connection pool allows concurrent reads
/// without blocking each other — each read acquires an idle connection and
/// proceeds in parallel.
pub struct Database {
    pool: Vec<Mutex<Connection>>,
    next: AtomicUsize,
    slow_query_threshold_us: AtomicI64,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("pool_size", &self.pool.len())
            .finish()
    }
}

impl Database {
    pub fn global() -> Result<&'static Database, DatabaseError> {
        GLOBAL_DB
            .get()
            .ok_or_else(|| DatabaseError::Internal("Database not initialized".to_string()))
    }

    pub fn set_global(db: Database) -> Result<(), DatabaseError> {
        GLOBAL_DB
            .set(db)
            .map_err(|_| DatabaseError::Internal("Database already initialized".to_string()))
    }

    /// Opens (or creates) the main database at `{app_local_data_dir}/sona.db`,
    /// enables WAL mode and foreign keys, ATTACHes the analytics database,
    /// creates a pool of connections, and runs pending schema migrations.
    pub fn open(app_local_data_dir: &Path) -> Result<Self, DatabaseError> {
        std::fs::create_dir_all(app_local_data_dir)
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;

        let db_path = app_local_data_dir.join("sona.db");
        let analytics_path = app_local_data_dir.join("sona-analytics.db");

        let mut pool = Vec::with_capacity(POOL_SIZE);
        for _ in 0..POOL_SIZE {
            let conn = Self::open_connection(&db_path)?;
            Self::attach_analytics(&conn, &analytics_path)?;
            pool.push(Mutex::new(conn));
        }

        let db = Self {
            pool,
            next: AtomicUsize::new(0),
            slow_query_threshold_us: AtomicI64::new(0),
        };

        schema::run_migrations(&db)?;
        db.run_optimize()?;

        Ok(db)
    }

    /// Opens an in-memory database for testing.
    /// Uses a single connection since in-memory databases are per-connection
    /// and cannot be shared across multiple handles.
    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        conn.execute_batch("ATTACH DATABASE ':memory:' AS analytics;")
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;

        let db = Self {
            pool: vec![Mutex::new(conn)],
            next: AtomicUsize::new(0),
            slow_query_threshold_us: AtomicI64::new(0),
        };

        schema::run_migrations(&db)?;

        Ok(db)
    }

    fn open_connection(db_path: &Path) -> Result<Connection, DatabaseError> {
        let conn =
            Connection::open(db_path).map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        Ok(conn)
    }

    fn attach_analytics(conn: &Connection, analytics_path: &Path) -> Result<(), DatabaseError> {
        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS analytics",
            analytics_path.to_string_lossy().replace('\'', "''")
        );
        conn.execute_batch(&attach_sql)
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))
    }

    fn try_acquire_slot(
        &self,
        idx: usize,
    ) -> Result<Option<std::sync::MutexGuard<'_, Connection>>, DatabaseError> {
        match self.pool[idx].try_lock() {
            Ok(guard) => Ok(Some(guard)),
            Err(TryLockError::Poisoned(poisoned)) => Ok(Some(poisoned.into_inner())),
            Err(TryLockError::WouldBlock) => Ok(None),
        }
    }

    /// Acquires a connection from the pool via round-robin with bounded retry.
    /// If all slots are momentarily busy, waits briefly before returning a
    /// PoolBusyError so normal bursts do not fail immediately.
    fn acquire(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DatabaseError> {
        if self.pool.is_empty() {
            return Err(DatabaseError::PoolBusyError);
        }

        let timeout = Duration::from_millis(POOL_ACQUIRE_TIMEOUT_MS);
        let retry_delay = Duration::from_millis(POOL_ACQUIRE_RETRY_DELAY_MS);
        let start = Instant::now();

        loop {
            let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.pool.len();
            if let Some(guard) = self.try_acquire_slot(idx)? {
                return Ok(guard);
            }

            for i in 0..self.pool.len() {
                if i != idx
                    && let Some(guard) = self.try_acquire_slot(i)?
                {
                    return Ok(guard);
                }
            }

            if start.elapsed() >= timeout {
                return Err(DatabaseError::PoolBusyError);
            }
            std::thread::sleep(retry_delay);
        }
    }

    /// Executes a closure with a shared reference to a pooled connection.
    /// The mutex for that specific connection is held for the duration of the
    /// closure, but other connections in the pool remain available.
    pub fn with_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        let conn = self.acquire()?;
        let start = Instant::now();
        let result = f(&conn);
        if let Some(elapsed) = self.check_slow(start.elapsed()) {
            log::warn!("slow with_connection ({elapsed:?})");
        }
        result
    }

    /// Sets a threshold for slow query logging.
    /// When a `with_connection` or `with_transaction` call exceeds this
    /// duration, a warning is logged via `log::warn!`.
    /// Pass `Duration::ZERO` to disable (default).
    pub fn set_slow_query_threshold(&self, threshold: Duration) {
        let us = if threshold.is_zero() {
            0
        } else {
            threshold.as_micros() as i64
        };
        self.slow_query_threshold_us.store(us, Ordering::Relaxed);
    }

    fn check_slow(&self, elapsed: Duration) -> Option<Duration> {
        let threshold_us = self.slow_query_threshold_us.load(Ordering::Relaxed);
        if threshold_us > 0 {
            let threshold = Duration::from_micros(threshold_us as u64);
            if elapsed > threshold {
                return Some(elapsed);
            }
        }
        None
    }

    /// Runs `PRAGMA optimize;` to let SQLite perform internal maintenance
    /// (analyze, query planner stats, etc.). Safe to call periodically.
    pub fn run_optimize(&self) -> Result<(), DatabaseError> {
        let conn = self
            .acquire()
            .map_err(|e| DatabaseError::Internal(format!("acquire for optimize: {e}")))?;
        conn.execute_batch("PRAGMA optimize;")
            .map_err(DatabaseError::QueryError)
    }

    /// Runs `VACUUM` on both the main and analytics databases to reclaim
    /// unused space and defragment. Should be called after large bulk deletes
    /// (e.g., clearing resolved tasks).
    pub fn vacuum(&self) -> Result<(), DatabaseError> {
        let conn = self
            .acquire()
            .map_err(|e| DatabaseError::Internal(format!("acquire for vacuum: {e}")))?;
        conn.execute_batch("VACUUM;")
            .map_err(DatabaseError::QueryError)?;
        conn.execute_batch("VACUUM analytics;")
            .map_err(DatabaseError::QueryError)?;
        Ok(())
    }

    fn with_transaction_behavior<F, T>(
        &self,
        behavior: TransactionBehavior,
        label: &str,
        f: F,
    ) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction) -> Result<T, DatabaseError>,
    {
        let mut conn = self.acquire()?;
        let start = Instant::now();
        let tx = conn
            .transaction_with_behavior(behavior)
            .map_err(DatabaseError::QueryError)?;
        let result = f(&tx);
        let elapsed = start.elapsed();
        let result = match result {
            Ok(value) => {
                tx.commit().map_err(DatabaseError::QueryError)?;
                Ok(value)
            }
            Err(e) => Err(e),
        };
        if let Some(_elapsed) = self.check_slow(elapsed) {
            log::warn!("slow {label} ({elapsed:?})");
        }
        result
    }

    /// Executes a closure inside a deferred transaction. The transaction is
    /// committed if the closure returns `Ok`, rolled back on `Err`.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction) -> Result<T, DatabaseError>,
    {
        self.with_transaction_behavior(TransactionBehavior::Deferred, "with_transaction", f)
    }

    /// Executes a read-then-write closure inside `BEGIN IMMEDIATE`.
    ///
    /// This avoids stale WAL snapshot failures for read-modify-write flows by
    /// acquiring the write reservation before the first read.
    pub fn with_rw_transaction<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction) -> Result<T, DatabaseError>,
    {
        self.with_transaction_behavior(TransactionBehavior::Immediate, "with_rw_transaction", f)
    }
}

/// Times a single query or query batch and logs a warning if it exceeds
/// `threshold`. The `label` identifies the operation in the log message.
///
/// ```ignore
/// time_query("list_items", "SELECT ...", Duration::from_millis(200), || {
///     let mut stmt = conn.prepare_cached("SELECT ...")?;
///     stmt.query_map([], |row| { ... })
/// })?;
/// ```
pub fn time_query<T>(
    label: &str,
    sql: &str,
    threshold: Duration,
    f: impl FnOnce() -> Result<T, DatabaseError>,
) -> Result<T, DatabaseError> {
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    if !threshold.is_zero() && elapsed > threshold {
        log::warn!("slow query [{label}] ({elapsed:?}): {sql}");
    }
    result
}

#[derive(Clone, Debug, Default)]
pub struct DbProvider {
    db: Option<Arc<Database>>,
}

impl DbProvider {
    pub fn new(db: Option<Arc<Database>>) -> Self {
        Self { db }
    }

    pub fn get(&self) -> Result<&Database, DatabaseError> {
        if let Some(ref db) = self.db {
            Ok(db)
        } else {
            Database::global()
        }
    }
}

/// Generates the standard `new()`, `with_db()` (test-only), and `get_db()` methods
/// for a SQLite repository struct with a `db: DbProvider` field.
#[macro_export]
macro_rules! impl_db_repository {
    ($name:ident) => {
        impl $name {
            pub fn new(_app_local_data_dir: std::path::PathBuf) -> Self {
                Self {
                    db: $crate::core::database::DbProvider::default(),
                }
            }

            #[cfg(test)]
            pub(crate) fn with_db(
                _app_local_data_dir: std::path::PathBuf,
                db: $crate::core::database::Database,
            ) -> Self {
                Self {
                    db: $crate::core::database::DbProvider::new(Some(std::sync::Arc::new(db))),
                }
            }

            fn get_db(
                &self,
            ) -> Result<&$crate::core::database::Database, $crate::core::database::DatabaseError>
            {
                self.db.get()
            }
        }
    };
    ($name:ident, app_local_data_dir) => {
        impl $name {
            pub fn new(app_local_data_dir: std::path::PathBuf) -> Self {
                Self {
                    app_local_data_dir,
                    db: $crate::core::database::DbProvider::default(),
                }
            }

            #[cfg(test)]
            pub(crate) fn with_db(
                app_local_data_dir: std::path::PathBuf,
                db: $crate::core::database::Database,
            ) -> Self {
                Self {
                    app_local_data_dir,
                    db: $crate::core::database::DbProvider::new(Some(std::sync::Arc::new(db))),
                }
            }

            fn get_db(
                &self,
            ) -> Result<&$crate::core::database::Database, $crate::core::database::DatabaseError>
            {
                self.db.get()
            }
        }
    };
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

        let result: Result<(), DatabaseError> = db.with_transaction(|tx| {
            tx.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)",
                ["rollback_key", "rollback_value"],
            )?;
            // Force an error
            Err(DatabaseError::NotFoundError("forced rollback".into()))
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

    #[test]
    fn test_concurrent_reads_on_pool() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Arc::new(Database::open(tmp.path()).unwrap());

        // Populate some test data
        db.with_connection(|conn| {
            conn.execute_batch(
                "INSERT INTO app_settings (key, value) VALUES ('c1', 'v1'), ('c2', 'v2'), ('c3', 'v3')",
            )?;
            Ok(())
        })
        .unwrap();

        // Spawn 4 threads that each perform a read concurrently.
        // With POOL_SIZE=4 they should use separate connections.
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let db = Arc::clone(&db);
                std::thread::spawn(move || {
                    db.with_connection(|conn| {
                        let count: i64 =
                            conn.query_row("SELECT COUNT(*) FROM app_settings", [], |row| {
                                row.get(0)
                            })?;
                        assert_eq!(count, 3);
                        Ok(())
                    })
                })
            })
            .collect();

        for t in threads {
            t.join()
                .expect("reader thread panicked")
                .expect("reader query failed");
        }
    }

    #[test]
    fn test_pool_acquire_waits_for_an_available_connection() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Arc::new(Database::open(tmp.path()).unwrap());
        let ready = Arc::new(std::sync::Barrier::new(POOL_SIZE + 1));
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let holders: Vec<_> = (0..POOL_SIZE)
            .map(|_| {
                let db = Arc::clone(&db);
                let ready = Arc::clone(&ready);
                let release = Arc::clone(&release);
                std::thread::spawn(move || {
                    db.with_connection(|_| {
                        ready.wait();
                        while !release.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Ok(())
                    })
                })
            })
            .collect();

        ready.wait();
        let release_after_delay = {
            let release = Arc::clone(&release);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                release.store(true, Ordering::SeqCst);
            })
        };

        db.with_connection(|conn| {
            conn.execute_batch("SELECT 1")?;
            Ok(())
        })
        .expect("connection acquisition should wait for a pooled connection to free up");

        release_after_delay.join().unwrap();
        for holder in holders {
            holder
                .join()
                .expect("holder thread panicked")
                .expect("holder query failed");
        }
    }

    #[test]
    fn test_rw_transaction_prevents_busy_snapshot_on_read_modify_write() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Arc::new(Database::open(tmp.path()).unwrap());
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES ('race', 'initial')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        let read_started = Arc::new(std::sync::Barrier::new(2));
        let first_writer = {
            let db = Arc::clone(&db);
            let read_started = Arc::clone(&read_started);
            std::thread::spawn(move || {
                db.with_rw_transaction(|tx| {
                    let _: String = tx.query_row(
                        "SELECT value FROM app_settings WHERE key = 'race'",
                        [],
                        |row| row.get(0),
                    )?;
                    read_started.wait();
                    std::thread::sleep(Duration::from_millis(100));
                    tx.execute(
                        "UPDATE app_settings SET value = 'first' WHERE key = 'race'",
                        [],
                    )?;
                    Ok(())
                })
            })
        };

        read_started.wait();
        let second_writer = {
            let db = Arc::clone(&db);
            std::thread::spawn(move || {
                db.with_connection(|conn| {
                    conn.execute(
                        "UPDATE app_settings SET value = 'second' WHERE key = 'race'",
                        [],
                    )?;
                    Ok(())
                })
            })
        };

        first_writer
            .join()
            .expect("first writer panicked")
            .expect("read-modify-write transaction should commit");
        second_writer
            .join()
            .expect("second writer panicked")
            .expect("concurrent writer should wait and then commit");
    }
}

#[cfg(test)]
mod tests_provider {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_db_provider_fallback() {
        let provider = DbProvider::default();
        assert!(provider.get().is_err());

        let local_db = Database::open_in_memory().unwrap();
        let provider_with_db = DbProvider::new(Some(Arc::new(local_db)));
        assert!(provider_with_db.get().is_ok());
    }
}
