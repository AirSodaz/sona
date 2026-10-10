use crate::client::IpcClient;
use crate::protocol::{ToolCallResult, ToolDefinition};

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "sona_get_client_state",
            title: Some("Get Client State"),
            description: "Query Sona desktop client online status, recording state, and active project.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_launch_desktop",
            title: Some("Launch Desktop Client"),
            description: "Launch Sona desktop client when offline and poll IPC until connection is ready.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Timeout in seconds to wait for client to start (default 10)",
                        "default": 10
                    },
                    "silent": {
                        "type": "boolean",
                        "description": "Whether to launch the client silently in the background without showing the window (default true)",
                        "default": true
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_focus_window",
            title: Some("Focus Window"),
            description: "Bring Sona desktop window to front and focus.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_start_recording",
            title: Some("Start Recording"),
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
            title: Some("Stop Recording"),
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
            title: Some("Query History"),
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
            title: Some("Read Transcript"),
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
            title: Some("Edit Transcript"),
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
            name: "sona_patch_segments",
            title: Some("Patch Transcript Segments"),
            description: "Incrementally patch transcript segments by ID (modify text, timing, or translation; remove specified segment IDs; append new segments), automatically creating a version snapshot and refreshing desktop UI.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    },
                    "segments": {
                        "type": "array",
                        "description": "Array of segment patch objects (id required; text, start, end, translation optional)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "Segment ID to patch or create" },
                                "text": { "type": "string", "description": "Updated segment text" },
                                "start": { "type": "number", "description": "Start timestamp in seconds" },
                                "end": { "type": "number", "description": "End timestamp in seconds" },
                                "translation": { "type": "string", "description": "Updated translation text for this segment" }
                            },
                            "required": ["id"]
                        }
                    },
                    "remove_ids": {
                        "type": "array",
                        "description": "Optional list of segment IDs to remove",
                        "items": { "type": "string" }
                    },
                    "reason": {
                        "type": "string",
                        "description": "Optional reason for editing (e.g. 'polish' to generate polish snapshot)"
                    }
                },
                "required": ["history_id", "segments"]
            }),
        },
        ToolDefinition {
            name: "sona_update_translations",
            title: Some("Update Translations"),
            description: "Batch write, update, or clear segment translations for a history record, generating a translation snapshot and notifying the desktop UI.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    },
                    "translations": {
                        "type": "array",
                        "description": "Array of segment translation entries",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "Target segment ID" },
                                "translation": { "type": ["string", "null"], "description": "Translated text, or null to clear" }
                            },
                            "required": ["id"]
                        }
                    },
                    "clear_all": {
                        "type": "boolean",
                        "description": "If true, clear all existing translations before applying new ones",
                        "default": false
                    }
                },
                "required": ["history_id", "translations"]
            }),
        },
        ToolDefinition {
            name: "sona_delete_history",
            title: Some("Delete History"),
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
            title: Some("List Projects"),
            description: "List all workspace projects and their configurations in Sona desktop.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_set_active_project",
            title: Some("Set Active Project"),
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
            title: Some("Get Settings"),
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
            title: Some("Update Setting"),
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
        ToolDefinition {
            name: "sona_transcribe_file",
            title: Some("Transcribe File"),
            description: "Transcribe a local audio or video file in batch mode and save into history.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to local media file"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID to associate with the imported history"
                    },
                    "language": {
                        "type": "string",
                        "description": "Optional language override (e.g. 'zh', 'en', 'ja')"
                    },
                    "save_to_path": {
                        "type": "string",
                        "description": "Optional destination path to save normalized audio copy"
                    },
                    "instance_id": {
                        "type": "string",
                        "description": "Optional custom instance ID for tracking and cancelling this batch task"
                    }
                },
                "required": ["file_path"]
            }),
        },
        ToolDefinition {
            name: "sona_cancel_batch_task",
            title: Some("Cancel Batch Task"),
            description: "Cancel an active in-flight batch transcription task by instance ID.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "instance_id": {
                        "type": "string",
                        "description": "Batch task instance ID to cancel"
                    }
                },
                "required": ["instance_id"]
            }),
        },
        ToolDefinition {
            name: "sona_export_transcript",
            title: Some("Export Transcript"),
            description: "Export transcript content into a file in SRT, VTT, Markdown, TXT, or JSON format.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "History item ID whose transcript will be exported"
                    },
                    "format": {
                        "type": "string",
                        "description": "Export format ('srt', 'vtt', 'markdown', 'md', 'txt', 'json')"
                    },
                    "output_path": {
                        "type": "string",
                        "description": "Target absolute file path where exported content will be written"
                    },
                    "mode": {
                        "type": "string",
                        "description": "Export mode ('original', 'translation', 'bilingual')"
                    }
                },
                "required": ["history_id", "format", "output_path"]
            }),
        },
        ToolDefinition {
            name: "sona_save_summary",
            title: Some("Save Summary"),
            description: "Persist AI-generated summary text for a history record and notify desktop UI to refresh.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "History item ID to attach the summary to"
                    },
                    "content": {
                        "type": "string",
                        "description": "Summary text content"
                    },
                    "template_id": {
                        "type": "string",
                        "description": "Optional summary template identifier (e.g. 'meeting', 'general')"
                    },
                    "thought": {
                        "type": "string",
                        "description": "Optional reasoning or thinking process text"
                    }
                },
                "required": ["history_id", "content"]
            }),
        },
        ToolDefinition {
            name: "sona_load_summary",
            title: Some("Load Summary"),
            description: "Load persisted AI summary payload for a specific history item.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "History item ID whose summary is being loaded"
                    }
                },
                "required": ["history_id"]
            }),
        },
        ToolDefinition {
            name: "sona_edit_summary",
            title: Some("Edit Summary"),
            description: "Edit existing AI summary content for a history record while retaining original template and thought attributes if not overridden.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    },
                    "content": {
                        "type": "string",
                        "description": "Updated summary text content"
                    },
                    "template_id": {
                        "type": "string",
                        "description": "Optional summary template identifier (preserves existing template if omitted)"
                    },
                    "thought": {
                        "type": "string",
                        "description": "Optional reasoning or thinking process text (preserves existing thought if omitted)"
                    }
                },
                "required": ["history_id", "content"]
            }),
        },
        ToolDefinition {
            name: "sona_delete_summary",
            title: Some("Delete Summary"),
            description: "Delete persisted AI summary for a specific history record and refresh desktop UI.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "History item ID whose summary should be deleted"
                    }
                },
                "required": ["history_id"]
            }),
        },
        ToolDefinition {
            name: "sona_get_model_catalog",
            title: Some("Get Model Catalog"),
            description: "Get snapshot of installed and available local ASR, VAD, and punctuation models.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_download_preset_model",
            title: Some("Download Preset Model"),
            description: "Start downloading a preset model by ID and return a unique download ID.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "model_id": {
                        "type": "string",
                        "description": "Preset model ID to download"
                    },
                    "mirror": {
                        "type": "string",
                        "description": "Optional download mirror identifier (e.g. 'modelscope', 'huggingface')"
                    },
                    "download_id": {
                        "type": "string",
                        "description": "Optional custom download ID to identify this download task"
                    }
                },
                "required": ["model_id"]
            }),
        },
        ToolDefinition {
            name: "sona_cancel_download",
            title: Some("Cancel Download"),
            description: "Cancel an active in-flight model download by download ID.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "download_id": {
                        "type": "string",
                        "description": "Download task ID to cancel"
                    }
                },
                "required": ["download_id"]
            }),
        },
        ToolDefinition {
            name: "sona_get_sync_status",
            title: Some("Get Sync Status"),
            description: "Get E2EE cloud sync configuration, vault status, and conflict overview.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_trigger_sync",
            title: Some("Trigger Sync"),
            description: "Trigger an immediate run of cloud synchronization.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_create_project",
            title: Some("Create Project"),
            description: "Create a new workspace project/folder for categorizing transcription items.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Project display name"
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional project description"
                    },
                    "icon": {
                        "type": "string",
                        "description": "Optional project icon name"
                    },
                    "color": {
                        "type": "string",
                        "description": "Optional color hex code"
                    }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "sona_update_project",
            title: Some("Update Project"),
            description: "Update existing workspace project details like name, description, icon, or color.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project ID to update"
                    },
                    "name": {
                        "type": "string",
                        "description": "New project name"
                    },
                    "description": {
                        "type": "string",
                        "description": "New project description"
                    },
                    "icon": {
                        "type": "string",
                        "description": "New project icon"
                    },
                    "color": {
                        "type": "string",
                        "description": "New project color hex code"
                    }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "sona_delete_project",
            title: Some("Delete Project"),
            description: "Delete a workspace project and specify cascade action for associated items.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project ID to delete"
                    },
                    "cascade_action": {
                        "type": "string",
                        "description": "Cascade action for items in project ('moveToInbox' or 'trash', default 'moveToInbox')"
                    }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "sona_query_trash",
            title: Some("Query Trash"),
            description: "List and search items currently in the trash / recycle bin.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Optional keyword search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of items to return (default 20)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Pagination offset"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_restore_history",
            title: Some("Restore History"),
            description: "Restore a previously trashed history item back into active history.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "History item ID to restore"
                    }
                },
                "required": ["history_id"]
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
            let silent = arguments
                .get("silent")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            match client.launch_desktop(timeout, silent).await {
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
        "sona_patch_segments" => match client.call("sona_patch_segments", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_update_translations" => {
            match client.call("sona_update_translations", arguments).await {
                Ok(val) => ToolCallResult::json(&val),
                Err(err) => ToolCallResult::error(err),
            }
        }
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
        "sona_transcribe_file" => match client.call("sona_transcribe_file", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_cancel_batch_task" => match client.call("sona_cancel_batch_task", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_export_transcript" => match client.call("sona_export_transcript", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_save_summary" => match client.call("sona_save_summary", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_edit_summary" => match client.call("sona_edit_summary", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_delete_summary" => match client.call("sona_delete_summary", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_load_summary" => match client.call("sona_load_summary", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_get_model_catalog" => match client.call("sona_get_model_catalog", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_download_preset_model" => {
            match client.call("sona_download_preset_model", arguments).await {
                Ok(val) => ToolCallResult::json(&val),
                Err(err) => ToolCallResult::error(err),
            }
        }
        "sona_cancel_download" => match client.call("sona_cancel_download", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_get_sync_status" => match client.call("sona_get_sync_status", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_trigger_sync" => match client.call("sona_trigger_sync", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_create_project" => match client.call("sona_create_project", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_update_project" => match client.call("sona_update_project", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_delete_project" => match client.call("sona_delete_project", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_query_trash" => match client.call("sona_query_trash", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        "sona_restore_history" => match client.call("sona_restore_history", arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
        _ => ToolCallResult::error(format!("Unknown tool: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_desktop_tool_schema() {
        let tools = list_tools();
        let launch_tool = tools
            .iter()
            .find(|t| t.name == "sona_launch_desktop")
            .expect("sona_launch_desktop should be listed");

        let properties = launch_tool
            .input_schema
            .get("properties")
            .expect("properties field required");

        let silent_prop = properties.get("silent").expect("silent property required");
        assert_eq!(
            silent_prop.get("type").and_then(|v| v.as_str()),
            Some("boolean")
        );
        assert_eq!(
            silent_prop.get("default").and_then(|v| v.as_bool()),
            Some(true)
        );

        let timeout_prop = properties
            .get("timeout_seconds")
            .expect("timeout_seconds property required");
        assert_eq!(
            timeout_prop.get("type").and_then(|v| v.as_str()),
            Some("integer")
        );
        assert_eq!(
            timeout_prop.get("default").and_then(|v| v.as_u64()),
            Some(10)
        );
    }
}
