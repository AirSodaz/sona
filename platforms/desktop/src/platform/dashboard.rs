use crate::platform::history_repository::SqliteHistoryStore;
use sona_core::dashboard::DashboardService;
use sona_sqlite::analytics::SqliteAnalyticsRepository;
use sona_sqlite::project::SqliteProjectRepository;
use std::path::PathBuf;
use std::sync::Arc;

pub type AppDashboardService =
    DashboardService<SqliteHistoryStore, SqliteProjectRepository, SqliteAnalyticsRepository>;

pub fn create_dashboard_service(
    app_local_data_dir: PathBuf,
    db: Arc<sona_sqlite::Database>,
) -> Arc<AppDashboardService> {
    let history_repo = Arc::new(SqliteHistoryStore::new(app_local_data_dir, Arc::clone(&db)));
    let project_repo = Arc::new(SqliteProjectRepository::new(Arc::clone(&db)));
    let analytics_repo = Arc::new(SqliteAnalyticsRepository::new(Arc::clone(&db)));

    Arc::new(AppDashboardService::new(
        history_repo,
        project_repo,
        analytics_repo,
    ))
}
