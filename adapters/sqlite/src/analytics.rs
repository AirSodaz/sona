use std::sync::Arc;

use sona_core::dashboard::{
    error::DashboardServiceError, models::LlmUsageDashboardStats, ports::AnalyticsRepository,
};

use crate::Database;

pub struct SqliteAnalyticsRepository {
    db: Arc<Database>,
}

impl SqliteAnalyticsRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn read_dashboard_stats_sync(&self) -> Result<LlmUsageDashboardStats, DashboardServiceError> {
        crate::llm_usage::read_dashboard_stats(self.db.as_ref())
            .map_err(|error| DashboardServiceError::AnalyticsRepository(error.to_string()))
    }
}

#[async_trait::async_trait]
impl AnalyticsRepository for SqliteAnalyticsRepository {
    async fn read_dashboard_stats(&self) -> Result<LlmUsageDashboardStats, DashboardServiceError> {
        self.read_dashboard_stats_sync()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_stats_default_to_empty_usage() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let repository = SqliteAnalyticsRepository::new(db);

        let stats = repository.read_dashboard_stats_sync().unwrap();

        assert_eq!(stats.totals.call_count, 0);
        assert!(stats.by_provider.is_empty());
        assert!(stats.by_category.is_empty());
        assert!(
            stats
                .recent_daily
                .iter()
                .all(|point| point.stats.call_count == 0)
        );
    }
}
