use serde_json::Value;

use crate::platform::paths::{PathKind, PathProvider};
use sona_core::recovery::service::RecoveryService;
use sona_core::recovery::types::RecoverySnapshot;
use sona_recovery_fs::FsRecoverySnapshotStore;
use sona_runtime_fs::{FsSourcePathStatusProvider, SystemClock};

async fn run_recovery_service_task<T, F>(provider: &dyn PathProvider, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: for<'a> FnOnce(&'a RecoveryService<'a>) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = FsRecoverySnapshotStore::new(app_local_data_dir);
        let source_paths = FsSourcePathStatusProvider;
        let clock = SystemClock;
        let service = RecoveryService::new(&store, &source_paths, &clock);
        task(&service)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn load_snapshot(provider: &dyn PathProvider) -> Result<RecoverySnapshot, String> {
    run_recovery_service_task(provider, |service| service.load_snapshot()).await
}

pub async fn load_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<RecoverySnapshot, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    load_snapshot(&provider).await
}

pub async fn save_snapshot(
    provider: &dyn PathProvider,
    items: Vec<Value>,
) -> Result<RecoverySnapshot, String> {
    run_recovery_service_task(provider, move |service| service.save_snapshot(items)).await
}

pub async fn save_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    items: Vec<Value>,
) -> Result<RecoverySnapshot, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    save_snapshot(&provider, items).await
}

pub async fn persist_queue_snapshot(
    provider: &dyn PathProvider,
    queue_items: Vec<Value>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    run_recovery_service_task(provider, move |service| {
        service
            .persist_queue_snapshot(queue_items, resolved_ids.unwrap_or_default())
            .map(|_| ())
    })
    .await
}

pub async fn persist_queue_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    queue_items: Vec<Value>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    persist_queue_snapshot(&provider, queue_items, resolved_ids).await
}
