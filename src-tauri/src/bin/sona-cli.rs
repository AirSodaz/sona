use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tauri_appsona_lib::cli::{
    download_model, list_models, load_config_file, required_companion_models,
    resolve_model_download, resolve_transcribe_options, write_output, OutputTarget,
    RequiredCompanionModels, TranscribeCliOptions,
};
use tauri_appsona_lib::export::export_segments;
use tauri_appsona_lib::sherpa::transcribe_batch_with_progress;

/// Sona command line interface.
#[derive(Debug, Parser)]
#[command(
    name = "sona-cli",
    version,
    about = "Offline batch transcription for Sona",
    after_help = "Examples:\n  sona-cli transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo --vad-model-id silero-vad\n  sona-cli models list --type whisper --language zh\n  sona-cli models download sherpa-onnx-whisper-turbo"
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
}

#[derive(Debug, Args)]
struct ModelsArgs {
    #[command(subcommand)]
    command: ModelCommands,
}

#[derive(Debug, Subcommand)]
enum ModelCommands {
    /// Lists preset models known to the CLI.
    #[command(after_help = "Examples:\n  sona-cli models list\n  sona-cli models list --mode offline --type whisper\n  sona-cli models list --language zh --installed")]
    List(ModelListArgs),
    /// Downloads a preset model into the models directory.
    #[command(after_help = "Examples:\n  sona-cli models download sherpa-onnx-whisper-turbo\n  sona-cli models download silero-vad --models-dir ./models")]
    Download(ModelDownloadArgs),
}

#[derive(Debug, Args)]
#[command(about = "List preset models with optional filters")]
struct ModelListArgs {
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the models directory used to detect installed models")]
    models_dir: Option<PathBuf>,
    /// Filter by supported mode.
    #[arg(long, value_name = "MODE", help = "Filter by mode: streaming or offline")]
    mode: Option<String>,
    /// Filter by model type.
    #[arg(long = "type", value_name = "TYPE", help = "Filter by type, for example whisper, vad, punctuation")]
    model_type: Option<String>,
    /// Filter by language token.
    #[arg(long, value_name = "LANG", help = "Filter by language token, for example zh, en, ja, yue")]
    language: Option<String>,
    /// Show only installed models.
    #[arg(long, help = "Only include models already present in the models directory")]
    installed: bool,
}

#[derive(Debug, Args)]
#[command(
    about = "Download a preset model and any required companion models",
    after_help = "Required companion models are downloaded automatically when the preset needs VAD or punctuation."
)]
struct ModelDownloadArgs {
    /// Preset model id to download.
    #[arg(help = "Preset model id, for example sherpa-onnx-whisper-turbo or silero-vad")]
    model_id: String,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the target models directory")]
    models_dir: Option<PathBuf>,
    /// Suppresses progress logs.
    #[arg(long, help = "Hide per-download progress output")]
    quiet: bool,
}

#[derive(Debug, serde::Serialize)]
struct ModelListEntry {
    id: String,
    name: String,
    #[serde(rename = "type")]
    model_type: String,
    language: String,
    size: String,
    modes: Vec<String>,
    installed: bool,
    install_path: String,
}

