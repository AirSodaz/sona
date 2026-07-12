use clap::{Args, Subcommand};
use sona_core::dashboard::DashboardService;
use sona_core::dashboard::models::DashboardSnapshotDomainModel;
use sona_sqlite::analytics::SqliteAnalyticsRepository;
use sona_sqlite::{Database, SqliteHistoryStore, SqliteProjectRepository};
use std::path::PathBuf;
use std::sync::Arc;

use crate::table::{append_table_row, append_table_separator, column_widths};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct DashboardArgs {
    #[command(subcommand)]
    command: DashboardCommands,
}

#[derive(Debug, Subcommand)]
enum DashboardCommands {
    /// Shows the current dashboard snapshot.
    Show(DashboardShowArgs),
}

#[derive(Debug, Args)]
struct DashboardShowArgs {
    /// Application data directory containing the Sona database.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// Includes transcript and speaker aggregation.
    #[arg(long)]
    deep: bool,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_dashboard(args: DashboardArgs) -> CliResult<CliOutput> {
    match args.command {
        DashboardCommands::Show(args) => run_dashboard_show(args),
    }
}

fn run_dashboard_show(args: DashboardShowArgs) -> CliResult<CliOutput> {
    let app_data_dir =
        std::path::absolute(args.app_data_dir).map_err(|error| CliError::Io(error.to_string()))?;
    let database = Arc::new(
        Database::open_read_only_with_analytics(&app_data_dir)
            .map_err(|error| CliError::Io(error.to_string()))?,
    );
    let service = DashboardService::new(
        Arc::new(SqliteHistoryStore::new(app_data_dir, Arc::clone(&database))),
        Arc::new(SqliteProjectRepository::new(Arc::clone(&database))),
        Arc::new(SqliteAnalyticsRepository::new(database)),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|error| CliError::Other(error.to_string()))?;
    let snapshot = runtime
        .block_on(
            service.build_snapshot_at(args.deep, sona_runtime_fs::dashboard_snapshot_time_now()),
        )
        .map_err(|error| CliError::Io(error.to_string()))?;
    let output = if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_dashboard_table(&snapshot)
    };

    Ok(CliOutput::stdout(output))
}

fn render_dashboard_table(snapshot: &DashboardSnapshotDomainModel) -> String {
    let headers = [
        "GENERATED",
        "ITEMS",
        "PROJECTS",
        "DURATION",
        "TOKENS",
        "DEEP",
    ];
    let row = [
        snapshot.generated_at.clone(),
        snapshot.content.overview.item_count_display.clone(),
        snapshot.content.overview.project_count_display.clone(),
        snapshot.content.overview.total_duration_display.clone(),
        snapshot.llm_usage.totals.total_tokens_display.clone(),
        snapshot.content.overview.is_deep_loaded.to_string(),
    ];
    let rows = [row];
    let widths = column_widths(&headers, &rows);
    let values = [
        rows[0][0].as_str(),
        rows[0][1].as_str(),
        rows[0][2].as_str(),
        rows[0][3].as_str(),
        rows[0][4].as_str(),
        rows[0][5].as_str(),
    ];
    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    append_table_row(&mut output, &values, &widths);
    output
}
