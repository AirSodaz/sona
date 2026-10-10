use serde::{Deserialize, Serialize};

pub const LATEST_PROTOCOL_VERSION: &str = "2026-07-28";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2026-07-28"];
pub const JSONRPC_VERSION: &str = "2.0";

// Standard JSON-RPC error codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// MCP 2026-07-28 specification error codes (SEP-2575 / SEP-2322)
pub const HEADER_MISMATCH: i32 = -32020;
pub const MISSING_REQUIRED_CLIENT_CAPABILITY: i32 = -32021;
pub const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32022;

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    pub fn error_with_data(
        id: serde_json::Value,
        code: i32,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
        }
    }

    pub fn unsupported_protocol_version(
        id: serde_json::Value,
        requested: impl Into<String>,
    ) -> Self {
        Self::error_with_data(
            id,
            UNSUPPORTED_PROTOCOL_VERSION,
            "Unsupported protocol version",
            serde_json::json!({
                "supported": SUPPORTED_PROTOCOL_VERSIONS,
                "requested": requested.into(),
            }),
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

pub fn server_info_meta() -> serde_json::Value {
    serde_json::json!({
        "io.modelcontextprotocol/serverInfo": {
            "name": "sona-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

pub fn server_capabilities() -> serde_json::Value {
    serde_json::json!({
        "tools": { "listChanged": false },
        "resources": { "subscribe": false, "listChanged": false },
        "prompts": { "listChanged": false }
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'static str>,
    pub description: &'static str,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub item_type: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallResult {
    #[serde(rename = "resultType")]
    pub result_type: &'static str,
    pub content: Vec<ContentItem>,
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl ToolCallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            result_type: "complete",
            content: vec![ContentItem {
                item_type: "text",
                text: text.into(),
            }],
            is_error: false,
            meta: Some(server_info_meta()),
        }
    }

    pub fn json(val: &serde_json::Value) -> Self {
        let text = serde_json::to_string_pretty(val).unwrap_or_else(|_| val.to_string());
        Self::text(text)
    }

    pub fn error(err: impl Into<String>) -> Self {
        Self {
            result_type: "complete",
            content: vec![ContentItem {
                item_type: "text",
                text: err.into(),
            }],
            is_error: true,
            meta: Some(server_info_meta()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceDefinition {
    pub uri: &'static str,
    pub name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'static str>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceTemplateDefinition {
    #[serde(rename = "uriTemplate")]
    pub uri_template: &'static str,
    pub name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'static str>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<&'static str>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptArgument {
    pub name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'static str>,
    pub description: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptDefinition {
    pub name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'static str>,
    pub description: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptMessage {
    pub role: &'static str,
    pub content: ContentItem,
}
