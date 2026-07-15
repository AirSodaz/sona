use std::path::PathBuf;
use std::sync::Arc;

use sona_core::storage_usage::{
    StorageUsageError, StorageUsageMeasurements, StorageUsageRepository, StorageUsageService,
    StorageUsageSnapshot,
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

pub fn load_storage_usage_snapshot(
    app_local_data_dir: PathBuf,
    generated_at: String,
) -> Result<StorageUsageSnapshot, StorageUsageError> {
    let repository = LazySqliteStorageUsageRepository::new(app_local_data_dir);
    StorageUsageService::new(Arc::new(repository)).load_snapshot_at(generated_at)
}

pub fn load_storage_usage_snapshot_with_database(
    app_local_data_dir: PathBuf,
    database: Arc<Database>,
    generated_at: String,
) -> Result<StorageUsageSnapshot, StorageUsageError> {
    let repository = SqliteStorageUsageRepository::new(app_local_data_dir, database);
    StorageUsageService::new(Arc::new(repository)).load_snapshot_at(generated_at)
}
