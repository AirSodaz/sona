use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Subcommand, ValueEnum};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryMutationError, HistoryReassignProjectRequest,
    HistoryUpdateItemMetaRequest, HistoryUpdateProjectAssignmentsRequest,
    HistoryUpdateTranscriptRequest,
};
use sona_core::history::mutation_service::HistoryMutationService;
use sona_core::history::query_repository::HistoryQueryError;
use sona_core::history::query_service::HistoryQueryService;
use sona_core::history::{
    HistoryCreateLiveDraftRequest, HistoryItemRecord, HistoryListOptions,
    HistorySaveImportedFileRequest, HistorySaveRecordingRequest, HistoryWorkspaceQueryRequest,
    LiveRecordingDraftResult, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
};
use sona_core::transcription::transcript::TranscriptSegment;
use sona_sqlite::{LazySqliteHistoryMutationRepository, LazySqliteHistoryQueryRepository};

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[command(subcommand)]
    command: HistoryCommands,
}

#[derive(Debug, Subcommand)]
enum HistoryCommands {
    /// Lists history items using the shared query service.
    List(HistoryListArgs),
    /// Queries the history workspace using a JSON request.
    Query(HistoryQueryArgs),
    /// Loads transcript segments for one history item.
    Transcript(HistoryItemArgs),
    /// Lists transcript snapshots for one history item.
    Snapshots(HistoryItemArgs),
    /// Loads one transcript snapshot.
    Snapshot(HistorySnapshotArgs),
    /// Creates a live recording draft.
    CreateLiveDraft(HistoryCreateLiveDraftArgs),
    /// Completes an existing live recording draft.
    CompleteLiveDraft(HistoryCompleteLiveDraftArgs),
    /// Saves a recording from a native audio file.
    SaveRecording(HistorySaveRecordingArgs),
    /// Imports a media file and its transcript.
    ImportFile(HistoryJsonInputArgs),
    /// Deletes one or more history items.
    Delete(HistoryItemsMutationArgs),
    /// Replaces one history transcript.
    UpdateTranscript(HistoryTranscriptMutationArgs),
    /// Creates a transcript snapshot.
    CreateSnapshot(HistoryCreateSnapshotArgs),
    /// Updates mutable history metadata.
    UpdateMeta(HistoryMetadataMutationArgs),
    /// Assigns selected history items to a project or inbox.
    AssignProject(HistoryAssignProjectArgs),
    /// Reassigns every item from one project to another project or inbox.
    ReassignProject(HistoryReassignProjectArgs),
}

#[derive(Debug, Args)]
struct HistoryLocationArgs {
    /// Existing application data directory containing Sona SQLite data.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
}

#[derive(Debug, Args)]
struct HistoryListArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// Maximum number of items to return.
    #[arg(long)]
    limit: Option<usize>,
    /// Number of items to skip.
    #[arg(long)]
    offset: Option<usize>,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryQueryArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// JSON file containing a HistoryWorkspaceQueryRequest.
    #[arg(long, value_name = "JSON_FILE")]
    input: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryItemArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier.
    #[arg(long)]
    history_id: String,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistorySnapshotArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier.
    #[arg(long)]
    history_id: String,
    /// Transcript snapshot identifier.
    #[arg(long)]
    snapshot_id: String,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryCreateLiveDraftArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// Optional caller-supplied history identifier.
    #[arg(long)]
    id: Option<String>,
    /// Audio file extension without a path.
    #[arg(long, default_value = "wav")]
    audio_extension: String,
    /// Optional project identifier.
    #[arg(long)]
    project_id: Option<String>,
    /// Optional item icon.
    #[arg(long)]
    icon: Option<String>,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryCompleteLiveDraftArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier.
    #[arg(long)]
    history_id: String,
    /// JSON file containing transcript segments.
    #[arg(long, value_name = "JSON_FILE")]
    segments: PathBuf,
    /// Recording duration in seconds.
    #[arg(long)]
    duration: f64,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistorySaveRecordingArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// JSON file containing segments, duration, projectId, and audioExtension.
    #[arg(long, value_name = "JSON_FILE")]
    input: PathBuf,
    /// Native audio file to copy into history storage.
    #[arg(long, value_name = "AUDIO_FILE")]
    audio: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryJsonInputArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// JSON request file.
    #[arg(long, value_name = "JSON_FILE")]
    input: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryItemsMutationArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier; repeat for multiple items.
    #[arg(long = "history-id", required = true)]
    history_ids: Vec<String>,
}

