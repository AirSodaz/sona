use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsageSnapshot {
    pub generated_at: String,
    pub total_bytes: u64,
    pub categories: StorageUsageCategories,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsageCategories {
    pub audio: AudioUsageCategory,
    pub database: DatabaseUsageCategory,
    pub models: FileUsageCategory,
    pub temporary: FileUsageCategory,
    pub webview_cache: WebviewCacheUsageCategory,
    pub other: FileUsageCategory,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioUsageCategory {
    pub bytes: u64,
    pub history_audio_bytes: u64,
    pub speaker_sample_bytes: u64,
    pub file_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseUsageCategory {
    pub bytes: u64,
    pub sqlite: SQLiteUsageSummary,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUsageCategory {
    pub bytes: u64,
    pub file_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewCacheUsageCategory {
    pub bytes: Option<u64>,
    pub clear_supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SQLiteUsageSummary {
    pub main_db_bytes: u64,
    pub main_wal_bytes: u64,
    pub main_shm_bytes: u64,
    pub analytics_db_bytes: u64,
    pub analytics_wal_bytes: u64,
    pub analytics_shm_bytes: u64,
    pub data_bytes: u64,
    pub index_bytes: u64,
    pub free_page_bytes: u64,
    pub index_entries: Vec<SQLiteIndexUsageEntry>,
    pub dbstat_available: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SQLiteIndexUsageEntry {
    pub schema: String,
    pub name: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StorageUsageMeasurements {
    pub history_audio: FileUsageCategory,
    pub speaker_samples: FileUsageCategory,
    pub database: DatabaseUsageCategory,
    pub models: FileUsageCategory,
    pub temporary: FileUsageCategory,
    pub webview_cache: WebviewCacheUsageCategory,
    pub other: FileUsageCategory,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewBrowsingDataClearResult {
    pub before_bytes: Option<u64>,
    pub after_bytes: Option<u64>,
    pub clear_requested: bool,
}
