# Sona MCP Guide & Reference

`sona-mcp` is a Model Context Protocol (MCP) server bridge for the Sona desktop client, compliant with standard MCP specifications (protocol version `2026-07-28`). It connects external AI Agents (Claude Desktop, Cursor, OpenCode, Codex, etc.) directly to Sona desktop's local transcription engine, history database, project workspaces, and audio recording lifecycle over an IPC pipe or Unix domain socket.

## Architecture & Boundary

`sona-mcp` maintains a clean separation of concerns between external AI Agents and the local Sona transcription host:

1. **AI Reasoning Boundary**: `sona-mcp` does not host LLM models or run AI inferences directly. External Agents naturally possess conversational and reasoning capabilities. Instead, `sona-mcp` exposes granular tools and contextual resources allowing the Agent to inspect, patch, translate, and summarize audio recordings.
2. **Stateless Protocol Bridge**: `sona-mcp` communicates with external Agents via standard JSON-RPC 2.0 over standard I/O (`stdio`). It translates protocol calls into lightweight local IPC messages sent to the running Sona desktop application.
3. **Data Integrity & Versioning**: All mutations (segment editing, batch patching, translation updates, and summary edits) committed through MCP trigger immutable snapshot creation within Sona desktop's SQLite repository and broadcast live update events to the desktop UI.

```text
+-----------------------+              +-----------------------+              +-----------------------+
|  External AI Agent    |              |       sona-mcp        |              |  Sona Desktop Client  |
|  (Claude/Cursor/etc.) | <--(stdio)-> |   (Protocol Bridge)   | <--(IPC)-->  | (Host ASR/DB/Storage) |
+-----------------------+  JSON-RPC    +-----------------------+  Pipe/Socket +-----------------------+
```

## Transport & Endpoint Resolution

The MCP server accepts one optional CLI argument for endpoint specification:

```text
Usage: sona-mcp [OPTIONS]

Options:
      --endpoint <ENDPOINT>  Custom IPC pipe or socket endpoint
  -h, --help                 Print help
  -V, --version              Print version
```

### Default IPC Endpoints

If `--endpoint` is not explicitly supplied, `sona-mcp` automatically detects the platform default:

- **Windows**: `\\.\pipe\sona-agent-control-v1`
- **macOS / Linux**: `$TMPDIR/sona-agent-control-v1.sock` (fallback: `/tmp/sona-agent-control-v1.sock`)

### Offline State & Lazy Launch

When the Sona desktop client is not currently running:

- `sona_get_client_state` returns `{ "online": false }` without crashing or throwing an error.
- All mutating or data-fetching tools return a graceful error payload (`Sona desktop client is offline.`).
- External Agents can invoke `sona_launch_desktop` to spawn the desktop client in the background and poll until IPC communication is established.

## Protocol Specification & Versioning

`sona-mcp` conforms to the Model Context Protocol:

- **Modern MCP (`2026-07-28`)**: Fully validates per-request `_meta` blocks (`io.modelcontextprotocol/protocolVersion` and `io.modelcontextprotocol/clientCapabilities`).
- **Standard Discovery**: Exposes `server/discover` capability negotiation.
- **Legacy Compatibility**: Supports conventional initialization workflows (`initialize` followed by `notifications/initialized`).
- **Declared Capabilities**:
  - `tools`: 32 granular tools covering state, recording, transcript editing, translations, summaries, projects, and settings.
  - `resources`: 6 static resources and 3 parameterized resource templates.
  - `prompts`: Pre-configured prompts for meeting summarization and transcript proofreading.

## Exit Codes & Errors

`sona-mcp` reports standard JSON-RPC 2.0 error codes:

| Code | Name | Description |
| --- | --- | --- |
| `-32700` | Parse Error | Request payload is not valid JSON |
| `-32600` | Invalid Request | Payload does not adhere to JSON-RPC 2.0 (`jsonrpc: "2.0"` required) |
| `-32601` | Method Not Found | Unrecognized RPC method |
| `-32602` | Invalid Params | Missing or malformed method arguments or `_meta` validation error |
| `-32603` | Internal Error | Unhandled internal server error |
| `-32000` | Host / Client Error | Desktop client offline, database query failure, or concurrent edit conflict |
| `-32022` | Unsupported Protocol | Requested protocol version in `_meta` is not supported by the server |

---

## Public Tools Reference

`sona-mcp` registers 32 public tools categorized by functionality:

### 1. Client State & Window Control

#### `sona_get_client_state`

