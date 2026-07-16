use std::path::PathBuf;
use std::sync::Arc;

use sona_core::dashboard::models::DashboardSnapshotDomainModel;
use sona_core::dashboard::{DashboardService, DashboardServiceError, DashboardSnapshotTime};

use crate::analytics::SqliteAnalyticsRepository;
use crate::{Database, SqliteHistoryStore, SqliteTagRepository};

pub type SqliteDashboardService =
    DashboardService<SqliteHistoryStore, SqliteTagRepository, SqliteAnalyticsRepository>;

pub fn create_dashboard_service(
    app_local_data_dir: PathBuf,
    database: Arc<Database>,
) -> SqliteDashboardService {
    DashboardService::new(
        Arc::new(SqliteHistoryStore::new(
            app_local_data_dir,
            Arc::clone(&database),
        )),
        Arc::new(SqliteTagRepository::new(Arc::clone(&database))),
        Arc::new(SqliteAnalyticsRepository::new(database)),
    )
}

pub async fn load_dashboard_snapshot(
    app_local_data_dir: PathBuf,
    deep: bool,
    time: DashboardSnapshotTime,
) -> Result<DashboardSnapshotDomainModel, DashboardServiceError> {
    let database = Database::open_read_only_with_analytics(&app_local_data_dir)
        .map(Arc::new)
        .map_err(|error| DashboardServiceError::Internal(error.to_string()))?;
    create_dashboard_service(app_local_data_dir, database)
        .build_snapshot_at(deep, time)
        .await
}
