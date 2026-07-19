use clap::{Args, Subcommand};
use sona_core::config::{AppConfigRepositorySnapshot, ConfigError};
use sona_runtime_fs::SystemClock;
use sona_sqlite::SqliteApplicationContext;
use std::path::PathBuf;
use std::sync::Arc;

use crate::table::{append_table_row, append_table_separator, column_widths};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct AppConfigArgs {
    #[command(subcommand)]
    command: AppConfigCommands,
}

#[derive(Debug, Subcommand)]
enum AppConfigCommands {
    /// Shows the persisted application configuration state.
    Show(AppConfigShowArgs),
}

#[derive(Debug, Args)]
struct AppConfigShowArgs {
    /// Application data directory containing the Sona database.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_app_config(args: AppConfigArgs) -> CliResult<CliOutput> {
    match args.command {
        AppConfigCommands::Show(args) => run_app_config_show(args),
    }
}

fn run_app_config_show(args: AppConfigShowArgs) -> CliResult<CliOutput> {
    let context = SqliteApplicationContext::open_read_only(&args.app_data_dir)
        .map_err(|error| CliError::Io(error.to_string()))?;
    let snapshot = context
        .app_config_adapter(Arc::new(SystemClock))
        .inspect_state()
        .map_err(map_config_error)?;
    let output = if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_app_config_table(snapshot.as_ref())
    };

    Ok(CliOutput::stdout(output))
}

fn map_config_error(error: ConfigError) -> CliError {
    let message = error.to_string();
    match error {
        ConfigError::Json(_) | ConfigError::Validation { .. } | ConfigError::InvalidLogLevel(_) => {
            CliError::Validation(message)
        }
        ConfigError::Serialization(_) => CliError::Serialize(message),
        ConfigError::Repository(_) => CliError::Io(message),
    }
}

fn render_app_config_table(snapshot: Option<&AppConfigRepositorySnapshot>) -> String {
    let rows = snapshot
        .map(|snapshot| {
            vec![[
                snapshot.config_version.to_string(),
                snapshot.updated_at.to_string(),
                snapshot.summary_template_count.to_string(),
                snapshot.polish_preset_count.to_string(),
                snapshot.vocabulary_set_count.to_string(),
                snapshot.speaker_profile_count.to_string(),
            ]]
        })
        .unwrap_or_default();
    let headers = [
        "VERSION",
        "UPDATED",
        "TEMPLATES",
        "PRESETS",
        "VOCABULARY_SETS",
        "SPEAKERS",
    ];
    let widths = column_widths(&headers, &rows);

    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in rows {
        let values = [
            row[0].as_str(),
            row[1].as_str(),
            row[2].as_str(),
            row[3].as_str(),
            row[4].as_str(),
            row[5].as_str(),
        ];
        append_table_row(&mut output, &values, &widths);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::map_config_error;
    use crate::CliError;
    use sona_core::config::ConfigError;

    fn json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("{").unwrap_err()
    }

    #[test]
    fn maps_config_error_variants_to_cli_categories() {
        assert!(matches!(
            map_config_error(ConfigError::Json(json_error())),
            CliError::Validation(_)
        ));
        assert!(matches!(
            map_config_error(ConfigError::Validation {
                field: "theme".to_string(),
                reason: "invalid".to_string(),
            }),
            CliError::Validation(_)
        ));
        assert!(matches!(
            map_config_error(ConfigError::InvalidLogLevel("verbose".to_string())),
            CliError::Validation(_)
        ));
        assert!(matches!(
            map_config_error(ConfigError::Serialization(json_error())),
            CliError::Serialize(_)
        ));
        assert!(matches!(
            map_config_error(ConfigError::Repository("database unavailable".to_string())),
            CliError::Io(_)
        ));
    }
}
