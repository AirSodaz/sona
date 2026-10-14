use clap::Parser;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub mod client;
pub mod prompts;
pub mod protocol;
pub mod resources;
pub mod tools;

use client::IpcClient;
use protocol::{JsonRpcRequest, JsonRpcResponse};

#[derive(Parser, Debug)]
#[command(
    name = "sona-mcp",
    about = "Model Context Protocol (MCP) server for Sona desktop client (MCP 2.0 / 2026-07-28)",
    version
)]
struct Args {
    #[arg(long, help = "Custom IPC pipe or socket endpoint")]
    endpoint: Option<String>,
}
#[derive(Debug, Clone)]
pub struct ActiveTaskInfo {
    pub abort_handle: tokio::task::AbortHandle,
    pub batch_instance_id: Option<String>,
}
pub type ActiveTaskRegistry = Arc<tokio::sync::Mutex<HashMap<serde_json::Value, ActiveTaskInfo>>>;

pub async fn cancel_active_task(
    active_tasks: &ActiveTaskRegistry,
    target_id: &serde_json::Value,
    client: Option<&IpcClient>,
) -> Option<ActiveTaskInfo> {
    let mut tasks = active_tasks.lock().await;
    if let Some(info) = tasks.remove(target_id) {
        info.abort_handle.abort();
        if let (Some(batch_id), Some(client)) = (&info.batch_instance_id, client) {
            let client = client.clone();
            let batch_id = batch_id.clone();
            tokio::spawn(async move {
                let _ = client
                    .call(
                        "sona_cancel_batch_task",
                        serde_json::json!({ "instance_id": batch_id }),
                    )
                    .await;
            });
        }
        Some(info)
    } else {
        None
    }
}
pub async fn handle_request(
    req: JsonRpcRequest,
    client: &IpcClient,
    is_initialized: &AtomicBool,
) -> Option<JsonRpcResponse> {
    // Notifications (requests without an ID) MUST NOT send responses per JSON-RPC 2.0
    let id = match req.id {
        Some(id) => id,
        None => {
            if req.method == "notifications/initialized" {
                is_initialized.store(true, Ordering::SeqCst);
            }
            return None;
        }
    };

    if req.jsonrpc != protocol::JSONRPC_VERSION {
        return Some(JsonRpcResponse::error(
            id,
            protocol::INVALID_REQUEST,
            "Invalid Request: jsonrpc must be '2.0'",
        ));
    }

    if req.method != "server/discover"
        && req.method != "initialize"
        && req.method != "ping"
        && !is_initialized.load(Ordering::SeqCst)
    {
        return Some(JsonRpcResponse::error(
            id,
            protocol::SERVER_NOT_INITIALIZED,
            "Server not initialized. Call 'initialize' or 'server/discover' first.",
        ));
    }
    let params_val = req.params.as_ref();
    let meta = params_val.and_then(|p| p.get("_meta"));

    // Modern MCP 2026-07-28 per-request _meta validation
    if let Some(meta_val) = meta {
        let Some(meta_obj) = meta_val.as_object() else {
            return Some(JsonRpcResponse::error(
                id,
                protocol::INVALID_PARAMS,
                "Invalid params: '_meta' must be an object",
            ));
        };

        if req.method == "server/discover"
            || meta_obj.contains_key("io.modelcontextprotocol/protocolVersion")
        {
            // Validate protocol version
            let version = meta_obj
                .get("io.modelcontextprotocol/protocolVersion")
                .and_then(|v| v.as_str());

            match version {
                Some(v) if protocol::SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => {}
                Some(v) => {
                    return Some(JsonRpcResponse::unsupported_protocol_version(id, v));
                }
                None => {
                    return Some(JsonRpcResponse::error(
                        id,
                        protocol::INVALID_PARAMS,
                        "Invalid params: missing required 'io.modelcontextprotocol/protocolVersion' in _meta",
                    ));
                }
            }

            // Validate client capabilities
            let client_caps = meta_obj.get("io.modelcontextprotocol/clientCapabilities");
            if !client_caps.is_some_and(|c| c.is_object()) {
                return Some(JsonRpcResponse::error(
                    id,
                    protocol::INVALID_PARAMS,
                    "Invalid params: missing required 'io.modelcontextprotocol/clientCapabilities' in _meta",
                ));
            }
        }
    } else if req.method == "server/discover" {
        return Some(JsonRpcResponse::error(
            id,
            protocol::INVALID_PARAMS,
            "Invalid params: 'server/discover' requires _meta with protocolVersion and clientCapabilities",
        ));
    }

    let response = match req.method.as_str() {
        "server/discover" => {
            is_initialized.store(true, Ordering::SeqCst);
            let result = serde_json::json!({
                "resultType": "complete",
                "supportedVersions": protocol::SUPPORTED_PROTOCOL_VERSIONS,
                "capabilities": protocol::server_capabilities(),
                "instructions": "Model Context Protocol (MCP) server for Sona desktop client, providing speech-to-text, transcript management, and recording controls.",
                "ttlMs": 3600000,
                "cacheScope": "public",
                "_meta": protocol::server_info_meta(),
            });
            JsonRpcResponse::success(id, result)
        }
        "initialize" => {
            is_initialized.store(true, Ordering::SeqCst);
            // Dual-era backward compatibility for legacy clients (SEP-2575)
            let requested_version = req
                .params
                .as_ref()
                .and_then(|p| p.get("protocolVersion"))
                .and_then(|v| v.as_str());
            let negotiated_version = match requested_version {
                Some(v) if protocol::SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => v,
                _ => protocol::LATEST_PROTOCOL_VERSION,
            };
            let result = serde_json::json!({
                "protocolVersion": negotiated_version,
                "capabilities": protocol::server_capabilities(),
                "serverInfo": {
                    "name": "sona-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });
            JsonRpcResponse::success(id, result)
        }
        "ping" => JsonRpcResponse::success(id, serde_json::json!({})),
        "tools/list" => {
            let tool_list = tools::list_tools();
            let result = serde_json::json!({
                "resultType": "complete",
                "tools": tool_list,
                "ttlMs": 300000,
                "cacheScope": "public",
                "_meta": protocol::server_info_meta(),
            });
            JsonRpcResponse::success(id, result)
        }
        "tools/call" => {
            let params = req.params.unwrap_or_else(|| serde_json::json!({}));
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            let tool_res = tools::call_tool(tool_name, arguments, client).await;
            match serde_json::to_value(&tool_res) {
                Ok(val) => JsonRpcResponse::success(id, val),
                Err(e) => JsonRpcResponse::error(id, protocol::INTERNAL_ERROR, e.to_string()),
            }
        }
        "resources/list" => {
            let resource_list = resources::list_resources();
            let result = serde_json::json!({
                "resultType": "complete",
                "resources": resource_list,
                "ttlMs": 300000,
                "cacheScope": "public",
                "_meta": protocol::server_info_meta(),
            });
            JsonRpcResponse::success(id, result)
        }
        "resources/read" => {
            let params = req.params.unwrap_or_else(|| serde_json::json!({}));
            let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            match resources::read_resource(uri, client).await {
                Ok(content) => {
                    let result = serde_json::json!({
                        "resultType": "complete",
                        "contents": [content],
                        "ttlMs": 60000,
                        "cacheScope": "private",
                        "_meta": protocol::server_info_meta(),
                    });
                    JsonRpcResponse::success(id, result)
                }
                Err(err) => JsonRpcResponse::error(id, protocol::INVALID_PARAMS, err),
            }
        }
        "resources/templates/list" => {
            let templates = resources::list_resource_templates();
            let result = serde_json::json!({
                "resultType": "complete",
                "resourceTemplates": templates,
                "ttlMs": 300000,
                "cacheScope": "public",
                "_meta": protocol::server_info_meta(),
            });
            JsonRpcResponse::success(id, result)
        }
        "prompts/list" => {
            let prompt_list = prompts::list_prompts();
            let result = serde_json::json!({
                "resultType": "complete",
                "prompts": prompt_list,
                "ttlMs": 600000,
                "cacheScope": "public",
                "_meta": protocol::server_info_meta(),
            });
            JsonRpcResponse::success(id, result)
        }
        "prompts/get" => {
            let params = req.params.unwrap_or_else(|| serde_json::json!({}));
            let prompt_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            match prompts::get_prompt(prompt_name, arguments, client).await {
                Ok(messages) => {
                    let result = serde_json::json!({
                        "resultType": "complete",
                        "messages": messages,
                        "_meta": protocol::server_info_meta(),
                    });
                    JsonRpcResponse::success(id, result)
                }
                Err(err) => JsonRpcResponse::error(id, protocol::INVALID_PARAMS, err),
            }
        }
        _ => JsonRpcResponse::error(
            id,
            protocol::METHOD_NOT_FOUND,
            format!("Method '{}' not found", req.method),
        ),
    };

    Some(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    eprintln!(
        "[sona-mcp] Starting Sona MCP Server v{} (spec 2026-07-28) over stdio...",
        env!("CARGO_PKG_VERSION")
    );

    let client = match args.endpoint {
        Some(ep) => IpcClient::with_endpoint(ep),
        None => IpcClient::new(),
    };
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel::<JsonRpcResponse>(64);
    let active_tasks: ActiveTaskRegistry = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let is_initialized = Arc::new(AtomicBool::new(false));
    let writer_handle = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(resp) = resp_rx.recv().await {
            if let Ok(resp_str) = serde_json::to_string(&resp) {
                let _ = stdout.write_all(resp_str.as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
            }
        }
    });

    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => {
                if req.id.is_none() {
                    // Handle MCP 2.0 / JSON-RPC notifications
                    if req.method == "notifications/initialized" {
                        is_initialized.store(true, Ordering::SeqCst);
                    }
                    if req.method == "notifications/cancelled" || req.method == "$/cancelRequest" {
                        let target_id = req
                            .params
                            .as_ref()
                            .and_then(|p| p.get("requestId").or_else(|| p.get("id")).cloned());
                        if let Some(target_id) = target_id
                            && cancel_active_task(&active_tasks, &target_id, Some(&client))
                                .await
                                .is_some()
                        {
                            eprintln!(
                                "[sona-mcp] Aborted in-flight task for request id: {target_id}"
                            );
                            let cancel_resp = JsonRpcResponse::error(
                                target_id,
                                protocol::REQUEST_CANCELLED,
                                "Request was cancelled by client",
                            );
                            let _ = resp_tx.send(cancel_resp).await;
                        }
                    }
                    // Notifications must not produce responses per JSON-RPC 2.0
                    line.clear();
                    continue;
                }

                let id = req.id.clone().unwrap();
                let mut req = req;
                let batch_instance_id = maybe_inject_transcribe_instance_id(&mut req);

                let client_clone = client.clone();
                let tx_clone = resp_tx.clone();
                let active_tasks_clone = active_tasks.clone();
                let is_initialized_clone = is_initialized.clone();
                let id_key = id.clone();

                let mut tasks = active_tasks.lock().await;
                let handle = tokio::spawn(async move {
                    let maybe_resp =
                        handle_request(req, &client_clone, &is_initialized_clone).await;
                    let mut tasks = active_tasks_clone.lock().await;
                    if tasks.remove(&id_key).is_some()
                        && let Some(resp) = maybe_resp
                    {
                        let _ = tx_clone.send(resp).await;
                    }
                });
                tasks.insert(
                    id,
                    ActiveTaskInfo {
                        abort_handle: handle.abort_handle(),
                        batch_instance_id,
                    },
                );
                drop(tasks);
            }
            Err(e) => {
                let err_resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    protocol::PARSE_ERROR,
                    format!("Parse error: {e}"),
                );
                let _ = resp_tx.send(err_resp).await;
            }
        }

        line.clear();
    }

    drop(resp_tx);
    let _ = writer_handle.await;
    eprintln!("[sona-mcp] Stdio loop finished.");
    Ok(())
}

pub(crate) fn maybe_inject_transcribe_instance_id(req: &mut JsonRpcRequest) -> Option<String> {
    if req.method == "tools/call"
        && let Some(params_obj) = req.params.as_mut().and_then(|p| p.as_object_mut())
        && params_obj
            .get("name")
            .and_then(|n| n.as_str())
            .and_then(tools::canonical_tool_name)
            == Some("sona_transcribe_file")
    {
        let args_val = params_obj
            .entry("arguments")
            .or_insert_with(|| serde_json::json!({}));
        if !args_val.is_object() {
            *args_val = serde_json::json!({});
        }
        if let Some(args_obj) = args_val.as_object_mut() {
            let instance_id = match args_obj.get("instance_id").and_then(|v| v.as_str()) {
                Some(id) if !id.trim().is_empty() => id.to_string(),
                _ => {
                    let id = format!("mcp-transcribe-{}", uuid::Uuid::new_v4());
                    args_obj.insert(
                        "instance_id".to_string(),
                        serde_json::Value::String(id.clone()),
                    );
                    id
                }
            };
            return Some(instance_id);
        }
    }
    None
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
