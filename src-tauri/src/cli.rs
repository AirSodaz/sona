use crate::export::{export_segments, ExportFormat};
use crate::preset_models::{find_preset_model, preset_models, PresetModel};
use crate::sherpa::{transcribe_batch_with_progress, BatchTranscriptionRequest};
use clap::{Args, Parser, Subcommand};
use futures_util::StreamExt;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

const DEFAULT_THREADS: i32 = 4;
const DEFAULT_LANGUAGE: &str = "auto";
const DEFAULT_VAD_BUFFER_SIZE: f32 = 5.0;
const CLI_COMMANDS: [&str; 7] = [
    "transcribe",
    "models",
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
}

#[derive(Debug, Args)]
struct ModelsArgs {
    #[command(subcommand)]
    command: ModelCommands,
}

#[derive(Debug, Subcommand)]
enum ModelCommands {
    /// Lists preset models known to the CLI.
    #[command(
        after_help = "Examples:\n  sona models list\n  sona models list --mode offline --type whisper\n  sona models list --language zh --installed"
    )]
    List(ModelListArgs),
    /// Downloads a preset model into the models directory.
    #[command(
        after_help = "Examples:\n  sona models download sherpa-onnx-whisper-turbo\n  sona models download silero-vad --models-dir ./models"
    )]
    Download(ModelDownloadArgs),
}

#[derive(Debug, Args)]
#[command(about = "List preset models with optional filters")]
struct ModelListArgs {
    /// Models directory containing installed presets.
    #[arg(
        long,
        help = "Override the models directory used to detect installed models"
    )]
    models_dir: Option<PathBuf>,
    /// Filter by supported mode.
    #[arg(
        long,
        value_name = "MODE",
        help = "Filter by mode: streaming or offline"
    )]
    mode: Option<String>,
    /// Filter by model type.
    #[arg(
        long = "type",
        value_name = "TYPE",
        help = "Filter by type, for example whisper, vad, punctuation"
    )]
    model_type: Option<String>,
    /// Filter by language token.
    #[arg(
        long,
        value_name = "LANG",
        help = "Filter by language token, for example zh, en, ja, yue"
    )]
    language: Option<String>,
    /// Show only installed models.
    #[arg(
        long,
        help = "Only include models already present in the models directory"
    )]
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
    after_help = "Examples:\n  sona transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo --vad-model-id silero-vad\n  sona transcribe ./sample.mp4 --config ./sona.toml --output ./sample.srt\n  sona transcribe ./sample.wav --model-id sherpa-onnx-funasr-nano-int8-2025-12-30 --vad-model-id silero-vad --punctuation-model-id sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"
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
    #[arg(
        long,
        value_name = "FORMAT",
        help = "Override export format, for example json, srt, txt, vtt"
    )]
    format: Option<String>,
    /// Override the transcription language.
    #[arg(
        long,
        value_name = "LANG",
        help = "Override language detection, for example auto, zh, en, ja"
    )]
    language: Option<String>,
    /// Offline preset model id.
    #[arg(
        long,
        value_name = "MODEL_ID",
        help = "Offline preset model id to use for transcription"
    )]
    model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(
        long,
        help = "Override the models directory used to resolve installed models"
    )]
    models_dir: Option<PathBuf>,
    /// VAD preset model id.
    #[arg(
        long,
        value_name = "MODEL_ID",
        help = "VAD companion model id, usually silero-vad"
    )]
    vad_model_id: Option<String>,
    /// Punctuation preset model id.
    #[arg(
        long,
        value_name = "MODEL_ID",
        help = "Punctuation companion model id when required by the main model"
    )]
    punctuation_model_id: Option<String>,
    /// Number of recognizer threads.
    #[arg(long, value_name = "N", help = "Recognizer thread count")]
    threads: Option<i32>,
    /// Enables ITN.
    #[arg(
        long,
        conflicts_with = "disable_itn",
        help = "Enable inverse text normalization"
    )]
    enable_itn: bool,
    /// Custom hotwords for ASR (currently supported by Transducer and Qwen3 models).
    #[arg(long, help = "Custom hotwords, comma separated")]
    hotwords: Option<String>,
    /// Disables ITN.
    #[arg(
        long,
        conflicts_with = "enable_itn",
        help = "Disable inverse text normalization"
    )]
    disable_itn: bool,
    /// VAD buffer size in seconds.
    #[arg(
        long = "vad-buffer",
        value_name = "SECONDS",
        help = "Voice activity buffer size in seconds"
    )]
    vad_buffer: Option<f32>,
    /// Optional path to save the resampled WAV file.
    #[arg(long, help = "Save the intermediate resampled WAV file to this path")]
    save_wav: Option<PathBuf>,
    /// Suppresses progress logs.
    #[arg(long, help = "Hide transcription progress output")]
    quiet: bool,
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
        Commands::Transcribe(args) => run_transcribe(args).await,
        Commands::Models(args) => run_models(args).await,
    }
}

