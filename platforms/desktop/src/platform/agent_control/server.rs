pub mod dispatch;
pub mod params;
pub mod tracker;

pub use dispatch::*;
pub use params::*;
pub use tracker::*;

use super::facade::AgentControlFacade;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::{RwLock, watch};
pub use sona_core::runtime::ipc::{
    WINDOWS_PIPE_NAME, get_ipc_endpoint, get_unix_socket_path, resolve_ipc_endpoint,
};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpActivityLogEntry {
    pub id: String,
    pub timestamp: u64,
    pub method: String,
    pub category: String,
    pub status: String, // "success" | "error" | "blocked"
    pub duration_ms: u64,
    pub params_summary: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct AgentControlServerController {
    active_shutdown_tx: Arc<std::sync::Mutex<Option<watch::Sender<bool>>>>,
    is_running: Arc<AtomicBool>,
    generation: Arc<std::sync::atomic::AtomicU64>,
    tracker: ActiveConnectionTracker,
    activity_logs: Arc<RwLock<VecDeque<McpActivityLogEntry>>>,
}

impl Default for AgentControlServerController {
    fn default() -> Self {
        Self::new(ActiveConnectionTracker::default())
    }
}

impl AgentControlServerController {
    pub fn new(tracker: ActiveConnectionTracker) -> Self {
        Self {
            active_shutdown_tx: Arc::new(std::sync::Mutex::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            tracker,
            activity_logs: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
        }
    }

    pub fn tracker(&self) -> &ActiveConnectionTracker {
        &self.tracker
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn start(&self, app_handle: Option<tauri::AppHandle>, facade: AgentControlFacade) {
        if self.is_running.load(Ordering::SeqCst) {
            log::info!("[AgentControlIPC] Server is already running");
            return;
        }
        let generation_id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        {
            if let Ok(mut active) = self.active_shutdown_tx.lock() {
                let old_tx = active.replace(shutdown_tx);
                if let Some(tx) = old_tx {
                    let _ = tx.send(true);
                }
            }
        }
        self.is_running.store(true, Ordering::SeqCst);

        let tracker = self.tracker.clone();
        let app_handle_clone = app_handle.clone();
        let controller_clone = self.clone();
        let is_running_flag = self.is_running.clone();
        let gen_checker = self.generation.clone();

        tauri::async_runtime::spawn(async move {
            log::info!("[AgentControlIPC] Starting IPC server worker generation {generation_id}");
            while is_running_flag.load(Ordering::SeqCst)
                && gen_checker.load(Ordering::SeqCst) == generation_id
            {
                if let Err(e) = run_ipc_server(
                    facade.clone(),
                    tracker.clone(),
                    app_handle_clone.clone(),
                    shutdown_rx.clone(),
                    Some(controller_clone.clone()),
                )
                .await
                {
                    if !is_running_flag.load(Ordering::SeqCst)
                        || gen_checker.load(Ordering::SeqCst) != generation_id
                    {
                        break;
                    }
                    log::error!(
                        "[AgentControlIPC] Server loop ended with error: {e}. Retrying in 1s..."
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {},
                        changed = shutdown_rx.changed() => {
                            if changed.is_ok() && *shutdown_rx.borrow() {
                                break;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
            if gen_checker.load(Ordering::SeqCst) == generation_id {
                is_running_flag.store(false, Ordering::SeqCst);
                if let Some(app) = &app_handle_clone {
                    let _ = app.emit("mcp-status-changed", ());
                }
            }
            log::info!("[AgentControlIPC] IPC server worker generation {generation_id} exited");
        });

        if let Some(app) = &app_handle {
            let _ = app.emit("mcp-status-changed", ());
        }
    }

    pub fn stop(&self, app_handle: Option<&tauri::AppHandle>) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if !self.is_running.swap(false, Ordering::SeqCst) {
            return;
        }
        {
            if let Some(tx) = self
                .active_shutdown_tx
                .lock()
                .ok()
                .and_then(|mut active| active.take())
            {
                let _ = tx.send(true);
            }
        }
        self.tracker.disconnect_all(app_handle);
        if let Some(app) = app_handle {
            let _ = app.emit("mcp-status-changed", ());
        }
        log::info!("[AgentControlIPC] IPC server stopped");
    }

    pub fn disconnect_all(&self, app_handle: Option<&tauri::AppHandle>) {
        self.tracker.disconnect_all(app_handle);
    }

    pub async fn record_activity(
        &self,
        entry: McpActivityLogEntry,
        app_handle: Option<&tauri::AppHandle>,
        notify_on_mutation: bool,
    ) {
        {
            let mut logs = self.activity_logs.write().await;
            if logs.len() >= 100 {
                logs.pop_front();
            }
            logs.push_back(entry.clone());
        }

        if let Some(app) = app_handle {
            let _ = app.emit("mcp-activity", &entry);
            if notify_on_mutation && entry.status == "success" {
                let is_mutation = matches!(
                    entry.category.as_str(),
                    "recording" | "transcript_edit" | "destructive" | "settings_update"
                );
                if is_mutation {
                    let _ = app.emit(
                        "mcp-mutation-notice",
                        serde_json::json!({
                            "id": entry.id,
                            "method": entry.method,
                            "category": entry.category,
                            "summary": entry.params_summary,
                            "timestamp": entry.timestamp,
                        }),
                    );
                }
            }
        }
    }

    pub async fn get_activity_logs(&self) -> Vec<McpActivityLogEntry> {
        let logs = self.activity_logs.read().await;
        logs.iter().cloned().collect()
    }

    pub async fn clear_activity_logs(&self) {
        let mut logs = self.activity_logs.write().await;
        logs.clear();
    }
}

pub async fn handle_connection<S>(
    stream: S,
    facade: AgentControlFacade,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    handle_connection_tracked(
        stream,
        facade,
        ActiveConnectionTracker::default(),
        None,
        None,
    )
    .await
}

pub async fn handle_connection_tracked<S>(
    stream: S,
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
    controller: Option<AgentControlServerController>,
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
        app_handle: app_handle.clone(),
    };

    let (reader, mut writer) = tokio::io::split(stream);
    let mut buf_reader = tokio::io::BufReader::new(reader);
    let mut line = String::new();
    let mut disconnect_rx = tracker.disconnect_rx.clone();
    let initial_version = *disconnect_rx.borrow();
    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel::<String>(64);
    let writer_task = tokio::spawn(async move {
        while let Some(resp_str) = resp_rx.recv().await {
            if writer.write_all(resp_str.as_bytes()).await.is_err() {
                break;
            }
            if writer.write_all(b"\n").await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
    });

    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel::<JsonRpcRequest>(32);
    let worker_facade = facade.clone();
    let worker_controller = controller.clone();
    let worker_app_handle = app_handle.clone();
    let worker_resp_tx = resp_tx.clone();

    let worker_task = tokio::spawn(async move {
        while let Some(rpc_req) = req_rx.recv().await {
            let start = std::time::Instant::now();
            let method = rpc_req.method;
            let params = rpc_req.params;
            let category = classify_tool(&method, params.as_ref());
            let params_summary = summarize_params(params.as_ref());

            let config = worker_facade.load_app_config();
            let perm_result = check_permission(&method, category, &config);

            let req_id = rpc_req.id.clone().unwrap_or(serde_json::Value::Null);
            let is_notification = rpc_req.id.is_none();

            let (response_str, log_status, error_msg) = match perm_result {
                Err((code, message)) => {
                    let resp = JsonRpcErrorResponse {
                        jsonrpc: "2.0",
                        id: req_id,
                        error: JsonRpcErrorObject {
                            code,
                            message: &message,
                            data: None,
                        },
                    };
                    let s = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                    (Some(s), "blocked", Some(message))
                }
                Ok(()) => match dispatch_rpc_call(&method, params, &worker_facade).await {
                    Ok(result) => {
                        let resp = JsonRpcSuccessResponse {
                            jsonrpc: "2.0",
                            id: req_id,
                            result,
                        };
                        let s = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                        (Some(s), "success", None)
                    }
                    Err((code, message)) => {
                        let resp = JsonRpcErrorResponse {
                            jsonrpc: "2.0",
                            id: req_id,
                            error: JsonRpcErrorObject {
                                code,
                                message: &message,
                                data: None,
                            },
                        };
                        let s = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                        (Some(s), "error", Some(message))
                    }
                },
            };

            let duration_ms = start.elapsed().as_millis() as u64;

            let send_failed = if !is_notification && let Some(resp_str) = response_str {
                worker_resp_tx.send(resp_str).await.is_err()
            } else {
                false
            };

            if let Some(ctrl) = &worker_controller
                && config.mcp_audit_log_enabled.unwrap_or(true)
            {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let entry = McpActivityLogEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: now_ms,
                    method,
                    category: category.as_str().to_string(),
                    status: log_status.to_string(),
                    duration_ms,
                    params_summary,
                    error_message: error_msg,
                };
                ctrl.record_activity(
                    entry,
                    worker_app_handle.as_ref(),
                    config.mcp_notify_on_mutation.unwrap_or(true),
                )
                .await;
            }

            if send_failed {
                break;
            }
        }
    });

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

                match serde_json::from_str::<JsonRpcRequest>(trimmed) {
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
                            let resp_str = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                            let _ = resp_tx.send(resp_str).await;
                        } else if rpc_req.method == "ping" {
                            if let Some(req_id) = rpc_req.id {
                                let resp = JsonRpcSuccessResponse {
                                    jsonrpc: "2.0",
                                    id: req_id,
                                    result: serde_json::json!({}),
                                };
                                let resp_str = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                                let _ = resp_tx.send(resp_str).await;
                            }
                        } else if rpc_req.method == "notifications/cancelled"
                            || rpc_req.method == "$/cancelRequest"
                        {
                            log::info!(
                                "[AgentControlIPC] Cancellation notice received for method '{}': {:?}",
                                rpc_req.method,
                                rpc_req.params
                            );
                        } else if req_tx.send(rpc_req).await.is_err() {
                            break;
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
                        let resp_str = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                        let _ = resp_tx.send(resp_str).await;
                    }
                }

                line.clear();
            }
        }
    }

    drop(req_tx);
    let abort_handle = worker_task.abort_handle();
    if tokio::time::timeout(std::time::Duration::from_secs(5), worker_task)
        .await
        .is_err()
    {
        log::warn!(
            "[AgentControlIPC] Worker task timed out after 5s during graceful shutdown; aborting"
        );
        abort_handle.abort();
    }
    drop(resp_tx);
    let _ = writer_task.await;

    Ok(())
}

#[cfg(windows)]
pub async fn run_ipc_server(
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
    mut shutdown_rx: watch::Receiver<bool>,
    controller: Option<AgentControlServerController>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tokio::net::windows::named_pipe::ServerOptions;

    let pipe_name = get_ipc_endpoint();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)?;

    log::info!("[AgentControlIPC] Named pipe server listening on {pipe_name}");

    loop {
        if *shutdown_rx.borrow() {
            log::info!("[AgentControlIPC] Shutdown requested, closing named pipe server");
            return Ok(());
        }

        tokio::select! {
            res = server.connect() => {
                if let Err(e) = res {
                    log::warn!("[AgentControlIPC] Named pipe connect error: {e}");
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                let connected_client = server;
                let facade_clone = facade.clone();
                let tracker_clone = tracker.clone();
                let app_handle_clone = app_handle.clone();
                let controller_clone = controller.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = handle_connection_tracked(
                        connected_client,
                        facade_clone,
                        tracker_clone,
                        app_handle_clone,
                        controller_clone,
                    )
                    .await
                    {
                        log::debug!("[AgentControlIPC] Client connection ended: {e}");
                    }
                });

                let mut retry_delay = tokio::time::Duration::from_millis(100);
                server = loop {
                    if *shutdown_rx.borrow() {
                        return Ok(());
                    }
                    match ServerOptions::new().create(&pipe_name) {
                        Ok(s) => break s,
                        Err(e) => {
                            log::error!(
                                "[AgentControlIPC] Failed to create next named pipe instance: {e}, retrying in {retry_delay:?}..."
                            );
                            tokio::select! {
                                _ = tokio::time::sleep(retry_delay) => {},
                                changed = shutdown_rx.changed() => {
                                    if changed.is_ok() && *shutdown_rx.borrow() {
                                        return Ok(());
                                    }
                                }
                            }
                            retry_delay = (retry_delay * 2).min(tokio::time::Duration::from_secs(5));
                        }
                    }
                };
            }
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    log::info!("[AgentControlIPC] Shutdown requested on named pipe accept loop");
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(unix)]
pub async fn run_ipc_server(
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
    mut shutdown_rx: watch::Receiver<bool>,
    controller: Option<AgentControlServerController>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket_path = PathBuf::from(get_ipc_endpoint());
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
        if *shutdown_rx.borrow() {
            let _ = std::fs::remove_file(&socket_path);
            return Ok(());
        }

        tokio::select! {
            accept_res = listener.accept() => {
                let (stream, _) = match accept_res {
                    Ok(conn) => conn,
                    Err(e) => {
                        log::warn!("[AgentControlIPC] Accept connection error: {e}");
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let facade_clone = facade.clone();
                let tracker_clone = tracker.clone();
                let app_handle_clone = app_handle.clone();
                let controller_clone = controller.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = handle_connection_tracked(
                        stream,
                        facade_clone,
                        tracker_clone,
                        app_handle_clone,
                        controller_clone,
                    )
                    .await
                    {
                        log::debug!("[AgentControlIPC] Client connection ended: {e}");
                    }
                });
            }
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    let _ = std::fs::remove_file(&socket_path);
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(not(any(windows, unix)))]
pub async fn run_ipc_server(
    _facade: AgentControlFacade,
    _tracker: ActiveConnectionTracker,
    _app_handle: Option<tauri::AppHandle>,
    mut shutdown_rx: watch::Receiver<bool>,
    _controller: Option<AgentControlServerController>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::warn!("[AgentControlIPC] IPC server is not supported on this platform");
    let _ = shutdown_rx.changed().await;
    Ok(())
}

pub fn start_agent_control_ipc_server(
    facade: AgentControlFacade,
    tracker: ActiveConnectionTracker,
    app_handle: Option<tauri::AppHandle>,
) -> AgentControlServerController {
    let controller = AgentControlServerController::new(tracker);
    controller.start(app_handle, facade);
    controller
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
    async fn test_handle_connection_concurrent_requests_no_blocking() {
        let (facade, _dir) = create_test_facade().await;
        let (client, server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let _ = handle_connection(server, facade).await;
        });

        let (client_read, mut client_write) = tokio::io::split(client);
        let mut reader = tokio::io::BufReader::new(client_read);

        let req1 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 101,
            "method": "sona_get_client_state",
            "params": {}
        });
        let req2 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 102,
            "method": "sona_get_sync_status",
            "params": {}
        });
        let payload = format!(
            "{}\n{}\n",
            serde_json::to_string(&req1).unwrap(),
            serde_json::to_string(&req2).unwrap()
        );
        client_write.write_all(payload.as_bytes()).await.unwrap();
        client_write.flush().await.unwrap();

        let mut resp1 = String::new();
        reader.read_line(&mut resp1).await.unwrap();
        let val1: serde_json::Value = serde_json::from_str(&resp1).unwrap();

        let mut resp2 = String::new();
        reader.read_line(&mut resp2).await.unwrap();
        let val2: serde_json::Value = serde_json::from_str(&resp2).unwrap();

        let ids: std::collections::HashSet<i64> = [
            val1.get("id").and_then(|v| v.as_i64()).unwrap(),
            val2.get("id").and_then(|v| v.as_i64()).unwrap(),
        ]
        .into_iter()
        .collect();

        assert!(ids.contains(&101));
        assert!(ids.contains(&102));
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
    async fn test_dispatch_error_mapping_cancellation() {
        let (facade, _dir) = create_test_facade().await;

        // Regular facade errors map to -32000
        let stop_err =
            dispatch_rpc_call("sona_stop_recording", Some(serde_json::json!({})), &facade)
                .await
                .unwrap_err();
        assert_eq!(stop_err.0, -32000);
        assert!(stop_err.1.starts_with("NO_ACTIVE_RECORDING"));

        // Cancellation errors map to -32800 (REQUEST_CANCELLED)
        let rpc_err = dispatch::map_rpc_error("TASK_CANCELLED: Task cancelled.".to_string());
        assert_eq!(rpc_err.0, -32800);
        assert_eq!(rpc_err.1, "TASK_CANCELLED: Task cancelled.");

        let rpc_err2 = dispatch::map_rpc_error("Task cancelled.".to_string());
        assert_eq!(rpc_err2.0, -32800);
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

    #[test]
    fn test_resolve_ipc_endpoint_precedence() {
        let ep1 = resolve_ipc_endpoint(|k| match k {
            "SONA_IPC_ENDPOINT" => Some("custom-endpoint".to_string()),
            "SONA_IPC_PIPE" => Some("custom-pipe".to_string()),
            _ => None,
        });
        assert_eq!(ep1, "custom-endpoint");

        let ep2 = resolve_ipc_endpoint(|k| match k {
            "SONA_IPC_PIPE" => Some(r"\\.\pipe\custom-pipe".to_string()),
            _ => None,
        });
        assert_eq!(ep2, r"\\.\pipe\custom-pipe");

        let ep3 = resolve_ipc_endpoint(|k| match k {
            "SONA_IPC_ENDPOINT" => Some("   ".to_string()),
            "SONA_IPC_PIPE" => Some("".to_string()),
            _ => None,
        });
        #[cfg(windows)]
        assert_eq!(ep3, WINDOWS_PIPE_NAME);
        #[cfg(unix)]
        assert_eq!(ep3, get_unix_socket_path().to_string_lossy().to_string());
    }

    #[test]
    fn test_find_string_param_and_extraction() {
        let mut map = serde_json::Map::new();
        map.insert("history_id".to_string(), serde_json::json!("h-snake"));
        map.insert("instanceId".to_string(), serde_json::json!("inst-camel"));
        map.insert("customField".to_string(), serde_json::json!("custom-val"));

        assert_eq!(find_string_param(&map, "history_id"), Some("h-snake"));
        assert_eq!(find_string_param(&map, "instance_id"), Some("inst-camel"));
        assert_eq!(find_string_param(&map, "custom_field"), Some("custom-val"));
        assert_eq!(find_string_param(&map, "missing_key"), None);

        let obj_params = Some(serde_json::Value::Object(map));
        assert_eq!(
            extract_string_param(&obj_params, "history_id").unwrap(),
            "h-snake"
        );
        assert_eq!(
            extract_string_param(&obj_params, "instance_id").unwrap(),
            "inst-camel"
        );
        assert_eq!(
            extract_string_param(&obj_params, "custom_field").unwrap(),
            "custom-val"
        );
        assert!(extract_string_param(&obj_params, "missing_key").is_err());
        let mut map_task_id = serde_json::Map::new();
        map_task_id.insert("task_id".to_string(), serde_json::json!("task-123"));
        assert_eq!(
            find_string_param(&map_task_id, "instance_id"),
            Some("task-123")
        );
        let obj_task_id = Some(serde_json::Value::Object(map_task_id));
        assert_eq!(
            extract_string_param(&obj_task_id, "instance_id").unwrap(),
            "task-123"
        );

        let mut map_task_id_camel = serde_json::Map::new();
        map_task_id_camel.insert("taskId".to_string(), serde_json::json!("task-camel-456"));
        assert_eq!(
            find_string_param(&map_task_id_camel, "instance_id"),
            Some("task-camel-456")
        );
        let obj_task_id_camel = Some(serde_json::Value::Object(map_task_id_camel));
        assert_eq!(
            extract_string_param(&obj_task_id_camel, "instance_id").unwrap(),
            "task-camel-456"
        );

        let bare_str = Some(serde_json::json!("bare-value"));
        assert_eq!(
            extract_string_param(&bare_str, "any_key").unwrap(),
            "bare-value"
        );

        assert_eq!(
            parse_optional_string(&obj_params, "instance_id"),
            Some("inst-camel".to_string())
        );
        assert_eq!(parse_optional_string(&obj_params, "missing_key"), None);
    }

    #[test]
    fn test_tool_permission_classification_and_check() {
        assert_eq!(
            classify_tool("sona_get_client_state", None),
            ToolPermissionCategory::ReadOnly
        );
        assert_eq!(
            classify_tool("sona_start_recording", None),
            ToolPermissionCategory::Recording
        );
        assert_eq!(
            classify_tool("sona_edit_transcript", None),
            ToolPermissionCategory::TranscriptEdit
        );
        assert_eq!(
            classify_tool("sona_delete_history", None),
            ToolPermissionCategory::Destructive
        );
        assert_eq!(
            classify_tool(
                "sona_stop_recording",
                Some(&serde_json::json!({ "discard": true }))
            ),
            ToolPermissionCategory::Destructive
        );
        assert_eq!(
            classify_tool(
                "sona_stop_recording",
                Some(&serde_json::json!({ "discard": false }))
            ),
            ToolPermissionCategory::Recording
        );
        assert_eq!(
            classify_tool("sona_delete_summary", None),
            ToolPermissionCategory::Destructive
        );
        assert_eq!(
            classify_tool("unknown_dangerous_method", None),
            ToolPermissionCategory::Destructive
        );

        let mut config = sona_core::config::AppConfig {
            mcp_permission_mode: Some("read_only".to_string()),
            ..Default::default()
        };
        assert!(
            check_permission(
                "get_client_state",
                ToolPermissionCategory::ReadOnly,
                &config
            )
            .is_ok()
        );
        assert!(
            check_permission(
                "start_recording",
                ToolPermissionCategory::Recording,
                &config
            )
            .is_err()
        );
        assert_eq!(
            check_permission(
                "start_recording",
                ToolPermissionCategory::Recording,
                &config
            )
            .unwrap_err()
            .0,
            -32001
        );

        config.mcp_permission_mode = Some("granular".to_string());
        config.mcp_allow_recording = Some(true);
        config.mcp_allow_destructive = Some(false);
        assert!(
            check_permission(
                "start_recording",
                ToolPermissionCategory::Recording,
                &config
            )
            .is_ok()
        );
        assert!(
            check_permission(
                "delete_history",
                ToolPermissionCategory::Destructive,
                &config
            )
            .is_err()
        );

        config.mcp_permission_mode = Some("full".to_string());
        assert!(
            check_permission(
                "delete_history",
                ToolPermissionCategory::Destructive,
                &config
            )
            .is_ok()
        );
    }

    #[tokio::test]
    async fn test_controller_lifecycle_and_activity_logs() {
        let tracker = ActiveConnectionTracker::new();
        let controller = AgentControlServerController::new(tracker);
        assert!(!controller.is_running());

        controller
            .record_activity(
                McpActivityLogEntry {
                    id: "act-1".to_string(),
                    timestamp: 1000,
                    method: "sona_query_history".to_string(),
                    category: "read_only".to_string(),
                    status: "success".to_string(),
                    duration_ms: 12,
                    params_summary: Some("history: 1".to_string()),
                    error_message: None,
                },
                None,
                false,
            )
            .await;

        let logs = controller.get_activity_logs().await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].id, "act-1");
        assert_eq!(logs[0].method, "sona_query_history");

        controller.clear_activity_logs().await;
        let logs_cleared = controller.get_activity_logs().await;
        assert!(logs_cleared.is_empty());
    }

    #[tokio::test]
    async fn test_permission_gate_blocks_mutation_in_read_only_mode() {
        let (facade, _dir) = create_test_facade().await;
        facade
            .services()
            .config
            .set_setting("mcpPermissionMode", &serde_json::json!("read_only"))
            .unwrap();

        let tracker = ActiveConnectionTracker::new();
        let controller = AgentControlServerController::new(tracker.clone());
        let (client, server) = tokio::io::duplex(4096);

        let facade_clone = facade.clone();
        let controller_clone = controller.clone();
        tokio::spawn(async move {
            let _ = handle_connection_tracked(
                server,
                facade_clone,
                tracker,
                None,
                Some(controller_clone),
            )
            .await;
        });

        let (client_read, mut client_write) = tokio::io::split(client);
        let mut buf_reader = tokio::io::BufReader::new(client_read);

        // Try to call sona_start_recording in read_only mode
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sona_start_recording",
            "params": {}
        });
        let mut req_bytes = serde_json::to_vec(&req).unwrap();
        req_bytes.push(b'\n');
        client_write.write_all(&req_bytes).await.unwrap();
        client_write.flush().await.unwrap();

        let mut line = String::new();
        buf_reader.read_line(&mut line).await.unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(resp.get("error").is_some(), "response should be an error");
        let err = resp.get("error").unwrap();
        assert_eq!(err.get("code").and_then(|v| v.as_i64()), Some(-32001));
        assert!(
            err.get("message")
                .and_then(|v| v.as_str())
                .unwrap()
                .contains("blocked by Sona desktop security policy")
        );

        // Verify log was recorded with blocked status
        let logs = controller.get_activity_logs().await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].status, "blocked");
        assert_eq!(logs[0].method, "sona_start_recording");
    }

    #[tokio::test]
    async fn test_sequential_fifo_execution_and_graceful_drain() {
        let (facade, _dir) = create_test_facade().await;
        let tracker = ActiveConnectionTracker::new();
        let (client, server) = tokio::io::duplex(8192);

        let facade_clone = facade.clone();
        let tracker_clone = tracker.clone();
        let server_handle = tokio::spawn(async move {
            let _ =
                handle_connection_tracked(server, facade_clone, tracker_clone, None, None).await;
        });

        let (client_read, mut client_write) = tokio::io::split(client);
        let mut buf_reader = tokio::io::BufReader::new(client_read);

        // Send 3 requests in quick succession
        for id in 1..=3 {
            let req = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "sona_get_client_state",
                "params": {}
            });
            let mut req_bytes = serde_json::to_vec(&req).unwrap();
            req_bytes.push(b'\n');
            client_write.write_all(&req_bytes).await.unwrap();
        }
        client_write.flush().await.unwrap();
        client_write.shutdown().await.unwrap();
        drop(client_write);

        // All 3 responses should be received in FIFO order even after client_write was closed
        for expected_id in 1..=3 {
            let mut line = String::new();
            let n = buf_reader.read_line(&mut line).await.unwrap();
            assert!(n > 0, "expected response for request {expected_id}");
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(
                resp.get("id").and_then(|v| v.as_i64()),
                Some(expected_id),
                "response id must match request id in FIFO order"
            );
        }

        // Now reader should see EOF
        let mut line = String::new();
        let n = buf_reader.read_line(&mut line).await.unwrap();
        assert_eq!(n, 0, "server should close stream after draining responses");

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_priority_ping_and_cancellation_dispatch() {
        let (facade, _dir) = create_test_facade().await;
        let tracker = ActiveConnectionTracker::new();
        let (client, server) = tokio::io::duplex(8192);

        let facade_clone = facade.clone();
        let tracker_clone = tracker.clone();
        let server_handle = tokio::spawn(async move {
            let _ =
                handle_connection_tracked(server, facade_clone, tracker_clone, None, None).await;
        });

        let (client_read, mut client_write) = tokio::io::split(client);
        let mut buf_reader = tokio::io::BufReader::new(client_read);

        // 1. Send ping with id 42
        let ping_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "ping"
        });
        let mut ping_bytes = serde_json::to_vec(&ping_req).unwrap();
        ping_bytes.push(b'\n');
        client_write.write_all(&ping_bytes).await.unwrap();
        client_write.flush().await.unwrap();

        let mut line = String::new();
        let n = buf_reader.read_line(&mut line).await.unwrap();
        assert!(n > 0, "expected ping response");
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp.get("id").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(resp.get("result"), Some(&serde_json::json!({})));

        // 1b. Send ping notification without id (must not receive any response)
        let ping_notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ping"
        });
        let mut notif_bytes = serde_json::to_vec(&ping_notif).unwrap();
        notif_bytes.push(b'\n');
        client_write.write_all(&notif_bytes).await.unwrap();
        client_write.flush().await.unwrap();

        // 2. Send cancellation notices
        for cancel_method in ["notifications/cancelled", "$/cancelRequest"] {
            let cancel_req = serde_json::json!({
                "jsonrpc": "2.0",
                "method": cancel_method,
                "params": { "requestId": 123 }
            });
            let mut cancel_bytes = serde_json::to_vec(&cancel_req).unwrap();
            cancel_bytes.push(b'\n');
            client_write.write_all(&cancel_bytes).await.unwrap();
        }
        client_write.flush().await.unwrap();

        // 3. Send regular request after cancellations to verify connection is intact
        let regular_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 43,
            "method": "sona_get_client_state",
            "params": {}
        });
        let mut regular_bytes = serde_json::to_vec(&regular_req).unwrap();
        regular_bytes.push(b'\n');
        client_write.write_all(&regular_bytes).await.unwrap();
        client_write.flush().await.unwrap();

        let mut reg_line = String::new();
        let n = buf_reader.read_line(&mut reg_line).await.unwrap();
        assert!(n > 0, "expected response for regular request");
        let reg_resp: serde_json::Value = serde_json::from_str(&reg_line).unwrap();
        assert_eq!(reg_resp.get("id").and_then(|v| v.as_i64()), Some(43));

        client_write.shutdown().await.unwrap();
        drop(client_write);
        server_handle.await.unwrap();
    }
}
