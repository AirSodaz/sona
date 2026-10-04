mod asr_adapter;
mod config_cmd;
mod config_template;
mod desktop_paths;
mod diagnostics;
mod doctor;
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
    fn read_to_end_stdin(&mut self, _buf: &mut Vec<u8>) -> io::Result<usize> {
        Ok(0)
    }
    fn read_bounded_stdin(&mut self, buf: &mut Vec<u8>, _limit: usize) -> io::Result<usize> {
        self.read_to_end_stdin(buf)
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
    stdin_bytes: Vec<u8>,
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
    fn read_to_end_stdin(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let len = self.stdin_bytes.len();
        buf.extend_from_slice(&self.stdin_bytes);
        self.stdin_bytes.clear();
        Ok(len)
    }
    fn read_bounded_stdin(&mut self, buf: &mut Vec<u8>, limit: usize) -> io::Result<usize> {
        let max_to_read = self.stdin_bytes.len().min(limit.saturating_add(1));
        buf.extend(self.stdin_bytes.drain(..max_to_read));
        Ok(max_to_read)
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
    fn read_to_end_stdin(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        use io::Read;
        io::stdin().read_to_end(buf)
    }
    fn read_bounded_stdin(&mut self, buf: &mut Vec<u8>, limit: usize) -> io::Result<usize> {
        use io::Read;
        io::stdin()
            .take(limit.saturating_add(1) as u64)
            .read_to_end(buf)
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
    about = "Standalone CLI backed by sona-core",
    after_help = "Quick Start:\n  1. Inspect & download a local ASR model:\n       sona-cli models list\n       sona-cli models download whisper-turbo\n  2. Transcribe an audio or video file:\n       sona-cli transcribe ./sample.wav -m whisper-turbo\n       sona-cli transcribe ./sample.wav -m whisper-turbo -o ./transcript.srt\n  3. Transcribe via cloud provider:\n       export GROQ_API_KEY=\"...\"\n       sona-cli transcribe ./sample.wav --online-provider groq-whisper\n  4. Live streaming transcription:\n       sona-cli transcribe-live -m sensevoice\n       ffmpeg -i audio.wav -f s16le -ac 1 -ar 16000 - | \\\n         sona-cli transcribe-live --input stdin -m sensevoice\n  5. Generate shell completion:\n       sona-cli completion bash > /etc/bash_completion.d/sona-cli\n\nUse 'sona-cli <COMMAND> --help' for command-specific options."
)]
struct Cli {
    /// Enable verbose logging (-v for info, -vv for debug).
    #[arg(short = 'v', long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Builds diagnostics snapshots from host-provided facts.
    #[command(hide = true)]
    Diagnostics(diagnostics::DiagnosticsArgs),
    /// Exports transcript segments through the shared core service.
    Export(export::ExportArgs),
    /// Resolves a filesystem path using the shared runtime status contract.
    #[command(hide = true)]
    PathStatus {
        /// Filesystem path to inspect.
        #[arg(value_name = "PATH", help = "Filesystem path to inspect")]
        path: String,
    },
    /// Inspects and manages Sona CLI configuration.
    Config(config_cmd::ConfigArgs),
    /// Manage, download, verify, and inspect preset ASR models.
    Models(models::ModelsArgs),
    /// Runs the shared local HTTP API server.
    Serve(serve::ServeArgs),
    /// Transcribe audio with local or online ASR; local ASR also accepts video.
    Transcribe(transcribe::TranscribeArgs),
    /// Transcribe live audio using local or online ASR.
    #[command(visible_alias = "live")]
    TranscribeLive(transcribe_live::TranscribeLiveArgs),
    /// Generates shell auto-completion scripts.
    Completion(CompletionArgs),
    /// Checks system dependencies, audio devices, and models directory.
    Doctor(doctor::DoctorArgs),
    /// Lists available audio input (microphone) devices.
    Devices(DevicesArgs),
    /// Lists supported online ASR providers.
    Providers(ProvidersArgs),
}

#[derive(Debug, clap::Args)]
#[command(
    about = "Lists available audio input (microphone) devices",
    after_help = "Examples:\n  sona-cli devices\n  sona-cli devices --json"
)]
pub struct DevicesArgs {
    /// Print machine-readable JSON.
    #[arg(short = 'j', long, help = "Print machine-readable JSON")]
    pub json: bool,
}

