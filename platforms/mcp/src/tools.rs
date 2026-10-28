use std::sync::LazyLock;

use crate::client::IpcClient;
use crate::protocol::{ToolCallResult, ToolDefinition};

static TOOL_DEFINITIONS: LazyLock<Vec<ToolDefinition>> = LazyLock::new(build_tool_definitions);

pub fn canonical_tool_name(name: &str) -> Option<&'static str> {
    let clean = name
        .strip_prefix("sona_")
        .or_else(|| name.strip_prefix("agent_"))
        .unwrap_or(name);
    TOOL_DEFINITIONS
        .iter()
        .find(|t| {
            t.name == name
                || t.name.strip_prefix("sona_") == Some(clean)
                || t.name.strip_prefix("agent_") == Some(clean)
        })
        .map(|t| t.name)
}

pub fn is_known_tool(name: &str) -> bool {
    canonical_tool_name(name).is_some()
}

pub fn list_tools() -> Vec<ToolDefinition> {
    TOOL_DEFINITIONS.clone()
}

fn build_tool_definitions() -> Vec<ToolDefinition> {
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
            description: "Start audio capture (microphone or system audio loopback) and live transcription in Sona desktop client.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "enum": ["microphone", "system"],
                        "description": "Audio source: 'microphone' for mic capture or 'system' for WASAPI system audio loopback (default 'microphone')",
                        "default": "microphone"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID to associate with the recording"
                    },
                    "device_name": {
                        "type": "string",
                        "description": "Optional specific audio device name"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "sona_control_recording",
            title: Some("Control Recording"),
            description: "Unified controller for active recording lifecycle: pause, resume, stop (save), or discard.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["pause", "resume", "stop", "discard"],
                        "description": "Recording action to execute: 'pause', 'resume', 'stop' (finalizes into history), or 'discard' (deletes draft)"
                    }
                },
                "required": ["action"]
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
            name: "sona_revert_transcript_snapshot",
            title: Some("Revert Transcript Snapshot"),
            description: "Rollback transcript segments to a previous historical version snapshot.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    },
                    "snapshot_id": {
                        "type": "string",
                        "description": "Target snapshot ID to revert back to"
                    }
                },
                "required": ["history_id", "snapshot_id"]
            }),
        },
        ToolDefinition {
            name: "sona_update_history_meta",
            title: Some("Update History Metadata"),
            description: "Update history item title, project assignment, or icon metadata.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "History item record ID"
                    },
                    "title": {
                        "type": "string",
                        "description": "New title for the history item"
                    },
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID to assign item to (null or empty string to unassign)"
                    },
                    "icon": {
                        "type": "string",
                        "description": "Optional icon identifier"
                    }
                },
                "required": ["history_id"]
            }),
        },
        ToolDefinition {
            name: "sona_delete_preset_model",
            title: Some("Delete Preset Model"),
            description: "Physically delete an installed local preset model from disk to free storage space.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "model_id": {
                        "type": "string",
                        "description": "Model ID to physically delete from disk"
                    }
                },
                "required": ["model_id"]
            }),
        },
        ToolDefinition {
            name: "sona_resolve_sync_conflict",
            title: Some("Resolve Sync Conflict"),
            description: "Resolve an unresolved E2EE sync conflict between local and remote versions.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "conflict_id": {
                        "type": "string",
                        "description": "Conflict ID from sona://sync/conflicts"
                    },
                    "resolution": {
                        "type": "string",
                        "enum": ["keep_current", "use_conflicting", "keep_both"],
                        "description": "Resolution decision: 'keep_current' (retain local), 'use_conflicting' (overwrite with remote), or 'keep_both' (fork both copies)"
                    }
                },
                "required": ["conflict_id", "resolution"]
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
            description: "Read transcript segments and text of a history record, with optional time filtering, cursor pagination, and task status.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "history_id": {
                        "type": "string",
                        "description": "Target history record ID"
                    },
                    "start_seconds": {
                        "type": "number",
                        "description": "Optional start time in seconds to filter segments"
                    },
                    "end_seconds": {
                        "type": "number",
                        "description": "Optional end time in seconds to filter segments"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Optional number of segments to skip"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional maximum number of segments to return"
                    },
                    "since_segment_id": {
                        "type": "string",
                        "description": "Optional segment ID cursor: only return segments occurring after this ID"
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
            description: "Start background transcription for a local audio/video file or remote network stream (HTTP/HTTPS/RTMP/RTSP/m3u8), returning task and history IDs immediately.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to local media file or remote stream URL"
                    },
                    "input": {
                        "type": "string",
                        "description": "Alias for file_path: local media file path or remote stream URL"
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
                "anyOf": [
                    { "required": ["file_path"] },
                    { "required": ["input"] }
                ]
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
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Optional alias for instance_id"
                    }
                }
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
        ToolDefinition {
            name: "sona_list_audio_devices",
            title: Some("List Audio Devices"),
            description: "List available microphone and system audio input devices in Sona desktop.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "sona_list_transcript_snapshots",
            title: Some("List Transcript Snapshots"),
            description: "List historical transcript version snapshots for a history record.",
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
            name: "sona_list_sync_conflicts",
            title: Some("List Sync Conflicts"),
            description: "List active cloud synchronization conflicts between local and remote versions.",
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

pub async fn call_tool(
    name: &str,
    arguments: serde_json::Value,
    client: &IpcClient,
) -> ToolCallResult {
    let Some(canonical) = canonical_tool_name(name) else {
        return ToolCallResult::error(format!("Unknown tool: {name}"));
    };

    match canonical {
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
        "sona_transcribe_file" => {
            let mut args = arguments;
            if let Some(obj) = args.as_object_mut() {
                let file_path_missing_or_empty = obj
                    .get("file_path")
                    .map(|v| {
                        v.is_null() || v.as_str().map(|s| s.trim().is_empty()).unwrap_or(false)
                    })
                    .unwrap_or(true);
                if file_path_missing_or_empty
                    && let Some(input) = obj.get("input").cloned()
                    && input
                        .as_str()
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false)
                {
                    obj.insert("file_path".to_string(), input);
                }
            }
            match client.call("sona_transcribe_file", args).await {
                Ok(val) => ToolCallResult::json(&val),
                Err(err) => ToolCallResult::error(err),
            }
        }
        "sona_cancel_batch_task" => {
            let mut args = arguments;
            if let Some(obj) = args.as_object_mut() {
                let task_id = if !obj.contains_key("instance_id") && !obj.contains_key("instanceId")
                {
                    obj.get("task_id").or_else(|| obj.get("taskId")).cloned()
                } else {
                    None
                };
                if let Some(task_id) = task_id {
                    obj.insert("instance_id".to_string(), task_id);
                }
            }
            match client.call("sona_cancel_batch_task", args).await {
                Ok(val) => ToolCallResult::json(&val),
                Err(err) => ToolCallResult::error(err),
            }
        }
        "sona_control_recording" => {
            let action = match arguments.get("action").and_then(|v| v.as_str()) {
                Some(a) => a,
                None => {
                    return ToolCallResult::error(
                        "INVALID_PARAMS: Missing required 'action' argument ('pause', 'resume', 'stop', or 'discard')"
                            .to_string(),
                    );
                }
            };
            match action {
                "pause" => match client
                    .call("sona_pause_recording", serde_json::json!({}))
                    .await
                {
                    Ok(val) => ToolCallResult::json(&val),
                    Err(err) => ToolCallResult::error(err),
                },
                "resume" => match client
                    .call("sona_resume_recording", serde_json::json!({}))
                    .await
                {
                    Ok(val) => ToolCallResult::json(&val),
                    Err(err) => ToolCallResult::error(err),
                },
                "stop" => {
                    match client
                        .call(
                            "sona_stop_recording",
                            serde_json::json!({ "discard": false }),
                        )
                        .await
                    {
                        Ok(val) => ToolCallResult::json(&val),
                        Err(err) => ToolCallResult::error(err),
                    }
                }
                "discard" => {
                    match client
                        .call(
                            "sona_stop_recording",
                            serde_json::json!({ "discard": true }),
                        )
                        .await
                    {
                        Ok(val) => ToolCallResult::json(&val),
                        Err(err) => ToolCallResult::error(err),
                    }
                }
                other => ToolCallResult::error(format!(
                    "INVALID_ACTION: Unknown recording control action '{other}', expected 'pause', 'resume', 'stop', or 'discard'"
                )),
            }
        }
        _ => match client.call(canonical, arguments).await {
            Ok(val) => ToolCallResult::json(&val),
            Err(err) => ToolCallResult::error(err),
        },
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

    #[test]
    fn test_canonical_tool_name_resolution() {
        assert_eq!(
            canonical_tool_name("query_history"),
            Some("sona_query_history")
        );
        assert_eq!(
            canonical_tool_name("sona_query_history"),
            Some("sona_query_history")
        );
        assert_eq!(
            canonical_tool_name("agent_query_history"),
            Some("sona_query_history")
        );
        assert_eq!(
            canonical_tool_name("launch_desktop"),
            Some("sona_launch_desktop")
        );
        assert_eq!(
            canonical_tool_name("sona_launch_desktop"),
            Some("sona_launch_desktop")
        );
        assert_eq!(
            canonical_tool_name("agent_launch_desktop"),
            Some("sona_launch_desktop")
        );
        assert_eq!(
            canonical_tool_name("transcribe_file"),
            Some("sona_transcribe_file")
        );
        assert_eq!(canonical_tool_name("unknown_tool"), None);
        assert_eq!(canonical_tool_name("sona_nonexistent"), None);
    }

    #[test]
    fn test_is_known_tool() {
        assert!(is_known_tool("sona_focus_window"));
        assert!(is_known_tool("focus_window"));
        assert!(is_known_tool("agent_focus_window"));
        assert!(is_known_tool("sona_query_history"));
        assert!(is_known_tool("query_history"));
        assert!(is_known_tool("agent_query_history"));
        assert!(!is_known_tool("sona_nonexistent_tool"));
        assert!(!is_known_tool("nonexistent_tool"));
    }

    #[test]
    fn test_new_read_only_tools_registered() {
        let tools = list_tools();
        assert_eq!(tools.len(), 40);
        assert!(is_known_tool("sona_list_audio_devices"));
        assert!(is_known_tool("sona_list_transcript_snapshots"));
        assert!(is_known_tool("sona_list_sync_conflicts"));

        let snap_tool = tools
            .iter()
            .find(|t| t.name == "sona_list_transcript_snapshots")
            .expect("sona_list_transcript_snapshots should be listed");
        let req = snap_tool
            .input_schema
            .get("required")
            .and_then(|r| r.as_array())
            .expect("required array present");
        assert!(req.iter().any(|v| v.as_str() == Some("history_id")));
    }

    #[test]
    fn test_stream_transcription_sync_tool_schemas() {
        let tools = list_tools();
        let transcribe_tool = tools
            .iter()
            .find(|t| t.name == "sona_transcribe_file")
            .expect("sona_transcribe_file should be listed");
        let any_of = transcribe_tool
            .input_schema
            .get("anyOf")
            .and_then(|v| v.as_array())
            .expect("anyOf should be present on sona_transcribe_file");
        assert_eq!(any_of.len(), 2);

        let read_tool = tools
            .iter()
            .find(|t| t.name == "sona_read_transcript")
            .expect("sona_read_transcript should be listed");
        let props = read_tool
            .input_schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("properties should be an object");
        assert!(props.contains_key("start_seconds"));
        assert!(props.contains_key("end_seconds"));
        assert!(props.contains_key("offset"));
        assert!(props.contains_key("limit"));
        assert!(props.contains_key("since_segment_id"));
    }
}
