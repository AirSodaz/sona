use std::path::PathBuf;
use std::sync::Arc;

use sona_core::storage_usage::{
    StorageUsageError, StorageUsageMeasurements, StorageUsageRepository,
};

use crate::Database;
use crate::storage_usage::SqliteStorageUsageRepository;

#[derive(Clone, Debug)]
pub struct LazySqliteStorageUsageRepository {
    app_local_data_dir: PathBuf,
}

impl LazySqliteStorageUsageRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }
}

impl StorageUsageRepository for LazySqliteStorageUsageRepository {
    fn collect_measurements(&self) -> Result<StorageUsageMeasurements, StorageUsageError> {
        let database = Database::open_read_only_with_analytics(&self.app_local_data_dir)
            .map_err(|error| StorageUsageError::Repository(error.to_string()))?;
        SqliteStorageUsageRepository::new(self.app_local_data_dir.clone(), Arc::new(database))
            .collect_measurements()
    }
}