#[derive(Debug, clap::Args)]
#[command(
    about = "Lists supported online ASR providers and their models",
    after_help = "Examples:\n  sona-cli providers\n  sona-cli providers --models\n  sona-cli providers groq-whisper\n  sona-cli providers --json"
)]
pub struct ProvidersArgs {
    /// Optional provider ID to inspect in detail (e.g. groq-whisper, volcengine-doubao).
    #[arg(
        value_name = "PROVIDER",
        help = "Optional provider ID to inspect in detail"
    )]
    pub provider: Option<String>,
    /// Display supported models and recommendations for all providers.
    #[arg(short = 'm', long, help = "Show supported models for each provider")]
    pub models: bool,
    /// Print machine-readable JSON.
    #[arg(short = 'j', long, help = "Print machine-readable JSON")]
    pub json: bool,
}
#[derive(Debug, clap::Args)]
#[command(
    about = "Generates shell auto-completion scripts",
    after_help = "Examples:\n  sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli\n  sona-cli completion zsh > ~/.zfunc/_sona-cli\n  sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish\n  sona-cli completion powershell | Out-File -Append -Encoding utf8 $PROFILE"
)]
pub struct CompletionArgs {
    /// Target shell to generate completions for: bash, elvish, fish, powershell, zsh.
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
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
    let (command, verbose) = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => (cli.command, cli.verbose),
        ParsedCli::EarlyExit(output) => return Ok(output),
    };
    let mut io = MemoryCliIo::default();
    match dispatch(command, verbose, &mut io).await? {
        Some(output) => Ok(output),
        None => Ok(io.into_output()),
    }
}

pub fn run_cli_from_args<I, T>(args: I) -> CliResult<CliOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let (command, verbose) = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => (cli.command, cli.verbose),
        ParsedCli::EarlyExit(output) => return Ok(output),
    };
    runtime::block_on(async move {
        let mut io = MemoryCliIo::default();
        match dispatch(command, verbose, &mut io).await? {
            Some(output) => Ok(output),
            None => Ok(io.into_output()),
        }
    })?
}

pub fn run_cli_from_args_with_stdin<I, T>(args: I, stdin_bytes: Vec<u8>) -> CliResult<CliOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let (command, verbose) = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => (cli.command, cli.verbose),
        ParsedCli::EarlyExit(output) => return Ok(output),
    };
    runtime::block_on(async move {
        let mut io = MemoryCliIo {
            stdin_bytes,
            ..Default::default()
        };
        match dispatch(command, verbose, &mut io).await? {
            Some(output) => Ok(output),
            None => Ok(io.into_output()),
        }
    })?
}

