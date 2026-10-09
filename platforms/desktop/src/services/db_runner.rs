//! Pure Rust blocking database executor and transport validator.
//!
//! Decoupled from Tauri runtime, uses `tokio::task::spawn_blocking`
//! and operates directly on `DesktopSqliteState`.

use serde::Serialize;
use sona_sqlite::SqliteApplicationContext;
use std::sync::{Arc, Mutex};

use crate::platform::database::DesktopSqliteState;

/// Map any displayable failure to a `String` boundary error.
#[inline]
pub fn map_err_string(error: impl ToString) -> String {
    error.to_string()
}

/// Run a blocking closure on Tokio's blocking pool and flatten join + task errors.
pub async fn spawn_blocking_map<T, E, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    E: ToString + Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(map_err_string)?
        .map_err(map_err_string)
}

/// Run a database task on a blocking thread with current SQLite application context.
pub async fn run_sqlite_task<T, E, F>(sqlite: &DesktopSqliteState, task: F) -> Result<T, String>
where
    T: Send + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let context = sqlite.current_context()?;
    spawn_blocking_map(move || task(context)).await
}

/// Same as [`run_sqlite_task`], then validates TypeScript-safe integers on the result.
pub async fn run_sqlite_task_transport<T, E, F>(
    sqlite: &DesktopSqliteState,
    task: F,
) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let result = run_sqlite_task(sqlite, task).await?;
    validate_transport(result)
}

/// Run a database task with an exclusive `Mutex` lock (e.g. history file write lock).
pub async fn run_sqlite_task_locked<T, E, F>(
    sqlite: &DesktopSqliteState,
    lock: Arc<Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let context = sqlite.current_context()?;
    spawn_blocking_map(move || -> Result<T, String> {
        let _guard = lock.lock().map_err(map_err_string)?;
        task(context).map_err(map_err_string)
    })
    .await
}

/// Locked SQLite runner with TypeScript transport integer validation on the result.
pub async fn run_sqlite_task_locked_transport<T, E, F>(
    sqlite: &DesktopSqliteState,
    lock: Arc<Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let result = run_sqlite_task_locked(sqlite, lock, task).await?;
    validate_transport(result)
}

/// Reject values that cannot cross the generated TypeScript boundary safely.
pub fn validate_transport<T: Serialize>(value: T) -> Result<T, String> {
    sona_ts_bind::validate_typescript_safe_integers(&value).map_err(map_err_string)?;
    Ok(value)
}
