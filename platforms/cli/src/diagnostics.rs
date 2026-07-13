use clap::{Args, Subcommand};
use sona_core::runtime::diagnostics::{
    DiagnosticsCoreInput, DiagnosticsCoreSnapshot, DiagnosticsService,
};
use sona_runtime_fs::FsDiagnosticsEnrichmentRepository;
use std::path::PathBuf;
use std::sync::Arc;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct DiagnosticsArgs {
    #[command(subcommand)]
    command: DiagnosticsCommands,
}

#[derive(Debug, Subcommand)]
enum DiagnosticsCommands {
    /// Builds a diagnostics snapshot from host-provided facts.
    Snapshot(DiagnosticsSnapshotArgs),
}

#[derive(Debug, Args)]
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
        DiagnosticsCommands::Snapshot(args) => run_diagnostics_snapshot(args),
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
    let snapshot = DiagnosticsService::new(Arc::new(repository))
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
