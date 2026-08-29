use std::sync::{Arc, Mutex};

use sona_archive::FsBackupArchiveRepository;

#[derive(Clone, Default)]
pub struct HistoryRepositoryState {
    /// Serializes history operations that combine SQLite state with filesystem
    /// side effects, such as audio promotion/removal and backup import/export.
    pub(crate) file_lock: Arc<Mutex<()>>,
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
