use serde::Deserialize;
use sona_core::dashboard::models::DashboardSnapshotDomainModel;
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
) -> Result<DashboardSnapshotDomainModel, String> {
    let time = sona_runtime_fs::dashboard_snapshot_time_now();
    let snapshot = service
        .build_snapshot_at(request.deep, time)
        .await
        .map_err(|error| error.to_string())?;
    sona_ts_bind::validate_dashboard_snapshot_for_typescript(&snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}
