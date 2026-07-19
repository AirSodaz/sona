use std::path::PathBuf;

use clap::{Args, Subcommand};
use sona_core::export::{
    ExportError, ExportFormat, ExportMode, ExportTranscriptFileRequest, ExportTranscriptFileResult,
};
use sona_core::transcription::transcript::TranscriptSegment;
use sona_export::export_transcript_file;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[command(subcommand)]
    command: ExportCommands,
}

#[derive(Debug, Subcommand)]
enum ExportCommands {
    /// Exports transcript segments to a file.
    Transcript(ExportTranscriptArgs),
}

#[derive(Debug, Args)]
struct ExportTranscriptArgs {
    /// JSON file containing an array of transcript segments.
    #[arg(long, value_name = "JSON_FILE")]
    input: PathBuf,
    /// Destination file path.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
    /// Export format; inferred from the output extension when omitted.
    #[arg(long, value_name = "FORMAT")]
    format: Option<String>,
    /// Text selection mode: original, translation, or bilingual.
    #[arg(long, default_value = "original", value_name = "MODE")]
    mode: String,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_export(args: ExportArgs) -> CliResult<CliOutput> {
    match args.command {
        ExportCommands::Transcript(args) => run_export_transcript(args),
    }
}

fn run_export_transcript(args: ExportTranscriptArgs) -> CliResult<CliOutput> {
    let input = std::fs::read(&args.input).map_err(|error| {
        CliError::Io(format!(
            "Failed to read transcript input {}: {error}",
            args.input.display()
        ))
    })?;
    let segments: Vec<TranscriptSegment> =
        serde_json::from_slice(&input).map_err(|error| CliError::Validation(error.to_string()))?;
    let format = match args.format {
        Some(value) => ExportFormat::parse(&value),
        None => ExportFormat::from_output_path(&args.output),
    }
    .map_err(|error| CliError::Validation(error.to_string()))?;
    let mode =
        ExportMode::parse(&args.mode).map_err(|error| CliError::Validation(error.to_string()))?;
    let request = ExportTranscriptFileRequest {
        segments,
        format,
        mode,
        output_path: args.output.to_string_lossy().into_owned(),
    };
    let result = export_transcript_file(request).map_err(map_export_error)?;
    let output = if args.json {
        serde_json::to_string_pretty(&result)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_export_result_table(&result)
    };
    Ok(CliOutput::stdout(output))
}

fn map_export_error(error: ExportError) -> CliError {
    match error {
        validation_error @ (ExportError::InvalidFormat { .. }
        | ExportError::MissingFormatExtension { .. }
        | ExportError::InvalidMode { .. }) => CliError::Validation(validation_error.to_string()),
        ExportError::Render { reason } => CliError::Serialize(reason),
        ExportError::Repository { reason } => CliError::Io(reason),
    }
}

fn render_export_result_table(result: &ExportTranscriptFileResult) -> String {
    let headers = ["OUTPUT", "BYTES"];
    let row = [
        sanitize_table_cell(&result.output_path),
        result.bytes_written.to_string(),
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
