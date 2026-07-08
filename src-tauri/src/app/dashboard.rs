use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

use crate::platform::history_repository::SqliteHistoryStore;
use sona_core::dashboard::{DashboardService, DashboardSnapshotTime};
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
    let now = chrono::Utc::now();
    let time = DashboardSnapshotTime {
        generated_at: now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        today: now.with_timezone(&chrono::Local).date_naive(),
    };
    let snapshot = service
        .build_snapshot_at(request.deep, time)
        .await
        .map_err(|error| error.to_string())?;

    serde_json::to_value(snapshot).map_err(|error| error.to_string())
}