#[derive(Debug, Args)]
struct HistoryTranscriptMutationArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier.
    #[arg(long)]
    history_id: String,
    /// JSON file containing transcript segments.
    #[arg(long, value_name = "JSON_FILE")]
    segments: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum HistorySnapshotReasonArg {
    Polish,
    Translate,
    Retranscribe,
    Restore,
}

impl From<HistorySnapshotReasonArg> for TranscriptSnapshotReason {
    fn from(value: HistorySnapshotReasonArg) -> Self {
        match value {
            HistorySnapshotReasonArg::Polish => Self::Polish,
            HistorySnapshotReasonArg::Translate => Self::Translate,
            HistorySnapshotReasonArg::Retranscribe => Self::Retranscribe,
            HistorySnapshotReasonArg::Restore => Self::Restore,
        }
    }
}

#[derive(Debug, Args)]
struct HistoryCreateSnapshotArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier.
    #[arg(long)]
    history_id: String,
    /// Snapshot reason.
    #[arg(long, value_enum)]
    reason: HistorySnapshotReasonArg,
    /// JSON file containing transcript segments.
    #[arg(long, value_name = "JSON_FILE")]
    segments: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryMetadataMutationArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier.
    #[arg(long)]
    history_id: String,
    /// JSON object containing mutable history fields.
    #[arg(long, value_name = "JSON_FILE")]
    updates: PathBuf,
}

#[derive(Debug, Args)]
struct HistoryAssignProjectArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// History item identifier; repeat for multiple items.
    #[arg(long = "history-id", required = true)]
    history_ids: Vec<String>,
    /// Target project identifier; omit to move items to inbox.
    #[arg(long)]
    project_id: Option<String>,
}

