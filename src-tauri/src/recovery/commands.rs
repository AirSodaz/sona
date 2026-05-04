use serde_json::Value;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

use super::repository::RecoveryRepository;
use super::types::RecoverySnapshot;

fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

async fn run_repository_task<R, T, F>(app: AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(RecoveryRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || task(RecoveryRepository::new(app_local_data_dir)))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn recovery_load_snapshot<R: Runtime>(
    app: AppHandle<R>,
) -> Result<RecoverySnapshot, String> {
    run_repository_task(app, |repository| repository.load_snapshot()).await
}

#[tauri::command]
pub async fn recovery_save_snapshot<R: Runtime>(
    app: AppHandle<R>,
    items: Vec<Value>,
) -> Result<RecoverySnapshot, String> {
    run_repository_task(app, move |repository| repository.save_snapshot(items)).await
}

#[tauri::command]
pub async fn recovery_persist_queue_snapshot<R: Runtime>(
    app: AppHandle<R>,
    queue_items: Vec<Value>,
) -> Result<(), String> {
    run_repository_task(app, move |repository| {
        repository.persist_queue_snapshot(queue_items).map(|_| ())
    })
    .await
}
