use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sona_core::recovery::normalization::{
    empty_snapshot, recovered_item_from_queue_value_with_source_paths,
    recovered_item_from_saved_value_with_source_paths, snapshot_from_items_with_timestamp,
    snapshot_from_value_with_source_paths_at,
};
use sona_core::recovery::repository::RecoveryRepository;
use sona_core::recovery::types::{QUEUE_RECOVERY_FILE_NAME, RECOVERY_DIR_NAME, RecoverySnapshot};
use sona_runtime_fs::{FsSourcePathStatusProvider, write_json_pretty_atomic};

#[derive(Clone, Debug)]
pub struct FsRecoveryRepository {
    app_local_data_dir: PathBuf,
}

impl FsRecoveryRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn recovery_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(RECOVERY_DIR_NAME)
    }

    fn queue_recovery_path(&self) -> PathBuf {
        self.recovery_dir().join(QUEUE_RECOVERY_FILE_NAME)
    }
}

impl RecoveryRepository for FsRecoveryRepository {
    fn ensure_ready(&self) -> Result<(), String> {
        fs::create_dir_all(self.recovery_dir()).map_err(|error| error.to_string())?;
        let recovery_path = self.queue_recovery_path();
        if !recovery_path.exists() {
            write_json_pretty_atomic(&recovery_path, &empty_snapshot())?;
        }
        Ok(())
    }

    fn load_snapshot(&self) -> Result<RecoverySnapshot, String> {
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
        Ok(snapshot_from_value_with_source_paths_at(
            value,
            false,
            &FsSourcePathStatusProvider,
            now_ms(),
        ))
    }

    fn save_snapshot(&self, items: Vec<Value>) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let now = now_ms();
        let normalized_items = items
            .into_iter()
            .filter_map(|item| {
                recovered_item_from_saved_value_with_source_paths(
                    item,
                    now,
                    &FsSourcePathStatusProvider,
                )
            })
            .filter(|item| item.resolution == "pending")
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_items_with_timestamp(normalized_items, now);
        write_json_pretty_atomic(&self.queue_recovery_path(), &snapshot)?;
        Ok(snapshot)
    }

    fn persist_queue_snapshot_with_resolved_ids(
        &self,
        queue_items: Vec<Value>,
        resolved_ids: Vec<String>,
    ) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let now = now_ms();
        let mut observed_item_ids = resolved_ids
            .into_iter()
            .filter_map(|id| non_empty_string(&id))
            .collect::<HashSet<_>>();
        let mut items = queue_items
            .into_iter()
            .filter_map(|item| {
                collect_queue_recovery_ids(&item)
                    .into_iter()
                    .for_each(|id| {
                        observed_item_ids.insert(id);
                    });
                recovered_item_from_queue_value_with_source_paths(
                    item,
                    now,
                    &FsSourcePathStatusProvider,
                )
            })
            .collect::<Vec<_>>();
        let current_item_ids = items
            .iter()
            .map(|item| item.id.clone())
            .collect::<HashSet<_>>();
        observed_item_ids.extend(current_item_ids);
        let existing_items = self.load_snapshot()?.items;
        items.extend(
            existing_items.into_iter().filter(|item| {
                item.resolution == "pending" && !observed_item_ids.contains(&item.id)
            }),
        );
        let snapshot = snapshot_from_items_with_timestamp(items, now);
        write_json_pretty_atomic(&self.queue_recovery_path(), &snapshot)?;
        Ok(snapshot)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn collect_queue_recovery_ids(value: &Value) -> Vec<String> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    ["id", "recoveryId"]
        .iter()
        .filter_map(|key| object.get(*key).and_then(Value::as_str))
        .filter_map(non_empty_string)
        .collect()
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
