use super::facade::{
    AgentControlFacade, CreateProjectRequest, DownloadPresetModelRequest, EditTranscriptRequest,
    ExportTranscriptRequest, PatchSegmentsRequest, QueryHistoryRequest, QueryTrashRequest,
    SaveSummaryRequest, StartRecordingRequest, StopRecordingRequest, TranscribeFileRequest,
    UpdateHistoryMetaRequest, UpdateProjectRequest, UpdateTranslationsRequest,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::watch;
pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\sona-agent-ipc";

pub fn get_unix_socket_path() -> PathBuf {
    if let Some(runtime_dir) = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|d| !d.trim().is_empty())
    {
        return PathBuf::from(runtime_dir).join("sona-agent.sock");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("sona")
        .join("agent.sock")
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcSuccessResponse<'a> {
    pub jsonrpc: &'a str,
    pub id: serde_json::Value,
    pub result: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcErrorObject<'a> {
    pub code: i32,
    pub message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcErrorResponse<'a> {
    pub jsonrpc: &'a str,
    pub id: serde_json::Value,
    pub error: JsonRpcErrorObject<'a>,
}

fn parse_params<T: serde::de::DeserializeOwned>(
    params: Option<serde_json::Value>,
) -> Result<T, (i32, String)> {
    let value = params.unwrap_or_else(|| serde_json::json!({}));
    serde_json::from_value(value).map_err(|e| (-32602, format!("Invalid params: {e}")))
}

fn extract_history_id(params: &Option<serde_json::Value>) -> Result<String, (i32, String)> {
    match params {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(map)) => {
            if let Some(serde_json::Value::String(s)) =
                map.get("history_id").or_else(|| map.get("historyId"))
            {
                Ok(s.clone())
            } else {
                Err((-32602, "Missing 'history_id' parameter".to_string()))
            }
        }
        _ => Err((
            -32602,
            "Invalid or missing parameters for history_id".to_string(),
        )),
    }
}

fn parse_delete_history(
    params: &Option<serde_json::Value>,
) -> Result<(String, bool), (i32, String)> {
    match params {
        Some(serde_json::Value::String(s)) => Ok((s.clone(), false)),
        Some(serde_json::Value::Object(map)) => {
            let id = if let Some(serde_json::Value::String(s)) =
                map.get("history_id").or_else(|| map.get("historyId"))
            {
                s.clone()
            } else {
                return Err((-32602, "Missing 'history_id' parameter".to_string()));
            };
            let permanent = map
                .get("permanent")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok((id, permanent))
        }
        _ => Err((-32602, "Invalid parameters for delete_history".to_string())),
    }
}

fn parse_optional_project_id(params: &Option<serde_json::Value>) -> Option<String> {
    match params {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Object(map)) => map
            .get("project_id")
            .or_else(|| map.get("projectId"))
            .or_else(|| map.get("id"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        _ => None,
    }
}

fn extract_project_id(params: &Option<serde_json::Value>) -> Result<String, (i32, String)> {
    match params.as_ref() {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(map)) => {
            if let Some(serde_json::Value::String(s)) = map
                .get("project_id")
                .or_else(|| map.get("projectId"))
                .or_else(|| map.get("id"))
            {
                Ok(s.clone())
            } else {
                Err((-32602, "Missing 'project_id' or 'id' parameter".to_string()))
            }
        }
        _ => Err((
            -32602,
            "Invalid or missing parameters for project id".to_string(),
        )),
    }
}

fn parse_optional_key(params: &Option<serde_json::Value>) -> Option<String> {
    match params {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Object(map)) => map
            .get("key")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        _ => None,
    }
}

fn parse_update_setting(
    params: &Option<serde_json::Value>,
) -> Result<(String, serde_json::Value), (i32, String)> {
    match params {
        Some(serde_json::Value::Object(map)) => {
            let key = if let Some(serde_json::Value::String(s)) = map.get("key") {
                s.clone()
            } else {
                return Err((-32602, "Missing 'key' parameter".to_string()));
            };
            let value = map.get("value").cloned().unwrap_or(serde_json::Value::Null);
            Ok((key, value))
        }
        _ => Err((-32602, "Invalid parameters for update_setting".to_string())),
    }
}

