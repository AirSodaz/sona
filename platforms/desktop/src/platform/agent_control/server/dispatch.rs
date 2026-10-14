use crate::platform::agent_control::facade::{
    AgentControlFacade, CreateProjectRequest, DownloadPresetModelRequest, EditTranscriptRequest,
    ExportTranscriptRequest, PatchSegmentsRequest, QueryHistoryRequest, QueryTrashRequest,
    SaveSummaryRequest, StartRecordingRequest, StopRecordingRequest, TranscribeFileRequest,
    UpdateHistoryMetaRequest, UpdateProjectRequest, UpdateTranslationsRequest,
};
use serde::{Deserialize, Serialize};
use sona_core::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionCategory {
    ReadOnly,
    Recording,
    TranscriptEdit,
    Destructive,
    SettingsUpdate,
    ModelManagement,
}

impl ToolPermissionCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Recording => "recording",
            Self::TranscriptEdit => "transcript_edit",
            Self::Destructive => "destructive",
            Self::SettingsUpdate => "settings_update",
            Self::ModelManagement => "model_management",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCatalogItem {
    pub name: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub allowed: bool,
}

pub fn classify_tool(method: &str, params: Option<&serde_json::Value>) -> ToolPermissionCategory {
    let clean_method = method
        .strip_prefix("sona_")
        .or_else(|| method.strip_prefix("agent_"))
        .unwrap_or(method);

    match clean_method {
        "stop_recording" => {
            if params
                .and_then(|p| p.get("discard"))
                .and_then(|d| d.as_bool())
                .unwrap_or(false)
            {
                ToolPermissionCategory::Destructive
            } else {
                ToolPermissionCategory::Recording
            }
        }
        "control_recording" => {
            let action = params
                .and_then(|p| p.get("action"))
                .and_then(|a| a.as_str());
            if action == Some("discard") {
                ToolPermissionCategory::Destructive
            } else {
                ToolPermissionCategory::Recording
            }
        }
        "start_recording" | "pause_recording" | "resume_recording" => {
            ToolPermissionCategory::Recording
        }
        "delete_history" | "delete_project" | "delete_preset_model" | "delete_summary" => {
            ToolPermissionCategory::Destructive
        }
        "update_setting" => ToolPermissionCategory::SettingsUpdate,
        "download_preset_model" | "cancel_download" => ToolPermissionCategory::ModelManagement,
        "edit_transcript"
        | "patch_segments"
        | "update_translations"
        | "edit_translations"
        | "revert_transcript_snapshot"
        | "update_history_meta"
        | "save_summary"
        | "edit_summary"
        | "restore_history"
        | "transcribe_file"
        | "cancel_batch_task"
        | "export_transcript"
        | "create_project"
        | "update_project"
        | "set_active_project"
        | "trigger_sync"
        | "resolve_sync_conflict" => ToolPermissionCategory::TranscriptEdit,
        "get_client_state"
        | "launch_desktop"
        | "focus_window"
        | "list_audio_devices"
        | "list_transcript_snapshots"
        | "read_transcript"
        | "query_history"
        | "get_history_detail"
        | "query_trash"
        | "load_summary"
        | "list_projects"
        | "get_project"
        | "list_preset_models"
        | "get_model_catalog"
        | "get_settings"
        | "get_sync_status"
        | "list_sync_conflicts" => ToolPermissionCategory::ReadOnly,
        _ => ToolPermissionCategory::Destructive,
    }
}

