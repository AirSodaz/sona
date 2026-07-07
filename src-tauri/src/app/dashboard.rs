use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

use crate::repositories::history::SqliteHistoryStore;
use sona_core::dashboard::DashboardService;
use sona_sqlite::analytics::SqliteAnalyticsRepository;
use sona_sqlite::project::SqliteProjectRepository;

pub type AppDashboardService =
    DashboardService<SqliteHistoryStore, SqliteProjectRepository, SqliteAnalyticsRepository>;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshotRequest {
    pub deep: bool,
}

pub async fn get_dashboard_snapshot(
    service: State<'_, Arc<AppDashboardService>>,
    request: DashboardSnapshotRequest,
) -> Result<Value, String> {
    let snapshot = service
        .build_snapshot(request.deep)
        .await
        .map_err(|error| error.to_string())?;

    serde_json::to_value(snapshot).map_err(|error| error.to_string())
}