/// File-backed CLI configuration loaded from TOML.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CliConfigFile {
    pub models_dir: Option<PathBuf>,
    pub model_id: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
    pub language: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub vad_buffer_size: Option<f32>,
    pub format: Option<String>,
}

/// CLI options after clap parsing but before config/default resolution.
#[derive(Debug, Clone)]
pub struct TranscribeCliOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub format: Option<String>,
    pub language: Option<String>,
    pub model_id: Option<String>,
    pub models_dir: Option<PathBuf>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub hotwords: Option<String>,
    pub vad_buffer: Option<f32>,
    pub save_wav: Option<PathBuf>,
    pub quiet: bool,
}

/// Output target resolved from CLI arguments and config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputTarget {
    Stdout,
    File(PathBuf),
}

/// Fully resolved transcription settings ready for batch execution.
#[derive(Debug, Clone)]
pub struct ResolvedTranscribeOptions {
    pub export_format: ExportFormat,
    pub output_target: OutputTarget,
    pub quiet: bool,
    pub request: BatchTranscriptionRequest,
}

/// Summary information for one preset model exposed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliModelSummary {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub language: String,
    pub size: String,
    pub modes: Vec<String>,
    pub installed: bool,
    pub install_path: PathBuf,
}

/// Download settings resolved from CLI arguments and defaults.
#[derive(Debug, Clone)]
pub struct ResolvedModelDownload {
    pub model: PresetModel,
    pub models_dir: PathBuf,
    pub download_path: PathBuf,
    pub install_path: PathBuf,
}

/// Companion model ids suggested after downloading a preset.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequiredCompanionModels {
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

/// Loads a TOML configuration file for the CLI.
pub fn load_config_file(path: &Path) -> Result<CliConfigFile, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read config file {}: {error}", path.display()))?;
    toml::from_str(&contents)
        .map_err(|error| format!("Failed to parse config file {}: {error}", path.display()))
}

/// Resolves CLI arguments, config-file values, and defaults into a concrete request.
pub fn resolve_transcribe_options(
    cli: TranscribeCliOptions,
    config: Option<CliConfigFile>,
) -> Result<ResolvedTranscribeOptions, String> {
    let config = config.unwrap_or_default();
    let output_target = resolve_output_target(cli.output.clone());
    let export_format = resolve_export_format(
        cli.format.as_deref().or(config.format.as_deref()),
        match &output_target {
            OutputTarget::Stdout => None,
            OutputTarget::File(path) => Some(path.as_path()),
        },
    )?;

    let models_dir = resolve_models_dir(cli.models_dir.or(config.models_dir))?;
    let model_id = cli.model_id.or(config.model_id).ok_or_else(|| {
        "Missing required offline model. Pass --model-id or set model_id in --config.".to_string()
    })?;
    let model = resolve_offline_model(&model_id)?;
    let rules = model.resolved_rules();

    let vad_model_id = cli.vad_model_id.or(config.vad_model_id);
    let punctuation_model_id = cli.punctuation_model_id.or(config.punctuation_model_id);

    let enable_itn = cli.enable_itn.or(config.enable_itn).unwrap_or(false);
    let threads = cli.threads.or(config.threads).unwrap_or(DEFAULT_THREADS);
    if threads <= 0 {
        return Err("threads must be greater than 0".to_string());
    }

    let vad_buffer = cli
        .vad_buffer
        .or(config.vad_buffer_size)
        .unwrap_or(DEFAULT_VAD_BUFFER_SIZE);
    if vad_buffer <= 0.0 {
        return Err("vad_buffer must be greater than 0".to_string());
    }

    let language = cli
        .language
        .or(config.language)
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());
    let model_path = require_installed_model(model, &models_dir)?;
    let vad_model = if rules.requires_vad {
        let companion_id = vad_model_id.ok_or_else(|| {
            format!(
                "Model '{model_id}' requires a VAD model. Pass --vad-model-id or set vad_model_id in --config."
            )
        })?;
        Some(require_installed_companion(&companion_id, &models_dir)?)
    } else {
        optional_installed_companion(vad_model_id.as_deref(), &models_dir)?
    };
    let punctuation_model = if rules.requires_punctuation {
        let companion_id = punctuation_model_id.ok_or_else(|| {
            format!(
                "Model '{model_id}' requires a punctuation model. Pass --punctuation-model-id or set punctuation_model_id in --config."
            )
        })?;
        Some(require_installed_companion(&companion_id, &models_dir)?)
    } else {
        optional_installed_companion(punctuation_model_id.as_deref(), &models_dir)?
    };

    Ok(ResolvedTranscribeOptions {
        export_format,
        output_target,
        quiet: cli.quiet,
        request: BatchTranscriptionRequest {
            file_path: cli.input.to_string_lossy().to_string(),
            save_to_path: cli.save_wav.map(|path| path.to_string_lossy().to_string()),
            model_path,
            num_threads: threads,
            enable_itn,
            language,
            punctuation_model,
            vad_model,
            vad_buffer,
            model_type: model.model_type.clone(),
            file_config: model.file_config.clone(),
            hotwords: cli.hotwords,
            speaker_processing: None,
        },
    })
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
    resolved: &ResolvedModelDownload,
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

    eprintln!(
        "Installed {} at {}",
        resolved.model.id,
        install_path.display()
    );
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
            threads: args.threads,
            enable_itn,
            hotwords: args.hotwords,
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

