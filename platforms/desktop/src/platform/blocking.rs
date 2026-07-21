//! Shared Desktop host runners for adapter/service calls.
//!
//! Keeps `platform/*` modules as thin forwards: resolve context → optional lock →
//! `spawn_blocking` → map errors to public `String` → return. Domain logic stays
//! in Core/adapters; Tauri command shapes stay at the command boundary.
//!
//! Out of scope for this module: OS watchers (`automation_runtime`), native input
//! (`system`), Sync lifecycle, and new workspace crates (later S4).

use std::sync::{Arc, Mutex};

use serde::Serialize;
use sona_sqlite::SqliteApplicationContext;
use tauri::{AppHandle, Runtime};

/// Map any displayable failure to the Desktop host string boundary.
#[inline]
pub fn map_err_string(error: impl ToString) -> String {
    error.to_string()
}

/// Run a blocking closure on Tauri's blocking pool and flatten join + task errors.
pub async fn spawn_blocking_map<T, E, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    E: ToString + Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(map_err_string)?
        .map_err(map_err_string)
}

/// Resolve the managed SQLite application context for a Desktop app handle.
pub fn sqlite_context<R: Runtime>(app: &AppHandle<R>) -> Arc<SqliteApplicationContext> {
    crate::platform::database::sqlite_application_context(app)
}

/// `spawn_blocking` with a cloned SQLite application context.
pub async fn with_sqlite_context<R, T, E, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let context = sqlite_context(app);
    spawn_blocking_map(move || task(context)).await
}

/// Same as [`with_sqlite_context`], then validates TypeScript-safe integers on the result.
pub async fn with_sqlite_context_transport<R, T, E, F>(
    app: &AppHandle<R>,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let result = with_sqlite_context(app, task).await?;
    validate_transport(result)
}

/// `spawn_blocking` with context plus an exclusive `Mutex` guard (history file tasks).
pub async fn with_sqlite_context_locked<R, T, E, F>(
    app: &AppHandle<R>,
    lock: Arc<Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let context = sqlite_context(app);
    spawn_blocking_map(move || -> Result<T, String> {
        let _guard = lock.lock().map_err(map_err_string)?;
        task(context).map_err(map_err_string)
    })
    .await
}

/// Locked context runner with TypeScript transport validation on the result.
pub async fn with_sqlite_context_locked_transport<R, T, E, F>(
    app: &AppHandle<R>,
    lock: Arc<Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    E: ToString + Send + 'static,
    F: FnOnce(Arc<SqliteApplicationContext>) -> Result<T, E> + Send + 'static,
{
    let result = with_sqlite_context_locked(app, lock, task).await?;
    validate_transport(result)
}

/// Reject values that cannot cross the generated TypeScript boundary safely.
pub fn validate_transport<T: Serialize>(value: T) -> Result<T, String> {
    sona_ts_bind::validate_typescript_safe_integers(&value).map_err(map_err_string)?;
    Ok(value)
}
