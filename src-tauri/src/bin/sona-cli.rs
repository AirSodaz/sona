use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tauri_appsona_lib::cli::{
    load_config_file, resolve_transcribe_options, write_output, TranscribeCliOptions,
};
use tauri_appsona_lib::export::export_segments;
use tauri_appsona_lib::sherpa::transcribe_batch_with_progress;

/// Sona command line interface.
#[derive(Debug, Parser)]
#[command(name = "sona-cli", version, about = "Offline batch transcription for Sona")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Transcribes a single local audio or video file.
    Transcribe(TranscribeArgs),
}

#[derive(Debug, Args)]
struct TranscribeArgs {
    /// Input audio or video file to transcribe.
    input: PathBuf,
    /// Path to a TOML config file.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Output file path. If omitted, JSON is written to stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Explicit export format.
    #[arg(long)]
    format: Option<String>,
    /// Override the transcription language.
    #[arg(long)]
    language: Option<String>,
    /// Offline preset model id.
    #[arg(long)]
    model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long)]
    models_dir: Option<PathBuf>,
    /// VAD preset model id.
    #[arg(long)]
    vad_model_id: Option<String>,
    /// Punctuation preset model id.
    #[arg(long)]
    punctuation_model_id: Option<String>,
    /// ITN preset model id. Can be passed multiple times.
    #[arg(long = "itn-model-id")]
    itn_model_ids: Vec<String>,
    /// Number of recognizer threads.
    #[arg(long)]
    threads: Option<i32>,
    /// Enables ITN.
    #[arg(long, conflicts_with = "disable_itn")]
    enable_itn: bool,
    /// Disables ITN.
    #[arg(long, conflicts_with = "enable_itn")]
    disable_itn: bool,
    /// VAD buffer size in seconds.
    #[arg(long = "vad-buffer")]
    vad_buffer: Option<f32>,
    /// Optional path to save the resampled WAV file.
    #[arg(long)]
    save_wav: Option<PathBuf>,
    /// Suppresses progress logs.
    #[arg(long)]
    quiet: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Transcribe(args) => run_transcribe(args).await,
    }
}

async fn run_transcribe(args: TranscribeArgs) -> Result<(), String> {
    let config = match args.config.as_deref() {
        Some(path) => Some(load_config_file(path)?),
        None => None,
    };

    let enable_itn = if args.enable_itn {
        Some(true)
    } else if args.disable_itn {
        Some(false)
    } else {
        None
    };

    let resolved = resolve_transcribe_options(
        TranscribeCliOptions {
            input: args.input,
            output: args.output,
            format: args.format,
            language: args.language,
            model_id: args.model_id,
            models_dir: args.models_dir,
            vad_model_id: args.vad_model_id,
            punctuation_model_id: args.punctuation_model_id,
            itn_model_ids: args.itn_model_ids,
            threads: args.threads,
            enable_itn,
            vad_buffer: args.vad_buffer,
            save_wav: args.save_wav,
            quiet: args.quiet,
        },
        config,
    )?;

    let quiet = resolved.quiet;
    let export_format = resolved.export_format;
    let output_target = resolved.output_target.clone();
    let request = resolved.request.clone();

    let mut last_reported_progress = -1_i32;
    let segments = transcribe_batch_with_progress(&request, |progress| {
        if quiet {
            return;
        }

        let rounded = progress.round() as i32;
        if rounded != last_reported_progress {
            eprintln!("Progress: {rounded}%");
            last_reported_progress = rounded;
        }
    })
    .await?;
    let content = export_segments(&segments, export_format)?;
    write_output(&output_target, &content)
}
