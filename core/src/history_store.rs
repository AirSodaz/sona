use crate::history::mutation_repository::HistoryMutationRepository;
pub use crate::history::query_repository::HistoryQueryError as HistoryStoreError;
use crate::history::query_repository::HistoryQueryRepository;
use crate::history::{
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistorySummaryPayload,
};

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
