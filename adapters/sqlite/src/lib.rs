pub mod analytics;
pub mod automation;
pub mod config_store;
pub mod error;
pub mod history_archive;
pub mod history_backup;
pub mod history_store;
pub mod legacy_migration;
pub mod llm_usage;
pub mod ports;
pub mod project;
pub mod schema;
pub mod storage_usage;
pub mod task_ledger;

pub use automation::{AutomationRepositoryState, SqliteAutomationRepository};
pub use config_store::SqliteConfigStore;
pub use error::DatabaseError;
pub use history_archive::HistoryRepository;
pub use history_store::SqliteHistoryStore;
pub use project::SqliteProjectRepository;
pub use task_ledger::SqliteLedgerRepository;

use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// Number of read connections in the pool. With WAL mode, multiple readers can
/// proceed concurrently through separate connections, and writers are
/// serialized by SQLite's internal locking (handled via `busy_timeout`).
const POOL_SIZE: usize = 4;
const WRITE_POOL_SIZE: usize = 1;
const POOL_ACQUIRE_TIMEOUT_MS: u64 = 5_000;

static GLOBAL_DB: OnceLock<Arc<Database>> = OnceLock::new();

#[derive(Clone)]
struct ConnectionPool {
    inner: Arc<ConnectionPoolInner>,
}

struct ConnectionPoolInner {
    available: Mutex<Vec<Connection>>,
    available_changed: Condvar,
    capacity: usize,
}

struct PooledConnection {
    conn: Option<Connection>,
    pool: Arc<ConnectionPoolInner>,
}

impl ConnectionPool {
    fn new(connections: Vec<Connection>) -> Self {
        Self {
            inner: Arc::new(ConnectionPoolInner {
                capacity: connections.len(),
                available: Mutex::new(connections),
                available_changed: Condvar::new(),
            }),
        }
    }

    fn capacity(&self) -> usize {
        self.inner.capacity
    }

    fn acquire(&self) -> Result<PooledConnection, DatabaseError> {
        if self.inner.capacity == 0 {
            return Err(DatabaseError::PoolBusyError);
        }

        let timeout = Duration::from_millis(POOL_ACQUIRE_TIMEOUT_MS);
        let start = Instant::now();
        let mut available = self.lock_available();

        loop {
            if let Some(conn) = available.pop() {
                return Ok(PooledConnection {
                    conn: Some(conn),
                    pool: Arc::clone(&self.inner),
                });
            }

            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(DatabaseError::PoolBusyError);
            }
            let remaining = timeout.saturating_sub(elapsed);
            match self
                .inner
                .available_changed
                .wait_timeout(available, remaining)
            {
                Ok((guard, wait_result)) => {
                    available = guard;
                    if wait_result.timed_out() && available.is_empty() {
                        return Err(DatabaseError::PoolBusyError);
                    }
                }
                Err(poisoned) => {
                    let (guard, wait_result) = poisoned.into_inner();
                    available = guard;
                    if wait_result.timed_out() && available.is_empty() {
                        return Err(DatabaseError::PoolBusyError);
                    }
                }
            }
        }
    }

    fn lock_available(&self) -> MutexGuard<'_, Vec<Connection>> {
        self.inner
            .available
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            {
                let mut available = self
                    .pool
                    .available
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                available.push(conn);
            }
            self.pool.available_changed.notify_one();
        }
    }
}

impl Deref for PooledConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect("pooled connection missing")
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().expect("pooled connection missing")
    }
}

/// Central database handle backed by a pool of SQLite connections.
///
/// WAL mode combined with a multi-connection pool allows concurrent reads
/// without blocking each other — each read acquires an idle connection and
/// proceeds in parallel.
pub struct Database {
    read_pool: ConnectionPool,
    write_pool: ConnectionPool,
    app_local_data_dir: Option<PathBuf>,
    slow_query_threshold_us: AtomicI64,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("read_pool_size", &self.read_pool.capacity())
            .field("write_pool_size", &self.write_pool.capacity())
            .field("app_local_data_dir", &self.app_local_data_dir)
            .finish()
    }
}

impl From<DatabaseError> for sona_core::history_store::HistoryStoreError {
    fn from(error: DatabaseError) -> Self {
        sona_core::history_store::HistoryStoreError::Database(error.to_string())
    }
}

