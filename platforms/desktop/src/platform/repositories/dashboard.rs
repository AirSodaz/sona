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

#[derive(Clone)]
pub struct DesktopDashboardState {
    service: Arc<std::sync::RwLock<Arc<AppDashboardService>>>,
}

impl DesktopDashboardState {
    pub fn new(service: Arc<AppDashboardService>) -> Self {
        Self {
            service: Arc::new(std::sync::RwLock::new(service)),
        }
    }

    pub fn current_service(&self) -> Result<Arc<AppDashboardService>, String> {
        let guard = self.service.read().map_err(|e| e.to_string())?;
        Ok(Arc::clone(&*guard))
    }

    pub fn reload(&self, new_service: Arc<AppDashboardService>) -> Result<(), String> {
        let mut guard = self.service.write().map_err(|e| e.to_string())?;
        *guard = new_service;
        Ok(())
    }
}
