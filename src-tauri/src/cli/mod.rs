pub mod models;
pub mod serve;
pub mod transcribe;

pub use self::models::*;
pub use self::serve::*;
pub use self::transcribe::*;


use clap::{Parser, Subcommand};
use std::ffi::OsString;
const CLI_COMMANDS: [&str; 8] = [
    "transcribe",
    "models",
    "serve",
    "--help",
    "-h",
    "--version",
    "-V",
    "-v",
];

/// Sona command line interface.
#[derive(Debug, Parser)]
#[command(
    name = "sona",
    version,
    about = "Offline batch transcription for Sona",
    after_help = "Examples:\n  sona transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo --vad-model-id silero-vad\n  sona models list --type whisper --language zh\n  sona models download sherpa-onnx-whisper-turbo"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Transcribes a single local audio or video file.
    Transcribe(TranscribeArgs),
    /// Lists or downloads preset models.
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

    match cli.command {
        Commands::Transcribe(args) => transcribe::run_transcribe(args).await,
        Commands::Models(args) => models::run_models(args).await,
        Commands::Serve(args) => serve::run_serve(args).await,
    }
}

