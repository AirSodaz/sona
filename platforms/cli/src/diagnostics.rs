use clap::{Args, Subcommand};
use sona_application::diagnostics::DiagnosticsService;
use sona_core::runtime::diagnostics::{DiagnosticsCoreInput, DiagnosticsCoreSnapshot};
use sona_runtime_fs::FsDiagnosticsEnrichmentRepository;
use std::path::PathBuf;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
#[command(
    about = "Builds diagnostics snapshots from host-provided facts",
    after_help = "Examples:\n  sona-cli diagnostics --app-data-dir ./app_data --input ./facts.json\n  sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json"
)]
pub struct DiagnosticsArgs {
    #[command(subcommand)]
    command: Option<DiagnosticsCommands>,
    #[command(flatten)]
    direct: DiagnosticsDirectArgs,
}

#[derive(Debug, Subcommand)]
enum DiagnosticsCommands {
    /// Builds a diagnostics snapshot from host-provided facts.
    Snapshot(DiagnosticsSnapshotArgs),
}

#[derive(Debug, Args)]
struct DiagnosticsDirectArgs {
    /// Application data directory containing the models directory.
    #[arg(long, value_name = "PATH")]
    app_data_dir: Option<PathBuf>,
    /// JSON file containing host diagnostics facts and model paths.
    #[arg(long, value_name = "JSON_FILE")]
    input: Option<PathBuf>,
    /// Prints JSON instead of the default table output.
    #[arg(long, default_value_t = false)]
    json: bool,
}
#[derive(Debug, Args)]
#[command(
    about = "Builds a diagnostics snapshot from host-provided facts",
    after_help = "Input JSON format (DiagnosticsCoreInput):\n  {\n    \"config\": {\n      \"streamingModelPath\": \"/path/to/streaming-model\",\n      \"batchModelPath\": \"/path/to/batch-model\",\n      \"vadModelPath\": \"\",\n      \"punctuationModelPath\": \"\",\n      \"microphoneId\": \"default\"\n    },\n    \"permissionState\": \"granted\",\n    \"microphoneProbe\": { \"options\": [], \"available\": true, \"errorMessage\": null },\n    \"systemAudioProbe\": { \"options\": [], \"available\": false, \"errorMessage\": null },\n    \"voiceTypingReadiness\": { \"state\": \"ready\", \"lastErrorMessage\": null }\n  }\n\nExamples:\n  sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json\n  sona-cli diagnostics snapshot --app-data-dir ./app_data --input ./facts.json --json"
)]
struct DiagnosticsSnapshotArgs {
    /// Application data directory containing the models directory.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// JSON file containing host diagnostics facts and model paths.
    #[arg(long, value_name = "JSON_FILE")]
    input: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_diagnostics(args: DiagnosticsArgs) -> CliResult<CliOutput> {
    match args.command {
        Some(DiagnosticsCommands::Snapshot(args)) => run_diagnostics_snapshot(args),
        None => {
            let app_data_dir = args.direct.app_data_dir.ok_or_else(|| {
                CliError::Validation("Missing required option: --app-data-dir <PATH>".to_string())
            })?;
            let input = args.direct.input.ok_or_else(|| {
                CliError::Validation("Missing required option: --input <JSON_FILE>".to_string())
            })?;
            run_diagnostics_snapshot(DiagnosticsSnapshotArgs {
                app_data_dir,
                input,
                json: args.direct.json,
            })
        }
    }
}

fn run_diagnostics_snapshot(args: DiagnosticsSnapshotArgs) -> CliResult<CliOutput> {
    let input_json = std::fs::read(&args.input).map_err(|error| {
        CliError::Io(format!(
            "Failed to read diagnostics input {}: {error}",
            args.input.display()
        ))
    })?;
    let input: DiagnosticsCoreInput = serde_json::from_slice(&input_json)
        .map_err(|error| CliError::Validation(error.to_string()))?;
    let app_data_dir =
        std::path::absolute(args.app_data_dir).map_err(|error| CliError::Io(error.to_string()))?;
    let repository = FsDiagnosticsEnrichmentRepository::new(app_data_dir.join("models"));
    let snapshot = DiagnosticsService::new(std::sync::Arc::new(repository))
        .build_snapshot_at(input, sona_runtime_fs::diagnostics_scanned_at_now())
        .map_err(|error| CliError::Io(error.to_string()))?;
    let output = if args.json {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_diagnostics_table(&snapshot)
    };

    Ok(CliOutput::stdout(output))
}

fn render_diagnostics_table(snapshot: &DiagnosticsCoreSnapshot) -> String {
    let headers = [
        "SCANNED",
        "LIVE_MODEL",
        "BATCH_MODEL",
        "ONBOARDING",
        "PUNCTUATION",
        "PERMISSION",
        "MIC",
        "SYSTEM_AUDIO",
    ];
    let row = [
        snapshot.scanned_at.clone(),
        selected_model_id(snapshot.selected_models.live.as_ref()),
        selected_model_id(snapshot.selected_models.batch.as_ref()),
        snapshot.onboarding_ready.to_string(),
        snapshot.punctuation_required.to_string(),
        sanitize_table_cell(&snapshot.permission_state),
        snapshot.microphone_probe.available.to_string(),
        snapshot.system_audio_probe.available.to_string(),
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

fn selected_model_id(model: Option<&sona_core::runtime::diagnostics::ModelSummaryInput>) -> String {
    model
        .map(|model| model.id.clone())
        .unwrap_or_else(|| "unresolved".to_string())
}
