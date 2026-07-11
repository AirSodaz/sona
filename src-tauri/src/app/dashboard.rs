use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

pub use crate::platform::dashboard::AppDashboardService;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshotRequest {
    pub deep: bool,
}

pub async fn get_dashboard_snapshot(
    service: State<'_, Arc<AppDashboardService>>,
    request: DashboardSnapshotRequest,
) -> Result<Value, String> {
    let time = crate::platform::time::dashboard_snapshot_time_now();
    let snapshot = service
        .build_snapshot_at(request.deep, time)
        .await
        .map_err(|error| error.to_string())?;

    serde_json::to_value(snapshot).map_err(|error| error.to_string())
}
