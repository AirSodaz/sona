pub mod config;
pub mod init_config;
pub mod models;
pub mod serve;
pub mod transcribe;

pub use self::init_config::InitConfigArgs;
pub use self::models::{ModelsArgs, resolve_models_dir};
pub use self::serve::ServeArgs;
pub use self::transcribe::{
    DEFAULT_GPU_ACCELERATION, TranscribeArgs, TranscribeCliOptions, resolve_cli_gpu_acceleration,
    resolve_transcribe_options,
};

use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use std::ffi::OsString;
use std::sync::Once;
use thiserror::Error;

const CLI_COMMANDS: [&str; 12] = [
    "transcribe",
    "models",
    "serve",
    "init-config",
    "completions",
    "help",
    "--help",
    "-h",
    "--version",
    "-V",
    "--verbose",
    "-v",
];
static CLI_LOGGER: CliLogger = CliLogger;
static INIT_CLI_LOGGER: Once = Once::new();

/// Sona command line interface.
#[derive(Debug, Parser)]
#[command(
    name = "sona",
    version,
    about = "Offline batch transcription for Sona",
    after_help = "Examples:\n  sona transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo\n  sona models list --type whisper --language zh\n  sona models download sherpa-onnx-whisper-turbo\n  sona models delete sherpa-onnx-whisper-turbo"
)]
struct Cli {
    #[arg(
        short = 'v',
        long = "verbose",
        global = true,
        action = ArgAction::SetTrue,
        help = "Enable detailed diagnostic logs on stderr"
    )]
    verbose: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
}

impl From<CompletionShell> for Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Zsh => Shell::Zsh,
            CompletionShell::Fish => Shell::Fish,
            CompletionShell::Powershell => Shell::PowerShell,
            CompletionShell::Elvish => Shell::Elvish,
        }
    }
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Model(String),
    #[error("{0}")]
    Other(String),
    #[error("{0}")]
    PartialFailure(String),
    #[error("Cancelled: {0}")]
    Cancelled(String),
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Other(_) => 1,
            CliError::Validation(_) => 2,
            CliError::Model(_) => 3,
            CliError::Network(_) => 4,
            CliError::Io(_) => 5,
            CliError::PartialFailure(_) => 6,
            CliError::Cancelled(_) => 130,
        }
    }
}

pub type CliResult<T> = Result<T, CliError>;

struct CliLogger;

impl log::Log for CliLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Transcribes local audio or video files.
    Transcribe(Box<TranscribeArgs>),
    /// Lists, downloads, or deletes preset models.
    Models(ModelsArgs),
    /// Starts the HTTP API server in headless mode.
    Serve(ServeArgs),
    /// Creates a commented TOML starter template.
    InitConfig(InitConfigArgs),
    /// Generates shell completion scripts.
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: CompletionShell,
    },
}

pub fn should_run_cli(args: &[OsString]) -> bool {
    if std::env::var_os("SONA_FORCE_CLI").is_some() {
        return true;
    }

    args.first()
        .and_then(|value| value.to_str())
        .map(|value| CLI_COMMANDS.contains(&value))
        .unwrap_or(false)
}

pub async fn run_cli_from_args(args: impl IntoIterator<Item = OsString>) -> CliResult<()> {
    #[cfg(target_os = "windows")]
    crate::init_dll_directory();

    let cli = Cli::parse_from(args);
    if cli.verbose {
        init_cli_logger();
    }

    match cli.command {
        Commands::Transcribe(args) => transcribe::run_transcribe(*args).await,
        Commands::Models(args) => models::run_models(args).await,
        Commands::Serve(args) => serve::run_serve(args).await,
        Commands::InitConfig(args) => init_config::run_init_config(args),
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            generate(
                Shell::from(shell),
                &mut command,
                "sona",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    }
}

fn init_cli_logger() {
    INIT_CLI_LOGGER.call_once(|| {
        if log::set_logger(&CLI_LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Debug);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use clap::error::ErrorKind;
    use std::ffi::OsString;

    #[test]
    fn should_run_cli_recognizes_verbose_long_flag() {
        assert!(should_run_cli(&[
            OsString::from("--verbose"),
            OsString::from("models"),
            OsString::from("list"),
        ]));
    }

    #[test]
    fn short_verbose_does_not_request_version_output() {
        let error = Cli::try_parse_from(["sona", "-v"]).unwrap_err();
        assert_ne!(error.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn short_version_requests_version_output() {
        let error = Cli::try_parse_from(["sona", "-V"]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn verbose_can_precede_models_subcommand() {
        let cli = Cli::try_parse_from(["sona", "-v", "models", "list"]).unwrap();

        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::Models(_)));
    }

    #[test]
    fn test_cli_error_exit_code_mapping() {
        assert_eq!(CliError::Other("test".to_string()).exit_code(), 1);
        assert_eq!(CliError::Validation("test".to_string()).exit_code(), 2);
        assert_eq!(CliError::Model("test".to_string()).exit_code(), 3);
        assert_eq!(CliError::Network("test".to_string()).exit_code(), 4);
        assert_eq!(CliError::Io("test".to_string()).exit_code(), 5);
        assert_eq!(CliError::PartialFailure("test".to_string()).exit_code(), 6);
        assert_eq!(CliError::Cancelled("test".to_string()).exit_code(), 130);
    }
}
