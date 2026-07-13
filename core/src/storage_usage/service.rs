use std::sync::Arc;

use super::{
    AudioUsageCategory, StorageUsageCategories, StorageUsageError, StorageUsageRepository,
    StorageUsageSnapshot, WebviewBrowsingDataClearResult,
};

pub struct StorageUsageService<R>
where
    R: StorageUsageRepository,
{
    repository: Arc<R>,
}

impl<R> StorageUsageService<R>
where
    R: StorageUsageRepository,
{
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }

    pub fn load_snapshot_at(
        &self,
        generated_at: String,
    ) -> Result<StorageUsageSnapshot, StorageUsageError> {
        let measurements = self.repository.collect_measurements()?;
        let audio = AudioUsageCategory {
            bytes: measurements
                .history_audio
                .bytes
                .saturating_add(measurements.speaker_samples.bytes),
            history_audio_bytes: measurements.history_audio.bytes,
            speaker_sample_bytes: measurements.speaker_samples.bytes,
            file_count: measurements
                .history_audio
                .file_count
                .saturating_add(measurements.speaker_samples.file_count),
        };
        let categories = StorageUsageCategories {
            audio,
            database: measurements.database,
            models: measurements.models,
            temporary: measurements.temporary,
            webview_cache: measurements.webview_cache,
            other: measurements.other,
        };
        let total_bytes = [
            categories.audio.bytes,
            categories.database.bytes,
            categories.models.bytes,
            categories.temporary.bytes,
            categories.webview_cache.bytes.unwrap_or(0),
            categories.other.bytes,
        ]
        .into_iter()
        .fold(0_u64, u64::saturating_add);

        Ok(StorageUsageSnapshot {
            generated_at,
            total_bytes,
            categories,
        })
    }
}

pub fn build_webview_clear_result(
    before_bytes: Option<u64>,
    after_bytes: Option<u64>,
) -> WebviewBrowsingDataClearResult {
    WebviewBrowsingDataClearResult {
        before_bytes,
        after_bytes,
        clear_requested: true,
    }
}
