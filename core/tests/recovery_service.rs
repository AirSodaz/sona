use serde_json::{Value, json};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::recovery::RecoveryError;
use sona_core::recovery::normalization::{SourcePathStatus, SourcePathStatusProvider};
use sona_core::recovery::repository::RecoverySnapshotStore;
use sona_core::recovery::service::RecoveryService;
use sona_core::recovery::types::{RecoveryItemInput, RecoverySnapshot, RecoverySnapshotInput};
use std::sync::Mutex;

struct MemoryRecoveryStore {
    input: Mutex<RecoverySnapshotInput>,
    saved: Mutex<Option<RecoverySnapshot>>,
}

impl MemoryRecoveryStore {
    fn new(value: Value) -> Self {
        Self {
            input: Mutex::new(serde_json::from_value(value).unwrap_or_default()),
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
    fn load_snapshot_input(&self) -> Result<RecoverySnapshotInput, RecoveryError> {
        Ok(self.input.lock().unwrap().clone())
    }

    fn save_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), RecoveryError> {
        *self.saved.lock().unwrap() = Some(snapshot.clone());
        *self.input.lock().unwrap() = serde_json::from_value(
            serde_json::to_value(snapshot)
                .map_err(|error| RecoveryError::Repository(error.to_string()))?,
        )
        .map_err(|error| RecoveryError::Repository(error.to_string()))?;
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

struct FixedClock(Result<u64, ClockError>);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        self.0.clone()
    }
}

static DEFAULT_CLOCK: FixedClock = FixedClock(Ok(0));

fn service<'a>(store: &'a MemoryRecoveryStore, paths: &'a FixedSourcePaths) -> RecoveryService<'a> {
    RecoveryService::new(store, paths, &DEFAULT_CLOCK)
}

fn input_item(value: Value) -> RecoveryItemInput {
    serde_json::from_value(value).unwrap()
}

fn input_items(value: Value) -> Vec<RecoveryItemInput> {
    serde_json::from_value(value).unwrap()
}

fn pending_saved_item() -> RecoveryItemInput {
    input_item(json!({
        "id": "pending",
        "filename": "pending.wav",
        "filePath": "C:/pending.wav",
        "resolution": "pending",
        "segments": []
    }))
}

fn resolved_saved_item() -> RecoveryItemInput {
    input_item(json!({
        "id": "resolved",
        "filename": "resolved.wav",
        "filePath": "C:/resolved.wav",
        "resolution": "discarded",
        "segments": []
    }))
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

fn queue_item(id: &str, recovery_id: Option<&str>) -> RecoveryItemInput {
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
    input_item(value)
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
    let service = service(&store, &paths);

    let snapshot = service.load_snapshot_at(5_000).unwrap();

    assert_eq!(snapshot.items[0].updated_at, 5_000);
    assert!(!snapshot.items[0].has_source_file);
    assert!(!snapshot.items[0].can_resume);
}

#[test]
fn save_keeps_only_pending_items_and_persists_canonical_snapshot() {
    let store = MemoryRecoveryStore::empty();
    let paths = FixedSourcePaths::file();
    let service = service(&store, &paths);

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
    let service = service(&store, &paths);

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
    let service = service(&store, &paths);

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
    let service = service(&store, &paths);

    let snapshot = service
        .persist_queue_snapshot_at(
            input_items(json!([
                {
                    "id": "first",
                    "status": "processing",
                    "filename": "first.wav",
                    "filePath": "C:/first.wav",
                    "segments": []
                },
                {"id": 42, "status": ["processing"]},
                {
                    "id": "second",
                    "status": "processing",
                    "filename": "second.wav",
                    "filePath": "C:/second.wav",
                    "segments": []
                }
            ])),
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
    let service = service(&store, &paths);
    let item = input_item(json!({
        "id": "recording",
        "filename": "recording.wav",
        "filePath": "C:/recording.wav",
        "resolution": "pending",
        "segments": [
            {"id": "segment-1", "text": "Hello", "start": 0.0, "end": 0.5},
            "locally-corrupt-segment",
            {"id": "segment-2", "text": "World", "start": 0.5, "end": 1.0}
        ]
    }));

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

#[test]
fn save_keeps_segment_when_legacy_nested_metadata_is_invalid() {
    let store = MemoryRecoveryStore::empty();
    let paths = FixedSourcePaths::file();
    let service = service(&store, &paths);
    let item = input_item(json!({
        "id": "recording",
        "filename": "recording.wav",
        "filePath": "C:/recording.wav",
        "resolution": "pending",
        "segments": [{
            "id": "segment-1",
            "text": "Hello",
            "start": 0.0,
            "end": 1.0,
            "timing": {
                "level": "word",
                "source": "legacy",
                "units": []
            },
            "speaker": "legacy-speaker",
            "speakerAttribution": {"state": "legacy"}
        }]
    }));

    let snapshot = service.save_snapshot_at(vec![item], 10_500).unwrap();
    let segment = &snapshot.items[0].segments[0];

    assert_eq!(segment.id, "segment-1");
    assert_eq!(
        segment.timing.as_ref().unwrap().level,
        sona_core::transcription::transcript::TranscriptTimingLevel::Segment
    );
    assert!(segment.speaker.is_none());
    assert!(segment.speaker_attribution.is_none());
}

#[test]
fn save_keeps_item_when_legacy_segments_field_is_not_an_array() {
    let store = MemoryRecoveryStore::empty();
    let paths = FixedSourcePaths::file();
    let service = service(&store, &paths);
    let item = input_item(json!({
        "id": "recording",
        "filename": "recording.wav",
        "filePath": "C:/recording.wav",
        "resolution": "pending",
        "segments": {"legacy": true}
    }));

    let snapshot = service.save_snapshot_at(vec![item], 10_600).unwrap();

    assert_eq!(snapshot.items[0].id, "recording");
    assert!(snapshot.items[0].segments.is_empty());
}

#[test]
fn recovery_runtime_methods_use_the_injected_clock() {
    let store = MemoryRecoveryStore::new(existing_snapshot_with_ids(&["loaded"]));
    let paths = FixedSourcePaths::file();
    let clock = FixedClock(Ok(11_000));
    let service = RecoveryService::new(&store, &paths, &clock);

    let loaded = service.load_snapshot().unwrap();
    let saved = service.save_snapshot(vec![pending_saved_item()]).unwrap();
    let persisted = service
        .persist_queue_snapshot(vec![queue_item("queued", None)], Vec::new())
        .unwrap();

    assert_eq!(loaded.items[0].updated_at, 11_000);
    assert_eq!(saved.updated_at, Some(11_000));
    assert_eq!(persisted.updated_at, Some(11_000));
}

#[test]
fn recovery_runtime_methods_propagate_clock_errors() {
    let store = MemoryRecoveryStore::empty();
    let paths = FixedSourcePaths::file();
    let clock = FixedClock(Err(ClockError::Unavailable(
        "recovery clock unavailable".to_string(),
    )));
    let service = RecoveryService::new(&store, &paths, &clock);

    let error = service.load_snapshot().unwrap_err();

    assert!(matches!(
        error,
        RecoveryError::Clock(ClockError::Unavailable(ref reason))
            if reason == "recovery clock unavailable"
    ));
    assert!(store.saved_snapshot().is_none());
}