Queries desktop client connectivity, recording status, active project ID, and active recording session details.

- **Parameters**: None.
- **Returns**: Client status object (`online`, `recording`, `activeSessionId`, `activeProject`).

#### `sona_launch_desktop`

Spawns the Sona desktop executable in the background when currently offline and polls the IPC endpoint until ready.

- **Parameters**:
  - `wait_timeout_seconds` (*number*, optional, default: `15`): Maximum seconds to wait for IPC connection.
- **Returns**: `{ "launched": true, "online": true }`.

#### `sona_focus_window`

Brings the Sona desktop application window to the foreground and focuses it.

- **Parameters**: None.
- **Returns**: `{ "success": true }`.

---

### 2. Live Recording & Microphone Capture

#### `sona_start_recording`

Initiates microphone capture and real-time live transcription in Sona desktop.

- **Parameters**:
  - `project_id` (*string*, optional): Assign active workspace project for the new recording.
  - `model_id` (*string*, optional): Preferred local ASR model ID override.
  - `online_provider` (*string*, optional): Cloud provider override.
  - `language` (*string*, optional): Spoken language hint (e.g. `en`, `zh`).
- **Returns**: `{ "sessionId": string, "startedAt": number }`.

#### `sona_stop_recording`

Stops active recording capture and either finalizes it into persistent history or discards the live draft.

- **Parameters**:
  - `discard` (*boolean*, optional, default: `false`): If `true`, aborts and deletes audio draft without saving.
- **Returns**: `{ "historyId": string, "duration": number, "segmentCount": number }`.

---

### 3. Transcript Inspection & Segment Editing

#### `sona_read_transcript`

Reads full transcript segments, concatenated original text, and optional concatenated translation text for a history record.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
- **Returns**: `{ "historyId": string, "text": string, "translationText": string | null, "segments": [...] }`.

#### `sona_edit_transcript`

Commits a modified list of transcript segments, creating an immutable history snapshot. Automatically fills missing start/end timestamps from previous segments if omitted by external LLMs.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
  - `segments` (*array*, required): Full list of segments (`[{ "id": string, "text": string, "start"?: number, "end"?: number, "translation"?: string }]`).
  - `reason` (*string*, optional): Snapshot reason tag (`manual_edit`, `polish`, `translate`).
- **Returns**: `{ "success": true, "snapshotId": string }`.

#### `sona_patch_segments`

Incrementally patches transcript segments by ID (modifies text, timing, or translation; removes specified segment IDs; appends newly declared segments) without needing to retransmit hundreds of unchanged segments.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
  - `segments` (*array*, required): Array of segment patch objects (`[{ "id": string, "text"?: string, "start"?: number, "end"?: number, "translation"?: string }]`).
  - `remove_ids` (*array of strings*, optional): Segment IDs to delete from the transcript.
  - `reason` (*string*, optional): Snapshot reason tag (defaults to `manual_edit`, or `polish` if containing "polish").
- **Returns**: `{ "success": true, "snapshotId": string, "updatedCount": number, "totalSegments": number }`.

#### `sona_update_translations`

Batch writes, updates, or clears segment translations for a history record. Generates a versioned `translate` snapshot and notifies the desktop UI to display bilingual subtitles.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
  - `translations` (*array*, required): Array of translation pairs (`[{ "id": string, "translation": string | null }]`).
  - `clear_all` (*boolean*, optional, default: `false`): Clear all existing segment translations before applying new entries.
- **Returns**: `{ "success": true, "snapshotId": string, "updatedCount": number }`.

---

### 4. Summary & Metadata Management

#### `sona_save_summary`

Persists AI-generated summary text for a history record. Retains original template ID and thought process if not explicitly overridden.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
  - `content` (*string*, required): Markdown or plain text summary content.
  - `template_id` (*string*, optional): Template identifier (retains existing if omitted).
  - `thought` (*string*, optional): Optional internal reasoning or thought log.
- **Returns**: `{ "success": true }`.

#### `sona_load_summary`

Retrieves the persisted summary record for a specific history item.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
- **Returns**: `{ "historyId": string, "activeTemplateId": string, "record": { "content": string, "thought"?: string, "generatedAt": number } | null }`.

#### `sona_edit_summary`

Edits existing AI summary content while preserving original template and thought attributes.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
  - `content` (*string*, required): Updated summary text.
  - `template_id` (*string*, optional): New template identifier override.
  - `thought` (*string*, optional): Updated reasoning log override.
- **Returns**: `{ "success": true }`.

