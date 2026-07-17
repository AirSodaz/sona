use crate::recovery::types::{RecoverySnapshot, RecoverySnapshotInput};

use super::RecoveryError;

pub trait RecoverySnapshotStore: Send + Sync {
    fn load_snapshot_input(&self) -> Result<RecoverySnapshotInput, RecoveryError>;
    fn save_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), RecoveryError>;
}
