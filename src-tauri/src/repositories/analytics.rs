use std::path::PathBuf;

pub struct AnalyticsRepositoryImpl {
    app_local_data_dir: PathBuf,
}

impl AnalyticsRepositoryImpl {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }
}

#[async_trait::async_trait]
impl crate::core::dashboard::ports::AnalyticsRepository for AnalyticsRepositoryImpl {
    async fn read_dashboard_stats(
        &self,
    ) -> Result<
        crate::integrations::llm::llm_usage::LlmUsageDashboardStats,
        crate::core::dashboard::error::DashboardServiceError,
    > {
        let stats =
            crate::integrations::llm::llm_usage::read_dashboard_stats(&self.app_local_data_dir);
        Ok(stats)
    }
}
