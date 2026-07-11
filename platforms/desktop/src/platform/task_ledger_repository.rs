use serde_json::Value;
use sona_core::task_ledger::types::{
    TASK_LEDGER_UPDATED_EVENT, TaskLedgerRecord, TaskLedgerSnapshot,
};
use sona_sqlite::DatabaseError;
use tauri::{AppHandle, Emitter, Runtime};

async fn run_task_ledger_repository_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(sona_sqlite::task_ledger::SqliteLedgerRepository) -> Result<T, DatabaseError>
        + Send
        + 'static,
{
    let db = crate::platform::database::sqlite_database(app);
    tauri::async_runtime::spawn_blocking(move || {
        task(sona_sqlite::task_ledger::SqliteLedgerRepository::new(db))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}

fn emit_task_ledger_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &TaskLedgerSnapshot,
) -> Result<(), String> {
    app.emit(TASK_LEDGER_UPDATED_EVENT, snapshot)
        .map_err(|error| error.to_string())
}

pub async fn load_snapshot<R: Runtime>(app: &AppHandle<R>) -> Result<TaskLedgerSnapshot, String> {
    run_task_ledger_repository_task(app, |repository| repository.load_snapshot()).await
}

pub async fn upsert_task<R: Runtime>(
    app: &AppHandle<R>,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_task_ledger_repository_task(app, move |repository| {
        repository.upsert_task(record)?;
        repository.load_snapshot()
    })
    .await?;
    let _ = emit_task_ledger_snapshot(app, &snapshot);
    Ok(snapshot)
}

pub async fn patch_task<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
    patch: Value,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_task_ledger_repository_task(app, move |repository| {
        repository.patch_task(&id, patch)?;
        repository.load_snapshot()
    })
    .await?;
    let _ = emit_task_ledger_snapshot(app, &snapshot);
    Ok(snapshot)
}

pub async fn remove_task<R: Runtime>(
    app: &AppHandle<R>,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_task_ledger_repository_task(app, move |repository| {
        repository.remove_task(&id)?;
        repository.load_snapshot()
    })
    .await?;
    let _ = emit_task_ledger_snapshot(app, &snapshot);
    Ok(snapshot)
}

pub async fn clear_resolved<R: Runtime>(app: &AppHandle<R>) -> Result<TaskLedgerSnapshot, String> {
    let snapshot = run_task_ledger_repository_task(app, |repository| {
        repository.clear_resolved()?;
        repository.load_snapshot()
    })
    .await?;
    let _ = emit_task_ledger_snapshot(app, &snapshot);
    Ok(snapshot)
}
