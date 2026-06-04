use super::error::DashboardServiceError;
use super::models::ParsedTranscriptSegment;
use crate::repositories::history::HistoryItemRecord;

#[async_trait::async_trait]
pub trait HistoryRepository: Send + Sync {
    /// Reads all history items for dashboard aggregation
    async fn read_history_items(&self) -> Result<Vec<HistoryItemRecord>, DashboardServiceError>;

    /// Reads and parses transcript segments for a given history item's transcript path
    async fn read_transcript_segments(
        &self,
        transcript_path: &str,
    ) -> Result<Vec<ParsedTranscriptSegment>, DashboardServiceError>;
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
    ) -> Result<crate::integrations::llm::llm_usage::LlmUsageDashboardStats, DashboardServiceError>;
}
