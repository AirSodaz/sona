use crate::platform::agent_control::facade::{
    AgentControlFacade, CreateProjectRequest, DownloadPresetModelRequest, EditTranscriptRequest,
    ExportTranscriptRequest, PatchSegmentsRequest, QueryHistoryRequest, QueryTrashRequest,
    SaveSummaryRequest, StartRecordingRequest, StopRecordingRequest, TranscribeFileRequest,
    UpdateHistoryMetaRequest, UpdateProjectRequest, UpdateTranslationsRequest,
};

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
