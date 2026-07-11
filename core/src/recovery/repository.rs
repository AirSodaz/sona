use crate::recovery::types::RecoverySnapshot;
use serde_json::Value;

pub trait RecoverySnapshotStore: Send + Sync {
    fn load_snapshot_value(&self) -> Result<Value, String>;
    fn save_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), String>;
}
