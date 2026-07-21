use crate::platform::blocking::{map_err_string, spawn_blocking_map, validate_transport};
use crate::platform::paths::{PathKind, PathProvider};
use sona_core::recovery::RecoveryError;
use sona_core::recovery::types::{RecoveryItemInput, RecoverySnapshot};
use sona_recovery_fs::FsRecoveryAdapter;

async fn run_recovery_adapter_task<T, F>(provider: &dyn PathProvider, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&FsRecoveryAdapter) -> Result<T, RecoveryError> + Send + 'static,
{
    let app_local_data_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(map_err_string)?;
    spawn_blocking_map(move || {
        let adapter = FsRecoveryAdapter::new(app_local_data_dir);
        task(&adapter)
    })
    .await
}

pub async fn load_snapshot(provider: &dyn PathProvider) -> Result<RecoverySnapshot, String> {
    let snapshot = run_recovery_adapter_task(provider, |adapter| adapter.load_snapshot()).await?;
    validate_transport(snapshot)
}

pub async fn load_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<RecoverySnapshot, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    load_snapshot(&provider).await
}

pub async fn save_snapshot(
    provider: &dyn PathProvider,
    items: Vec<RecoveryItemInput>,
) -> Result<RecoverySnapshot, String> {
    sona_ts_bind::validate_typescript_safe_integers(&items).map_err(map_err_string)?;
    let snapshot =
        run_recovery_adapter_task(provider, move |adapter| adapter.save_snapshot(items)).await?;
    validate_transport(snapshot)
}

pub async fn save_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    items: Vec<RecoveryItemInput>,
) -> Result<RecoverySnapshot, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    save_snapshot(&provider, items).await
}

pub async fn persist_queue_snapshot(
    provider: &dyn PathProvider,
    queue_items: Vec<RecoveryItemInput>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    sona_ts_bind::validate_typescript_safe_integers(&queue_items).map_err(map_err_string)?;
    let snapshot = run_recovery_adapter_task(provider, move |adapter| {
        adapter.persist_queue_snapshot(queue_items, resolved_ids.unwrap_or_default())
    })
    .await?;
    validate_transport(snapshot).map(|_| ())
}

pub async fn persist_queue_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue_items: Vec<RecoveryItemInput>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    persist_queue_snapshot(&provider, queue_items, resolved_ids).await
}
