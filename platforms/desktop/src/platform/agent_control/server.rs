pub mod dispatch;
pub mod params;
pub mod tracker;

pub use dispatch::*;
pub use params::*;
pub use tracker::*;

use super::facade::AgentControlFacade;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

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

    let mut task_set = tokio::task::JoinSet::new();

    loop {
        while task_set.try_join_next().is_some() {}

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
                        } else if rpc_req.id.is_none() {
                            // JSON-RPC 2.0 Notification: process without response
                            let facade_clone = facade.clone();
                            task_set.spawn(async move {
                                let _ = dispatch_rpc_call(&rpc_req.method, rpc_req.params, &facade_clone).await;
                            });
                        } else {
                            let id = rpc_req.id.unwrap();
                            let facade_clone = facade.clone();
                            let resp_tx_clone = resp_tx.clone();
                            task_set.spawn(async move {
                                let resp_str = match dispatch_rpc_call(&rpc_req.method, rpc_req.params, &facade_clone).await {
                                    Ok(result) => {
                                        let resp = JsonRpcSuccessResponse {
                                            jsonrpc: "2.0",
                                            id,
                                            result,
                                        };
                                        serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string())
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
                                        serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string())
                                    }
                                };
                                let _ = resp_tx_clone.send(resp_str).await;
                            });
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

    task_set.abort_all();
    drop(resp_tx);
    let _ = writer_task.await;

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

        let mut retry_delay = tokio::time::Duration::from_millis(100);
        server = loop {
            match ServerOptions::new().create(WINDOWS_PIPE_NAME) {
                Ok(s) => break s,
                Err(e) => {
                    log::error!(
                        "[AgentControlIPC] Failed to create next named pipe instance: {e}, retrying in {retry_delay:?}..."
                    );
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(tokio::time::Duration::from_secs(5));
                }
            }
        };
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
        let (stream, _) = match listener.accept().await {
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
        while let Err(e) = run_ipc_server(facade.clone(), tracker.clone(), app_handle.clone()).await
        {
            log::error!("[AgentControlIPC] Server loop ended with error: {e}. Restarting in 1s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
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
}
