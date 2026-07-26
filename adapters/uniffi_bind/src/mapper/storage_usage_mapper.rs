use sona_core::storage_usage::{
    AudioUsageCategory, DatabaseUsageCategory, FileUsageCategory, SQLiteIndexUsageEntry,
    SQLiteUsageSummary, StorageUsageCategories, StorageUsageSnapshot, WebviewCacheUsageCategory,
};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiStorageUsageSnapshotV1 {
    pub generated_at: String,
    pub total_bytes: u64,
    pub categories: FfiStorageUsageCategoriesV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiStorageUsageCategoriesV1 {
    pub audio: FfiAudioUsageCategoryV1,
    pub database: FfiDatabaseUsageCategoryV1,
    pub models: FfiFileUsageCategoryV1,
    pub temporary: FfiFileUsageCategoryV1,
    pub webview_cache: FfiWebviewCacheUsageCategoryV1,
    pub other: FfiFileUsageCategoryV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAudioUsageCategoryV1 {
    pub bytes: u64,
    pub history_audio_bytes: u64,
    pub speaker_sample_bytes: u64,
    pub file_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiDatabaseUsageCategoryV1 {
    pub bytes: u64,
    pub sqlite: FfiSqliteUsageSummaryV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiFileUsageCategoryV1 {
    pub bytes: u64,
    pub file_count: u64,
}

/// `bytes` stays optional because a host without a webview reports "unknown"
/// rather than zero, and `path` is absent on hosts that expose no cache dir.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiWebviewCacheUsageCategoryV1 {
    pub bytes: Option<u64>,
    pub clear_supported: bool,
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSqliteUsageSummaryV1 {
    pub main_db_bytes: u64,
    pub main_wal_bytes: u64,
    pub main_shm_bytes: u64,
    pub analytics_db_bytes: u64,
    pub analytics_wal_bytes: u64,
    pub analytics_shm_bytes: u64,
    pub data_bytes: u64,
    pub index_bytes: u64,
    pub free_page_bytes: u64,
    pub index_entries: Vec<FfiSqliteIndexUsageEntryV1>,
    pub dbstat_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSqliteIndexUsageEntryV1 {
    pub schema: String,
    pub name: String,
    pub bytes: u64,
}

impl From<StorageUsageSnapshot> for FfiStorageUsageSnapshotV1 {
    fn from(value: StorageUsageSnapshot) -> Self {
        Self {
            generated_at: value.generated_at,
            total_bytes: value.total_bytes,
            categories: value.categories.into(),
        }
    }
}

impl From<StorageUsageCategories> for FfiStorageUsageCategoriesV1 {
    fn from(value: StorageUsageCategories) -> Self {
        Self {
            audio: value.audio.into(),
            database: value.database.into(),
            models: value.models.into(),
            temporary: value.temporary.into(),
            webview_cache: value.webview_cache.into(),
            other: value.other.into(),
        }
    }
}

impl From<AudioUsageCategory> for FfiAudioUsageCategoryV1 {
    fn from(value: AudioUsageCategory) -> Self {
        Self {
            bytes: value.bytes,
            history_audio_bytes: value.history_audio_bytes,
            speaker_sample_bytes: value.speaker_sample_bytes,
            file_count: value.file_count,
        }
    }
}

impl From<DatabaseUsageCategory> for FfiDatabaseUsageCategoryV1 {
    fn from(value: DatabaseUsageCategory) -> Self {
        Self {
            bytes: value.bytes,
            sqlite: value.sqlite.into(),
        }
    }
}

impl From<FileUsageCategory> for FfiFileUsageCategoryV1 {
    fn from(value: FileUsageCategory) -> Self {
        Self {
            bytes: value.bytes,
            file_count: value.file_count,
        }
    }
}

impl From<WebviewCacheUsageCategory> for FfiWebviewCacheUsageCategoryV1 {
    fn from(value: WebviewCacheUsageCategory) -> Self {
        Self {
            bytes: value.bytes,
            clear_supported: value.clear_supported,
            path: value.path,
        }
    }
}

impl From<SQLiteUsageSummary> for FfiSqliteUsageSummaryV1 {
    fn from(value: SQLiteUsageSummary) -> Self {
        Self {
            main_db_bytes: value.main_db_bytes,
            main_wal_bytes: value.main_wal_bytes,
            main_shm_bytes: value.main_shm_bytes,
            analytics_db_bytes: value.analytics_db_bytes,
            analytics_wal_bytes: value.analytics_wal_bytes,
            analytics_shm_bytes: value.analytics_shm_bytes,
            data_bytes: value.data_bytes,
            index_bytes: value.index_bytes,
            free_page_bytes: value.free_page_bytes,
            index_entries: value.index_entries.into_iter().map(Into::into).collect(),
            dbstat_available: value.dbstat_available,
        }
    }
}

impl From<SQLiteIndexUsageEntry> for FfiSqliteIndexUsageEntryV1 {
    fn from(value: SQLiteIndexUsageEntry) -> Self {
        Self {
            schema: value.schema,
            name: value.name,
            bytes: value.bytes,
        }
    }
}