#[derive(Debug, Args)]
#[command(
    about = "Transcribe a local audio or video file with offline models",
    after_help = "Examples:\n  sona-cli transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo --vad-model-id silero-vad\n  sona-cli transcribe ./sample.mp4 --config ./sona-cli.toml --output ./sample.srt\n  sona-cli transcribe ./sample.wav --model-id sherpa-onnx-funasr-nano-int8-2025-12-30 --vad-model-id silero-vad --punctuation-model-id sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"
)]
struct TranscribeArgs {
    /// Input audio or video file to transcribe.
    #[arg(help = "Local audio or video file path")]
    input: PathBuf,
    /// Path to a TOML config file.
    #[arg(long, help = "Load default options from a TOML config file")]
    config: Option<PathBuf>,
    /// Output file path. If omitted, JSON is written to stdout.
    #[arg(long, help = "Write output to a file instead of stdout")]
    output: Option<PathBuf>,
    /// Explicit export format.
    #[arg(long, value_name = "FORMAT", help = "Override export format, for example json, srt, txt, vtt")]
    format: Option<String>,
    /// Override the transcription language.
    #[arg(long, value_name = "LANG", help = "Override language detection, for example auto, zh, en, ja")]
    language: Option<String>,
    /// Offline preset model id.
    #[arg(long, value_name = "MODEL_ID", help = "Offline preset model id to use for transcription")]
    model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the models directory used to resolve installed models")]
    models_dir: Option<PathBuf>,
    /// VAD preset model id.
    #[arg(long, value_name = "MODEL_ID", help = "VAD companion model id, usually silero-vad")]
    vad_model_id: Option<String>,
    /// Punctuation preset model id.
    #[arg(long, value_name = "MODEL_ID", help = "Punctuation companion model id when required by the main model")]
    punctuation_model_id: Option<String>,
    /// ITN preset model id. Can be passed multiple times.
    #[arg(long = "itn-model-id", value_name = "MODEL_ID", help = "ITN companion model id. Pass multiple times to chain ITN models")]
    itn_model_ids: Vec<String>,
    /// Number of recognizer threads.
    #[arg(long, value_name = "N", help = "Recognizer thread count")]
    threads: Option<i32>,
    /// Enables ITN.
    #[arg(long, conflicts_with = "disable_itn", help = "Enable inverse text normalization")]
    enable_itn: bool,
    /// Disables ITN.
    #[arg(long, conflicts_with = "enable_itn", help = "Disable inverse text normalization")]
    disable_itn: bool,
    /// VAD buffer size in seconds.
    #[arg(long = "vad-buffer", value_name = "SECONDS", help = "Voice activity buffer size in seconds")]
    vad_buffer: Option<f32>,
    /// Optional path to save the resampled WAV file.
    #[arg(long, help = "Save the intermediate resampled WAV file to this path")]
    save_wav: Option<PathBuf>,
    /// Suppresses progress logs.
    #[arg(long, help = "Hide transcription progress output")]
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
        Commands::Models(args) => run_models(args).await,
    }
}

async fn run_models(args: ModelsArgs) -> Result<(), String> {
    match args.command {
        ModelCommands::List(args) => run_model_list(args),
        ModelCommands::Download(args) => run_model_download(args).await,
    }
}

fn run_model_list(args: ModelListArgs) -> Result<(), String> {
    let models = list_models(args.models_dir)?;
    let language_filter = args.language.as_deref().map(str::to_lowercase);
    let output = serde_json::to_string_pretty(
        &models
            .into_iter()
            .filter(|model| {
                args.mode
                    .as_deref()
                    .map(|mode| model.modes.iter().any(|item| item == mode))
                    .unwrap_or(true)
            })
            .filter(|model| {
                args.model_type
                    .as_deref()
                    .map(|model_type| model.model_type == model_type)
                    .unwrap_or(true)
            })
            .filter(|model| {
                language_filter
                    .as_deref()
                    .map(|language| {
                        model
                            .language
                            .split(',')
                            .any(|item| item.trim().eq_ignore_ascii_case(language))
                    })
                    .unwrap_or(true)
            })
            .filter(|model| !args.installed || model.installed)
            .map(|model| ModelListEntry {
                id: model.id,
                name: model.name,
                model_type: model.model_type,
                language: model.language,
                size: model.size,
                modes: model.modes,
                installed: model.installed,
                install_path: model.install_path.to_string_lossy().to_string(),
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|error| format!("Failed to serialize model list: {error}"))?;
    write_output(&OutputTarget::Stdout, &(output + "\n"))
}

async fn run_model_download(args: ModelDownloadArgs) -> Result<(), String> {
    let quiet = args.quiet;
    let models_dir = args.models_dir;
    let resolved = resolve_model_download(&args.model_id, models_dir.clone())?;
    download_one_model(&resolved.model.id, &resolved, quiet).await?;

    let RequiredCompanionModels {
        vad_model_id,
        punctuation_model_id,
    } = required_companion_models(&resolved.model);

    if let Some(vad_model_id) = vad_model_id {
        let vad_resolved = resolve_model_download(&vad_model_id, models_dir.clone())?;
        download_one_model(&vad_model_id, &vad_resolved, quiet).await?;
    }
    if let Some(punctuation_model_id) = punctuation_model_id {
        let punctuation_resolved = resolve_model_download(&punctuation_model_id, models_dir)?;
        download_one_model(&punctuation_model_id, &punctuation_resolved, quiet).await?;
    }

    Ok(())
}

async fn download_one_model(
    display_model_id: &str,
    resolved: &tauri_appsona_lib::cli::ResolvedModelDownload,
    quiet: bool,
) -> Result<(), String> {
    let install_path = download_model(resolved, |downloaded, total| {
        if quiet || total == 0 {
            return;
        }
        let percentage = ((downloaded as f64 / total as f64) * 100.0).round() as i32;
        eprintln!("Downloading {display_model_id}: {percentage}%");
    })
    .await?;

    eprintln!("Installed {} at {}", resolved.model.id, install_path.display());
    Ok(())
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
