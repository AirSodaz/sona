use crate::recovery::types::{RecoverySnapshot, RecoverySnapshotInput};

pub trait RecoverySnapshotStore: Send + Sync {
    fn load_snapshot_input(&self) -> Result<RecoverySnapshotInput, String>;
    fn save_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), String>;
}
