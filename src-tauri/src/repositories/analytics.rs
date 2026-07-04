use std::sync::Arc;

pub struct AnalyticsRepositoryImpl {
    db: Arc<crate::core::database::Database>,
}

impl AnalyticsRepositoryImpl {
    pub fn new(db: Arc<crate::core::database::Database>) -> Self {
        Self { db }
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
        let stats = crate::integrations::llm_usage_sqlite::read_dashboard_stats(self.db.as_ref())
            .map_err(
            crate::core::dashboard::error::DashboardServiceError::AnalyticsRepository,
        )?;
        Ok(stats)
    }
}
