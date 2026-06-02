use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::types::PreparedBackupImportSnapshot;

#[derive(Clone, Default)]
pub struct HistoryRepositoryState {
    pub(super) lock: Arc<Mutex<()>>,
}

#[derive(Clone, Default)]
pub struct PreparedBackupImportState {
    inner: Arc<Mutex<HashMap<String, PreparedBackupImportSnapshot>>>,
}

impl PreparedBackupImportState {
    pub(super) fn insert(
        &self,
        import_id: String,
        snapshot: PreparedBackupImportSnapshot,
    ) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|error| error.to_string())?;
        guard.insert(import_id, snapshot);
        Ok(())
    }

    pub(super) fn get(
        &self,
        import_id: &str,
    ) -> Result<Option<PreparedBackupImportSnapshot>, String> {
        let guard = self.inner.lock().map_err(|error| error.to_string())?;
        Ok(guard.get(import_id).cloned())
    }

    pub(super) fn remove(
        &self,
        import_id: &str,
    ) -> Result<Option<PreparedBackupImportSnapshot>, String> {
        let mut guard = self.inner.lock().map_err(|error| error.to_string())?;
        Ok(guard.remove(import_id))
    }
}
