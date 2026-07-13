use clap::{Args, Subcommand};
use sona_core::storage_usage::{StorageUsageService, StorageUsageSnapshot};
use sona_sqlite::Database;
use sona_sqlite::storage_usage::SqliteStorageUsageRepository;
use std::path::PathBuf;
use std::sync::Arc;

use crate::table::{append_table_row, append_table_separator, column_widths};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct StorageArgs {
    #[command(subcommand)]
    command: StorageCommands,
}

#[derive(Debug, Subcommand)]
enum StorageCommands {
    /// Shows storage usage by category.
    Usage(StorageUsageArgs),
}

#[derive(Debug, Args)]
struct StorageUsageArgs {
    /// Application data directory containing the Sona databases and files.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_storage(args: StorageArgs) -> CliResult<CliOutput> {
    match args.command {
        StorageCommands::Usage(args) => run_storage_usage(args),
    }
}

fn run_storage_usage(args: StorageUsageArgs) -> CliResult<CliOutput> {
    let app_data_dir =
        std::path::absolute(args.app_data_dir).map_err(|error| CliError::Io(error.to_string()))?;
    let database = Arc::new(
        Database::open_read_only_with_analytics(&app_data_dir)
            .map_err(|error| CliError::Io(error.to_string()))?,
    );
    let repository = SqliteStorageUsageRepository::new(app_data_dir, database);
    let snapshot = StorageUsageService::new(Arc::new(repository))
        .load_snapshot_at(sona_runtime_fs::storage_usage_generated_at_now())
        .map_err(|error| CliError::Io(error.to_string()))?;
    let output = if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_storage_usage_table(&snapshot)
    };

    Ok(CliOutput::stdout(output))
}

fn render_storage_usage_table(snapshot: &StorageUsageSnapshot) -> String {
    let headers = [
        "GENERATED",
        "TOTAL",
        "AUDIO",
        "DATABASE",
        "MODELS",
        "TEMPORARY",
        "WEBVIEW",
        "OTHER",
    ];
    let row = [
        snapshot.generated_at.clone(),
        snapshot.total_bytes.to_string(),
        snapshot.categories.audio.bytes.to_string(),
        snapshot.categories.database.bytes.to_string(),
        snapshot.categories.models.bytes.to_string(),
        snapshot.categories.temporary.bytes.to_string(),
        snapshot
            .categories
            .webview_cache
            .bytes
            .map_or_else(|| "unknown".to_string(), |bytes| bytes.to_string()),
        snapshot.categories.other.bytes.to_string(),
    ];
    let rows = [row];
    let widths = column_widths(&headers, &rows);
    let values = std::array::from_fn(|index| rows[0][index].as_str());
    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    append_table_row(&mut output, &values, &widths);
    output
}
