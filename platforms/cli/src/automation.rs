use clap::{Args, Subcommand};
use sona_core::automation::{
    AutomationError,
    repository::{AutomationRepositoryState, AutomationRuleRecord},
};
use sona_runtime_fs::UuidGenerator;
use sona_sqlite::{Database, SqliteAutomationAdapter};
use std::path::PathBuf;
use std::sync::Arc;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct AutomationArgs {
    #[command(subcommand)]
    command: AutomationCommands,
}

#[derive(Debug, Subcommand)]
enum AutomationCommands {
    /// Lists configured automation rules.
    List(AutomationListArgs),
}

#[derive(Debug, Args)]
struct AutomationListArgs {
    /// Application data directory containing the Sona database.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_automation(args: AutomationArgs) -> CliResult<CliOutput> {
    match args.command {
        AutomationCommands::List(args) => run_automation_list(args),
    }
}

fn run_automation_list(args: AutomationListArgs) -> CliResult<CliOutput> {
    let database = Database::open_read_only(&args.app_data_dir)
        .map_err(|error| CliError::Io(error.to_string()))?;
    let state = SqliteAutomationAdapter::new(Arc::new(database), Arc::new(UuidGenerator))
        .load_state()
        .map_err(map_automation_error)?;
    let output = if args.json {
        serde_json::to_string_pretty(&state)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_automation_table(&state)
    };

    Ok(CliOutput::stdout(output))
}

fn map_automation_error(error: AutomationError) -> CliError {
    let message = error.to_string();
    match error {
        AutomationError::Repository(_) => CliError::Io(message),
    }
}

fn render_automation_table(state: &AutomationRepositoryState) -> String {
    let rows = state.rules.iter().map(rule_row).collect::<Vec<_>>();
    let headers = ["ID", "NAME", "TAGS", "ENABLED", "WATCH"];
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
        ];
        append_table_row(&mut output, &values, &widths);
    }
    output.push_str(&format!(
        "Processed entries: {}\n",
        state.processed_entries.len()
    ));
    output
}

fn rule_row(rule: &AutomationRuleRecord) -> [String; 5] {
    [
        sanitize_table_cell(&rule.id),
        sanitize_table_cell(&rule.name),
        sanitize_table_cell(&rule.tag_ids.join(",")),
        rule.enabled.to_string(),
        sanitize_table_cell(&rule.watch_directory),
    ]
}

#[cfg(test)]
mod tests {
    use super::map_automation_error;
    use crate::CliError;
    use sona_core::automation::AutomationError;

    #[test]
    fn maps_automation_repository_errors_to_io() {
        assert!(matches!(
            map_automation_error(AutomationError::Repository(
                "database unavailable".to_string()
            )),
            CliError::Io(_)
        ));
    }
}
