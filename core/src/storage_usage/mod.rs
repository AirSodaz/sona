mod models;
mod ports;
mod service;

pub use models::{
    AudioUsageCategory, DatabaseUsageCategory, FileUsageCategory, SQLiteIndexUsageEntry,
    SQLiteUsageSummary, StorageUsageCategories, StorageUsageMeasurements, StorageUsageSnapshot,
    WebviewBrowsingDataClearResult, WebviewCacheUsageCategory,
};
pub use ports::StorageUsageRepository;
pub use service::{StorageUsageService, build_webview_clear_result};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StorageUsageError {
    #[error(transparent)]
    FileSystem(#[from] crate::ports::fs::FileSystemError),

    #[error("Storage usage database error: {0}")]
    Database(String),

    #[error("Storage usage repository error: {0}")]
    Repository(String),
}
