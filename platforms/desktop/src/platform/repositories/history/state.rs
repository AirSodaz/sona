use std::sync::{Arc, Mutex};

use sona_archive::FsBackupArchiveRepository;

#[derive(Clone, Default)]
pub struct HistoryRepositoryState {
    /// Serializes history operations that combine SQLite state with filesystem
    /// side effects, such as audio promotion/removal and backup import/export.
    pub(crate) file_lock: Arc<Mutex<()>>,
}

impl HistoryRepositoryState {
    pub(crate) fn is_file_task_active(&self) -> bool {
        self.file_lock.try_lock().is_err()
    }
}

#[derive(Clone, Default)]
pub struct PreparedBackupImportState {
    archive: Arc<FsBackupArchiveRepository>,
}

impl PreparedBackupImportState {
    pub(crate) fn archive(&self) -> Arc<FsBackupArchiveRepository> {
        Arc::clone(&self.archive)
    }
}
