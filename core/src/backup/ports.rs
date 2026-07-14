use super::{
    BackupApplyResult, BackupDataset, BackupError, BackupManifest, BackupRestoreDataset,
    PreparedBackupImport, PreparedBackupSession,
};

pub trait BackupArchivePort: Send + Sync {
    fn write_archive(
        &self,
        archive_path: &str,
        manifest: &BackupManifest,
        dataset: &BackupDataset,
    ) -> Result<(), BackupError>;
    fn prepare_import(&self, archive_path: &str) -> Result<PreparedBackupImport, BackupError>;
    fn load_prepared(&self, import_id: &str) -> Result<PreparedBackupSession, BackupError>;
    fn dispose_prepared(&self, import_id: &str) -> Result<(), BackupError>;
}

pub trait BackupStateRepository: Send + Sync {
    fn snapshot(&self) -> Result<BackupDataset, BackupError>;
    fn replace_all(&self, dataset: BackupRestoreDataset) -> Result<BackupApplyResult, BackupError>;
}
