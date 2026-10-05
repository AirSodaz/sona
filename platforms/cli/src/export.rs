use std::path::PathBuf;

use clap::Args;
use sona_core::export::{
    ExportError, ExportFormat, ExportMode, ExportTranscriptFileRequest, ExportTranscriptFileResult,
};
use sona_core::transcription::transcript::TranscriptSegment;
use sona_export::export_transcript_file;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
#[command(
    about = "Converts transcript JSON segments into subtitle or text files (srt, vtt, txt, md, json)",
    after_help = "Input JSON format:\n  [\n    {\n      \"id\": \"segment-1\",\n      \"text\": \"Hello\",\n      \"start\": 0.0,\n      \"end\": 2.5,\n      \"isFinal\": true,\n      \"translation\": \"Bonjour\"\n    }\n  ]\n\nSupported export formats:\n  json, txt, srt, vtt, md (inferred from output file extension when omitted; required when exporting to stdout)\n\nExamples:\n  sona-cli export ./segments.json -o ./transcript.srt\n  sona-cli convert ./segments.json -o ./transcript.vtt\n  cat ./segments.json | sona-cli export -f srt > ./transcript.srt\n  sona-cli export ./segments.json -f txt"
)]
pub struct ExportArgs {
    /// Positional input JSON file containing an array of transcript segments.
    #[arg(value_name = "INPUT")]
    pub positional_input: Option<PathBuf>,
    /// Optional flag for JSON file containing an array of transcript segments (alternative to positional INPUT).
    #[arg(
        short = 'i',
        long,
        value_name = "JSON_FILE",
        help = "JSON file containing transcript segments (alternative to positional INPUT)"
    )]
    pub input: Option<PathBuf>,
    /// Destination file path, or "-" for stdout.
    #[arg(short = 'o', long, value_name = "PATH", default_value = "-")]
    pub output: PathBuf,
    /// Export format: json, txt, srt, vtt, or md; required when output is stdout ("-"), inferred from output file extension otherwise.
    #[arg(
        short = 'f',
        long,
        value_name = "FORMAT",
        value_parser = ["json", "txt", "srt", "vtt", "md"]
    )]
    pub format: Option<String>,
    /// Text selection mode: original, translation, or bilingual.
    #[arg(
        long,
        default_value = "original",
        value_name = "MODE",
        value_parser = ["original", "translation", "bilingual"]
    )]
    pub mode: String,
    /// Prints JSON summary instead of the default table output (file output only).
    #[arg(short = 'j', long)]
    pub json: bool,
    /// Overwrites existing destination file without confirmation.
    #[arg(short = 'F', long, help = "Overwrite existing destination file")]
    pub force: bool,
}

pub fn run_export(args: ExportArgs, io: &mut (dyn crate::CliIo + Send)) -> CliResult<CliOutput> {
    let mode =
        ExportMode::parse(&args.mode).map_err(|error| CliError::Validation(error.to_string()))?;

    let input_path = match (&args.positional_input, &args.input) {
        (Some(pos), Some(flag)) if pos != flag => {
            return Err(CliError::Validation(format!(
                "Conflicting input files: positional '{}' vs flag '{}'. Specify only one.",
                pos.display(),
                flag.display()
            )));
        }
        (Some(pos), _) => pos.clone(),
        (None, Some(flag)) => flag.clone(),
        (None, None) => PathBuf::from("-"),
    };
    if input_path.as_os_str() != "-" {
        let metadata = std::fs::metadata(&input_path).map_err(|error| {
            CliError::Io(format!(
                "Failed to read transcript input {}: {error}",
                input_path.display()
            ))
        })?;
        const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
        if metadata.len() > MAX_INPUT_BYTES {
            return Err(CliError::Validation(
                "Transcript input exceeds maximum supported size (64 MB).".to_string(),
            ));
        }
        if metadata.len() == 0 {
            return Err(CliError::Validation(format!(
                "Transcript input file {} is empty.",
                input_path.display()
            )));
        }
    }
    if input_path.as_os_str() == "-" && io.stdin_is_terminal() {
        return Err(CliError::Validation(
            "No transcript input provided. Specify an input file or pipe JSON segments via stdin."
                .to_string(),
        ));
    }

    let is_stdout = args.output.as_os_str() == "-";
    let format = match args.format {
        Some(value) => ExportFormat::parse(&value),
        None if !is_stdout => ExportFormat::from_output_path(&args.output),
        None => {
            return Err(CliError::Validation(
                "Export format must be specified with -f/--format when exporting to stdout ('-')."
                    .to_string(),
            ));
        }
    }
    .map_err(|error| CliError::Validation(error.to_string()))?;

    let input_bytes = if input_path.as_os_str() == "-" {
        if io.stdin_is_terminal() {
            return Err(CliError::Validation(
                "No transcript input provided. Specify an input file or pipe JSON segments via stdin.".to_string(),
            ));
        }
        const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
        let mut buf = Vec::new();
        io.read_bounded_stdin(&mut buf, MAX_INPUT_BYTES)
            .map_err(|error| {
                CliError::Io(format!(
                    "Failed to read transcript input from stdin: {error}"
                ))
            })?;
        if buf.is_empty() {
            return Err(CliError::Validation(
                "Standard input was empty; no transcript segments received.".to_string(),
            ));
        }
        if buf.len() > MAX_INPUT_BYTES {
            return Err(CliError::Validation(
                "Transcript input exceeds maximum supported size (64 MB).".to_string(),
            ));
        }
        buf
    } else {
        std::fs::read(&input_path).map_err(|error| {
            CliError::Io(format!(
                "Failed to read transcript input {}: {error}",
                input_path.display()
            ))
        })?
    };

    let segments: Vec<TranscriptSegment> = serde_json::from_slice(&input_bytes)
        .map_err(|error| CliError::Validation(format!("Invalid transcript JSON: {error}")))?;
    if is_stdout {
        let content = sona_core::export::export_segments_with_mode(&segments, format, mode)
            .map_err(map_export_error)?;
        return Ok(CliOutput::stdout(content));
    }

    if args.output.exists() && !args.force {
        return Err(CliError::Io(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            args.output.display()
        )));
    }

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