impl Database {
    pub fn global() -> Result<&'static Database, DatabaseError> {
        GLOBAL_DB
            .get()
            .map(Arc::as_ref)
            .ok_or_else(|| DatabaseError::Internal("Database not initialized".to_string()))
    }

    pub fn set_global(db: Arc<Database>) -> Result<(), DatabaseError> {
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

        let mut read_connections = Vec::with_capacity(POOL_SIZE);
        for _ in 0..POOL_SIZE {
            let conn = Self::open_connection(&db_path)?;
            Self::attach_analytics(&conn, &analytics_path)?;
            read_connections.push(conn);
        }

        let mut write_connections = Vec::with_capacity(WRITE_POOL_SIZE);
        for _ in 0..WRITE_POOL_SIZE {
            let conn = Self::open_connection(&db_path)?;
            Self::attach_analytics(&conn, &analytics_path)?;
            write_connections.push(conn);
        }

        let db = Self {
            read_pool: ConnectionPool::new(read_connections),
            write_pool: ConnectionPool::new(write_connections),
            app_local_data_dir: Some(app_local_data_dir.to_path_buf()),
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

        let shared_pool = ConnectionPool::new(vec![conn]);
        let db = Self {
            read_pool: shared_pool.clone(),
            write_pool: shared_pool,
            app_local_data_dir: None,
            slow_query_threshold_us: AtomicI64::new(0),
        };

        schema::run_migrations(&db)?;

        Ok(db)
    }

    pub fn is_for_app_local_data_dir(&self, app_local_data_dir: &Path) -> bool {
        self.app_local_data_dir.as_deref() == Some(app_local_data_dir)
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

    /// Executes a closure against a pooled read connection.
    /// Prefer `with_write_connection` or a transaction helper for writes.
    pub fn with_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        self.with_read_connection(f)
    }

    pub fn with_read_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        let conn = self.read_pool.acquire()?;
        let start = Instant::now();
        let result = f(&conn);
        if let Some(elapsed) = self.check_slow(start.elapsed()) {
            log::warn!("slow with_read_connection ({elapsed:?})");
        }
        result
    }

    pub fn with_write_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        let conn = self.write_pool.acquire()?;
        let start = Instant::now();
        let result = f(&conn);
        if let Some(elapsed) = self.check_slow(start.elapsed()) {
            log::warn!("slow with_write_connection ({elapsed:?})");
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
            .write_pool
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
            .write_pool
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
        let mut conn = self.write_pool.acquire()?;
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

/// Generates the standard `new()`, `with_db()` (test-only), and `get_db()` methods
/// for a SQLite repository struct with an injected `Arc<D>` database field.
#[macro_export]
macro_rules! impl_db_repository {
    ($name:ident) => {
        impl<D> $name<D>
        where
            D: $crate::ports::Database,
        {
            pub fn new(db: std::sync::Arc<D>) -> Self {
                Self { db }
            }

            fn get_db(&self) -> Result<&D, $crate::DatabaseError> {
                Ok(self.db.as_ref())
            }
        }

        impl $name<$crate::Database> {
            #[cfg(test)]
            pub(crate) fn with_db(
                _app_local_data_dir: std::path::PathBuf,
                db: $crate::Database,
            ) -> Self {
                Self::new(std::sync::Arc::new(db))
            }
        }
    };
    ($name:ident, app_local_data_dir) => {
        impl<D> $name<D>
        where
            D: $crate::ports::Database,
        {
            pub fn new(app_local_data_dir: std::path::PathBuf, db: std::sync::Arc<D>) -> Self {
                Self {
                    app_local_data_dir,
                    db,
                }
            }

            fn get_db(&self) -> Result<&D, $crate::DatabaseError> {
                Ok(self.db.as_ref())
            }
        }

        impl $name<$crate::Database> {
            #[cfg(test)]
            pub(crate) fn with_db(
                app_local_data_dir: std::path::PathBuf,
                db: $crate::Database,
            ) -> Self {
                Self::new(app_local_data_dir, std::sync::Arc::new(db))
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
        db.with_write_connection(|conn| {
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
        db.with_write_connection(|conn| {
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
        db.with_write_connection(|conn| {
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
        db.with_write_connection(|conn| {
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
    fn write_connection_is_available_when_read_pool_is_exhausted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Arc::new(Database::open(tmp.path()).unwrap());
        let ready = Arc::new(std::sync::Barrier::new(POOL_SIZE + 1));
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let readers: Vec<_> = (0..POOL_SIZE)
            .map(|_| {
                let db = Arc::clone(&db);
                let ready = Arc::clone(&ready);
                let release = Arc::clone(&release);
                std::thread::spawn(move || {
                    db.with_read_connection(|_| {
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
        db.with_write_connection(|conn| {
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES ('write-while-readers-held', 'ok')",
                [],
            )?;
            Ok(())
        })
        .expect("write pool should not wait for read slots to free up");
        release.store(true, Ordering::SeqCst);

        for reader in readers {
            reader
                .join()
                .expect("reader thread panicked")
                .expect("reader query failed");
        }
    }

    #[test]
    fn read_pool_is_available_while_write_connection_is_checked_out() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Arc::new(Database::open(tmp.path()).unwrap());
        db.with_write_connection(|conn| {
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES ('read-while-writer-held', 'ok')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        let write_ready = Arc::new(std::sync::Barrier::new(2));
        let release_write = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = {
            let db = Arc::clone(&db);
            let write_ready = Arc::clone(&write_ready);
            let release_write = Arc::clone(&release_write);
            std::thread::spawn(move || {
                db.with_write_connection(|_| {
                    write_ready.wait();
                    while !release_write.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Ok(())
                })
            })
        };

        write_ready.wait();
        db.with_read_connection(|conn| {
            let value: String = conn.query_row(
                "SELECT value FROM app_settings WHERE key = 'read-while-writer-held'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(value, "ok");
            Ok(())
        })
        .expect("read pool should remain available while write slot is checked out");
        release_write.store(true, Ordering::SeqCst);
        writer
            .join()
            .expect("writer thread panicked")
            .expect("writer task failed");
    }

    #[test]
    fn open_in_memory_shares_one_connection_for_reads_and_writes() {
        let db = Database::open_in_memory().unwrap();

        db.with_write_connection(|conn| {
            conn.execute_batch(
                "CREATE TABLE in_memory_pool_probe (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO in_memory_pool_probe (value) VALUES ('shared');",
            )?;
            Ok(())
        })
        .unwrap();

        db.with_read_connection(|conn| {
            let value: String =
                conn.query_row("SELECT value FROM in_memory_pool_probe", [], |row| {
                    row.get(0)
                })?;
            assert_eq!(value, "shared");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_rw_transaction_prevents_busy_snapshot_on_read_modify_write() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Arc::new(Database::open(tmp.path()).unwrap());
        db.with_write_connection(|conn| {
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
                db.with_write_connection(|conn| {
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

    #[test]
    fn set_global_accepts_shared_database_handle() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let _ = Database::set_global(Arc::clone(&db));

        let global = Database::global().expect("global database should be initialized");
        global
            .with_connection(|conn| {
                conn.execute_batch("SELECT 1")?;
                Ok(())
            })
            .unwrap();
    }
}
