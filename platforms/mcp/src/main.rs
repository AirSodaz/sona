use clap::Parser;
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
    if let Some(meta_obj) = meta {
        if !meta_obj.is_object() {
            return Some(JsonRpcResponse::error(
                id,
                protocol::INVALID_PARAMS,
                "Invalid params: '_meta' must be an object",
            ));
        }

        // Validate protocol version
        let version = meta_obj
            .get("io.modelcontextprotocol/protocolVersion")
            .and_then(|v| v.as_str());

        match version {
            Some(v) if v == protocol::LATEST_PROTOCOL_VERSION => {}
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
        if client_caps.is_none() || !client_caps.unwrap().is_object() {
            return Some(JsonRpcResponse::error(
                id,
                protocol::INVALID_PARAMS,
                "Invalid params: missing required 'io.modelcontextprotocol/clientCapabilities' in _meta",
            ));
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
            let result = serde_json::json!({
                "protocolVersion": protocol::LATEST_PROTOCOL_VERSION,
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
    let mut stdout = tokio::io::stdout();

    let mut line = String::new();
    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => {
                if let Some(resp) = handle_request(req, &client).await {
                    let resp_str = serde_json::to_string(&resp)?;
                    stdout.write_all(resp_str.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
            }
            Err(e) => {
                let err_resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    protocol::PARSE_ERROR,
                    format!("Parse error: {e}"),
                );
                let resp_str = serde_json::to_string(&err_resp)?;
                stdout.write_all(resp_str.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        line.clear();
    }

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
                    "io.modelcontextprotocol/protocolVersion": "2024-11-05",
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
        assert_eq!(
            data.get("requested").and_then(|r| r.as_str()),
            Some("2024-11-05")
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
        assert_eq!(tools.len(), 13);
        assert_eq!(
            tools[0].get("title").and_then(|t| t.as_str()),
            Some("Get Client State")
        );
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
        assert_eq!(resources.len(), 3);
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
}
