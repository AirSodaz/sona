use crate::services::DesktopServices;
use sona_core::sync::{SyncConflictResolution, SyncConflictSummary};

pub async fn get_sync_status(services: &DesktopServices) -> Result<serde_json::Value, String> {
    let status = services.sync.get_status().await?;
    serde_json::to_value(&status).map_err(|e| e.to_string())
}

pub async fn trigger_sync(services: &DesktopServices) -> Result<serde_json::Value, String> {
    let result = services.sync.run_now().await?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

pub async fn list_sync_conflicts(
    services: &DesktopServices,
) -> Result<Vec<SyncConflictSummary>, String> {
    services.sync.list_conflicts().await
}

pub async fn resolve_sync_conflict(
    services: &DesktopServices,
    conflict_id: String,
    resolution: String,
) -> Result<bool, String> {
    let res = match resolution.trim().to_lowercase().as_str() {
        "keep_current" | "keepcurrent" => SyncConflictResolution::KeepCurrent,
        "use_conflicting" | "useconflicting" => SyncConflictResolution::UseConflicting,
        "keep_both" | "keepboth" => SyncConflictResolution::KeepBoth,
        other => {
            return Err(format!(
                "INVALID_RESOLUTION: Unsupported conflict resolution '{other}', expected 'keep_current', 'use_conflicting', or 'keep_both'"
            ));
        }
    };
    services.sync.resolve_conflict(&conflict_id, res).await?;
    Ok(true)
}