#[derive(Debug, Args)]
struct HistoryReassignProjectArgs {
    #[command(flatten)]
    location: HistoryLocationArgs,
    /// Project identifier currently assigned to history items.
    #[arg(long)]
    current_project_id: String,
    /// Target project identifier; omit to move items to inbox.
    #[arg(long)]
    next_project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistorySaveRecordingInput {
    segments: Value,
    duration: f64,
    project_id: Option<String>,
    audio_extension: Option<String>,
}

pub fn run_history(args: HistoryArgs) -> CliResult<CliOutput> {
    match args.command {
        HistoryCommands::List(args) => run_list(args),
        HistoryCommands::Query(args) => run_query(args),
        HistoryCommands::Transcript(args) => run_transcript(args),
        HistoryCommands::Snapshots(args) => run_snapshots(args),
        HistoryCommands::Snapshot(args) => run_snapshot(args),
        HistoryCommands::CreateLiveDraft(args) => run_create_live_draft(args),
        HistoryCommands::CompleteLiveDraft(args) => run_complete_live_draft(args),
        HistoryCommands::SaveRecording(args) => run_save_recording(args),
        HistoryCommands::ImportFile(args) => run_import_file(args),
        HistoryCommands::Delete(args) => run_delete(args),
        HistoryCommands::UpdateTranscript(args) => run_update_transcript(args),
        HistoryCommands::CreateSnapshot(args) => run_create_snapshot(args),
        HistoryCommands::UpdateMeta(args) => run_update_meta(args),
        HistoryCommands::AssignProject(args) => run_assign_project(args),
        HistoryCommands::ReassignProject(args) => run_reassign_project(args),
    }
}

fn run_list(args: HistoryListArgs) -> CliResult<CliOutput> {
    let service = open_service(args.location.app_data_dir)?;
    let items = service
        .list_items(HistoryListOptions {
            limit: args.limit,
            offset: args.offset,
        })
        .map_err(map_history_error)?;
    render_items(items, args.json)
}

fn run_query(args: HistoryQueryArgs) -> CliResult<CliOutput> {
    let input = std::fs::read(&args.input).map_err(|error| {
        CliError::Io(format!(
            "Failed to read history query input {}: {error}",
            args.input.display()
        ))
    })?;
    let request: HistoryWorkspaceQueryRequest =
        serde_json::from_slice(&input).map_err(|error| CliError::Validation(error.to_string()))?;
    let service = open_service(args.location.app_data_dir)?;
    let result = service
        .query_workspace(request)
        .map_err(map_history_error)?;
    if args.json {
        pretty_json(&result)
    } else {
        Ok(CliOutput::stdout(render_history_items_table(
            &result.filtered_items,
        )))
    }
}

fn run_transcript(args: HistoryItemArgs) -> CliResult<CliOutput> {
    let service = open_service(args.location.app_data_dir)?;
    let segments = service
        .load_transcript(&args.history_id)
        .map_err(map_history_error)?;
    if args.json {
        pretty_json(&segments)
    } else {
        Ok(CliOutput::stdout(render_segments_table(
            segments.as_deref().unwrap_or_default(),
        )))
    }
}

fn run_snapshots(args: HistoryItemArgs) -> CliResult<CliOutput> {
    let service = open_service(args.location.app_data_dir)?;
    let snapshots = service
        .list_transcript_snapshots(&args.history_id)
        .map_err(map_history_error)?;
    if args.json {
        pretty_json(&snapshots)
    } else {
        Ok(CliOutput::stdout(render_snapshots_table(&snapshots)))
    }
}

fn run_snapshot(args: HistorySnapshotArgs) -> CliResult<CliOutput> {
    let service = open_service(args.location.app_data_dir)?;
    let snapshot = service
        .load_transcript_snapshot(&args.history_id, &args.snapshot_id)
        .map_err(map_history_error)?;
    if args.json {
        pretty_json(&snapshot)
    } else {
        Ok(CliOutput::stdout(render_segments_table(
            snapshot
                .as_ref()
                .map(|record| record.segments.as_slice())
                .unwrap_or_default(),
        )))
    }
}

fn run_create_live_draft(args: HistoryCreateLiveDraftArgs) -> CliResult<CliOutput> {
    let service = open_mutation_service(args.location.app_data_dir)?;
    let result = service
        .create_live_draft(HistoryCreateLiveDraftRequest {
            id: args.id,
            audio_extension: args.audio_extension,
            project_id: args.project_id,
            icon: args.icon,
        })
        .map_err(map_history_mutation_error)?;
    render_live_draft(result, args.json)
}

fn run_complete_live_draft(args: HistoryCompleteLiveDraftArgs) -> CliResult<CliOutput> {
    let segments = read_json(&args.segments, "history transcript segments")?;
    let service = open_mutation_service(args.location.app_data_dir)?;
    let item = service
        .complete_live_draft(HistoryCompleteLiveDraftRequest {
            history_id: args.history_id,
            segments,
            duration: args.duration,
        })
        .map_err(map_history_mutation_error)?;
    render_item(item, args.json)
}

fn run_save_recording(args: HistorySaveRecordingArgs) -> CliResult<CliOutput> {
    let input: HistorySaveRecordingInput = read_json(&args.input, "history recording input")?;
    let audio_path = absolute_existing_file(&args.audio, "recording audio")?;
    let service = open_mutation_service(args.location.app_data_dir)?;
    let item = service
        .save_recording(HistorySaveRecordingRequest {
            segments: input.segments,
            duration: input.duration,
            project_id: input.project_id,
            audio_bytes: None,
            native_audio_path: Some(utf8_path(&audio_path, "recording audio")?),
            audio_extension: input.audio_extension,
        })
        .map_err(map_history_mutation_error)?;
    render_item(item, args.json)
}

fn run_import_file(args: HistoryJsonInputArgs) -> CliResult<CliOutput> {
    let mut request: HistorySaveImportedFileRequest =
        read_json(&args.input, "history import input")?;
    let source_path = absolute_path(PathBuf::from(&request.source_path))?;
    request.source_path = utf8_path(&source_path, "import source")?;
    if let Some(path) = request.converted_source_path.take() {
        let converted = absolute_existing_file(&PathBuf::from(path), "converted import source")?;
        request.converted_source_path = Some(utf8_path(&converted, "converted import source")?);
    } else {
        absolute_existing_file(&PathBuf::from(&request.source_path), "import source")?;
    }
    let service = open_mutation_service(args.location.app_data_dir)?;
    let item = service
        .save_imported_file(request)
        .map_err(map_history_mutation_error)?;
    render_item(item, args.json)
}

fn run_delete(args: HistoryItemsMutationArgs) -> CliResult<CliOutput> {
    let service = open_mutation_service(args.location.app_data_dir)?;
    service
        .delete_items(HistoryDeleteItemsRequest {
            ids: args.history_ids,
        })
        .map_err(map_history_mutation_error)?;
    Ok(CliOutput::default())
}

fn run_update_transcript(args: HistoryTranscriptMutationArgs) -> CliResult<CliOutput> {
    let segments = read_json(&args.segments, "history transcript segments")?;
    let service = open_mutation_service(args.location.app_data_dir)?;
    let item = service
        .update_transcript(HistoryUpdateTranscriptRequest {
            history_id: args.history_id,
            segments,
        })
        .map_err(map_history_mutation_error)?;
    render_item(item, args.json)
}

fn run_create_snapshot(args: HistoryCreateSnapshotArgs) -> CliResult<CliOutput> {
    let segments = read_json(&args.segments, "history snapshot segments")?;
    let service = open_mutation_service(args.location.app_data_dir)?;
    let snapshot = service
        .create_transcript_snapshot(HistoryCreateTranscriptSnapshotRequest {
            history_id: args.history_id,
            reason: args.reason.into(),
            segments,
        })
        .map_err(map_history_mutation_error)?;
    if args.json {
        pretty_json(&snapshot)
    } else {
        Ok(CliOutput::stdout(render_snapshots_table(&[snapshot])))
    }
}

fn run_update_meta(args: HistoryMetadataMutationArgs) -> CliResult<CliOutput> {
    let updates = read_json(&args.updates, "history metadata updates")?;
    let service = open_mutation_service(args.location.app_data_dir)?;
    service
        .update_item_meta(HistoryUpdateItemMetaRequest {
            history_id: args.history_id,
            updates,
        })
        .map_err(map_history_mutation_error)?;
    Ok(CliOutput::default())
}

fn run_assign_project(args: HistoryAssignProjectArgs) -> CliResult<CliOutput> {
    let service = open_mutation_service(args.location.app_data_dir)?;
    service
        .update_project_assignments(HistoryUpdateProjectAssignmentsRequest {
            ids: args.history_ids,
            project_id: args.project_id,
        })
        .map_err(map_history_mutation_error)?;
    Ok(CliOutput::default())
}

fn run_reassign_project(args: HistoryReassignProjectArgs) -> CliResult<CliOutput> {
    let service = open_mutation_service(args.location.app_data_dir)?;
    service
        .reassign_project(HistoryReassignProjectRequest {
            current_project_id: args.current_project_id,
            next_project_id: args.next_project_id,
        })
        .map_err(map_history_mutation_error)?;
    Ok(CliOutput::default())
}

fn open_service(app_data_dir: PathBuf) -> CliResult<HistoryQueryService> {
    let app_data_dir = existing_app_data_dir(app_data_dir)?;
    Ok(HistoryQueryService::new(Arc::new(
        LazySqliteHistoryQueryRepository::new(app_data_dir),
    )))
}

fn open_mutation_service(app_data_dir: PathBuf) -> CliResult<HistoryMutationService> {
    let app_data_dir = existing_app_data_dir(app_data_dir)?;
    Ok(HistoryMutationService::new(Arc::new(
        LazySqliteHistoryMutationRepository::new(app_data_dir),
    )))
}

fn existing_app_data_dir(app_data_dir: PathBuf) -> CliResult<PathBuf> {
    let app_data_dir = absolute_path(app_data_dir)?;
    if !app_data_dir.is_dir() {
        return Err(CliError::Io(format!(
            "History app data directory does not exist: {}",
            app_data_dir.display()
        )));
    }
    Ok(app_data_dir)
}

fn absolute_path(path: PathBuf) -> CliResult<PathBuf> {
    std::path::absolute(path).map_err(|error| CliError::Io(error.to_string()))
}

fn absolute_existing_file(path: &PathBuf, label: &str) -> CliResult<PathBuf> {
    let path = absolute_path(path.clone())?;
    if !path.is_file() {
        return Err(CliError::Io(format!(
            "History {label} does not exist: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn utf8_path(path: &std::path::Path, label: &str) -> CliResult<String> {
    path.to_str().map(ToString::to_string).ok_or_else(|| {
        CliError::Validation(format!(
            "History {label} path is not valid UTF-8: {}",
            path.display()
        ))
    })
}

fn read_json<T: DeserializeOwned>(path: &PathBuf, label: &str) -> CliResult<T> {
    let input = std::fs::read(path).map_err(|error| {
        CliError::Io(format!(
            "Failed to read {label} {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_slice(&input).map_err(|error| CliError::Validation(error.to_string()))
}

fn map_history_error(error: HistoryQueryError) -> CliError {
    match error {
        HistoryQueryError::InvalidRequest(reason) => CliError::Validation(reason),
        HistoryQueryError::Serialization(error) => CliError::Serialize(error.to_string()),
        HistoryQueryError::Database(reason) | HistoryQueryError::Internal(reason) => {
            CliError::Io(reason)
        }
    }
}

fn map_history_mutation_error(error: HistoryMutationError) -> CliError {
    match error {
        HistoryMutationError::InvalidRequest(reason) => CliError::Validation(reason),
        HistoryMutationError::Serialization(error) => CliError::Serialize(error.to_string()),
        HistoryMutationError::NotFound(reason)
        | HistoryMutationError::Database(reason)
        | HistoryMutationError::Internal(reason) => CliError::Io(reason),
    }
}

fn render_live_draft(result: LiveRecordingDraftResult, json: bool) -> CliResult<CliOutput> {
    if json {
        pretty_json(&result)
    } else {
        render_items(vec![result.item], false)
    }
}

fn render_item(item: HistoryItemRecord, json: bool) -> CliResult<CliOutput> {
    if json {
        pretty_json(&item)
    } else {
        render_items(vec![item], false)
    }
}

fn render_items(items: Vec<HistoryItemRecord>, json: bool) -> CliResult<CliOutput> {
    if json {
        pretty_json(&items)
    } else {
        Ok(CliOutput::stdout(render_history_items_table(&items)))
    }
}

fn pretty_json(value: &impl serde::Serialize) -> CliResult<CliOutput> {
    serde_json::to_string_pretty(value)
        .map(CliOutput::stdout)
        .map_err(|error| CliError::Serialize(error.to_string()))
}

fn render_history_items_table(items: &[HistoryItemRecord]) -> String {
    let headers = ["ID", "TITLE", "KIND", "STATUS", "DURATION", "PROJECT"];
    let rows = items
        .iter()
        .map(|item| {
            [
                sanitize_table_cell(&item.id),
                sanitize_table_cell(&item.title),
                item.kind.to_string(),
                item.status.to_string(),
                format!("{:.3}", item.duration),
                sanitize_table_cell(item.project_id.as_deref().unwrap_or("-")),
            ]
        })
        .collect::<Vec<_>>();
    render_table(&headers, &rows)
}

fn render_segments_table(segments: &[TranscriptSegment]) -> String {
    let headers = ["ID", "START", "END", "FINAL", "TEXT"];
    let rows = segments
        .iter()
        .map(|segment| {
            [
                sanitize_table_cell(&segment.id),
                format!("{:.3}", segment.start),
                format!("{:.3}", segment.end),
                segment.is_final.to_string(),
                sanitize_table_cell(&segment.text),
            ]
        })
        .collect::<Vec<_>>();
    render_table(&headers, &rows)
}

fn render_snapshots_table(snapshots: &[TranscriptSnapshotMetadata]) -> String {
    let headers = ["ID", "REASON", "CREATED_AT", "SEGMENTS"];
    let rows = snapshots
        .iter()
        .map(|snapshot| {
            [
                sanitize_table_cell(&snapshot.id),
                format!("{:?}", snapshot.reason).to_ascii_lowercase(),
                snapshot.created_at.to_string(),
                snapshot.segment_count.to_string(),
            ]
        })
        .collect::<Vec<_>>();
    render_table(&headers, &rows)
}

fn render_table<const N: usize>(headers: &[&str; N], rows: &[[String; N]]) -> String {
    let widths = column_widths(headers, rows);
    let mut output = String::new();
    append_table_row(&mut output, headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in rows {
        let values = std::array::from_fn(|index| row[index].as_str());
        append_table_row(&mut output, &values, &widths);
    }
    output
}
