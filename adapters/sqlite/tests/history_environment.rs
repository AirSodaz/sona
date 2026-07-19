use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use sona_core::history::HistoryIdGenerator;
use sona_core::history::mutation_repository::{
    HistoryCreateTranscriptSnapshotRequest, HistoryMutationError, HistoryMutationRepository,
};
use sona_core::history::query_repository::HistoryQueryRepository;
use sona_core::history::{HistorySaveRecordingRequest, TranscriptSnapshotReason};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_sqlite::{Database, SqliteHistoryStore};

struct FixedClock(Result<u64, ClockError>);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        self.0.clone()
    }
}

struct SequenceIds(Mutex<VecDeque<String>>);

impl SequenceIds {
    fn new(values: impl IntoIterator<Item = &'static str>) -> Self {
        Self(Mutex::new(values.into_iter().map(str::to_string).collect()))
    }
}

impl HistoryIdGenerator for SequenceIds {
    fn generate_id(&self) -> String {
        self.0.lock().unwrap().pop_front().unwrap()
    }
}

fn recording_request() -> HistorySaveRecordingRequest {
    HistorySaveRecordingRequest {
        segments: Vec::new(),
        duration: 1.25,
        tag_ids: Vec::new(),
        audio_bytes: Some(vec![1, 2, 3]),
        native_audio_path: None,
        audio_extension: Some("wav".into()),
    }
}

#[test]
fn history_mutations_use_the_injected_clock_and_id_generator() {
    let root = tempfile::tempdir().unwrap();
    let database = Arc::new(Database::open(root.path()).unwrap());
    let store = SqliteHistoryStore::with_environment(
        root.path().to_path_buf(),
        database,
        Arc::new(FixedClock(Ok(1_700_000_000_123))),
        Arc::new(SequenceIds::new([
            "history-id",
            "staging-id",
            "snapshot-id",
        ])),
    );

    let item = store.save_recording(recording_request()).unwrap();
    let snapshot = store
        .create_transcript_snapshot(HistoryCreateTranscriptSnapshotRequest {
            history_id: item.id.clone(),
            reason: TranscriptSnapshotReason::Polish,
            segments: Vec::new(),
        })
        .unwrap();

    assert_eq!(item.id, "history-id");
    assert_eq!(item.timestamp, 1_700_000_000_123);
    assert_eq!(snapshot.created_at, 1_700_000_000_123);
    assert_eq!(snapshot.id, "1700000000123-snapshot-id");
}

#[test]
fn clock_failure_stops_history_mutation_before_database_or_audio_write() {
    let root = tempfile::tempdir().unwrap();
    let database = Arc::new(Database::open(root.path()).unwrap());
    let store = SqliteHistoryStore::with_environment(
        root.path().to_path_buf(),
        database,
        Arc::new(FixedClock(Err(ClockError::Unavailable(
            "clock offline".into(),
        )))),
        Arc::new(SequenceIds::new(["unused"])),
    );

    let error = store.save_recording(recording_request()).unwrap_err();

    assert!(matches!(
        error,
        HistoryMutationError::Clock(ClockError::Unavailable(reason))
            if reason == "clock offline"
    ));
    assert!(store.list_items().unwrap().is_empty());
    let history_dir = root.path().join("history");
    assert!(
        !history_dir.exists()
            || std::fs::read_dir(history_dir)
                .unwrap()
                .all(|entry| entry.unwrap().file_name() == ".sona-history.lock")
    );
}
