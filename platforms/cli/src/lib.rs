mod asr_adapter;
mod config_template;
mod desktop_paths;
mod diagnostics;
mod export;
mod init_config;
pub mod live_audio;
pub mod live_output;
pub mod logger;
mod models;
mod online_asr;
pub(crate) mod runtime;
mod serve;
mod table;
mod transcribe;
pub mod transcribe_live;

use clap::{Parser, Subcommand};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::future::Future;
use std::io::{self, IsTerminal, Write};
use std::pin::Pin;
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

pub(crate) trait CliIo: Send {
    fn stdout(&mut self) -> &mut (dyn Write + Send);
    fn stderr(&mut self) -> &mut (dyn Write + Send);
    fn stdout_is_terminal(&self) -> bool;
    fn stderr_is_terminal(&self) -> bool {
        false
    }
    fn stdin_is_terminal(&self) -> bool {
        false
    }
    fn read_line_stdin(&mut self, _buf: &mut String) -> io::Result<usize> {
        Ok(0)
    }
    fn prompt_line_async<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = io::Result<Option<String>>> + Send + 'a>> {
        Box::pin(async { Ok(None) })
    }
}

#[derive(Default)]
struct MemoryCliIo {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdin_lines: VecDeque<String>,
    stdin_is_terminal: bool,
    stderr_is_terminal: bool,
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
    fn stdout(&mut self) -> &mut (dyn Write + Send) {
        &mut self.stdout
    }

    fn stderr(&mut self) -> &mut (dyn Write + Send) {
        &mut self.stderr
    }
    fn stdout_is_terminal(&self) -> bool {
        false
    }
    fn stderr_is_terminal(&self) -> bool {
        self.stderr_is_terminal
    }
    fn stdin_is_terminal(&self) -> bool {
        self.stdin_is_terminal
    }
    fn read_line_stdin(&mut self, buf: &mut String) -> io::Result<usize> {
        if let Some(line) = self.stdin_lines.pop_front() {
            buf.push_str(&line);
            Ok(line.len())
        } else {
            Ok(0)
        }
    }
    fn prompt_line_async<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = io::Result<Option<String>>> + Send + 'a>> {
        let line = self.stdin_lines.pop_front();
        Box::pin(async move { Ok(line) })
    }
}

struct StdCliIo {
    stdout: io::Stdout,
    stderr: io::Stderr,
}

impl Default for StdCliIo {
    fn default() -> Self {
        Self {
            stdout: io::stdout(),
            stderr: io::stderr(),
        }
    }
}

impl CliIo for StdCliIo {
    fn stdout(&mut self) -> &mut (dyn Write + Send) {
        &mut self.stdout
    }

    fn stderr(&mut self) -> &mut (dyn Write + Send) {
        &mut self.stderr
    }
    fn stdout_is_terminal(&self) -> bool {
        self.stdout.is_terminal()
    }
    fn stderr_is_terminal(&self) -> bool {
        self.stderr.is_terminal()
    }
    fn stdin_is_terminal(&self) -> bool {
        io::stdin().is_terminal()
    }
    fn read_line_stdin(&mut self, buf: &mut String) -> io::Result<usize> {
        io::stdin().read_line(buf)
    }
    fn prompt_line_async<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = io::Result<Option<String>>> + Send + 'a>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let spawn_res = std::thread::Builder::new()
            .name("cli-stdin-prompt".into())
            .spawn(move || {
                let mut line = String::new();
                let res = match io::stdin().read_line(&mut line) {
                    Ok(0) => Ok(None),
                    Ok(_) => Ok(Some(line)),
                    Err(error) => Err(error),
                };
                let _ = tx.send(res);
            });

        Box::pin(async move {
            match spawn_res {
                Ok(_) => match rx.await {
                    Ok(res) => res,
                    Err(_) => Ok(None),
                },
                Err(e) => Err(e),
            }
        })
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
    /// Builds diagnostics snapshots from host-provided facts.
    Diagnostics(diagnostics::DiagnosticsArgs),
    /// Exports transcript segments through the shared core service.
    Export(export::ExportArgs),
    /// Resolves a filesystem path using the shared runtime status contract.
    PathStatus {
        /// Filesystem path to inspect.
        #[arg(value_name = "PATH", help = "Filesystem path to inspect")]
        path: String,
    },
    InitConfig(init_config::InitConfigArgs),
    /// Lists and manages preset models.
    Models(models::ModelsArgs),
    /// Runs the shared local HTTP API server.
    Serve(serve::ServeArgs),
    /// Transcribe audio with local or online ASR; local ASR also accepts video.
    Transcribe(transcribe::TranscribeArgs),
    /// Transcribe live audio using local or online ASR.
    TranscribeLive(transcribe_live::TranscribeLiveArgs),
}

enum ParsedCli {
    Command(Box<Cli>),
    EarlyExit(CliOutput),
}

fn parse_cli_args<I, T>(args: I) -> CliResult<ParsedCli>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(cli) => Ok(ParsedCli::Command(Box::new(cli))),
        Err(error) => match error.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                Ok(ParsedCli::EarlyExit(CliOutput::stdout(error.to_string())))
            }
            _ => Err(CliError::Usage(error.to_string())),
        },
    }
}

pub async fn run_cli_from_args_async<I, T>(args: I) -> CliResult<CliOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let command = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => cli.command,
        ParsedCli::EarlyExit(output) => return Ok(output),
    };
    let mut io = MemoryCliIo::default();
    match dispatch(command, &mut io).await? {
        Some(output) => Ok(output),
        None => Ok(io.into_output()),
    }
}

pub fn run_cli_from_args<I, T>(args: I) -> CliResult<CliOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let command = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => cli.command,
        ParsedCli::EarlyExit(output) => return Ok(output),
    };
    runtime::block_on(async move {
        let mut io = MemoryCliIo::default();
        match dispatch(command, &mut io).await? {
            Some(output) => Ok(output),
            None => Ok(io.into_output()),
        }
    })?
}

pub async fn execute_cli_from_args<I, T>(args: I) -> CliResult<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut io = StdCliIo::default();
    let command = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => cli.command,
        ParsedCli::EarlyExit(output) => {
            if !output.stdout.is_empty() {
                write!(io.stdout(), "{}", output.stdout)
                    .map_err(|error| CliError::Io(format!("Failed to write stdout: {error}")))?;
            }
            return Ok(());
        }
    };
    if let Some(output) = dispatch(command, &mut io).await? {
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

async fn dispatch(command: Commands, io: &mut (dyn CliIo + Send)) -> CliResult<Option<CliOutput>> {
    let default_level = match &command {
        Commands::Serve(_) => log::LevelFilter::Info,
        _ => log::LevelFilter::Warn,
    };
    logger::init_logger(default_level);

    let output = match command {
        Commands::Diagnostics(args) => diagnostics::run_diagnostics(args),
        Commands::Export(args) => export::run_export(args),
        Commands::PathStatus { path } => render_path_status_json(&path).map(CliOutput::stdout),
        Commands::InitConfig(args) => init_config::run_init_config(args),
        Commands::Models(args) => models::run_models(args, io).await,
        Commands::Serve(args) => serve::run_serve(args, io).await,
        Commands::Transcribe(args) => transcribe::run_transcribe(args).await,
        Commands::TranscribeLive(args) => {
            transcribe_live::run_transcribe_live(args, io).await?;
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
