use super::mutation_repository::HistoryMutationRepository;
pub use super::query_repository::HistoryQueryError as HistoryStoreError;
use super::query_repository::HistoryQueryRepository;
use super::{HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistorySummaryPayload};

/// Combined history query + mutation port used by hosts and composite repositories.
pub trait HistoryStore: HistoryQueryRepository + HistoryMutationRepository {
    fn ensure_ready(&self) -> Result<(), HistoryStoreError>;
    fn load_summary(
        &self,
        history_id: &str,
    ) -> Result<Option<HistorySummaryPayload>, HistoryStoreError>;
    fn save_summary(
        &self,
        history_id: &str,
        summary_payload: HistorySummaryPayload,
    ) -> Result<(), HistoryStoreError>;
    fn delete_summary(&self, history_id: &str) -> Result<(), HistoryStoreError>;
    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, HistoryStoreError>;
    fn preview_audio_cleanup(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError>;
    fn cleanup_audio(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError>;
}
