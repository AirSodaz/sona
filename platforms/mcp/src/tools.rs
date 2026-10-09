use crate::client::IpcClient;
use crate::protocol::{ToolCallResult, ToolDefinition};

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "sona_get_client_state",
            description: "Query Sona desktop client online status, recording state, and active project.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_launch_desktop",
            description: "Launch Sona desktop client when offline and poll IPC until connection is ready.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Timeout in seconds to wait for client to start (default 10)",
                        "default": 10
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_focus_window",
            description: "Bring Sona desktop window to front and focus.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_start_recording",
            description: "Start microphone capture and live transcription in Sona desktop client.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID to associate with the recording"
                    },
                    "device_name": {
                        "type": "string",
                        "description": "Optional specific microphone device name"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_stop_recording",
            description: "Stop active recording session and finalize into history or discard draft.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "discard": {
                        "type": "boolean",
                        "description": "If true, discard and delete the recording draft instead of saving",
                        "default": false
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_query_history",
            description: "Full-text search and filter transcript history in Sona desktop client.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query keyword",
                        "default": ""
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID filter"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max items to return (default 20)",
                        "default": 20
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Pagination offset (default 0)",
                        "default": 0
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_read_transcript",
            description: "Read full transcript segments and text of a history record.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    }
                },
                "required": ["history_id"]
            }),
        },
        ToolDefinition {
            name: "sona_edit_transcript",
            description: "Commit modified transcript segments and create an immutable history version snapshot.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    },
                    "segments": {
                        "type": "array",
                        "description": "Updated transcript segments array",
                        "items": { "type": "object" }
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason for editing"
                    }
                },
                "required": ["history_id", "segments"]
            }),
        },
        ToolDefinition {
            name: "sona_delete_history",
            description: "Delete or permanently purge a history record.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    },
                    "permanent": {
                        "type": "boolean",
                        "description": "If true, permanently remove from disk and DB; otherwise move to trash",
                        "default": false
                    }
                },
                "required": ["history_id"]
            }),
        },
        ToolDefinition {
            name: "sona_list_projects",
            description: "List all workspace projects and their configurations in Sona desktop.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_set_active_project",
            description: "Switch the active workspace project in Sona desktop.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project ID to activate, or null to clear active project"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_get_settings",
            description: "Read global or specific client preference settings.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Specific setting key to read, or omit for all settings"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_update_setting",
            description: "Update and persist a client preference setting.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Setting key to update"
                    },
                    "value": {
                        "description": "Setting value to persist"
                    }
                },
                "required": ["key", "value"]
            }),
        },
    ]
}

pub async fn call_tool(
    name: &str,
    arguments: serde_json::Value,
    client: &IpcClient,
) -> ToolCallResult {
    match name {
        "sona_get_client_state" => match client.call("sona_get_client_state", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(_) => ToolCallResult::json(&serde_json::json!({
                "online": false,
                "is_recording": false,
                "active_project_id": null
            })),
        },
        "sona_launch_desktop" => {
            let timeout = arguments
                .get("timeout_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(10);
            match client.launch_desktop(timeout).await {
                Ok(msg) => ToolCallResult::json(&serde_json::json!({
                    "success": true,
                    "message": msg
                })),
                Err(err) => ToolCallResult::json(&serde_json::json!({
                    "success": false,
                    "message": err
                })),
            }
        }
        "sona_focus_window" => match client.call("sona_focus_window", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_start_recording" => match client.call("sona_start_recording", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_stop_recording" => match client.call("sona_stop_recording", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_query_history" => match client.call("sona_query_history", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_read_transcript" => match client.call("sona_read_transcript", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_edit_transcript" => match client.call("sona_edit_transcript", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_delete_history" => match client.call("sona_delete_history", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_list_projects" => match client.call("sona_list_projects", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_set_active_project" => {
            match client.call("sona_set_active_project", arguments).await {
                Ok(val) => ToolCallResult::json(&val),
                Err(err) => ToolCallResult::error(err),
            }
        }
        "sona_get_settings" => match client.call("sona_get_settings", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_update_setting" => match client.call("sona_update_setting", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        _ => ToolCallResult::error(format!("Unknown tool: {name}")),
    }
}
