mod app_config;
mod asr_adapter;
mod automation;
mod backup;
mod config_template;
mod dashboard;
mod desktop_paths;
mod diagnostics;
mod export;
mod history;
mod init_config;
mod live_audio;
mod live_output;
mod llm;
mod models;
mod projects;
mod recovery;
mod serve;
mod storage;
mod table;
mod task_ledger;
mod transcribe;
mod transcribe_live;

use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
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

pub(crate) trait CliIo {
    fn stdout(&mut self) -> &mut dyn Write;
    fn stderr(&mut self) -> &mut dyn Write;
    fn stdout_is_terminal(&self) -> bool;
}

#[derive(Default)]
struct MemoryCliIo {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl MemoryCliIo {
    fn into_output(self) -> CliOutput {
        CliOutput {
            stdout: String::from_utf8_lossy(&self.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&self.stderr).into_owned(),
        }
    }
}

impl CliIo for MemoryCliIo {
    fn stdout(&mut self) -> &mut dyn Write {
        &mut self.stdout
    }

    fn stderr(&mut self) -> &mut dyn Write {
        &mut self.stderr
    }

    fn stdout_is_terminal(&self) -> bool {
        false
    }
}

struct StdCliIo {
    stdout: io::Stdout,
    stderr: io::Stderr,
    stdout_is_terminal: bool,
}

impl Default for StdCliIo {
    fn default() -> Self {
        Self {
            stdout: io::stdout(),
            stderr: io::stderr(),
            stdout_is_terminal: io::stdout().is_terminal(),
        }
    }
}

impl CliIo for StdCliIo {
    fn stdout(&mut self) -> &mut dyn Write {
        &mut self.stdout
    }

    fn stderr(&mut self) -> &mut dyn Write {
        &mut self.stderr
    }

    fn stdout_is_terminal(&self) -> bool {
        self.stdout_is_terminal
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

pub(crate) fn map_runtime_fs_error(error: sona_runtime_fs::RuntimeFsError) -> CliError {
    let message = error.to_string();
    match error {
        sona_runtime_fs::RuntimeFsError::FileSystem(_) => CliError::Io(message),
        sona_runtime_fs::RuntimeFsError::Serialization { .. } => CliError::Serialize(message),
        sona_runtime_fs::RuntimeFsError::Config(_)
        | sona_runtime_fs::RuntimeFsError::Validation(_)
        | sona_runtime_fs::RuntimeFsError::AlreadyExists { .. } => CliError::Validation(message),
    }
}

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
    /// Inspects persisted application configuration.
    AppConfig(app_config::AppConfigArgs),
    /// Inspects persisted automation rules and processed entries.
    Automation(automation::AutomationArgs),
    /// Exports, inspects, or imports complete Sona backup archives.
    Backup(backup::BackupArgs),
    /// Shows a read-only dashboard snapshot.
    Dashboard(dashboard::DashboardArgs),
    /// Builds diagnostics snapshots from host-provided facts.
    Diagnostics(diagnostics::DiagnosticsArgs),
    /// Exports transcript segments through the shared core service.
    Export(export::ExportArgs),
    /// Queries persisted history through the shared core service.
    History(history::HistoryArgs),
    /// Generates text or structured output and discovers online LLM models.
    Llm(llm::LlmArgs),
    /// Resolves a filesystem path using the shared runtime status contract.
    PathStatus { path: String },
    /// Creates a commented TOML starter template.
    InitConfig(init_config::InitConfigArgs),
    /// Lists and manages preset models.
    Models(models::ModelsArgs),
    /// Inspects persisted projects.
    Projects(projects::ProjectsArgs),
    /// Inspects persisted recovery snapshots.
    Recovery(recovery::RecoveryArgs),
    /// Runs the shared local HTTP API server.
    Serve(serve::ServeArgs),
    /// Shows read-only application storage usage.
    Storage(storage::StorageArgs),
    /// Inspects the shared task ledger.
    TaskLedger(task_ledger::TaskLedgerArgs),
    /// Transcribes a local audio or video file using offline ASR.
    Transcribe(transcribe::TranscribeArgs),
    /// Transcribe live audio from a microphone or stdin PCM using offline ASR.
    TranscribeLive(transcribe_live::TranscribeLiveArgs),
}

pub fn run_cli_from_args<I, T>(args: I) -> CliResult<CliOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(|error| CliError::Usage(error.to_string()))?;
    let mut io = MemoryCliIo::default();
    match dispatch(cli.command, &mut io)? {
        Some(output) => Ok(output),
        None => Ok(io.into_output()),
    }
}

pub fn execute_cli_from_args<I, T>(args: I) -> CliResult<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(|error| CliError::Usage(error.to_string()))?;
    let mut io = StdCliIo::default();
    if let Some(output) = dispatch(cli.command, &mut io)? {
        if !output.stdout.is_empty() {
            writeln!(io.stdout(), "{}", output.stdout)
                .map_err(|error| CliError::Io(format!("Failed to write stdout: {error}")))?;
        }
        if !output.stderr.is_empty() {
            writeln!(io.stderr(), "{}", output.stderr)
                .map_err(|error| CliError::Io(format!("Failed to write stderr: {error}")))?;
        }
    }
    Ok(())
}