/// Writes CLI output to the selected destination.
pub fn write_output(target: &OutputTarget, content: &str) -> Result<(), String> {
    match target {
        OutputTarget::Stdout => {
            print!("{content}");
            std::io::stdout()
                .flush()
                .map_err(|error| format!("Failed to flush stdout: {error}"))?;
            Ok(())
        }
        OutputTarget::File(path) => fs::write(path, content)
            .map_err(|error| format!("Failed to write output file {}: {error}", path.display())),
    }
}

/// Returns preset models known to the CLI with installation status.
pub fn list_models(models_dir: Option<PathBuf>) -> Result<Vec<CliModelSummary>, String> {
    let models_dir = resolve_models_dir(models_dir)?;
    Ok(preset_models()
        .iter()
        .map(|model| {
            let install_path = model.resolve_install_path(&models_dir);
            CliModelSummary {
                id: model.id.clone(),
                name: model.name.clone(),
                model_type: model.model_type.clone(),
                language: model.language.clone(),
                size: model.size.clone(),
                modes: model.modes.clone().unwrap_or_default(),
                installed: install_path.exists(),
                install_path,
            }
        })
        .collect())
}

/// Resolves model download settings from CLI arguments and defaults.
pub fn resolve_model_download(
    model_id: &str,
    models_dir: Option<PathBuf>,
) -> Result<ResolvedModelDownload, String> {
    let models_dir = resolve_models_dir(models_dir)?;
    let model = find_preset_model(model_id)
        .ok_or_else(|| format!("Unknown model id: {model_id}"))?
        .clone();
    let download_path = model.resolve_download_path(&models_dir);
    let install_path = model.resolve_install_path(&models_dir);

    Ok(ResolvedModelDownload {
        model,
        models_dir,
        download_path,
        install_path,
    })
}

