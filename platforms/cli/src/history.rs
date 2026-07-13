use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Subcommand};
use sona_core::history::query_repository::HistoryQueryError;
use sona_core::history::query_service::HistoryQueryService;
use sona_core::history::{
    HistoryItemRecord, HistoryListOptions, HistoryWorkspaceQueryRequest, TranscriptSnapshotMetadata,
};
use sona_core::transcription::transcript::TranscriptSegment;
use sona_sqlite::{Database, SqliteHistoryStore};

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

pub fn run_history(args: HistoryArgs) -> CliResult<CliOutput> {
    match args.command {
        HistoryCommands::List(args) => run_list(args),
        HistoryCommands::Query(args) => run_query(args),
        HistoryCommands::Transcript(args) => run_transcript(args),
        HistoryCommands::Snapshots(args) => run_snapshots(args),
        HistoryCommands::Snapshot(args) => run_snapshot(args),
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

fn open_service(app_data_dir: PathBuf) -> CliResult<HistoryQueryService> {
    let app_data_dir =
        std::path::absolute(app_data_dir).map_err(|error| CliError::Io(error.to_string()))?;
    if !app_data_dir.is_dir() {
        return Err(CliError::Io(format!(
            "History app data directory does not exist: {}",
            app_data_dir.display()
        )));
    }
    let database =
        Database::open(&app_data_dir).map_err(|error| CliError::Io(error.to_string()))?;
    let repository = SqliteHistoryStore::new(app_data_dir, Arc::new(database));
    Ok(HistoryQueryService::new(Arc::new(repository)))
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
