use clap::{Args, Subcommand};
use sona_core::automation::{
    AutomationError,
    repository::{AutomationProfileRecord, AutomationRepositoryState, AutomationRuleRecord},
};
use sona_runtime_fs::UuidGenerator;
use sona_sqlite::SqliteApplicationContext;
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
    let context = SqliteApplicationContext::open_read_only(&args.app_data_dir)
        .map_err(|error| CliError::Io(error.to_string()))?;
    let state = context
        .automation_adapter(Arc::new(UuidGenerator))
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
        AutomationError::Repository(_) | AutomationError::FileSystem(_) => CliError::Io(message),
    }
}

fn render_automation_table(state: &AutomationRepositoryState) -> String {
    let profile_rows = state.profiles.iter().map(profile_row).collect::<Vec<_>>();
    let profile_headers = ["ID", "NAME", "LANGUAGE", "POLISH", "SUMMARY"];
    let profile_widths = column_widths(&profile_headers, &profile_rows);
    let rows = state.rules.iter().map(rule_row).collect::<Vec<_>>();
    let headers = [
        "ID", "KIND", "NAME", "PRIORITY", "PROFILE", "TAGS", "ENABLED", "WATCH",
    ];
    let widths = column_widths(&headers, &rows);

    let mut output = String::from("PROFILES\n");
    append_table_row(&mut output, &profile_headers, &profile_widths);
    append_table_separator(&mut output, &profile_widths);
    for row in profile_rows {
        let values = [
            row[0].as_str(),
            row[1].as_str(),
            row[2].as_str(),
            row[3].as_str(),
            row[4].as_str(),
        ];
        append_table_row(&mut output, &values, &profile_widths);
    }
    output.push_str("\nRULES\n");
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
            row[6].as_str(),
            row[7].as_str(),
        ];
        append_table_row(&mut output, &values, &widths);
    }
    output.push_str(&format!(
        "Processed entries: {}\n",
        state.processed_entries.len()
    ));
    output
}

fn profile_row(profile: &AutomationProfileRecord) -> [String; 5] {
    [
        sanitize_table_cell(&profile.id),
        sanitize_table_cell(&profile.name),
        sanitize_table_cell(&profile.translation_language),
        sanitize_table_cell(&profile.polish_preset_id),
        sanitize_table_cell(&profile.summary_template_id),
    ]
}

fn rule_row(rule: &AutomationRuleRecord) -> [String; 8] {
    [
        sanitize_table_cell(&rule.id),
        sanitize_table_cell(&rule.kind),
        sanitize_table_cell(&rule.name),
        rule.priority.to_string(),
        sanitize_table_cell(rule.profile_id.as_deref().unwrap_or("global")),
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