/// Downloads a preset model into the local models directory.
pub async fn download_model<F>(
    resolved: &ResolvedModelDownload,
    mut on_progress: F,
) -> Result<PathBuf, String>
where
    F: FnMut(u64, u64),
{
    if resolved.install_path.exists() {
        return Ok(resolved.install_path.clone());
    }

    tokio::fs::create_dir_all(&resolved.models_dir)
        .await
        .map_err(|error| {
            format!(
                "Failed to create models directory {}: {error}",
                resolved.models_dir.display()
            )
        })?;

    let client = reqwest::Client::builder()
        .user_agent("Sona/1.0")
        .build()
        .map_err(|error| format!("Failed to create HTTP client: {error}"))?;
    let response = client
        .get(&resolved.model.url)
        .send()
        .await
        .map_err(|error| format!("Failed to download model: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status: {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let file = tokio::fs::File::create(&resolved.download_path)
        .await
        .map_err(|error| {
            format!(
                "Failed to create download file {}: {error}",
                resolved.download_path.display()
            )
        })?;
    let mut writer = tokio::io::BufWriter::new(file);
    let mut stream = response
        .bytes_stream()
        .map(|item| item.map_err(|error| error.to_string()));

    process_download_stream(&mut stream, &mut writer, total_size, |downloaded, total| {
        on_progress(downloaded, total);
    })
    .await?;
    writer
        .flush()
        .await
        .map_err(|error| format!("Failed to flush download file: {error}"))?;

    if resolved.model.is_archive() {
        extract_tar_bz2_archive(&resolved.download_path, &resolved.models_dir).await?;
        tokio::fs::remove_file(&resolved.download_path)
            .await
            .map_err(|error| {
                format!(
                    "Failed to remove archive {}: {error}",
                    resolved.download_path.display()
                )
            })?;
    }

    Ok(resolved.install_path.clone())
}

/// Returns suggested companion model ids for the given preset model.
pub fn required_companion_models(model: &PresetModel) -> RequiredCompanionModels {
    let rules = model.resolved_rules();
    RequiredCompanionModels {
        vad_model_id: rules.requires_vad.then(|| "silero-vad".to_string()),
        punctuation_model_id: rules.requires_punctuation.then(|| {
            "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8".to_string()
        }),
    }
}

fn resolve_export_format(
    format: Option<&str>,
    output: Option<&Path>,
) -> Result<ExportFormat, String> {
    if let Some(value) = format {
        return ExportFormat::parse(value);
    }

    match output {
        Some(path) => ExportFormat::from_output_path(path),
        None => Ok(ExportFormat::Json),
    }
}

async fn process_download_stream<S, W, F>(
    stream: &mut S,
    writer: &mut W,
    total_size: u64,
    mut on_progress: F,
) -> Result<(), String>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, String>> + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
    F: FnMut(u64, u64),
{
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        writer
            .write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
        downloaded += chunk.len() as u64;

        if total_size > 0 && (downloaded == total_size || last_emit.elapsed().as_millis() >= 100) {
            on_progress(downloaded, total_size);
            last_emit = std::time::Instant::now();
        }
    }

    Ok(())
}

async fn extract_tar_bz2_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let archive_path = archive_path.to_path_buf();
    let target_dir = target_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path).map_err(|error| {
            format!("Failed to open archive {}: {error}", archive_path.display())
        })?;
        let buffered = std::io::BufReader::new(file);
        let tar = bzip2::read::BzDecoder::new(buffered);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&target_dir).map_err(|error| {
            format!(
                "Failed to extract archive into {}: {error}",
                target_dir.display()
            )
        })
    })
    .await
    .map_err(|error| format!("Failed to join extraction task: {error}"))?
}

fn resolve_output_target(output: Option<PathBuf>) -> OutputTarget {
    match output {
        Some(path) => OutputTarget::File(path),
        None => OutputTarget::Stdout,
    }
}

fn resolve_models_dir(configured: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = configured {
        return Ok(path);
    }

    default_models_dir().ok_or_else(|| {
        "Unable to infer the desktop models directory. Pass --models-dir explicitly.".to_string()
    })
}

fn resolve_offline_model(model_id: &str) -> Result<&'static PresetModel, String> {
    let model =
        find_preset_model(model_id).ok_or_else(|| format!("Unknown model id: {model_id}"))?;
    if !model.supports_mode("offline") {
        return Err(format!(
            "Model '{model_id}' does not support offline transcription."
        ));
    }
    Ok(model)
}

fn require_installed_model(model: &PresetModel, models_dir: &Path) -> Result<String, String> {
    let path = model.resolve_install_path(models_dir);
    if !path.exists() {
        return Err(format!(
            "Model '{}' was not found at {}. Pass --models-dir explicitly if your desktop models live elsewhere.",
            model.id,
            path.display()
        ));
    }
    Ok(path.to_string_lossy().to_string())
}

fn require_installed_companion(model_id: &str, models_dir: &Path) -> Result<String, String> {
    let model = find_preset_model(model_id)
        .ok_or_else(|| format!("Unknown companion model id: {model_id}"))?;
    let path = model.resolve_install_path(models_dir);
    if !path.exists() {
        return Err(format!(
            "Companion model '{model_id}' was not found at {}. Pass --models-dir explicitly if your desktop models live elsewhere.",
            path.display()
        ));
    }
    Ok(path.to_string_lossy().to_string())
}

