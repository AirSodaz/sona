use serde_json::json;
use sona_core::ports::path::{PathKind, PathProvider, PathProviderError};
use sona_core::recovery::types::RecoveryItemInput;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri_appsona_lib::platform::recovery_repository::{
    load_snapshot, persist_queue_snapshot, save_snapshot,
};
use tempfile::tempdir;

struct RecordingPathProvider {
    app_local_data_dir: PathBuf,
    resolved_kinds: Mutex<Vec<PathKind>>,
}

fn recovery_item(value: serde_json::Value) -> RecoveryItemInput {
    serde_json::from_value(value).unwrap()
}

impl RecordingPathProvider {
    fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            app_local_data_dir,
            resolved_kinds: Mutex::new(Vec::new()),
        }
    }

    fn resolved_kinds(&self) -> Vec<PathKind> {
        self.resolved_kinds.lock().unwrap().clone()
    }
}

impl PathProvider for RecordingPathProvider {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathProviderError> {
        self.resolved_kinds.lock().unwrap().push(kind);
        match kind {
            PathKind::AppLocalData => Ok(self.app_local_data_dir.clone()),
            _ => Err(PathProviderError::new(
                kind,
                format!("unexpected path kind: {kind:?}"),
            )),
        }
    }
}

struct FailingPathProvider;

impl PathProvider for FailingPathProvider {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathProviderError> {
        Err(PathProviderError::new(
            kind,
            "desktop test path unavailable",
        ))
    }
}

#[tokio::test]
async fn load_snapshot_preserves_path_kind_at_the_string_command_boundary() {
    let error = load_snapshot(&FailingPathProvider).await.unwrap_err();

    assert_eq!(
        error,
        "Failed to resolve AppLocalData path: desktop test path unavailable"
    );
}

#[tokio::test]
async fn load_snapshot_resolves_app_local_data_before_delegating() {
    let dir = tempdir().unwrap();
    let provider = RecordingPathProvider::new(dir.path().to_path_buf());

    let snapshot = load_snapshot(&provider).await.unwrap();

    assert_eq!(provider.resolved_kinds(), vec![PathKind::AppLocalData]);
    assert_eq!(
        serde_json::to_value(snapshot).unwrap(),
        json!({
            "version": 2,
            "updatedAt": null,
            "items": []
        })
    );
}

#[tokio::test]
async fn save_snapshot_composes_the_service_and_filesystem_adapters() {
    let dir = tempdir().unwrap();
    let source_file = dir.path().join("recording.wav");
    File::create(&source_file).unwrap();
    let provider = RecordingPathProvider::new(dir.path().to_path_buf());

    let snapshot = save_snapshot(
        &provider,
        vec![recovery_item(json!({
            "id": "saved-1",
            "filename": "recording.wav",
            "filePath": source_file,
            "resolution": "pending",
            "segments": []
        }))],
    )
    .await
    .unwrap();

    assert_eq!(provider.resolved_kinds(), vec![PathKind::AppLocalData]);
    assert_eq!(snapshot.items.len(), 1);
    assert!(snapshot.items[0].has_source_file);
    assert!(snapshot.items[0].can_resume);
    let stored: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("recovery/queue-recovery.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(stored, serde_json::to_value(snapshot).unwrap());
}

#[tokio::test]
async fn persist_queue_snapshot_defaults_missing_resolved_ids_to_empty() {
    let dir = tempdir().unwrap();
    let source_file = dir.path().join("retained.wav");
    File::create(&source_file).unwrap();
    let provider = RecordingPathProvider::new(dir.path().to_path_buf());
    save_snapshot(
        &provider,
        vec![recovery_item(json!({
            "id": "retained",
            "filename": "retained.wav",
            "filePath": source_file,
            "resolution": "pending",
            "segments": []
        }))],
    )
    .await
    .unwrap();

    persist_queue_snapshot(&provider, Vec::new(), None)
        .await
        .unwrap();

    assert_eq!(
        provider.resolved_kinds(),
        vec![PathKind::AppLocalData, PathKind::AppLocalData]
    );
    let stored: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("recovery/queue-recovery.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(stored["items"][0]["id"], "retained");
}

#[tokio::test]
async fn save_snapshot_rejects_unsafe_typescript_integer_input() {
    let dir = tempdir().unwrap();
    let provider = RecordingPathProvider::new(dir.path().to_path_buf());
    let item = recovery_item(json!({
        "id": "unsafe-input",
        "filename": "unsafe.wav",
        "filePath": "",
        "resolution": "pending",
        "updatedAt": 9_007_199_254_740_992_u64,
        "segments": []
    }));

    let error = save_snapshot(&provider, vec![item]).await.unwrap_err();

    assert!(error.contains("exceeds TypeScript's safe range"), "{error}");
    assert!(provider.resolved_kinds().is_empty());
}

#[tokio::test]
async fn load_snapshot_rejects_unsafe_typescript_integer_output() {
    let dir = tempdir().unwrap();
    let recovery_dir = dir.path().join("recovery");
    fs::create_dir_all(&recovery_dir).unwrap();
    fs::write(
        recovery_dir.join("queue-recovery.json"),
        serde_json::to_vec(&json!({
            "version": 1,
            "updatedAt": 9_007_199_254_740_992_u64,
            "items": [{
                "id": "unsafe-output",
                "filename": "unsafe.wav",
                "filePath": "",
                "resolution": "pending",
                "segments": []
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let provider = RecordingPathProvider::new(dir.path().to_path_buf());

    let error = load_snapshot(&provider).await.unwrap_err();

    assert!(error.contains("exceeds TypeScript's safe range"), "{error}");
}
