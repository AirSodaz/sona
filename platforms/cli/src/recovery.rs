use clap::{Args, Subcommand};
use sona_core::recovery::service::RecoveryService;
use sona_core::recovery::types::RecoverySnapshot;
use sona_recovery_fs::FsRecoverySnapshotStore;
use sona_runtime_fs::FsSourcePathStatusProvider;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthStr;

use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct RecoveryArgs {
    #[command(subcommand)]
    command: RecoveryCommands,
}

#[derive(Debug, Subcommand)]
enum RecoveryCommands {
    /// Lists persisted recovery items.
    List(RecoveryListArgs),
}

#[derive(Debug, Args)]
struct RecoveryListArgs {
    /// Application data directory containing recovery state.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_recovery(args: RecoveryArgs) -> CliResult<CliOutput> {
    match args.command {
        RecoveryCommands::List(args) => run_recovery_list(args),
    }
}

fn run_recovery_list(args: RecoveryListArgs) -> CliResult<CliOutput> {
    let store = FsRecoverySnapshotStore::new(args.app_data_dir);
    let source_paths = FsSourcePathStatusProvider;
    let service = RecoveryService::new(&store, &source_paths);
    let snapshot = service.load_snapshot_at(now_ms()).map_err(CliError::Io)?;
    let output = if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_recovery_table(&snapshot)
    };

    Ok(CliOutput::stdout(output))
}

fn now_ms() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn render_recovery_table(snapshot: &RecoverySnapshot) -> String {
    let rows = snapshot
        .items
        .iter()
        .map(|item| {
            let values = [
                item.id.clone(),
                item.filename.clone(),
                item.last_known_stage.clone(),
                format!("{:.0}%", item.progress.clamp(0.0, 100.0)),
                if item.can_resume { "yes" } else { "no" }.to_string(),
            ];
            values.map(|value| sanitize_table_cell(&value))
        })
        .collect::<Vec<_>>();
    let headers = ["ID", "FILE", "STAGE", "PROGRESS", "RESUMABLE"];
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
        ];
        append_table_row(&mut output, &values, &widths);
    }
    output
}

fn append_table_row(output: &mut String, values: &[&str; 5], widths: &[usize; 5]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(value);
        output.push_str(&" ".repeat(widths[index].saturating_sub(UnicodeWidthStr::width(*value))));
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

fn append_table_separator(output: &mut String, widths: &[usize; 5]) {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::render_recovery_table;
    use serde_json::json;
    use sona_core::recovery::types::RecoverySnapshot;

    fn snapshot_with_rows(rows: &[(&str, &str)]) -> RecoverySnapshot {
        serde_json::from_value(json!({
            "version": 1,
            "updatedAt": 42,
            "items": rows.iter().map(|(id, filename)| json!({
                "id": id,
                "filename": filename,
                "filePath": "",
                "source": "batch_import",
                "resolution": "pending",
                "progress": 42,
                "segments": [],
                "projectId": null,
                "lastKnownStage": "transcribing",
                "updatedAt": 42,
                "hasSourceFile": true,
                "canResume": true,
                "exportConfig": null,
                "stageConfig": null
            })).collect::<Vec<_>>()
        }))
        .unwrap()
    }

    fn simple_display_width(value: &str) -> usize {
        value
            .chars()
            .map(|character| if character.is_ascii() { 1 } else { 2 })
            .sum()
    }

    #[test]
    fn table_escapes_control_characters_without_creating_extra_rows() {
        let snapshot = snapshot_with_rows(&[("row\nid\t\u{1b}\u{7}", "file\rname.wav")]);

        let table = render_recovery_table(&snapshot);

        assert_eq!(table.lines().count(), 3);
        assert!(table.contains(r"row\nid\t\u{1b}\u{7}"));
        assert!(table.contains(r"file\rname.wav"));
        assert!(
            !table
                .chars()
                .any(|character| character != '\n' && character.is_control())
        );
    }

    #[test]
    fn table_aligns_multibyte_filenames_by_visible_width() {
        let snapshot = snapshot_with_rows(&[("row-1", "会议.wav"), ("row-2", "meeting1")]);

        let table = render_recovery_table(&snapshot);
        let stage_columns = table
            .lines()
            .skip(2)
            .map(|line| {
                let stage_byte_index = line.find("transcribing").unwrap();
                simple_display_width(&line[..stage_byte_index])
            })
            .collect::<Vec<_>>();

        assert_eq!(stage_columns.len(), 2);
        assert_eq!(stage_columns[0], stage_columns[1]);
    }
}