pub fn check_permission(
    method: &str,
    category: ToolPermissionCategory,
    config: &AppConfig,
) -> Result<(), (i32, String)> {
    let mode = config.mcp_permission_mode.as_deref().unwrap_or("full");
    match mode {
        "read_only" => {
            if category != ToolPermissionCategory::ReadOnly {
                return Err((
                    -32001,
                    format!(
                        "Permission Denied: Method '{method}' is blocked by Sona desktop security policy (read-only mode)"
                    ),
                ));
            }
        }
        "granular" => {
            let allowed = match category {
                ToolPermissionCategory::ReadOnly => true,
                ToolPermissionCategory::Recording => config.mcp_allow_recording.unwrap_or(true),
                ToolPermissionCategory::TranscriptEdit => {
                    config.mcp_allow_transcript_edit.unwrap_or(true)
                }
                ToolPermissionCategory::Destructive => {
                    config.mcp_allow_destructive.unwrap_or(false)
                }
                ToolPermissionCategory::SettingsUpdate => {
                    config.mcp_allow_settings_update.unwrap_or(false)
                }
                ToolPermissionCategory::ModelManagement => {
                    config.mcp_allow_model_management.unwrap_or(true)
                }
            };
            if !allowed {
                return Err((
                    -32001,
                    format!(
                        "Permission Denied: Method '{method}' is blocked by Sona desktop security policy"
                    ),
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn summarize_params(params: Option<&serde_json::Value>) -> Option<String> {
    let p = params?;
    if p.is_null() {
        return None;
    }
    if let Some(obj) = p.as_object() {
        if obj.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(history_id) = obj
            .get("history_id")
            .or_else(|| obj.get("historyId"))
            .and_then(|v| v.as_str())
        {
            parts.push(format!("history: {history_id}"));
        }
        if let Some(project_id) = obj
            .get("project_id")
            .or_else(|| obj.get("projectId"))
            .and_then(|v| v.as_str())
        {
            parts.push(format!("project: {project_id}"));
        }
        if let Some(model_id) = obj
            .get("model_id")
            .or_else(|| obj.get("modelId"))
            .and_then(|v| v.as_str())
        {
            parts.push(format!("model: {model_id}"));
        }
        if let Some(key) = obj.get("key").and_then(|v| v.as_str()) {
            parts.push(format!("key: {key}"));
        }
        if let Some(action) = obj.get("action").and_then(|v| v.as_str()) {
            parts.push(format!("action: {action}"));
        }
        if obj.get("discard").and_then(|v| v.as_bool()) == Some(true) {
            parts.push("discard: true".to_string());
        }
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            parts.push(format!("name: {name}"));
        }
        if !parts.is_empty() {
            return Some(parts.join(", "));
        }
    }
    let s = p.to_string();
    if s.chars().count() > 100 {
        let truncated: String = s.chars().take(100).collect();
        Some(format!("{truncated}..."))
    } else {
        Some(s)
    }
}

pub static TOOLS_CATALOG: &[(&str, &str, &str, ToolPermissionCategory)] = &[
    (
        "sona_get_client_state",
        "Get Client State",
        "Query Sona desktop client online status, recording state, and active project.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_launch_desktop",
        "Launch Desktop Client",
        "Launch Sona desktop client when offline and poll IPC until connection is ready.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_focus_window",
        "Focus Window",
        "Bring Sona desktop window to front and focus.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_start_recording",
        "Start Recording",
        "Start audio capture (microphone or system audio loopback) and live transcription.",
        ToolPermissionCategory::Recording,
    ),
    (
        "sona_control_recording",
        "Control Recording",
        "Unified controller for active recording lifecycle: pause, resume, stop (save), or discard.",
        ToolPermissionCategory::Recording,
    ),
    (
        "sona_stop_recording",
        "Stop Recording",
        "Stop active recording session and finalize into history or discard draft.",
        ToolPermissionCategory::Recording,
    ),
    (
        "sona_list_audio_devices",
        "List Audio Devices",
        "List available microphone and system audio input devices in Sona desktop.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_query_history",
        "Query History",
        "Full-text search and filter transcript history in Sona desktop client.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_read_transcript",
        "Read Transcript",
        "Read full transcript segments and text of a history record.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_edit_transcript",
        "Edit Transcript",
        "Commit modified transcript segments and create an immutable history version snapshot.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_patch_segments",
        "Patch Transcript Segments",
        "Incrementally patch transcript segments by ID (modify text, timing, or translation).",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_update_translations",
        "Update Translations",
        "Batch write, update, or clear segment translations for a history record.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_list_transcript_snapshots",
        "List Transcript Snapshots",
        "List historical transcript version snapshots for a history record.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_revert_transcript_snapshot",
        "Revert Transcript Snapshot",
        "Rollback transcript segments to a previous historical version snapshot.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_update_history_meta",
        "Update History Metadata",
        "Update history item title, project assignment, or icon metadata.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_delete_history",
        "Delete History",
        "Delete or permanently purge a history record.",
        ToolPermissionCategory::Destructive,
    ),
    (
        "sona_query_trash",
        "Query Trash",
        "List and search items currently in the trash / recycle bin.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_restore_history",
        "Restore History",
        "Restore a previously trashed history item back into active history.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_list_projects",
        "List Projects",
        "List all workspace projects and their configurations in Sona desktop.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_create_project",
        "Create Project",
        "Create a new workspace project/folder for categorizing transcription items.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_update_project",
        "Update Project",
        "Update existing workspace project details like name, description, icon, or color.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_delete_project",
        "Delete Project",
        "Delete a workspace project and specify cascade action for associated items.",
        ToolPermissionCategory::Destructive,
    ),
    (
        "sona_set_active_project",
        "Set Active Project",
        "Switch the active workspace project in Sona desktop.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_transcribe_file",
        "Transcribe File",
        "Transcribe a local audio or video file in batch mode and save into history.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_cancel_batch_task",
        "Cancel Batch Task",
        "Cancel an active in-flight batch transcription task by instance ID.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_export_transcript",
        "Export Transcript",
        "Export transcript content into a file in SRT, VTT, Markdown, TXT, or JSON format.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_save_summary",
        "Save Summary",
        "Persist AI-generated summary text for a history record and notify desktop UI.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_load_summary",
        "Load Summary",
        "Load persisted AI summary payload for a specific history item.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_edit_summary",
        "Edit Summary",
        "Edit existing AI summary content for a history record.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_delete_summary",
        "Delete Summary",
        "Delete persisted AI summary for a specific history record and refresh desktop UI.",
        ToolPermissionCategory::Destructive,
    ),
    (
        "sona_get_settings",
        "Get Settings",
        "Read global or specific client preference settings.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_update_setting",
        "Update Setting",
        "Update and persist a client preference setting.",
        ToolPermissionCategory::SettingsUpdate,
    ),
    (
        "sona_get_model_catalog",
        "Get Model Catalog",
        "Get snapshot of installed and available local ASR, VAD, and punctuation models.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_download_preset_model",
        "Download Preset Model",
        "Start downloading a preset model by ID and return a unique download ID.",
        ToolPermissionCategory::ModelManagement,
    ),
    (
        "sona_cancel_download",
        "Cancel Download",
        "Cancel an active in-flight model download by download ID.",
        ToolPermissionCategory::ModelManagement,
    ),
    (
        "sona_delete_preset_model",
        "Delete Preset Model",
        "Physically delete an installed local preset model from disk to free storage space.",
        ToolPermissionCategory::Destructive,
    ),
    (
        "sona_get_sync_status",
        "Get Sync Status",
        "Get E2EE cloud sync configuration, vault status, and conflict overview.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_trigger_sync",
        "Trigger Sync",
        "Trigger an immediate run of cloud synchronization.",
        ToolPermissionCategory::TranscriptEdit,
    ),
    (
        "sona_list_sync_conflicts",
        "List Sync Conflicts",
        "List active cloud synchronization conflicts between local and remote versions.",
        ToolPermissionCategory::ReadOnly,
    ),
    (
        "sona_resolve_sync_conflict",
        "Resolve Sync Conflict",
        "Resolve an unresolved E2EE sync conflict between local and remote versions.",
        ToolPermissionCategory::TranscriptEdit,
    ),
];

pub fn get_tools_catalog(config: &AppConfig) -> Vec<McpToolCatalogItem> {
    TOOLS_CATALOG
        .iter()
        .map(|(name, title, description, category)| {
            let allowed = check_permission(name, *category, config).is_ok();
            McpToolCatalogItem {
                name: (*name).to_string(),
                title: (*title).to_string(),
                description: (*description).to_string(),
                category: category.as_str().to_string(),
                allowed,
            }
        })
        .collect()
}

use super::params::*;

pub(crate) fn map_rpc_error(e: String) -> (i32, String) {
    if e.starts_with("TASK_CANCELLED") || e.contains("Task cancelled") {
        (-32800, e)
    } else {
        (-32000, e)
    }
}

pub async fn dispatch_rpc_call(
    method: &str,
    params: Option<serde_json::Value>,
    facade: &AgentControlFacade,
) -> Result<serde_json::Value, (i32, String)> {
    let clean_method = method
        .strip_prefix("sona_")
        .or_else(|| method.strip_prefix("agent_"))
        .unwrap_or(method);

    match clean_method {
        "get_client_state" => {
            let res = facade.get_client_state().await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "focus_window" => {
            let res = facade.focus_window().map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "start_recording" => {
            let req: StartRecordingRequest = parse_params(params)?;
            let res = facade.start_recording(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "stop_recording" => {
            let req: StopRecordingRequest = parse_params(params)?;
            let res = facade.stop_recording(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "pause_recording" => {
            let res = facade.pause_recording().await.map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "resume_recording" => {
            let res = facade.resume_recording().await.map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "control_recording" => {
            let action = params
                .as_ref()
                .and_then(|p| p.get("action"))
                .and_then(|a| a.as_str())
                .ok_or_else(|| (-32602, "Missing 'action' parameter".to_string()))?;
            match action {
                "pause" => {
                    let res = facade.pause_recording().await.map_err(map_rpc_error)?;
                    Ok(serde_json::json!({ "success": res }))
                }
                "resume" => {
                    let res = facade.resume_recording().await.map_err(map_rpc_error)?;
                    Ok(serde_json::json!({ "success": res }))
                }
                "stop" => {
                    let res = facade
                        .stop_recording(StopRecordingRequest { discard: false })
                        .await
                        .map_err(map_rpc_error)?;
                    serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
                }
                "discard" => {
                    let res = facade
                        .stop_recording(StopRecordingRequest { discard: true })
                        .await
                        .map_err(map_rpc_error)?;
                    serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
                }
                other => Err((-32602, format!("Invalid action '{other}'"))),
            }
        }
        "list_audio_devices" => {
            let res = facade.list_audio_devices().await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "list_transcript_snapshots" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .list_transcript_snapshots(history_id)
                .await
                .map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "revert_transcript_snapshot" => {
            let history_id = extract_history_id(&params)?;
            let snapshot_id = extract_string_param(&params, "snapshot_id")?;
            let res = facade
                .revert_transcript_snapshot(history_id, snapshot_id)
                .await
                .map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "update_history_meta" => {
            let history_id = extract_history_id(&params)?;
            let req: UpdateHistoryMetaRequest = parse_params(params)?;
            let res = facade
                .update_history_meta(history_id, req)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "delete_preset_model" => {
            let model_id = extract_string_param(&params, "model_id")?;
            let res = facade
                .delete_preset_model(model_id)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "list_sync_conflicts" => {
            let res = facade.list_sync_conflicts().await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "resolve_sync_conflict" => {
            let conflict_id = extract_string_param(&params, "conflict_id")?;
            let resolution = extract_string_param(&params, "resolution")?;
            let res = facade
                .resolve_sync_conflict(conflict_id, resolution)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "query_history" => {
            let req: QueryHistoryRequest = parse_params(params)?;
            let res = facade.query_history(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "read_transcript" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .read_transcript(history_id)
                .await
                .map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "edit_transcript" => {
            let req: EditTranscriptRequest = parse_params(params)?;
            let res = facade.edit_transcript(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "patch_segments" => {
            let req: PatchSegmentsRequest = parse_params(params)?;
            let res = facade.patch_segments(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "update_translations" | "edit_translations" => {
            let req: UpdateTranslationsRequest = parse_params(params)?;
            let res = facade
                .update_translations(req)
                .await
                .map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "delete_history" => {
            let (history_id, permanent) = parse_delete_history(&params)?;
            let res = facade
                .delete_history(history_id, permanent)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "list_projects" => {
            let res = facade.list_projects().await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "set_active_project" => {
            let project_id = parse_optional_project_id(&params);
            let res = facade
                .set_active_project(project_id)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "get_settings" => {
            let key = parse_optional_key(&params);
            let res = facade.get_settings(key).map_err(map_rpc_error)?;
            Ok(res)
        }
        "update_setting" => {
            let (key, value) = parse_update_setting(&params)?;
            let res = facade.update_setting(key, value).map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "transcribe_file" => {
            let req: TranscribeFileRequest = parse_params(params)?;
            let res = facade.transcribe_file(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "cancel_batch_task" => {
            let instance_id = extract_string_param(&params, "instance_id")?;
            let res = facade
                .cancel_batch_task(instance_id)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "export_transcript" => {
            let req: ExportTranscriptRequest = parse_params(params)?;
            let res = facade.export_transcript(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "save_summary" | "edit_summary" => {
            let req: SaveSummaryRequest = parse_params(params)?;
            let res = facade.save_summary(req).await.map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "delete_summary" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .delete_summary(history_id)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "load_summary" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .load_summary(history_id)
                .await
                .map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "get_model_catalog" => {
            let res = facade.get_model_catalog().await.map_err(map_rpc_error)?;
            Ok(res)
        }
        "download_preset_model" => {
            let req: DownloadPresetModelRequest = parse_params(params)?;
            let res = facade
                .download_preset_model(req)
                .await
                .map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "cancel_download" => {
            let download_id = extract_string_param(&params, "download_id")?;
            let res = facade
                .cancel_download(download_id)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "get_sync_status" => {
            let res = facade.get_sync_status().await.map_err(map_rpc_error)?;
            Ok(res)
        }
        "trigger_sync" => {
            let res = facade.trigger_sync().await.map_err(map_rpc_error)?;
            Ok(res)
        }
        "create_project" => {
            let req: CreateProjectRequest = parse_params(params)?;
            let res = facade.create_project(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "update_project" => {
            let project_id = extract_project_id(&params)?;
            let req: UpdateProjectRequest = parse_params(params)?;
            let res = facade
                .update_project(project_id, req)
                .await
                .map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "delete_project" => {
            let project_id = extract_project_id(&params)?;
            let cascade_action = parse_optional_string(&params, "cascade_action");
            let res = facade
                .delete_project(project_id, cascade_action)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        "query_trash" => {
            let req: QueryTrashRequest = parse_params(params)?;
            let res = facade.query_trash(req).await.map_err(map_rpc_error)?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "restore_history" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .restore_history(history_id)
                .await
                .map_err(map_rpc_error)?;
            Ok(serde_json::json!({ "success": res }))
        }
        _ => Err((-32601, format!("Method '{method}' not found"))),
    }
}
