use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::sync::{SyncError, SyncLocalRepository, SyncPresetV1};
use sona_sqlite::{Database, SqliteSyncRepository};

struct FixedClock(Result<u64, ClockError>);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        self.0.clone()
    }
}

struct SequenceClock(Mutex<VecDeque<Result<u64, ClockError>>>);

impl SequenceClock {
    fn new(values: impl IntoIterator<Item = Result<u64, ClockError>>) -> Self {
        Self(Mutex::new(values.into_iter().collect()))
    }
}

impl UnixMillisClock for SequenceClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        self.0.lock().unwrap().pop_front().unwrap()
    }
}

#[test]
fn sync_initialization_uses_the_injected_clock_for_seed_operations() {
    let root = tempfile::tempdir().unwrap();
    let database = Arc::new(Database::open(root.path()).unwrap());
    database
        .with_rw_transaction(|transaction| {
            transaction.execute(
                "INSERT INTO history_items (id, timestamp)
                 VALUES ('clock-history', 1)",
                [],
            )?;
            transaction.execute(
                "INSERT INTO history_transcripts (history_id, segments)
                 VALUES ('clock-history', '[]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

    let repository = SqliteSyncRepository::initialize(
        database,
        Arc::new(FixedClock(Ok(42_424))),
        "clock-vault",
        "clock-device",
        SyncPresetV1::Content,
    )
    .unwrap();

    let pending = repository
        .load_pending_operations(SyncPresetV1::Content, 256, usize::MAX)
        .unwrap();
    assert!(pending.iter().any(|operation| operation.entity.kind
        == sona_core::sync::SyncEntityKind::HistoryTranscript
        && operation.version.clock.physical_ms == 42_424));
}

#[test]
fn sync_clock_failure_stops_initialization_before_local_state_is_written() {
    let root = tempfile::tempdir().unwrap();
    let database = Arc::new(Database::open(root.path()).unwrap());

    let error = match SqliteSyncRepository::initialize(
        Arc::clone(&database),
        Arc::new(FixedClock(Err(ClockError::Unavailable(
            "sync clock offline".to_string(),
        )))),
        "clock-vault",
        "clock-device",
        SyncPresetV1::Content,
    ) {
        Ok(_) => panic!("sync initialization unexpectedly succeeded"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        SyncError::Clock(ClockError::Unavailable(reason)) if reason == "sync clock offline"
    ));
    let state_exists = database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sync_state WHERE id = 1)",
                    [],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(sona_sqlite::DatabaseError::QueryError)
        })
        .unwrap();
    assert!(!state_exists);
}

#[test]
fn sync_clock_failure_rolls_back_a_shrinking_preset_change() {
    let root = tempfile::tempdir().unwrap();
    let database = Arc::new(Database::open(root.path()).unwrap());
    database
        .with_rw_transaction(|transaction| {
            transaction.execute(
                "INSERT INTO automation_rules (id, name) VALUES ('clock-rule', 'Clock Rule')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    let clock = Arc::new(SequenceClock::new([
        Ok(100),
        Err(ClockError::Unavailable(
            "sync preset clock offline".to_string(),
        )),
    ]));
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&database),
        clock,
        "clock-vault",
        "clock-device",
        SyncPresetV1::Full,
    )
    .unwrap();
    let pending_before = repository
        .load_pending_operations(SyncPresetV1::Full, 256, usize::MAX)
        .unwrap();

    let error = repository
        .change_preset(SyncPresetV1::Standard, true)
        .unwrap_err();

    assert!(matches!(
        error,
        SyncError::Clock(ClockError::Unavailable(reason))
            if reason == "sync preset clock offline"
    ));
    assert_eq!(
        repository.load_runtime_state().unwrap().preset,
        SyncPresetV1::Full
    );
    assert_eq!(
        repository
            .load_pending_operations(SyncPresetV1::Full, 256, usize::MAX)
            .unwrap(),
        pending_before
    );
}
