use serde_json::{Value, json};
use sona_core::recovery::normalization::{SourcePathStatus, SourcePathStatusProvider};
use sona_core::recovery::repository::RecoverySnapshotStore;
use sona_core::recovery::service::RecoveryService;
use sona_core::recovery::types::RecoverySnapshot;
use std::sync::Mutex;

struct MemoryRecoveryStore {
    value: Mutex<Value>,
    saved: Mutex<Option<RecoverySnapshot>>,
}

impl MemoryRecoveryStore {
    fn new(value: Value) -> Self {
        Self {
            value: Mutex::new(value),
            saved: Mutex::new(None),
        }
    }

    fn empty() -> Self {
        Self::new(json!({"version": 1, "items": []}))
    }

    fn saved_snapshot(&self) -> Option<RecoverySnapshot> {
        self.saved.lock().unwrap().clone()
    }
}

impl RecoverySnapshotStore for MemoryRecoveryStore {
    fn load_snapshot_value(&self) -> Result<Value, String> {
        Ok(self.value.lock().unwrap().clone())
    }

    fn save_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), String> {
        *self.saved.lock().unwrap() = Some(snapshot.clone());
        *self.value.lock().unwrap() =
            serde_json::to_value(snapshot).map_err(|error| error.to_string())?;
        Ok(())
    }
}

struct FixedSourcePaths {
    status: SourcePathStatus,
}

impl FixedSourcePaths {
    fn missing() -> Self {
        Self {
            status: SourcePathStatus::Missing,
        }
    }

    fn file() -> Self {
        Self {
            status: SourcePathStatus::File,
        }
    }
}

impl SourcePathStatusProvider for FixedSourcePaths {
    fn status_for_path(&self, _path: &str) -> SourcePathStatus {
        self.status
    }
}

fn pending_saved_item() -> Value {
    json!({
        "id": "pending",
        "filename": "pending.wav",
        "filePath": "C:/pending.wav",
        "resolution": "pending",
        "segments": []
    })
}

fn resolved_saved_item() -> Value {
    json!({
        "id": "resolved",
        "filename": "resolved.wav",
        "filePath": "C:/resolved.wav",
        "resolution": "discarded",
        "segments": []
    })
}

fn existing_snapshot_with_ids(ids: &[&str]) -> Value {
    json!({
        "version": 1,
        "items": ids.iter().map(|id| json!({
            "id": id,
            "filename": format!("{id}.wav"),
            "filePath": format!("C:/{id}.wav"),
            "resolution": "pending",
            "segments": []
        })).collect::<Vec<_>>()
    })
}

fn queue_item(id: &str, recovery_id: Option<&str>) -> Value {
    let mut value = json!({
        "id": id,
        "status": "processing",
        "filename": format!("{id}.wav"),
        "filePath": format!("C:/{id}.wav"),
        "segments": []
    });
    if let Some(recovery_id) = recovery_id {
        value["recoveryId"] = json!(recovery_id);
    }
    value
}

#[test]
fn load_normalizes_stored_items_with_supplied_time_and_source_status() {
    let store = MemoryRecoveryStore::new(json!({
        "version": 1,
        "items": [{
            "id": "saved-1",
            "filename": "missing.wav",
            "filePath": "C:/missing.wav",
            "resolution": "pending",
            "segments": []
        }]
    }));
    let paths = FixedSourcePaths::missing();
    let service = RecoveryService::new(&store, &paths);

    let snapshot = service.load_snapshot_at(5_000).unwrap();

    assert_eq!(snapshot.items[0].updated_at, 5_000);
    assert!(!snapshot.items[0].has_source_file);
    assert!(!snapshot.items[0].can_resume);
}

#[test]
fn save_keeps_only_pending_items_and_persists_canonical_snapshot() {
    let store = MemoryRecoveryStore::empty();
    let paths = FixedSourcePaths::file();
    let service = RecoveryService::new(&store, &paths);

    let snapshot = service
        .save_snapshot_at(vec![pending_saved_item(), resolved_saved_item()], 6_000)
        .unwrap();

    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["pending"]
    );
    assert_eq!(snapshot.updated_at, Some(6_000));
    assert_eq!(store.saved_snapshot().unwrap(), snapshot);
}

#[test]
fn persist_removes_resolved_ids_and_retains_unobserved_pending_items() {
    let store = MemoryRecoveryStore::new(existing_snapshot_with_ids(&["resolved", "retained"]));
    let paths = FixedSourcePaths::file();
    let service = RecoveryService::new(&store, &paths);

    let snapshot = service
        .persist_queue_snapshot_at(
            vec![
                queue_item("active", None),
                queue_item("queue-alias", Some("recovery-alias")),
            ],
            vec!["resolved".to_string()],
            7_000,
        )
        .unwrap();

    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["active", "recovery-alias", "retained"]
    );
}

#[test]
fn load_keeps_valid_stored_items_when_a_sibling_is_corrupt() {
    let store = MemoryRecoveryStore::new(json!({
        "version": 1,
        "updatedAt": 100,
        "items": [
            pending_saved_item(),
            "locally-corrupt-item",
            {
                "id": "second",
                "filename": "second.wav",
                "filePath": "C:/second.wav",
                "resolution": "pending",
                "segments": []
            }
        ]
    }));
    let paths = FixedSourcePaths::file();
    let service = RecoveryService::new(&store, &paths);

    let snapshot = service.load_snapshot_at(8_000).unwrap();

    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["pending", "second"]
    );
    assert_eq!(snapshot.updated_at, Some(100));
}

#[test]
fn persist_keeps_valid_queue_items_when_a_sibling_is_corrupt() {
    let store = MemoryRecoveryStore::empty();
    let paths = FixedSourcePaths::file();
    let service = RecoveryService::new(&store, &paths);

    let snapshot = service
        .persist_queue_snapshot_at(
            vec![
                queue_item("first", None),
                json!({"id": 42, "status": ["processing"]}),
                queue_item("second", None),
            ],
            Vec::new(),
            9_000,
        )
        .unwrap();

    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

#[test]
fn save_keeps_valid_segments_when_a_sibling_segment_is_corrupt() {
    let store = MemoryRecoveryStore::empty();
    let paths = FixedSourcePaths::file();
    let service = RecoveryService::new(&store, &paths);
    let item = json!({
        "id": "recording",
        "filename": "recording.wav",
        "filePath": "C:/recording.wav",
        "resolution": "pending",
        "segments": [
            {"id": "segment-1", "text": "Hello", "start": 0.0, "end": 0.5},
            "locally-corrupt-segment",
            {"id": "segment-2", "text": "World", "start": 0.5, "end": 1.0}
        ]
    });

    let snapshot = service.save_snapshot_at(vec![item], 10_000).unwrap();

    assert_eq!(
        snapshot.items[0]
            .segments
            .iter()
            .map(|segment| segment.id.as_str())
            .collect::<Vec<_>>(),
        vec!["segment-1", "segment-2"]
    );
    assert_eq!(store.saved_snapshot().unwrap(), snapshot);
}
