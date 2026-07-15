use serde_json::Value;
use sona_core::recovery::normalization::empty_snapshot;
use sona_core::recovery::repository::RecoverySnapshotStore;
use sona_core::recovery::service::RecoveryService;
use sona_core::recovery::types::{QUEUE_RECOVERY_FILE_NAME, RECOVERY_DIR_NAME, RecoverySnapshot};
use sona_runtime_fs::{
    FsSourcePathStatusProvider, SystemClock, ensure_directory_exists, write_json_pretty_atomic,
};
use std::fs;
use std::path::PathBuf;

pub struct FsRecoveryAdapter {
    store: FsRecoverySnapshotStore,
    source_paths: FsSourcePathStatusProvider,
    clock: SystemClock,
}

impl FsRecoveryAdapter {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            store: FsRecoverySnapshotStore::new(app_local_data_dir),
            source_paths: FsSourcePathStatusProvider,
            clock: SystemClock,
        }
    }

    pub fn load_snapshot(&self) -> Result<RecoverySnapshot, String> {
        self.service().load_snapshot()
    }

    pub fn load_snapshot_at(&self, now_ms: u64) -> Result<RecoverySnapshot, String> {
        self.service().load_snapshot_at(now_ms)
    }

    pub fn save_snapshot(&self, items: Vec<Value>) -> Result<RecoverySnapshot, String> {
        self.service().save_snapshot(items)
    }

    pub fn save_snapshot_at(
        &self,
        items: Vec<Value>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, String> {
        self.service().save_snapshot_at(items, now_ms)
    }

    pub fn persist_queue_snapshot(
        &self,
        queue_items: Vec<Value>,
        resolved_ids: Vec<String>,
    ) -> Result<RecoverySnapshot, String> {
        self.service()
            .persist_queue_snapshot(queue_items, resolved_ids)
    }

    pub fn persist_queue_snapshot_at(
        &self,
        queue_items: Vec<Value>,
        resolved_ids: Vec<String>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, String> {
        self.service()
            .persist_queue_snapshot_at(queue_items, resolved_ids, now_ms)
    }

    fn service(&self) -> RecoveryService<'_> {
        RecoveryService::new(&self.store, &self.source_paths, &self.clock)
    }
}

pub struct FsRecoverySnapshotStore {
    app_local_data_dir: PathBuf,
}

impl FsRecoverySnapshotStore {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    pub fn recovery_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(RECOVERY_DIR_NAME)
    }

    pub fn queue_recovery_path(&self) -> PathBuf {
        self.recovery_dir().join(QUEUE_RECOVERY_FILE_NAME)
    }
}

impl RecoverySnapshotStore for FsRecoverySnapshotStore {
    fn load_snapshot_value(&self) -> Result<Value, String> {
        let recovery_dir = self.recovery_dir();
        ensure_directory_exists(&recovery_dir)?;
        let recovery_path = self.queue_recovery_path();
        if !recovery_path.exists() {
            write_json_pretty_atomic(&recovery_path, &empty_snapshot())?;
        }

        let content = fs::read_to_string(recovery_path).map_err(|error| error.to_string())?;
        match serde_json::from_str::<Value>(&content) {
            Ok(value) => Ok(value),
            Err(error) => {
                log::error!("[Recovery] Failed to parse recovery snapshot: {}", error);
                Ok(Value::Null)
            }
        }
    }

    fn save_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), String> {
        write_json_pretty_atomic(&self.queue_recovery_path(), snapshot)
    }
}
