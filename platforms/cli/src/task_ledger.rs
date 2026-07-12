use clap::{Args, Subcommand};
use serde::Serialize;
use sona_core::task_ledger::service::TaskLedgerService;
use sona_core::task_ledger::types::{TaskLedgerRecord, TaskLedgerSnapshot};
use sona_sqlite::{Database, SqliteLedgerRepository};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthStr;

use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct TaskLedgerArgs {
    #[command(subcommand)]
    command: TaskLedgerCommands,
}

#[derive(Debug, Subcommand)]
enum TaskLedgerCommands {
    /// Lists retained task ledger records.
    List(TaskLedgerListArgs),
}

#[derive(Debug, Args)]
struct TaskLedgerListArgs {
    /// Application data directory containing the Sona database.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_task_ledger(args: TaskLedgerArgs) -> CliResult<CliOutput> {
    match args.command {
        TaskLedgerCommands::List(args) => run_task_ledger_list(args),
    }
}

fn run_task_ledger_list(args: TaskLedgerListArgs) -> CliResult<CliOutput> {
    let database = Database::open_read_only(&args.app_data_dir)
        .map_err(|error| CliError::Io(error.to_string()))?;
    let repository = SqliteLedgerRepository::new(Arc::new(database));
    let snapshot = TaskLedgerService::new(&repository)
        .load_snapshot_at(now_ms())
        .map_err(CliError::Io)?;
    let output = if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_task_ledger_table(&snapshot)?
    };

    Ok(CliOutput::stdout(output))
}

fn render_task_ledger_table(snapshot: &TaskLedgerSnapshot) -> CliResult<String> {
    let rows = snapshot
        .tasks
        .iter()
        .map(task_row)
        .collect::<CliResult<Vec<_>>>()?;
    let headers = ["ID", "KIND", "STATUS", "TITLE", "PROGRESS", "UPDATED"];
    let mut widths = headers.map(UnicodeWidthStr::width);

    for row in &rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(value.as_str()));
        }
    }

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
    Ok(output)
}

fn task_row(task: &TaskLedgerRecord) -> CliResult<[String; 6]> {
    Ok([
        sanitize_table_cell(&task.id),
        enum_label(&task.kind)?,
        enum_label(&task.status)?,
        sanitize_table_cell(&task.title),
        format!("{}%", task.progress),
        task.updated_at.to_string(),
    ])
}

fn enum_label<T: Serialize>(value: &T) -> CliResult<String> {
    match serde_json::to_value(value).map_err(|error| CliError::Serialize(error.to_string()))? {
        serde_json::Value::String(value) => Ok(value),
        _ => Err(CliError::Serialize(
            "Task ledger enum did not serialize as a string".to_string(),
        )),
    }
}

fn append_table_row(output: &mut String, values: &[&str; 6], widths: &[usize; 6]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(value);
        output.push_str(&" ".repeat(widths[index].saturating_sub(UnicodeWidthStr::width(*value))));
    }
    output.push('\n');
}

fn append_table_separator(output: &mut String, widths: &[usize; 6]) {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');
}

fn sanitize_table_cell(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            sanitized.extend(character.escape_default());
        } else {
            sanitized.push(character);
        }
    }
    sanitized
}

fn now_ms() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
