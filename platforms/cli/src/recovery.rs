use clap::{Args, Subcommand};
use sona_core::recovery::{RecoveryError, types::RecoverySnapshot};
use sona_recovery_fs::FsRecoveryAdapter;
use std::path::PathBuf;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
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
    let snapshot = FsRecoveryAdapter::new(args.app_data_dir)
        .load_snapshot()
        .map_err(map_recovery_error)?;
    let output = if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_recovery_table(&snapshot)
    };

    Ok(CliOutput::stdout(output))
}

fn map_recovery_error(error: RecoveryError) -> CliError {
    let message = error.to_string();
    match error {
        RecoveryError::Repository(_) | RecoveryError::Path(_) | RecoveryError::Clock(_) => {
            CliError::Io(message)
        }
    }
}

fn render_recovery_table(snapshot: &RecoverySnapshot) -> String {
    let rows = snapshot
        .items
        .iter()
        .map(|item| {
            let values = [
                item.id.clone(),
                item.filename.clone(),
                item.last_known_stage.to_string(),
                format!("{:.0}%", item.progress.clamp(0.0, 100.0)),
                if item.can_resume { "yes" } else { "no" }.to_string(),
            ];
            values.map(|value| sanitize_table_cell(&value))
        })
        .collect::<Vec<_>>();
    let headers = ["ID", "FILE", "STAGE", "PROGRESS", "RESUMABLE"];
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
    output
}

#[cfg(test)]
mod tests {
    use super::render_recovery_table;
    use serde_json::json;
    use sona_core::recovery::types::RecoverySnapshot;

    fn snapshot_with_rows(rows: &[(&str, &str)]) -> RecoverySnapshot {
        serde_json::from_value(json!({
            "version": 2,
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

    #[test]
    fn maps_recovery_error_variants_to_io() {
        use crate::CliError;
        use sona_core::ports::{
            path::{PathKind, PathProviderError},
            time::ClockError,
        };
        use sona_core::recovery::RecoveryError;

        assert!(matches!(
            super::map_recovery_error(RecoveryError::Repository("storage unavailable".to_string())),
            CliError::Io(_)
        ));
        assert!(matches!(
            super::map_recovery_error(RecoveryError::Path(PathProviderError::new(
                PathKind::AppData,
                "path unavailable"
            ))),
            CliError::Io(_)
        ));
        assert!(matches!(
            super::map_recovery_error(RecoveryError::Clock(ClockError::Unavailable(
                "system clock unavailable".to_string()
            ))),
            CliError::Io(_)
        ));
    }
}
