use clap::{Parser, Subcommand};
use std::ffi::OsString;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Serialize(String),
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Usage(_) => 2,
            CliError::Serialize(_) => 1,
        }
    }
}

pub type CliResult<T> = Result<T, CliError>;

/// Standalone Sona command line interface.
#[derive(Debug, Parser)]
#[command(
    name = "sona-cli",
    version,
    about = "Standalone CLI backed by sona-core"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Resolves a filesystem path using the shared runtime status contract.
    PathStatus { path: String },
}

pub fn run_cli_from_args<I, T>(args: I) -> CliResult<String>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(|error| CliError::Usage(error.to_string()))?;

    match cli.command {
        Commands::PathStatus { path } => render_path_status_json(&path),
    }
}

pub fn render_path_status_json(path: &str) -> CliResult<String> {
    let status = sona_core::runtime::resolve_runtime_path_status(path);
    serde_json::to_string_pretty(&status).map_err(|error| CliError::Serialize(error.to_string()))
}
