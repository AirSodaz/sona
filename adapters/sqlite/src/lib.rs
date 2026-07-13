pub mod analytics;
pub mod automation;
pub mod config_store;
pub mod error;
pub mod history_archive;
pub mod history_backup;
pub mod history_fs_utils;
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

use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::fs::File;
use std::io::Read;
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
const READ_ONLY_SNAPSHOT_ATTEMPTS: usize = 3;
const FILE_COMPARE_BUFFER_SIZE: usize = 64 * 1024;

static GLOBAL_DB: OnceLock<Arc<Database>> = OnceLock::new();

#[cfg(test)]
type WorkspaceMatchTestHook = Box<dyn FnOnce() + Send + 'static>;

#[cfg(test)]
static WORKSPACE_MATCH_TEST_HOOK: Mutex<Option<(String, WorkspaceMatchTestHook)>> =
    Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_workspace_match_test_hook(
    normalized_query: impl Into<String>,
    hook: WorkspaceMatchTestHook,
) {
    *WORKSPACE_MATCH_TEST_HOOK.lock().unwrap() = Some((normalized_query.into(), hook));
}

#[cfg(test)]
fn run_workspace_match_test_hook(normalized_query: &str) {
    let hook = {
        let mut guard = WORKSPACE_MATCH_TEST_HOOK.lock().unwrap();
        if guard
            .as_ref()
            .is_some_and(|(expected_query, _)| expected_query == normalized_query)
        {
            guard.take().map(|(_, hook)| hook)
        } else {
            None
        }
    };
    if let Some(hook) = hook {
        hook();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadOnlySidecarState {
    Clean,
    ActiveWal,
}

fn sqlite_sidecar_paths(db_path: &Path) -> (PathBuf, PathBuf) {
    let mut wal_path = db_path.as_os_str().to_os_string();
    wal_path.push("-wal");
    let mut shm_path = db_path.as_os_str().to_os_string();
    shm_path.push("-shm");
    (PathBuf::from(wal_path), PathBuf::from(shm_path))
}

fn validate_read_only_sidecars(
    wal_path: &Path,
    shm_path: &Path,
) -> Result<ReadOnlySidecarState, DatabaseError> {
    let wal_exists = wal_path.is_file();
    let shm_exists = shm_path.is_file();
    match (wal_exists, shm_exists) {
        (false, false) => Ok(ReadOnlySidecarState::Clean),
        (true, true) => Ok(ReadOnlySidecarState::ActiveWal),
        _ => Err(DatabaseError::ConnectionError(format!(
            "Cannot safely open read-only database with incomplete WAL sidecars: WAL exists={wal_exists}, SHM exists={shm_exists}"
        ))),
    }
}

fn copy_database_snapshot(
    snapshot_dir: &Path,
    db_path: &Path,
    wal_path: &Path,
    sidecar_state: ReadOnlySidecarState,
) -> Result<PathBuf, DatabaseError> {
    let file_name = db_path.file_name().ok_or_else(|| {
        DatabaseError::ConnectionError("Database path has no file name".to_string())
    })?;
    let snapshot_db_path = snapshot_dir.join(file_name);
    std::fs::copy(db_path, &snapshot_db_path)
        .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
    if sidecar_state == ReadOnlySidecarState::ActiveWal {
        let (snapshot_wal_path, _) = sqlite_sidecar_paths(&snapshot_db_path);
        std::fs::copy(wal_path, snapshot_wal_path)
            .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
    }
    Ok(snapshot_db_path)
}

fn files_equal(left_path: &Path, right_path: &Path) -> Result<bool, DatabaseError> {
    let mut left =
        File::open(left_path).map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
    let mut right = File::open(right_path)
        .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
    let mut left_buffer = [0_u8; FILE_COMPARE_BUFFER_SIZE];
    let mut right_buffer = [0_u8; FILE_COMPARE_BUFFER_SIZE];

    loop {
        let left_count = left
            .read(&mut left_buffer)
            .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
        let right_count = right
            .read(&mut right_buffer)
            .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
        if left_count != right_count || left_buffer[..left_count] != right_buffer[..right_count] {
            return Ok(false);
        }
        if left_count == 0 {
            return Ok(true);
        }
    }
}

fn database_snapshot_matches_source(
    db_path: &Path,
    wal_path: &Path,
    snapshot_db_path: &Path,
    sidecar_state: ReadOnlySidecarState,
) -> Result<bool, DatabaseError> {
    if !files_equal(db_path, snapshot_db_path)? {
        return Ok(false);
    }
    if sidecar_state == ReadOnlySidecarState::ActiveWal {
        let (snapshot_wal_path, _) = sqlite_sidecar_paths(snapshot_db_path);
        return files_equal(wal_path, &snapshot_wal_path);
    }
    Ok(true)
}

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
    _read_only_snapshot: Option<tempfile::TempDir>,
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

impl From<DatabaseError> for sona_core::history::mutation_repository::HistoryMutationError {
    fn from(error: DatabaseError) -> Self {
        use sona_core::history::mutation_repository::HistoryMutationError;

        match error {
            DatabaseError::NotFoundError(reason) => HistoryMutationError::NotFound(reason),
            DatabaseError::SerializationError(error) => HistoryMutationError::Serialization(error),
            DatabaseError::Internal(reason) => HistoryMutationError::Internal(reason),
            error => HistoryMutationError::Database(error.to_string()),
        }
    }
}

impl Database {
    pub fn global_arc() -> Result<Arc<Database>, DatabaseError> {
        GLOBAL_DB
            .get()
            .map(Arc::clone)
            .ok_or_else(|| DatabaseError::Internal("Database not initialized".to_string()))
    }

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
            _read_only_snapshot: None,
            slow_query_threshold_us: AtomicI64::new(0),
        };

        schema::run_migrations(&db)?;
        db.run_optimize()?;

        Ok(db)
    }

    /// Opens the existing main database without creating files, migrating the
    /// schema, attaching analytics, or performing maintenance writes.
    pub fn open_read_only(app_local_data_dir: &Path) -> Result<Self, DatabaseError> {
        Self::open_read_only_with_hook(app_local_data_dir, || {})
    }

    /// Opens consistent read-only snapshots of the main and analytics databases.
    pub fn open_read_only_with_analytics(app_local_data_dir: &Path) -> Result<Self, DatabaseError> {
        Self::open_read_only_with_analytics_hook(app_local_data_dir, || {})
    }

    fn open_read_only_with_hook(
        app_local_data_dir: &Path,
        after_initial_snapshot: impl FnOnce(),
    ) -> Result<Self, DatabaseError> {
        Self::open_read_only_snapshot(app_local_data_dir, false, after_initial_snapshot)
    }

    fn open_read_only_with_analytics_hook(
        app_local_data_dir: &Path,
        after_initial_snapshot: impl FnOnce(),
    ) -> Result<Self, DatabaseError> {
        Self::open_read_only_snapshot(app_local_data_dir, true, after_initial_snapshot)
    }

    fn open_read_only_snapshot(
        app_local_data_dir: &Path,
        include_analytics: bool,
        after_initial_snapshot: impl FnOnce(),
    ) -> Result<Self, DatabaseError> {
        let app_local_data_dir = std::path::absolute(app_local_data_dir)
            .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
        let db_path = app_local_data_dir.join("sona.db");
        let analytics_path = app_local_data_dir.join("sona-analytics.db");
        let (wal_path, shm_path) = sqlite_sidecar_paths(&db_path);
        let (analytics_wal_path, analytics_shm_path) = sqlite_sidecar_paths(&analytics_path);
        let mut after_initial_snapshot = Some(after_initial_snapshot);
        let source_states = || {
            let main = validate_read_only_sidecars(&wal_path, &shm_path)?;
            let analytics = include_analytics
                .then(|| validate_read_only_sidecars(&analytics_wal_path, &analytics_shm_path))
                .transpose()?;
            Ok::<_, DatabaseError>((main, analytics))
        };

        for _ in 0..READ_ONLY_SNAPSHOT_ATTEMPTS {
            let initial_states = source_states()?;
            let snapshot_dir = tempfile::Builder::new()
                .prefix("sona-read-only-")
                .tempdir()
                .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
            let capture = (|| {
                let snapshot_db_path = copy_database_snapshot(
                    snapshot_dir.path(),
                    &db_path,
                    &wal_path,
                    initial_states.0,
                )?;
                let snapshot_analytics_path = initial_states
                    .1
                    .map(|state| {
                        copy_database_snapshot(
                            snapshot_dir.path(),
                            &analytics_path,
                            &analytics_wal_path,
                            state,
                        )
                    })
                    .transpose()?;
                Ok((snapshot_db_path, snapshot_analytics_path))
            })();
            let (snapshot_db_path, snapshot_analytics_path) = match capture {
                Ok(snapshot) => snapshot,
                Err(_) if source_states().ok() != Some(initial_states) => {
                    continue;
                }
                Err(error) => return Err(error),
            };
            if let Some(hook) = after_initial_snapshot.take() {
                hook();
            }
            if source_states().ok() != Some(initial_states) {
                continue;
            }
            let main_matches = database_snapshot_matches_source(
                &db_path,
                &wal_path,
                &snapshot_db_path,
                initial_states.0,
            )?;
            let analytics_matches = match (snapshot_analytics_path.as_ref(), initial_states.1) {
                (Some(snapshot_path), Some(state)) => database_snapshot_matches_source(
                    &analytics_path,
                    &analytics_wal_path,
                    snapshot_path,
                    state,
                )?,
                (None, None) => true,
                _ => false,
            };
            if !main_matches || !analytics_matches || source_states().ok() != Some(initial_states) {
                continue;
            }

            let conn = Connection::open(&snapshot_db_path)
                .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
            if let Some(snapshot_analytics_path) = snapshot_analytics_path {
                Self::attach_analytics(&conn, &snapshot_analytics_path)?;
            }
            conn.execute_batch("PRAGMA query_only=ON; PRAGMA foreign_keys=ON;")
                .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
            conn.busy_timeout(Duration::from_millis(POOL_ACQUIRE_TIMEOUT_MS))
                .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
            Self::register_connection_functions(&conn)?;
            schema::validate_current_schema(&conn)?;

            return Ok(Self {
                read_pool: ConnectionPool::new(vec![conn]),
                write_pool: ConnectionPool::new(Vec::new()),
                app_local_data_dir: Some(app_local_data_dir),
                _read_only_snapshot: Some(snapshot_dir),
                slow_query_threshold_us: AtomicI64::new(0),
            });
        }

        Err(DatabaseError::ConnectionError(format!(
            "Database changed while creating read-only snapshot after {READ_ONLY_SNAPSHOT_ATTEMPTS} attempts"
        )))
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
        Self::register_connection_functions(&conn)?;

        let shared_pool = ConnectionPool::new(vec![conn]);
        let db = Self {
            read_pool: shared_pool.clone(),
            write_pool: shared_pool,
            app_local_data_dir: None,
            _read_only_snapshot: None,
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
        Self::register_connection_functions(&conn)?;
        Ok(conn)
    }

    fn register_connection_functions(conn: &Connection) -> Result<(), DatabaseError> {
        let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
        conn.create_scalar_function("sona_workspace_matches", 4, flags, |context| {
            let title = context.get::<String>(0)?;
            let preview_text = context.get::<String>(1)?;
            let search_content = context.get::<String>(2)?;
            let normalized_query = context.get::<String>(3)?;
            #[cfg(test)]
            run_workspace_match_test_hook(&normalized_query);
            Ok(i64::from(
                sona_core::history::workspace_query::workspace_search_fields_match(
                    &title,
                    &preview_text,
                    &search_content,
                    &normalized_query,
                ),
            ))
        })?;
        conn.create_scalar_function("sona_workspace_title_key", 1, flags, |context| {
            let title = context.get::<String>(0)?;
            Ok(sona_core::history::workspace_query::workspace_title_sort_key(&title))
        })?;
        Ok(())
    }

    fn attach_analytics(conn: &Connection, analytics_path: &Path) -> Result<(), DatabaseError> {
        let attach_sql = format!(
            "ATTACH DATABASE '{}' AS analytics",
            analytics_path.to_string_lossy().replace('\'', "''")
        );
        conn.execute_batch(&attach_sql)
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        conn.execute_batch("PRAGMA analytics.journal_mode=WAL;")
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
    fn read_only_open_preserves_main_only_database_support() {
        let tmp = tempfile::TempDir::new().unwrap();
        let database = Database::open(tmp.path()).unwrap();
        database
            .with_write_connection(|connection| {
                connection.execute(
                    "INSERT INTO app_settings (key, value) VALUES ('main-only', 'available')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        drop(database);
        std::fs::remove_file(tmp.path().join("sona-analytics.db")).unwrap();

        let read_only = Database::open_read_only(tmp.path()).unwrap();

        read_only
            .with_connection(|connection| {
                let value: String = connection.query_row(
                    "SELECT value FROM app_settings WHERE key = 'main-only'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(value, "available");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn read_only_open_retries_clean_database_that_changes_to_active_wal() {
        let tmp = tempfile::TempDir::new().unwrap();
        drop(Database::open(tmp.path()).unwrap());
        assert!(!tmp.path().join("sona.db-wal").exists());
        assert!(!tmp.path().join("sona.db-shm").exists());
        let mut writer = None;

        let database = Database::open_read_only_with_hook(tmp.path(), || {
            let database = Database::open(tmp.path()).unwrap();
            database
                .with_write_connection(|connection| {
                    connection.execute(
                        "INSERT INTO app_settings (key, value) VALUES ('race', 'committed')",
                        [],
                    )?;
                    Ok(())
                })
                .unwrap();
            writer = Some(database);
        })
        .unwrap();

        database
            .with_connection(|connection| {
                let value: String = connection.query_row(
                    "SELECT value FROM app_settings WHERE key = 'race'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(value, "committed");
                Ok(())
            })
            .unwrap();
        drop(writer);
    }

    #[test]
    fn read_only_open_retries_wal_append_without_sidecar_state_change() {
        let tmp = tempfile::TempDir::new().unwrap();
        let writer = Database::open(tmp.path()).unwrap();
        assert!(tmp.path().join("sona.db-wal").exists());
        assert!(tmp.path().join("sona.db-shm").exists());

        let database = Database::open_read_only_with_hook(tmp.path(), || {
            writer
                .with_write_connection(|connection| {
                    connection.execute(
                        "INSERT INTO app_settings (key, value) VALUES ('append', 'committed')",
                        [],
                    )?;
                    Ok(())
                })
                .unwrap();
        })
        .unwrap();

        database
            .with_connection(|connection| {
                let value: String = connection.query_row(
                    "SELECT value FROM app_settings WHERE key = 'append'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(value, "committed");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn read_only_open_retries_analytics_wal_append() {
        let tmp = tempfile::TempDir::new().unwrap();
        let writer = Database::open(tmp.path()).unwrap();
        writer
            .with_write_connection(|connection| {
                connection.execute(
                    "INSERT INTO analytics.llm_usage (occurred_at, total_tokens)
                     VALUES ('2026-07-13T07:00:00Z', 10)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(tmp.path().join("sona-analytics.db-wal").is_file());
        assert!(tmp.path().join("sona-analytics.db-shm").is_file());

        let database = Database::open_read_only_with_analytics_hook(tmp.path(), || {
            writer
                .with_write_connection(|connection| {
                    connection.execute(
                        "INSERT INTO analytics.llm_usage (occurred_at, total_tokens)
                         VALUES ('2026-07-13T08:00:00Z', 20)",
                        [],
                    )?;
                    Ok(())
                })
                .unwrap();
        })
        .unwrap();

        database
            .with_connection(|connection| {
                let total_tokens: i64 = connection.query_row(
                    "SELECT SUM(total_tokens) FROM analytics.llm_usage",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(total_tokens, 30);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn read_only_open_retries_wal_checkpoint_reset() {
        let tmp = tempfile::TempDir::new().unwrap();
        let writer = Database::open(tmp.path()).unwrap();
        writer
            .with_write_connection(|connection| {
                connection.execute(
                    "INSERT INTO app_settings (key, value) VALUES ('checkpoint', 'committed')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let database = Database::open_read_only_with_hook(tmp.path(), || {
            writer
                .with_write_connection(|connection| {
                    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
                    Ok(())
                })
                .unwrap();
        })
        .unwrap();

        database
            .with_connection(|connection| {
                let value: String = connection.query_row(
                    "SELECT value FROM app_settings WHERE key = 'checkpoint'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(value, "committed");
                Ok(())
            })
            .unwrap();
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
    fn disk_database_configures_analytics_for_wal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(tmp.path()).unwrap();

        db.with_connection(|connection| {
            let mode: String =
                connection.query_row("PRAGMA analytics.journal_mode", [], |row| row.get(0))?;
            assert_eq!(mode, "wal");
            Ok(())
        })
        .unwrap();
        assert!(tmp.path().join("sona-analytics.db-wal").is_file());
        assert!(tmp.path().join("sona-analytics.db-shm").is_file());
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
