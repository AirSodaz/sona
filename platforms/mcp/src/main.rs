use clap::Parser;
use std::collections::HashMap;
use std::sync::Arc;
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
pub async fn handle_request(req: JsonRpcRequest, client: &IpcClient) -> Option<JsonRpcResponse> {
    // Notifications (requests without an ID) MUST NOT send responses per JSON-RPC 2.0
    let id = match req.id {
        Some(id) => id,
        None => return None,
    };

    if req.jsonrpc != protocol::JSONRPC_VERSION {
        return Some(JsonRpcResponse::error(
            id,
            protocol::INVALID_REQUEST,
            "Invalid Request: jsonrpc must be '2.0'",
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
                let mut batch_instance_id = None;
                if req.method == "tools/call"
                    && let Some(params_obj) = req.params.as_mut().and_then(|p| p.as_object_mut())
                    && params_obj.get("name").and_then(|n| n.as_str())
                        == Some("sona_transcribe_file")
                {
                    let args_val = params_obj
                        .entry("arguments")
                        .or_insert_with(|| serde_json::json!({}));
                    if !args_val.is_object() {
                        *args_val = serde_json::json!({});
                    }
                    if let Some(args_obj) = args_val.as_object_mut() {
                        let instance_id = match args_obj.get("instance_id").and_then(|v| v.as_str())
                        {
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
                        batch_instance_id = Some(instance_id);
                    }
                }

                let client_clone = client.clone();
                let tx_clone = resp_tx.clone();
                let active_tasks_clone = active_tasks.clone();
                let id_key = id.clone();

                let mut tasks = active_tasks.lock().await;
                let handle = tokio::spawn(async move {
                    let maybe_resp = handle_request(req, &client_clone).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn offline_test_client() -> IpcClient {
        #[cfg(windows)]
        {
            IpcClient::with_endpoint(r"\\.\pipe\sona-offline-test-dummy".to_string())
        }
        #[cfg(not(windows))]
        {
            IpcClient::with_endpoint("/tmp/sona-offline-test-dummy.sock".to_string())
        }
    }

    #[tokio::test]
    async fn test_handle_server_discover() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "server/discover".to_string(),
            params: Some(serde_json::json!({
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {},
                    "io.modelcontextprotocol/clientInfo": {
                        "name": "TestClient",
                        "version": "1.0.0"
                    }
                }
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(1));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("resultType").and_then(|r| r.as_str()),
            Some("complete")
        );
        let versions = result
            .get("supportedVersions")
            .and_then(|v| v.as_array())
            .unwrap();
        assert!(versions.iter().any(|v| v.as_str() == Some("2026-07-28")));
        assert!(result.get("capabilities").is_some());
        assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(3600000));
        assert_eq!(
            result.get("cacheScope").and_then(|s| s.as_str()),
            Some("public")
        );
        let meta = result.get("_meta").unwrap();
        assert_eq!(
            meta.get("io.modelcontextprotocol/serverInfo")
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str()),
            Some("sona-mcp")
        );
    }

    #[tokio::test]
    async fn test_handle_server_discover_missing_meta() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "server/discover".to_string(),
            params: None,
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(1));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_handle_unsupported_protocol_version() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "tools/list".to_string(),
            params: Some(serde_json::json!({
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2023-01-01",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(2));
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, protocol::UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(err.message, "Unsupported protocol version");
        let data = err.data.unwrap();
        let supported = data.get("supported").and_then(|s| s.as_array()).unwrap();
        assert!(supported.iter().any(|v| v.as_str() == Some("2026-07-28")));
        assert!(supported.iter().any(|v| v.as_str() == Some("2024-11-05")));
        assert_eq!(
            data.get("requested").and_then(|r| r.as_str()),
            Some("2023-01-01")
        );
    }

    #[tokio::test]
    async fn test_handle_supported_protocol_version_2024_11_05() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(200)),
            method: "tools/list".to_string(),
            params: Some(serde_json::json!({
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2024-11-05",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(200));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("resultType").and_then(|r| r.as_str()),
            Some("complete")
        );
    }

    #[tokio::test]
    async fn test_handle_missing_client_capabilities() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(3)),
            method: "tools/list".to_string(),
            params: Some(serde_json::json!({
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28"
                }
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(3));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_handle_standard_mcp_meta_progress_token() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".to_string(),
            params: Some(serde_json::json!({
                "_meta": {
                    "progressToken": 123
                }
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(1));
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(10)),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({})),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(10));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("protocolVersion").and_then(|v| v.as_str()),
            Some("2026-07-28")
        );
        assert_eq!(
            result
                .get("serverInfo")
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str()),
            Some("sona-mcp")
        );
    }

    #[tokio::test]
    async fn test_handle_initialize_negotiates_version() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(11)),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05"
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(11));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("protocolVersion").and_then(|v| v.as_str()),
            Some("2024-11-05")
        );
    }

    #[tokio::test]
    async fn test_handle_tools_list() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(20)),
            method: "tools/list".to_string(),
            params: Some(serde_json::json!({
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {}
                }
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(20));
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("resultType").and_then(|t| t.as_str()),
            Some("complete")
        );
        assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(300000));
        assert_eq!(
            result.get("cacheScope").and_then(|s| s.as_str()),
            Some("public")
        );
        let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
        assert_eq!(tools.len(), 40);
        assert_eq!(
            tools[0].get("title").and_then(|t| t.as_str()),
            Some("Get Client State")
        );
    }

    #[tokio::test]
    async fn test_handle_resource_templates_list() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(301)),
            method: "resources/templates/list".to_string(),
            params: None,
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(301));
        let result = resp.result.unwrap();
        let templates = result
            .get("resourceTemplates")
            .and_then(|r| r.as_array())
            .unwrap();
        assert_eq!(templates.len(), 4);
        let uris: Vec<&str> = templates
            .iter()
            .filter_map(|t| t.get("uriTemplate").and_then(|u| u.as_str()))
            .collect();
        assert!(uris.contains(&"sona://history/{history_id}/translation"));
    }

    #[tokio::test]
    async fn test_handle_resources_list() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(30)),
            method: "resources/list".to_string(),
            params: None,
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(30));
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("resultType").and_then(|t| t.as_str()),
            Some("complete")
        );
        assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(300000));
        let resources = result.get("resources").and_then(|r| r.as_array()).unwrap();
        assert_eq!(resources.len(), 7);
        assert!(!resources.iter().any(|r| {
            r.get("uri")
                .and_then(|u| u.as_str())
                .is_some_and(|u| u.contains("{history_id}"))
        }));
        assert_eq!(
            resources[0].get("title").and_then(|t| t.as_str()),
            Some("Client Status")
        );
    }

    #[tokio::test]
    async fn test_handle_resources_read_not_found() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(31)),
            method: "resources/read".to_string(),
            params: Some(serde_json::json!({
                "uri": "sona://unknown-resource"
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(31));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_handle_prompts_list() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(40)),
            method: "prompts/list".to_string(),
            params: None,
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(40));
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("resultType").and_then(|t| t.as_str()),
            Some("complete")
        );
        assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(600000));
        let prompts = result.get("prompts").and_then(|p| p.as_array()).unwrap();
        assert_eq!(prompts.len(), 2);
        assert_eq!(
            prompts[0].get("title").and_then(|t| t.as_str()),
            Some("Summarize Meeting")
        );
    }

    #[tokio::test]
    async fn test_handle_prompts_get_not_found() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(41)),
            method: "prompts/get".to_string(),
            params: Some(serde_json::json!({
                "name": "nonexistent_prompt",
                "arguments": { "history_id": "hist-1" }
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(41));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_handle_offline_get_client_state() {
        let client = offline_test_client();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(50)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "sona_get_client_state",
                "arguments": {}
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(50));
        let result = resp.result.unwrap();
        assert_eq!(
            result.get("resultType").and_then(|t| t.as_str()),
            Some("complete")
        );
        let content = result.get("content").and_then(|c| c.as_array()).unwrap();
        let text = content[0].get("text").and_then(|t| t.as_str()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.get("online").and_then(|o| o.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn test_handle_notification_no_response() {
        let client = offline_test_client();
        let notification = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let resp = handle_request(notification, &client).await;
        assert!(resp.is_none());
    }

    #[tokio::test]
    async fn test_handle_invalid_jsonrpc_version() {
        let client = offline_test_client();
        let invalid = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            id: Some(serde_json::json!(99)),
            method: "ping".to_string(),
            params: None,
        };
        let resp = handle_request(invalid, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(99));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, protocol::INVALID_REQUEST);
    }

    #[tokio::test]
    async fn test_notification_cancellation_aborts_active_task() {
        use std::time::Duration;

        let active_tasks: ActiveTaskRegistry = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let task_started = Arc::new(tokio::sync::Notify::new());
        let task_started_clone = task_started.clone();

        let req_id = serde_json::json!(42);
        let id_key = req_id.clone();
        let active_tasks_clone = active_tasks.clone();

        let handle = tokio::spawn(async move {
            task_started_clone.notify_waiters();
            tokio::time::sleep(Duration::from_secs(30)).await;
            active_tasks_clone.lock().await.remove(&id_key);
        });

        active_tasks.lock().await.insert(
            req_id.clone(),
            ActiveTaskInfo {
                abort_handle: handle.abort_handle(),
                batch_instance_id: None,
            },
        );

        task_started.notified().await;

        let cancelled = cancel_active_task(&active_tasks, &req_id, None).await;
        assert!(cancelled.is_some(), "Task should be found and cancelled");

        assert!(active_tasks.lock().await.get(&req_id).is_none());

        let join_res = handle.await;
        assert!(join_res.is_err());
        assert!(join_res.unwrap_err().is_cancelled());

        let second_cancel = cancel_active_task(&active_tasks, &req_id, None).await;
        assert!(second_cancel.is_none());
    }

    #[tokio::test]
    async fn test_notification_cancelled_parsing() {
        use std::time::Duration;

        let active_tasks: ActiveTaskRegistry = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let req_id = serde_json::json!("task-100");
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        active_tasks.lock().await.insert(
            req_id.clone(),
            ActiveTaskInfo {
                abort_handle: handle.abort_handle(),
                batch_instance_id: Some("batch-100".to_string()),
            },
        );

        let notification_json = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"task-100","reason":"user requested"}}"#;
        let req: JsonRpcRequest = serde_json::from_str(notification_json).unwrap();
        assert!(req.id.is_none());

        let target_id = req
            .params
            .as_ref()
            .and_then(|p| p.get("requestId").or_else(|| p.get("id")).cloned());
        assert_eq!(target_id, Some(req_id.clone()));

        let cancelled = cancel_active_task(&active_tasks, &target_id.unwrap(), None).await;
        assert!(cancelled.is_some());
        let info = cancelled.unwrap();
        assert_eq!(info.batch_instance_id.as_deref(), Some("batch-100"));
        assert!(handle.await.unwrap_err().is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellation_jsonrpc_error_response() {
        let resp = JsonRpcResponse::error(
            serde_json::json!(101),
            protocol::REQUEST_CANCELLED,
            "Request was cancelled by client",
        );
        assert_eq!(resp.id, serde_json::json!(101));
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32800);
        assert_eq!(err.message, "Request was cancelled by client");
    }

    #[test]
    fn test_batch_instance_id_injection() {
        let mut req: JsonRpcRequest = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sona_transcribe_file","arguments":{"file_path":"test.wav"}}}"#,
        )
        .unwrap();

        let mut batch_instance_id = None;
        if req.method == "tools/call"
            && let Some(params_obj) = req.params.as_mut().and_then(|p| p.as_object_mut())
            && params_obj.get("name").and_then(|n| n.as_str()) == Some("sona_transcribe_file")
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
                batch_instance_id = Some(instance_id);
            }
        }

        assert!(batch_instance_id.is_some());
        let id = batch_instance_id.unwrap();
        assert!(id.starts_with("mcp-transcribe-"));
        let injected = req.params.unwrap()["arguments"]["instance_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(injected, id);
    }
}