fn dispatch(command: Commands, io: &mut dyn CliIo) -> CliResult<Option<CliOutput>> {
    let output = match command {
        Commands::AppConfig(args) => app_config::run_app_config(args),
        Commands::Automation(args) => automation::run_automation(args),
        Commands::Backup(args) => backup::run_backup(args),
        Commands::Dashboard(args) => dashboard::run_dashboard(args),
        Commands::Diagnostics(args) => diagnostics::run_diagnostics(args),
        Commands::Export(args) => export::run_export(args),
        Commands::History(args) => history::run_history(args),
        Commands::Llm(args) => llm::run_llm(args),
        Commands::PathStatus { path } => render_path_status_json(&path).map(CliOutput::stdout),
        Commands::InitConfig(args) => init_config::run_init_config(args),
        Commands::Models(args) => models::run_models(args),
        Commands::Projects(args) => projects::run_projects(args),
        Commands::Recovery(args) => recovery::run_recovery(args),
        Commands::Serve(args) => serve::run_serve(args),
        Commands::Storage(args) => storage::run_storage(args),
        Commands::TaskLedger(args) => task_ledger::run_task_ledger(args),
        Commands::Transcribe(args) => transcribe::run_transcribe(args),
        Commands::TranscribeLive(args) => {
            transcribe_live::run_transcribe_live(args, io)?;
            return Ok(None);
        }
    }?;
    Ok(Some(output))
}

pub fn render_path_status_json(path: &str) -> CliResult<String> {
    let status = sona_runtime_fs::resolve_runtime_path_status(path);
    serde_json::to_string_pretty(&status).map_err(|error| CliError::Serialize(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_invalid_ip_whitelist_is_validation_error() {
        let error = run_cli_from_args(["sona-cli", "serve", "--ip-whitelist", "not-a-rule"])
            .expect_err("invalid whitelist should fail before server startup");

        assert!(matches!(error, CliError::Validation(_)));
        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "Invalid IP rule format: not-a-rule");
    }

    #[test]
    fn structured_error_categories_keep_exit_code_contracts() {
        assert_eq!(
            CliError::Validation("invalid input".to_string()).exit_code(),
            2
        );
        assert_eq!(
            CliError::Serialize("failed to render output".to_string()).exit_code(),
            1
        );
        assert_eq!(
            CliError::Io("storage unavailable".to_string()).exit_code(),
            5
        );
    }

    #[test]
    fn runtime_filesystem_errors_map_to_existing_cli_categories() {
        let filesystem = map_runtime_fs_error(sona_runtime_fs::RuntimeFsError::FileSystem(
            sona_core::ports::fs::FileSystemError::new(
                sona_core::ports::fs::FileSystemOperation::ReadText,
                "missing.toml",
                "not found",
            ),
        ));
        let serialization = map_runtime_fs_error(sona_runtime_fs::RuntimeFsError::Serialization {
            path: "settings.json".into(),
            reason: "invalid JSON".into(),
        });
        let validation = map_runtime_fs_error(sona_runtime_fs::RuntimeFsError::Validation(
            sona_core::runtime::error::RuntimeValidationError::new("input", "invalid input"),
        ));

        assert!(matches!(filesystem, CliError::Io(_)));
        assert_eq!(filesystem.exit_code(), 5);
        assert!(matches!(serialization, CliError::Serialize(_)));
        assert_eq!(serialization.exit_code(), 1);
        assert!(matches!(validation, CliError::Validation(_)));
        assert_eq!(validation.exit_code(), 2);
    }
}