#### `sona_delete_summary`

Deletes the persisted AI summary for a history record and signals desktop UI refresh.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
- **Returns**: `{ "success": true }`.

---

### 5. History Query, Trash & Recovery

#### `sona_query_history`

Performs full-text search, project filtering, and time-range queries across saved transcription history records.

- **Parameters**:
  - `query` (*string*, optional): Keyword search string.
  - `project_id` (*string*, optional): Filter items belonging to a specific workspace project.
  - `limit` (*number*, optional, default: `50`): Maximum records to return.
  - `offset` (*number*, optional, default: `0`): Pagination offset.
- **Returns**: Array of history summaries (`[{ "id", "title", "previewText", "timestamp", "duration", "projectId" }]`).

#### `sona_delete_history`

Moves a history record to the trash or permanently purges it from the database.

- **Parameters**:
  - `history_id` (*string*, required): Target history record ID.
  - `permanent` (*boolean*, optional, default: `false`): If `true`, permanently deletes the record.
- **Returns**: `{ "success": true }`.

#### `sona_query_trash`

Lists items currently in the trash / recycle bin.

- **Parameters**:
  - `query` (*string*, optional): Search filter for trashed items.
  - `limit` (*number*, optional, default: `50`): Maximum items to return.
- **Returns**: Array of trashed item summaries.

#### `sona_restore_history`

Restores a previously soft-deleted item from the trash back to active history.

- **Parameters**:
  - `history_id` (*string*, required): Trashed record ID to restore.
- **Returns**: `{ "success": true }`.

---

### 6. Batch Transcription & File Export

#### `sona_transcribe_file`

Submits a local audio or video file for offline batch transcription and stores results in history.

- **Parameters**:
  - `file_path` (*string*, required): Absolute path to audio or video file.
  - `model_id` (*string*, optional): Local ASR model ID.
  - `project_id` (*string*, optional): Target project ID.
  - `language` (*string*, optional): Spoken language hint.
- **Returns**: `{ "taskId": string, "status": "queued" | "running" }`.

#### `sona_cancel_batch_task`

Cancels an in-flight batch transcription task by instance ID.

- **Parameters**:
  - `task_id` (*string*, required): In-flight batch task ID.
- **Returns**: `{ "success": true }`.

#### `sona_export_transcript`

Exports transcript content to a target file in SRT, VTT, Markdown, TXT, or JSON format.

- **Parameters**:
  - `history_id` (*string*, required): History record ID.
  - `output_path` (*string*, required): Target file path.
  - `format` (*string*, required): Format identifier (`srt`, `vtt`, `markdown`, `txt`, `json`).
- **Returns**: `{ "success": true, "path": string }`.

---

### 7. Workspace Projects

#### `sona_list_projects`

Lists all workspace projects and their configurations.

- **Parameters**: None.
- **Returns**: Array of project objects (`[{ "id", "name", "description", "color", "icon" }]`).

#### `sona_create_project`

Creates a new workspace project folder.

- **Parameters**:
  - `name` (*string*, required): Project name.
  - `description` (*string*, optional): Project description.
  - `color` (*string*, optional): Color theme identifier.
  - `icon` (*string*, optional): Icon name.
- **Returns**: Created project object.

#### `sona_update_project`

Modifies an existing workspace project.

- **Parameters**:
  - `id` (*string*, required): Target project ID.
  - `name` (*string*, optional): New project name.
  - `description` (*string*, optional): New description.
  - `color` (*string*, optional): New color.
  - `icon` (*string*, optional): New icon.
- **Returns**: Updated project object.

#### `sona_delete_project`

Deletes a workspace project.

- **Parameters**:
  - `id` (*string*, required): Project ID to delete.
  - `cascade_action` (*string*, optional, default: `"keep"`): Handling of associated history items (`"keep"` or `"delete"`).
- **Returns**: `{ "success": true }`.

#### `sona_set_active_project`

Switches the currently active workspace project in the desktop UI.

- **Parameters**:
  - `project_id` (*string*, optional): Target project ID, or `null` to clear active project.
- **Returns**: `{ "success": true }`.

---

### 8. Settings & Preferences

#### `sona_get_settings`

Reads global client preferences or a specific configuration key.

- **Parameters**:
  - `key` (*string*, optional): Specific setting key. If omitted, returns all settings.
- **Returns**: Key-value settings map.

#### `sona_update_setting`

Persists a client preference setting into desktop configuration.

