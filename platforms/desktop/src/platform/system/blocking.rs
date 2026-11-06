//! Shared Desktop host runners for adapter/service calls.
//!
//! Provides `spawn_blocking_map` on top of Tokio's blocking threadpool and
//! flattens join and task errors to `String`.

use serde::Serialize;

pub use crate::services::db_runner::{
    run_sqlite_task, run_sqlite_task_locked, run_sqlite_task_locked_transport,
    run_sqlite_task_transport,
};

/// Map any displayable failure to the Desktop host string boundary.
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

/// Validate values crossing transport boundary (e.g. integer range safety).
pub fn validate_transport<T: Serialize>(value: T) -> Result<T, String> {
    sona_ts_bind::validate_typescript_safe_integers(&value).map_err(map_err_string)?;
    Ok(value)
}