fn optional_installed_companion(
    model_id: Option<&str>,
    models_dir: &Path,
) -> Result<Option<String>, String> {
    model_id
        .map(|id| require_installed_companion(id, models_dir))
        .transpose()
}

fn default_models_dir() -> Option<PathBuf> {
    default_models_dir_candidates()
        .into_iter()
        .map(|path| path.join("models"))
        .find(|path| path.exists())
        .or_else(|| {
            default_models_dir_candidates()
                .into_iter()
                .next()
                .map(|path| path.join("models"))
        })
}

fn default_models_dir_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
            return Vec::new();
        };
        return vec![base.join("com.asoda.sona"), base.join("Sona")];
    }

    #[cfg(target_os = "macos")]
    {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        let base = home.join("Library").join("Application Support");
        return vec![base.join("com.asoda.sona"), base.join("Sona")];
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            return vec![data_home.join("com.asoda.sona"), data_home.join("Sona")];
        }
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        let base = home.join(".local").join("share");
        return vec![base.join("com.asoda.sona"), base.join("Sona")];
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn temp_cli_options() -> TranscribeCliOptions {
        TranscribeCliOptions {
            input: PathBuf::from("sample.wav"),
            output: None,
            format: None,
            language: None,
            model_id: None,
            models_dir: None,
            vad_model_id: None,
            punctuation_model_id: None,
            threads: None,
            enable_itn: None,
            hotwords: None,
            vad_buffer: None,
            save_wav: None,
            quiet: false,
        }
    }

    #[test]
    fn config_file_is_loaded_from_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sona-cli.toml");
        fs::write(
            &path,
            "model_id = \"silero-vad\"\nthreads = 3\nenable_itn = true\n",
        )
        .unwrap();

        let config = load_config_file(&path).unwrap();
        assert_eq!(config.model_id.as_deref(), Some("silero-vad"));
        assert_eq!(config.threads, Some(3));
        assert_eq!(config.enable_itn, Some(true));
    }

    #[test]
    fn explicit_cli_values_override_config_file_values() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();
        fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

        let mut cli = temp_cli_options();
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir.clone());
        cli.vad_model_id = Some("silero-vad".to_string());
        cli.threads = Some(8);
        cli.enable_itn = Some(true);

        let resolved = resolve_transcribe_options(
            cli,
            Some(CliConfigFile {
                threads: Some(2),
                enable_itn: Some(false),
                model_id: Some("ignored".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.request.num_threads, 8);
        assert!(resolved.request.enable_itn);
        assert_eq!(
            resolved.request.model_path,
            models_dir
                .join("sherpa-onnx-whisper-turbo")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn infers_export_format_from_output_path() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        fs::create_dir_all(models_dir.join("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25")).unwrap();
        fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

        let mut cli = temp_cli_options();
        cli.output = Some(PathBuf::from("out.srt"));
        cli.model_id = Some("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved = resolve_transcribe_options(cli, None).unwrap();
        assert_eq!(resolved.export_format, ExportFormat::Srt);
    }

    #[test]
    fn format_flag_overrides_output_extension() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();
        fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

        let mut cli = temp_cli_options();
        cli.output = Some(PathBuf::from("out.txt"));
        cli.format = Some("json".to_string());
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved = resolve_transcribe_options(cli, None).unwrap();
        assert_eq!(resolved.export_format, ExportFormat::Json);
    }

    #[test]
    fn missing_required_companion_model_fails_fast() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();

        let mut cli = temp_cli_options();
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);

        let error = resolve_transcribe_options(cli, None).unwrap_err();
        assert!(error.contains("requires a VAD model"));
    }

    #[test]
    fn writes_output_to_file() {
        let dir = tempdir().unwrap();
        let output_path = dir.path().join("result.json");
        write_output(&OutputTarget::File(output_path.clone()), "{\"ok\":true}").unwrap();
        assert_eq!(fs::read_to_string(output_path).unwrap(), "{\"ok\":true}");
    }

    #[test]
    fn defaults_to_stdout_json_output() {
        let target = resolve_output_target(None);
        assert_eq!(target, OutputTarget::Stdout);
        assert_eq!(
            resolve_export_format(None, None).unwrap(),
            ExportFormat::Json
        );
    }
}
