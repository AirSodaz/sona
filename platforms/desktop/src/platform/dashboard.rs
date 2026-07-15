use std::path::PathBuf;
use std::sync::Arc;

pub use sona_sqlite::SqliteDashboardService as AppDashboardService;

pub fn create_dashboard_service(
    app_local_data_dir: PathBuf,
    db: Arc<sona_sqlite::Database>,
) -> Arc<AppDashboardService> {
    Arc::new(sona_sqlite::create_dashboard_service(
        app_local_data_dir,
        db,
    ))
}
