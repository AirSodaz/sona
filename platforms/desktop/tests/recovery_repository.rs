use serde_json::json;
use sona_core::ports::path::{PathKind, PathProvider};
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
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, String> {
        self.resolved_kinds.lock().unwrap().push(kind);
        match kind {
            PathKind::AppLocalData => Ok(self.app_local_data_dir.clone()),
            _ => Err(format!("unexpected path kind: {kind:?}")),
        }
    }
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
            "version": 1,
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
        vec![json!({
            "id": "saved-1",
            "filename": "recording.wav",
            "filePath": source_file,
            "resolution": "pending",
            "segments": []
        })],
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
        vec![json!({
            "id": "retained",
            "filename": "retained.wav",
            "filePath": source_file,
            "resolution": "pending",
            "segments": []
        })],
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
