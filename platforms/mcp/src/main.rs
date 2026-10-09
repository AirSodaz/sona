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
    about = "Model Context Protocol (MCP) server for Sona desktop client",
    version
)]
struct Args {}

pub async fn handle_request(req: JsonRpcRequest, client: &IpcClient) -> Option<JsonRpcResponse> {
    // Notifications (requests without an ID) MUST NOT send responses per JSON-RPC 2.0
    let id = match req.id {
        Some(id) => id,
        None => return None,
    };

    if req.jsonrpc != "2.0" {
        return Some(JsonRpcResponse::error(
            id,
            -32600,
            "Invalid Request: jsonrpc must be '2.0'",
        ));
    }
    let response = match req.method.as_str() {
        "initialize" => {
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false },
                    "prompts": { "listChanged": false }
                },
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
                "tools": tool_list
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
                Err(e) => JsonRpcResponse::error(id, -32603, e.to_string()),
            }
        }
        "resources/list" => {
            let resource_list = resources::list_resources();
            let result = serde_json::json!({
                "resources": resource_list
            });
            JsonRpcResponse::success(id, result)
        }
        "resources/read" => {
            let params = req.params.unwrap_or_else(|| serde_json::json!({}));
            let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            match resources::read_resource(uri, client).await {
                Ok(content) => {
                    let result = serde_json::json!({
                        "contents": [content]
                    });
                    JsonRpcResponse::success(id, result)
                }
                Err(err) => JsonRpcResponse::error(id, -32000, err),
            }
        }
        "resources/templates/list" => {
            let templates = resources::list_resource_templates();
            let result = serde_json::json!({
                "resourceTemplates": templates
            });
            JsonRpcResponse::success(id, result)
        }
        "prompts/list" => {
            let prompt_list = prompts::list_prompts();
            let result = serde_json::json!({
                "prompts": prompt_list
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
                        "messages": messages
                    });
                    JsonRpcResponse::success(id, result)
                }
                Err(err) => JsonRpcResponse::error(id, -32000, err),
            }
        }
        _ => JsonRpcResponse::error(id, -32601, format!("Method '{}' not found", req.method)),
    };

    Some(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _args = Args::parse();

    eprintln!(
        "[sona-mcp] Starting Sona MCP Server v{} over stdio...",
        env!("CARGO_PKG_VERSION")
    );

    let client = IpcClient::new();
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
                    -32700,
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

    #[tokio::test]
    async fn test_handle_initialize() {
        let client = IpcClient::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({})),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(1));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
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
        let client = IpcClient::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(2));
        let result = resp.result.unwrap();
        let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
        assert_eq!(tools.len(), 13);
    }

    #[tokio::test]
    async fn test_handle_resources_list() {
        let client = IpcClient::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(3)),
            method: "resources/list".to_string(),
            params: None,
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(3));
        let result = resp.result.unwrap();
        let resources = result.get("resources").and_then(|r| r.as_array()).unwrap();
        assert_eq!(resources.len(), 3);
    }

    #[tokio::test]
    async fn test_handle_prompts_list() {
        let client = IpcClient::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(4)),
            method: "prompts/list".to_string(),
            params: None,
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(4));
        let result = resp.result.unwrap();
        let prompts = result.get("prompts").and_then(|p| p.as_array()).unwrap();
        assert_eq!(prompts.len(), 2);
    }

    #[tokio::test]
    async fn test_handle_offline_get_client_state() {
        let client = IpcClient::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(5)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "sona_get_client_state",
                "arguments": {}
            })),
        };

        let resp = handle_request(req, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(5));
        let result = resp.result.unwrap();
        let content = result.get("content").and_then(|c| c.as_array()).unwrap();
        let text = content[0].get("text").and_then(|t| t.as_str()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.get("online").and_then(|o| o.as_bool()), Some(false));
    }
    #[tokio::test]
    async fn test_handle_notification_no_response() {
        let client = IpcClient::new();
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
        let client = IpcClient::new();
        let invalid = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            id: Some(serde_json::json!(99)),
            method: "ping".to_string(),
            params: None,
        };
        let resp = handle_request(invalid, &client).await.unwrap();
        assert_eq!(resp.id, serde_json::json!(99));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32600);
    }
}
