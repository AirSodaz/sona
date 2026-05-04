use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::history_repository::fs_utils::write_json_pretty_atomic;

use super::normalization::{
    empty_snapshot, now_ms, recovered_item_from_queue_value, recovered_item_from_saved_value,
    snapshot_from_items, snapshot_from_value,
};
use super::types::{RecoverySnapshot, QUEUE_RECOVERY_FILE_NAME, RECOVERY_DIR_NAME};

#[derive(Clone, Debug)]
pub struct RecoveryRepository {
    app_local_data_dir: PathBuf,
}

impl RecoveryRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn recovery_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(RECOVERY_DIR_NAME)
    }

    fn queue_recovery_path(&self) -> PathBuf {
        self.recovery_dir().join(QUEUE_RECOVERY_FILE_NAME)
    }

    pub fn ensure_ready(&self) -> Result<(), String> {
        fs::create_dir_all(self.recovery_dir()).map_err(|error| error.to_string())?;
        let recovery_path = self.queue_recovery_path();
        if !recovery_path.exists() {
            write_json_pretty_atomic(&recovery_path, &empty_snapshot())?;
        }
        Ok(())
    }

    pub fn load_snapshot(&self) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let content =
            fs::read_to_string(self.queue_recovery_path()).map_err(|error| error.to_string())?;
        let value = match serde_json::from_str::<Value>(&content) {
            Ok(value) => value,
            Err(error) => {
                log::error!("[Recovery] Failed to parse recovery snapshot: {}", error);
                return Ok(empty_snapshot());
            }
        };

        Ok(snapshot_from_value(value, false))
    }

    pub fn save_snapshot(&self, items: Vec<Value>) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let now = now_ms();
        let normalized_items = items
            .into_iter()
            .filter_map(|item| recovered_item_from_saved_value(item, now))
            .filter(|item| item.resolution == "pending")
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_items(normalized_items);
        write_json_pretty_atomic(&self.queue_recovery_path(), &snapshot)?;
        Ok(snapshot)
    }

    pub fn persist_queue_snapshot(
        &self,
        queue_items: Vec<Value>,
    ) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let now = now_ms();
        let items = queue_items
            .into_iter()
            .filter_map(|item| recovered_item_from_queue_value(item, now))
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_items(items);
        write_json_pretty_atomic(&self.queue_recovery_path(), &snapshot)?;
        Ok(snapshot)
    }
}
