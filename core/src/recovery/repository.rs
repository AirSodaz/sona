use crate::recovery::types::RecoverySnapshot;
use serde_json::Value;

pub trait RecoveryRepository: Send + Sync {
    fn ensure_ready(&self) -> Result<(), String>;
    fn load_snapshot(&self) -> Result<RecoverySnapshot, String>;
    fn save_snapshot(&self, items: Vec<Value>) -> Result<RecoverySnapshot, String>;
    fn persist_queue_snapshot_with_resolved_ids(
        &self,
        queue_items: Vec<Value>,
        resolved_ids: Vec<String>,
    ) -> Result<RecoverySnapshot, String>;
}