pub fn run_cli_from_args_with_terminal_stdin<I, T>(args: I) -> CliResult<CliOutput>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let (command, verbose) = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => (cli.command, cli.verbose),
        ParsedCli::EarlyExit(output) => return Ok(output),
    };
    runtime::block_on(async move {
        let mut io = MemoryCliIo {
            stdin_is_terminal: true,
            ..Default::default()
        };
        match dispatch(command, verbose, &mut io).await? {
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
    let (command, verbose) = match parse_cli_args(args)? {
        ParsedCli::Command(cli) => (cli.command, cli.verbose),
        ParsedCli::EarlyExit(output) => {
            if !output.stdout.is_empty() {
                write!(io.stdout(), "{}", output.stdout)
                    .map_err(|error| CliError::Io(format!("Failed to write stdout: {error}")))?;
            }
            return Ok(());
        }
    };
    if let Some(output) = dispatch(command, verbose, &mut io).await? {
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

async fn dispatch(
    command: Commands,
    verbose: u8,
    io: &mut (dyn CliIo + Send),
) -> CliResult<Option<CliOutput>> {
    let default_level = match verbose {
        0 => match &command {
            Commands::Serve(_) => log::LevelFilter::Info,
            _ => log::LevelFilter::Warn,
        },
        1 => log::LevelFilter::Info,
        _ => log::LevelFilter::Debug,
    };
    logger::init_logger(default_level);

    let output = match command {
        Commands::Diagnostics(args) => diagnostics::run_diagnostics(args),
        Commands::Export(args) => export::run_export(args, io),
        Commands::PathStatus { path } => render_path_status_json(&path).map(CliOutput::stdout),
        Commands::Config(args) => config_cmd::run_config(args),
        Commands::Models(args) => models::run_models(args, io).await,
        Commands::Serve(args) => serve::run_serve(args, io).await,
        Commands::Transcribe(args) => transcribe::run_transcribe(args, io).await,
        Commands::TranscribeLive(args) => {
            transcribe_live::run_transcribe_live(args, io).await?;
            return Ok(None);
        }
        Commands::Completion(args) => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            let mut buf = Vec::new();
            clap_complete::generate(args.shell, &mut cmd, "sona-cli", &mut buf);
            let script =
                String::from_utf8(buf).map_err(|error| CliError::Serialize(error.to_string()))?;
            Ok(CliOutput::stdout(script))
        }
        Commands::Doctor(args) => doctor::run_doctor(args).await,
        Commands::Devices(args) => run_devices(args),
        Commands::Providers(args) => run_providers(args),
    }?;
    Ok(Some(output))
}

fn run_devices(args: DevicesArgs) -> CliResult<CliOutput> {
    let devices = live_audio::microphone_device_names().map_err(CliError::Io)?;
    let default_device = live_audio::default_microphone_device_name();
    if args.json {
        let json_val = serde_json::json!({
            "default_device": default_device,
            "devices": devices.iter().enumerate().map(|(idx, d)| {
                serde_json::json!({
                    "index": idx,
                    "name": d,
                    "is_default": default_device.as_deref() == Some(d.as_str()),
                })
            }).collect::<Vec<_>>()
        });
        let output = serde_json::to_string_pretty(&json_val)
            .map_err(|e| CliError::Serialize(e.to_string()))?;
        Ok(CliOutput::stdout(output))
    } else {
        let output = transcribe_live::format_input_device_list(&devices, default_device.as_deref());
        Ok(CliOutput::stdout(output))
    }
}

fn run_providers(args: ProvidersArgs) -> CliResult<CliOutput> {
    if args.json {
        if let Some(provider_id) = &args.provider {
            let p = sona_core::ports::asr::find_online_asr_provider(provider_id).ok_or_else(|| {
                CliError::Validation(format!(
                    "Unknown online ASR provider '{provider_id}'. Run 'sona-cli providers' to see available providers."
                ))
            })?;
            let mut modes = vec!["batch"];
            if p.streaming.supported.unwrap_or(false) {
                modes.push("streaming");
            }
            let configured = p
                .default_api_key_env()
                .is_some_and(|var| std::env::var_os(var).is_some_and(|val| !val.is_empty()));
            let json_val = serde_json::json!({
                "id": p.id,
                "default_env_var": p.default_api_key_env(),
                "configured": configured,
                "modes": modes,
                "spec": p.spec,
                "models": p.models,
            });
            let output = serde_json::to_string_pretty(&json_val)
                .map_err(|e| CliError::Serialize(e.to_string()))?;
            return Ok(CliOutput::stdout(output));
        }
        let providers = sona_core::ports::asr::online_asr_providers();
        let json_arr = providers
            .iter()
            .map(|p| {
                let mut modes = vec!["batch"];
                if p.streaming.supported.unwrap_or(false) {
                    modes.push("streaming");
                }
                let configured = p
                    .default_api_key_env()
                    .is_some_and(|var| std::env::var_os(var).is_some_and(|val| !val.is_empty()));
                serde_json::json!({
                    "id": p.id,
                    "default_env_var": p.default_api_key_env(),
                    "configured": configured,
                    "modes": modes,
                    "models": p.models,
                })
            })
            .collect::<Vec<_>>();
        let output = serde_json::to_string_pretty(&json_arr)
            .map_err(|e| CliError::Serialize(e.to_string()))?;
        Ok(CliOutput::stdout(output))
    } else if let Some(provider_id) = &args.provider {
        let output = transcribe::render_online_provider_detail(provider_id)?;
        Ok(CliOutput::stdout(output))
    } else if args.models {
        Ok(CliOutput::stdout(
            transcribe::render_online_providers_with_models(),
        ))
    } else {
        Ok(CliOutput::stdout(
            transcribe::render_online_providers_table(),
        ))
    }
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