fn extract_string_param(
    params: &Option<serde_json::Value>,
    key: &str,
) -> Result<String, (i32, String)> {
    let camel_key = key
        .split('_')
        .enumerate()
        .map(|(i, part)| {
            if i == 0 {
                part.to_string()
            } else {
                let mut c = part.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        })
        .collect::<String>();

    match params.as_ref() {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(map)) => {
            if let Some(serde_json::Value::String(s)) = map.get(key).or_else(|| map.get(&camel_key))
            {
                Ok(s.clone())
            } else {
                Err((-32602, format!("Missing '{key}' parameter")))
            }
        }
        _ => Err((-32602, format!("Invalid or missing parameters for '{key}'"))),
    }
}

fn parse_optional_string(params: &Option<serde_json::Value>, key: &str) -> Option<String> {
    let camel_key = key
        .split('_')
        .enumerate()
        .map(|(i, part)| {
            if i == 0 {
                part.to_string()
            } else {
                let mut c = part.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        })
        .collect::<String>();

    match params.as_ref() {
        Some(serde_json::Value::Object(map)) => map
            .get(key)
            .or_else(|| map.get(&camel_key))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        _ => None,
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
            let res = facade.get_client_state().await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "focus_window" => {
            let res = facade.focus_window().map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "start_recording" => {
            let req: StartRecordingRequest = parse_params(params)?;
            let res = facade.start_recording(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "stop_recording" => {
            let req: StopRecordingRequest = parse_params(params)?;
            let res = facade.stop_recording(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "pause_recording" => {
            let res = facade.pause_recording().await.map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "resume_recording" => {
            let res = facade.resume_recording().await.map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "list_audio_devices" => {
            let res = facade.list_audio_devices().await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "list_transcript_snapshots" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .list_transcript_snapshots(history_id)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "revert_transcript_snapshot" => {
            let history_id = extract_history_id(&params)?;
            let snapshot_id = extract_string_param(&params, "snapshot_id")?;
            let res = facade
                .revert_transcript_snapshot(history_id, snapshot_id)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "update_history_meta" => {
            let history_id = extract_history_id(&params)?;
            let req: UpdateHistoryMetaRequest = parse_params(params)?;
            let res = facade
                .update_history_meta(history_id, req)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "delete_preset_model" => {
            let model_id = extract_string_param(&params, "model_id")?;
            let res = facade
                .delete_preset_model(model_id)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "list_sync_conflicts" => {
            let res = facade
                .list_sync_conflicts()
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "resolve_sync_conflict" => {
            let conflict_id = extract_string_param(&params, "conflict_id")?;
            let resolution = extract_string_param(&params, "resolution")?;
            let res = facade
                .resolve_sync_conflict(conflict_id, resolution)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "query_history" => {
            let req: QueryHistoryRequest = parse_params(params)?;
            let res = facade.query_history(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "read_transcript" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .read_transcript(history_id)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "edit_transcript" => {
            let req: EditTranscriptRequest = parse_params(params)?;
            let res = facade.edit_transcript(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "patch_segments" => {
            let req: PatchSegmentsRequest = parse_params(params)?;
            let res = facade.patch_segments(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "update_translations" | "edit_translations" => {
            let req: UpdateTranslationsRequest = parse_params(params)?;
            let res = facade
                .update_translations(req)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "delete_history" => {
            let (history_id, permanent) = parse_delete_history(&params)?;
            let res = facade
                .delete_history(history_id, permanent)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "list_projects" => {
            let res = facade.list_projects().await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "set_active_project" => {
            let project_id = parse_optional_project_id(&params);
            let res = facade
                .set_active_project(project_id)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "get_settings" => {
            let key = parse_optional_key(&params);
            let res = facade.get_settings(key).map_err(|e| (-32000, e))?;
            Ok(res)
        }
        "update_setting" => {
            let (key, value) = parse_update_setting(&params)?;
            let res = facade.update_setting(key, value).map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "transcribe_file" => {
            let req: TranscribeFileRequest = parse_params(params)?;
            let res = facade.transcribe_file(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "cancel_batch_task" => {
            let instance_id = extract_string_param(&params, "instance_id")?;
            let res = facade
                .cancel_batch_task(instance_id)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "export_transcript" => {
            let req: ExportTranscriptRequest = parse_params(params)?;
            let res = facade
                .export_transcript(req)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "save_summary" | "edit_summary" => {
            let req: SaveSummaryRequest = parse_params(params)?;
            let res = facade.save_summary(req).await.map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "delete_summary" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .delete_summary(history_id)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "load_summary" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .load_summary(history_id)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "get_model_catalog" => {
            let res = facade.get_model_catalog().await.map_err(|e| (-32000, e))?;
            Ok(res)
        }
        "download_preset_model" => {
            let req: DownloadPresetModelRequest = parse_params(params)?;
            let res = facade
                .download_preset_model(req)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "cancel_download" => {
            let download_id = extract_string_param(&params, "download_id")?;
            let res = facade
                .cancel_download(download_id)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "get_sync_status" => {
            let res = facade.get_sync_status().await.map_err(|e| (-32000, e))?;
            Ok(res)
        }
        "trigger_sync" => {
            let res = facade.trigger_sync().await.map_err(|e| (-32000, e))?;
            Ok(res)
        }
        "create_project" => {
            let req: CreateProjectRequest = parse_params(params)?;
            let res = facade.create_project(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "update_project" => {
            let project_id = extract_project_id(&params)?;
            let req: UpdateProjectRequest = parse_params(params)?;
            let res = facade
                .update_project(project_id, req)
                .await
                .map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "delete_project" => {
            let project_id = extract_project_id(&params)?;
            let cascade_action = parse_optional_string(&params, "cascade_action");
            let res = facade
                .delete_project(project_id, cascade_action)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        "query_trash" => {
            let req: QueryTrashRequest = parse_params(params)?;
            let res = facade.query_trash(req).await.map_err(|e| (-32000, e))?;
            serde_json::to_value(res).map_err(|e| (-32603, e.to_string()))
        }
        "restore_history" => {
            let history_id = extract_history_id(&params)?;
            let res = facade
                .restore_history(history_id)
                .await
                .map_err(|e| (-32000, e))?;
            Ok(serde_json::json!({ "success": res }))
        }
        _ => Err((-32601, format!("Method '{method}' not found"))),
    }
}

#[derive(Clone)]
pub struct ActiveConnectionTracker {
    count: Arc<AtomicUsize>,
    disconnect_tx: Arc<watch::Sender<u64>>,
    disconnect_rx: watch::Receiver<u64>,
}

impl Default for ActiveConnectionTracker {
    fn default() -> Self {
        let (disconnect_tx, disconnect_rx) = watch::channel(0);
        Self {
            count: Arc::new(AtomicUsize::new(0)),
            disconnect_tx: Arc::new(disconnect_tx),
            disconnect_rx,
        }
    }
}

impl ActiveConnectionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_connected(&self, app_handle: Option<&tauri::AppHandle>) {
        self.count.fetch_add(1, Ordering::SeqCst);
        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, true);
        }
    }

    pub fn on_disconnected(&self, app_handle: Option<&tauri::AppHandle>) {
        let mut current = self.count.load(Ordering::SeqCst);
        let prev = loop {
            if current == 0 {
                break 0;
            }
            match self.count.compare_exchange_weak(
                current,
                current - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(prev) => break prev,
                Err(actual) => current = actual,
            }
        };
        if prev == 1
            && let Some(app) = app_handle
        {
            crate::app::tray::set_agent_connected(app, false);
        }
    }

    pub fn active_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    pub fn disconnect_all(&self, app_handle: Option<&tauri::AppHandle>) {
        self.disconnect_tx.send_modify(|v| *v += 1);
        if let Some(app) = app_handle {
            crate::app::tray::set_agent_connected(app, false);
        }
    }
}

pub fn disconnect_all_agents(app_handle: &tauri::AppHandle) {
    if let Some(tracker) = app_handle.try_state::<ActiveConnectionTracker>() {
        tracker.disconnect_all(Some(app_handle));
    }
    if let Some(facade) = app_handle.try_state::<AgentControlFacade>() {
        let facade_clone = facade.inner().clone();
        tauri::async_runtime::spawn(async move {
            let _ = facade_clone
                .stop_recording(
                    crate::platform::agent_control::facade::StopRecordingRequest { discard: false },
                )
                .await;
        });
    }
    crate::app::tray::set_agent_connected(app_handle, false);
}

pub fn stop_recording_from_tray(app_handle: &tauri::AppHandle) {
    if let Some(facade) = app_handle.try_state::<AgentControlFacade>() {
        let facade_clone = facade.inner().clone();
        tauri::async_runtime::spawn(async move {
            let _ = facade_clone
                .stop_recording(
                    crate::platform::agent_control::facade::StopRecordingRequest { discard: false },
                )
                .await;
        });
    }
    if let Some(services) = app_handle.try_state::<crate::services::DesktopServices>() {
        let audio = services.audio.clone();
        tauri::async_runtime::spawn(async move {
            crate::integrations::audio::stop_all_captures(&audio).await;
        });
    }
    let _ = app_handle.emit("tray-stop-recording", ());
    crate::app::tray::set_recording_active(app_handle, false);
}

pub async fn handle_connection<S>(
    stream: S,
    facade: AgentControlFacade,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    handle_connection_tracked(stream, facade, ActiveConnectionTracker::default(), None).await
}

pub async fn handle_connection_tracked<S>(
    stream: S,
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    tracker.on_connected(app_handle.as_ref());
    struct ConnectionGuard {
        tracker: ActiveConnectionTracker,
        app_handle: Option<tauri::AppHandle>,
    }
    impl Drop for ConnectionGuard {
        fn drop(&mut self) {
            self.tracker.on_disconnected(self.app_handle.as_ref());
        }
    }
    let _guard = ConnectionGuard {
        tracker: tracker.clone(),
        app_handle,
    };

    let (reader, mut writer) = tokio::io::split(stream);
    let mut buf_reader = tokio::io::BufReader::new(reader);
    let mut line = String::new();
    let mut disconnect_rx = tracker.disconnect_rx.clone();
    let initial_version = *disconnect_rx.borrow();

    loop {
        tokio::select! {
            changed = disconnect_rx.changed() => {
                match changed {
                    Ok(()) => {
                        if *disconnect_rx.borrow() != initial_version {
                            log::info!("[AgentControlIPC] Client connection terminated by disconnect request");
                            break;
                        }
                    }
                    Err(_) => {
                        log::info!("[AgentControlIPC] Disconnect channel closed, terminating connection");
                        break;
                    }
                }
            }
            read_res = buf_reader.read_line(&mut line) => {
                let bytes_read = read_res?;
                if bytes_read == 0 {
                    break;
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    line.clear();
                    continue;
                }

        let resp_json: Option<String> = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(rpc_req) => {
                if rpc_req.jsonrpc != "2.0" {
                    let resp = JsonRpcErrorResponse {
                        jsonrpc: "2.0",
                        id: rpc_req.id.unwrap_or(serde_json::Value::Null),
                        error: JsonRpcErrorObject {
                            code: -32600,
                            message: "Invalid Request: jsonrpc must be '2.0'",
                            data: None,
                        },
                    };
                    Some(serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string()))
                } else if rpc_req.id.is_none() {
                    // JSON-RPC 2.0 Notification: process without response
                    let _ = dispatch_rpc_call(&rpc_req.method, rpc_req.params, &facade).await;
                    None
                } else {
                    let id = rpc_req.id.unwrap();
                    match dispatch_rpc_call(&rpc_req.method, rpc_req.params, &facade).await {
                        Ok(result) => {
                            let resp = JsonRpcSuccessResponse {
                                jsonrpc: "2.0",
                                id,
                                result,
                            };
                            Some(serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string()))
                        }
                        Err((code, message)) => {
                            let resp = JsonRpcErrorResponse {
                                jsonrpc: "2.0",
                                id,
                                error: JsonRpcErrorObject {
                                    code,
                                    message: &message,
                                    data: None,
                                },
                            };
                            Some(serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string()))
                        }
                    }
                }
            }
            Err(e) => {
                let resp = JsonRpcErrorResponse {
                    jsonrpc: "2.0",
                    id: serde_json::Value::Null,
                    error: JsonRpcErrorObject {
                        code: -32700,
                        message: &format!("Parse error: {e}"),
                        data: None,
                    },
                };
                Some(serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string()))
            }
        };

                if let Some(resp_str) = resp_json {
                    writer.write_all(resp_str.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                }
                line.clear();
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
pub async fn run_ipc_server(
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(WINDOWS_PIPE_NAME)?;

    log::info!("[AgentControlIPC] Named pipe server listening on {WINDOWS_PIPE_NAME}");

    loop {
        if let Err(e) = server.connect().await {
            log::warn!("[AgentControlIPC] Named pipe connect error: {e}");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            continue;
        }

        let connected_client = server;
        server = ServerOptions::new().create(WINDOWS_PIPE_NAME)?;

        let facade_clone = facade.clone();
        let tracker_clone = tracker.clone();
        let app_handle_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = handle_connection_tracked(
                connected_client,
                facade_clone,
                tracker_clone,
                app_handle_clone,
            )
            .await
            {
                log::debug!("[AgentControlIPC] Client connection ended: {e}");
            }
        });
    }
}

#[cfg(unix)]
pub async fn run_ipc_server(
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket_path = get_unix_socket_path();
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&socket_path);

    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));
    }
    log::info!(
        "[AgentControlIPC] Unix domain socket server listening on {}",
        socket_path.display()
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let facade_clone = facade.clone();
        let tracker_clone = tracker.clone();
        let app_handle_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) =
                handle_connection_tracked(stream, facade_clone, tracker_clone, app_handle_clone)
                    .await
            {
                log::debug!("[AgentControlIPC] Client connection ended: {e}");
            }
        });
    }
}

#[cfg(not(any(windows, unix)))]
pub async fn run_ipc_server(
    _facade: AgentControlFacade,
    _tracker: ActiveConnectionTracker,
    _app_handle: Option<tauri::AppHandle>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::warn!("[AgentControlIPC] IPC server is not supported on this platform");
    Ok(())
}

pub fn start_agent_control_ipc_server(
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_ipc_server(facade, tracker, app_handle).await {
            log::error!("[AgentControlIPC] Server terminated with error: {e}");
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::database::DesktopSqliteState;
    use crate::platform::event::MockEventEmitter;
    use crate::services::DesktopServices;
    use std::sync::Arc;

    async fn create_test_facade() -> (AgentControlFacade, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let sqlite = DesktopSqliteState::new(ctx);
        let emitter = Arc::new(MockEventEmitter::new());

        let services = DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .sync_config_path(temp.path().join("sync.json"))
            .build()
            .unwrap();

        let facade = AgentControlFacade::new(services, None);
        (facade, temp)
    }

    #[tokio::test]
    async fn test_dispatch_rpc_variants() {
        let (facade, _dir) = create_test_facade().await;

        let res1 = dispatch_rpc_call("sona_get_client_state", None, &facade)
            .await
            .unwrap();
        assert_eq!(res1.get("online").and_then(|v| v.as_bool()), Some(true));

        let res2 = dispatch_rpc_call("get_client_state", None, &facade)
            .await
            .unwrap();
        assert_eq!(res2.get("online").and_then(|v| v.as_bool()), Some(true));

        let res3 = dispatch_rpc_call("agent_get_client_state", None, &facade)
            .await
            .unwrap();
        assert_eq!(res3.get("online").and_then(|v| v.as_bool()), Some(true));

        let unknown = dispatch_rpc_call("unknown_method", None, &facade).await;
        assert!(unknown.is_err());
        assert_eq!(unknown.unwrap_err().0, -32601);
    }

    #[tokio::test]
    async fn test_project_id_alias_dispatch() {
        let (facade, _dir) = create_test_facade().await;

        // 1. Create a project
        let create_res = dispatch_rpc_call(
            "sona_create_project",
            Some(serde_json::json!({ "name": "Test Project" })),
            &facade,
        )
        .await
        .unwrap();
        let proj_id = create_res.get("id").and_then(|v| v.as_str()).unwrap();

        // 2. Update with {"id": ...} instead of {"project_id": ...}
        let update_res = dispatch_rpc_call(
            "sona_update_project",
            Some(serde_json::json!({ "id": proj_id, "name": "Renamed Project" })),
            &facade,
        )
        .await
        .unwrap();
        assert_eq!(
            update_res.get("name").and_then(|v| v.as_str()),
            Some("Renamed Project")
        );

        // 3. Delete with {"id": ...}
        let delete_res = dispatch_rpc_call(
            "sona_delete_project",
            Some(serde_json::json!({ "id": proj_id })),
            &facade,
        )
        .await
        .unwrap();
        assert_eq!(
            delete_res.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_handle_connection_duplex() {
        let (facade, _dir) = create_test_facade().await;
        let (client, server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let _ = handle_connection(server, facade).await;
        });

        let (client_read, mut client_write) = tokio::io::split(client);
        let mut reader = tokio::io::BufReader::new(client_read);

        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "sona_get_client_state",
            "params": {}
        });
        let req_str = format!("{}\n", serde_json::to_string(&req).unwrap());
        client_write.write_all(req_str.as_bytes()).await.unwrap();
        client_write.flush().await.unwrap();

        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).await.unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
        assert_eq!(parsed.get("id").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(
            parsed
                .get("result")
                .and_then(|r| r.get("online"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }
    #[tokio::test]
    async fn test_handle_connection_invalid_version() {
        let (facade, _dir) = create_test_facade().await;
        let (client, server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let _ = handle_connection(server, facade).await;
        });

        let (client_read, mut client_write) = tokio::io::split(client);
        let mut reader = tokio::io::BufReader::new(client_read);

        let req = serde_json::json!({
            "jsonrpc": "1.0",
            "id": 99,
            "method": "sona_get_client_state",
            "params": {}
        });
        let req_str = format!("{}\n", serde_json::to_string(&req).unwrap());
        client_write.write_all(req_str.as_bytes()).await.unwrap();
        client_write.flush().await.unwrap();

        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).await.unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
        assert_eq!(parsed.get("id").and_then(|v| v.as_i64()), Some(99));
        assert_eq!(
            parsed
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|c| c.as_i64()),
            Some(-32600)
        );
    }

    #[tokio::test]
    async fn test_handle_connection_notification_silent() {
        let (facade, _dir) = create_test_facade().await;
        let (client, server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let _ = handle_connection(server, facade).await;
        });

        let (client_read, mut client_write) = tokio::io::split(client);
        let mut reader = tokio::io::BufReader::new(client_read);

        // Send a notification (no id)
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "sona_focus_window"
        });
        let req_str = format!("{}\n", serde_json::to_string(&notification).unwrap());
        client_write.write_all(req_str.as_bytes()).await.unwrap();
        client_write.flush().await.unwrap();

        // Follow up with a request with id
        let ping_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "sona_get_client_state"
        });
        let ping_str = format!("{}\n", serde_json::to_string(&ping_req).unwrap());
        client_write.write_all(ping_str.as_bytes()).await.unwrap();
        client_write.flush().await.unwrap();

        // The first response received must be for id: 100, NOT the notification
        let mut resp_line = String::new();
        reader.read_line(&mut resp_line).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
        assert_eq!(parsed.get("id").and_then(|v| v.as_i64()), Some(100));
    }

    #[tokio::test]
    async fn test_dispatch_new_methods() {
        let (facade, _dir) = create_test_facade().await;

        // Model catalog
        let cat = dispatch_rpc_call("sona_get_model_catalog", None, &facade)
            .await
            .unwrap();
        assert!(cat.is_object());

        // Sync status
        let sync = dispatch_rpc_call("sona_get_sync_status", None, &facade)
            .await
            .unwrap();
        assert!(sync.is_object());

        // Query trash
        let trash = dispatch_rpc_call(
            "sona_query_trash",
            Some(serde_json::json!({ "query": "" })),
            &facade,
        )
        .await
        .unwrap();
        assert!(trash.is_array());

        // Project lifecycle via RPC
        let proj = dispatch_rpc_call(
            "sona_create_project",
            Some(serde_json::json!({ "name": "RPC Project" })),
            &facade,
        )
        .await
        .unwrap();
        let proj_id = proj.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        assert_eq!(
            proj.get("name").and_then(|v| v.as_str()),
            Some("RPC Project")
        );

        let updated = dispatch_rpc_call(
            "sona_update_project",
            Some(serde_json::json!({ "project_id": &proj_id, "name": "Updated RPC Project" })),
            &facade,
        )
        .await
        .unwrap();
        assert_eq!(
            updated.get("name").and_then(|v| v.as_str()),
            Some("Updated RPC Project")
        );

        let deleted = dispatch_rpc_call(
            "sona_delete_project",
            Some(serde_json::json!({ "projectId": &proj_id })),
            &facade,
        )
        .await
        .unwrap();
        assert_eq!(deleted.get("success").and_then(|v| v.as_bool()), Some(true));

        // Cancel non-existent task returns false
        let cancel_batch_miss = dispatch_rpc_call(
            "sona_cancel_batch_task",
            Some(serde_json::json!({ "instance_id": "inst-miss" })),
            &facade,
        )
        .await
        .unwrap();
        assert_eq!(
            cancel_batch_miss.get("success").and_then(|v| v.as_bool()),
            Some(false)
        );

        // Register an active task in batch_cancel and cancel it -> returns true
        let (_rx, _guard) = facade
            .services
            .asr
            .batch_cancel
            .register("inst-active")
            .await;
        let cancel_batch_hit = dispatch_rpc_call(
            "sona_cancel_batch_task",
            Some(serde_json::json!({ "instance_id": "inst-active" })),
            &facade,
        )
        .await
        .unwrap();
        assert_eq!(
            cancel_batch_hit.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );
        let cancel_dl = dispatch_rpc_call(
            "sona_cancel_download",
            Some(serde_json::json!({ "download_id": "dl-1" })),
            &facade,
        )
        .await
        .unwrap();
        assert_eq!(
            cancel_dl.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_active_connection_tracker() {
        let tracker = ActiveConnectionTracker::new();
        assert_eq!(tracker.active_count(), 0);

        tracker.on_connected(None);
        assert_eq!(tracker.active_count(), 1);

        tracker.on_connected(None);
        assert_eq!(tracker.active_count(), 2);

        tracker.on_disconnected(None);
        assert_eq!(tracker.active_count(), 1);

        tracker.on_disconnected(None);
        assert_eq!(tracker.active_count(), 0);

        // Defensive check: redundant disconnects do not underflow
        tracker.on_disconnected(None);
        assert_eq!(tracker.active_count(), 0);

        tracker.disconnect_all(None);
        assert_eq!(tracker.active_count(), 0);
    }
}