- **Parameters**:
  - `key` (*string*, required): Setting key to update.
  - `value` (*any*, required): New value.
- **Returns**: `{ "success": true }`.

---

### 9. Models & Hardware Management

#### `sona_get_model_catalog`

Returns a catalog snapshot of available and locally installed ASR, VAD, and punctuation models.

- **Parameters**: None.
- **Returns**: `{ "models": [...] }`.

#### `sona_download_preset_model`

Starts downloading a preset model by identifier.

- **Parameters**:
  - `model_id` (*string*, required): Preset model identifier (e.g. `whisper-turbo`, `sensevoice`).
  - `mirror` (*string*, optional): Download mirror override (`huggingface`, `modelscope`).
- **Returns**: `{ "downloadId": string, "status": "downloading" }`.

#### `sona_cancel_download`

Aborts an in-flight model download.

- **Parameters**:
  - `download_id` (*string*, required): Active download ID.
- **Returns**: `{ "success": true }`.

---

### 10. Cloud Synchronization

#### `sona_get_sync_status`

Queries end-to-end encrypted (E2EE) cloud sync configuration, vault state, and sync status.

- **Parameters**: None.
- **Returns**: `{ "enabled": boolean, "connected": boolean, "lastSyncTime"?: number, "pendingOutbox"?: number }`.

#### `sona_trigger_sync`

Triggers an immediate cloud synchronization cycle.

- **Parameters**: None.
- **Returns**: `{ "success": true }`.

---

## Public Resources Reference

`sona-mcp` exposes read-only contextual resources via standard `resources/list`, `resources/templates/list`, and `resources/read`:

### Static Resources

| URI | Name | MIME Type | Description |
| --- | --- | --- | --- |
| `sona://client/status` | Sona Client Status | `application/json` | Online status, recording state, and active project |
| `sona://projects` | Sona Workspace Projects | `application/json` | List of workspace projects and configurations |
| `sona://models` | Sona Model Catalog | `application/json` | Installed and available local models snapshot |
| `sona://sync/status` | Sona Sync Status | `application/json` | Cloud sync status, connection, and conflict summary |
| `sona://trash` | Sona Trash Items | `application/json` | List of deleted history items currently in trash |

### Resource Templates

| URI Template | Name | MIME Type | Description |
| --- | --- | --- | --- |
| `sona://history/{history_id}` | Sona History Transcript | `text/plain` | Plain text transcript of a specific history item |
| `sona://history/{history_id}/summary` | Sona History Summary | `text/markdown` | Persisted Markdown AI summary of a specific history item |
| `sona://history/{history_id}/translation` | Sona History Translation | `text/plain` | Concatenated segment translation text for a specific item |

---

## Public Prompts Reference

`sona-mcp` provides pre-configured prompt templates accessible via `prompts/list` and `prompts/get`:

### 1. `summarize_meeting`

- **Title**: Summarize Meeting
- **Description**: Summarizes meeting transcripts into key decisions, discussion highlights, and action items.
- **Arguments**:
  - `history_id` (*string*, required): Target history record ID.

### 2. `proofread_transcript`

- **Title**: Proofread Transcript
- **Description**: Reviews transcript segments for homophones, speech recognition typos, and proper noun corrections.
- **Arguments**:
  - `history_id` (*string*, required): Target history record ID.

---

## Integration & Client Configuration

### Claude Desktop

Add `sona-mcp` to your Claude Desktop configuration file:

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "sona": {
      "command": "sona-mcp",
      "args": []
    }
  }
}
```

*Note for custom builds or development checkouts:*

```json
{
  "mcpServers": {
    "sona": {
      "command": "cargo",
      "args": ["run", "-p", "sona-mcp", "--release", "--"]
    }
  }
}
```

### Cursor

Create or update `.cursor/mcp.json` in your project or global Cursor settings:

```json
{
  "mcpServers": {
    "sona": {
      "command": "sona-mcp"
    }
  }
}
```

### Windsurf & VS Code

In `~/.codeium/windsurf/mcp_config.json` or your MCP client configuration:

```json
{
  "mcpServers": {
    "sona": {
      "command": "sona-mcp",
      "args": []
    }
  }
}
```

### Command-Line Verification & Testing

To test the MCP server manually over standard I/O:

```bash
# Launch interactive stdio session
sona-mcp

# Or specify a custom IPC pipe/socket endpoint
sona-mcp --endpoint \\.\pipe\sona-agent-control-v1
```

Send standard JSON-RPC requests via stdin:

```json
{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"resources/list","params":{}}
```
