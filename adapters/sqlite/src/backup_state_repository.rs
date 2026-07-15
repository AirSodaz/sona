use std::path::{Path, PathBuf};
use std::sync::Arc;

use sona_core::backup::{
    BackupApplyResult, BackupDataset, BackupError, BackupRestoreDataset, BackupStateRepository,
};

use crate::{Database, SqliteBackupStateRepository, validate_backup_restore_dataset};

#[derive(Clone, Debug)]
pub struct LazySqliteBackupStateRepository {
    app_local_data_dir: PathBuf,
}

impl LazySqliteBackupStateRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn with_repository<T>(
        &self,
        operation: impl FnOnce(&SqliteBackupStateRepository) -> Result<T, BackupError>,
    ) -> Result<T, BackupError> {
        ensure_existing_directory(&self.app_local_data_dir)?;
        let database = Database::open(&self.app_local_data_dir)
            .map_err(|error| BackupError::State(error.to_string()))?;
        let repository =
            SqliteBackupStateRepository::new(self.app_local_data_dir.clone(), Arc::new(database));
        operation(&repository)
    }
}

impl BackupStateRepository for LazySqliteBackupStateRepository {
    fn snapshot(&self) -> Result<BackupDataset, BackupError> {
        self.with_repository(BackupStateRepository::snapshot)
    }

    fn replace_all(&self, dataset: BackupRestoreDataset) -> Result<BackupApplyResult, BackupError> {
        validate_backup_restore_dataset(&dataset)?;
        self.with_repository(|repository| repository.replace_all(dataset))
    }
}

fn ensure_existing_directory(path: &Path) -> Result<(), BackupError> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(BackupError::State(format!(
            "Application data directory does not exist or is not a directory: {}",
            path.display()
        )))
    }
}
