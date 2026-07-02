pub struct AnalyticsRepositoryImpl;

impl Default for AnalyticsRepositoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyticsRepositoryImpl {
    pub fn new() -> Self {
        Self
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
        let stats = crate::integrations::llm_usage_sqlite::read_dashboard_stats(
            crate::core::database::Database::global().map_err(|e| {
                crate::core::dashboard::error::DashboardServiceError::AnalyticsRepository(e)
            })?,
        )
        .map_err(crate::core::dashboard::error::DashboardServiceError::AnalyticsRepository)?;
        Ok(stats)
    }
}
