use serde_json::Value;
use sona_core::recovery::normalization::empty_snapshot;
use sona_core::recovery::repository::RecoverySnapshotStore;
use sona_core::recovery::types::{QUEUE_RECOVERY_FILE_NAME, RECOVERY_DIR_NAME, RecoverySnapshot};
use sona_runtime_fs::{ensure_directory_exists, write_json_pretty_atomic};
use std::fs;
use std::path::PathBuf;

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
