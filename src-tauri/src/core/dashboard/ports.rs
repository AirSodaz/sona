use super::error::DashboardServiceError;

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
    ) -> Result<crate::core::dashboard::models::LlmUsageDashboardStats, DashboardServiceError>;
}
