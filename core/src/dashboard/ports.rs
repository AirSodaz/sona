use super::error::DashboardServiceError;
use crate::history::HistoryItemRecord;
use crate::transcript::TranscriptSegment;

#[async_trait::async_trait]
pub trait HistoryRepository: Send + Sync {
    /// Lists history records used by dashboard content aggregation.
    async fn list_items(&self) -> Result<Vec<HistoryItemRecord>, DashboardServiceError>;

    /// Loads transcript segments for deep dashboard aggregation.
    async fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, DashboardServiceError>;
}

#[async_trait::async_trait]
pub trait ProjectRepository: Send + Sync {
    /// Counts the total number of projects
    async fn count_projects(&self) -> Result<u64, DashboardServiceError>;
}

#[async_trait::async_trait]
pub trait AnalyticsRepository: Send + Sync {
    /// Reads the dashboard stats for LLM usage
    async fn read_dashboard_stats(
        &self,
    ) -> Result<crate::dashboard::models::LlmUsageDashboardStats, DashboardServiceError>;
}
