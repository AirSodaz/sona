use super::{StorageUsageError, StorageUsageMeasurements};

pub trait StorageUsageRepository: Send + Sync {
    fn collect_measurements(&self) -> Result<StorageUsageMeasurements, StorageUsageError>;
}
