mod desktop_paths;
mod init_config;
mod models;
mod transcribe;

use clap::{Parser, Subcommand};
use std::ffi::OsString;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    pub fn stdout(value: String) -> Self {
        Self {
            stdout: value,
            stderr: String::new(),
        }
    }

    pub fn stderr(value: String) -> Self {
        Self {
            stdout: String::new(),
            stderr: value,
        }
    }
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Serialize(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Model(String),
    #[error("{0}")]
    Other(String),
    #[error("Cancelled: {0}")]
    Cancelled(String),
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Usage(_) => 2,
            CliError::Validation(_) => 2,
            CliError::Serialize(_) => 1,
            CliError::Io(_) => 5,
            CliError::Network(_) => 4,
            CliError::Model(_) => 3,
            CliError::Other(_) => 1,
            CliError::Cancelled(_) => 130,
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
    /// Creates a commented TOML starter template.
    InitConfig(init_config::InitConfigArgs),
    /// Lists and manages preset models.
    Models(models::ModelsArgs),
    /// Transcribes a local audio or video file using offline ASR.
    Transcribe(transcribe::TranscribeArgs),
}

pub fn run_cli_from_args<I, T>(args: I) -> CliResult<CliOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(|error| CliError::Usage(error.to_string()))?;

    match cli.command {
        Commands::PathStatus { path } => render_path_status_json(&path).map(CliOutput::stdout),
        Commands::InitConfig(args) => init_config::run_init_config(args),
        Commands::Models(args) => models::run_models(args),
        Commands::Transcribe(args) => transcribe::run_transcribe(args),
    }
}

pub fn render_path_status_json(path: &str) -> CliResult<String> {
    let status = sona_core::runtime::resolve_runtime_path_status(path);
    serde_json::to_string_pretty(&status).map_err(|error| CliError::Serialize(error.to_string()))
}
