pub mod models;
pub mod serve;
pub mod transcribe;

pub use self::models::*;
pub use self::serve::*;
pub use self::transcribe::*;

use clap::{ArgAction, Parser, Subcommand};
use std::ffi::OsString;
use std::sync::Once;

const CLI_COMMANDS: [&str; 10] = [
    "transcribe",
    "models",
    "serve",
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
    after_help = "Examples:\n  sona transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo --vad-model-id silero-vad\n  sona models list --type whisper --language zh\n  sona models download sherpa-onnx-whisper-turbo\n  sona models delete sherpa-onnx-whisper-turbo"
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
    /// Transcribes a single local audio or video file.
    Transcribe(Box<TranscribeArgs>),
    /// Lists, downloads, or deletes preset models.
    Models(ModelsArgs),
    /// Starts the HTTP API server in headless mode.
    Serve(ServeArgs),
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

pub async fn run_cli_from_args(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let cli = Cli::parse_from(args);
    if cli.verbose {
        init_cli_logger();
    }

    match cli.command {
        Commands::Transcribe(args) => transcribe::run_transcribe(*args).await,
        Commands::Models(args) => models::run_models(args).await,
        Commands::Serve(args) => serve::run_serve(args).await,
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
}
