use crate::cli::models::resolve_models_dir;
use crate::cli::{CliError, CliResult};
use crate::core::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, PresetModel, find_preset_model,
};
use crate::integrations::asr::{
    BatchTranscriptionRequest, Recognizer, transcribe_batch_with_progress_and_fallback_notice,
};
use crate::repositories::export::{ExportFormat, export_segments};
use clap::Args;
use futures_util::stream::{self, StreamExt};
use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const DEFAULT_THREADS: i32 = 4;
pub const DEFAULT_LANGUAGE: &str = "auto";
pub const DEFAULT_VAD_BUFFER_SIZE: f32 = 5.0;
pub const DEFAULT_GPU_ACCELERATION: &str = "auto";
pub const GPU_ACCELERATION_VALUES: &[&str] = &["auto", "cpu", "cuda", "coreml", "directml"];
pub const DEFAULT_BATCH_JOBS: usize = 1;
const SUPPORTED_BATCH_MEDIA_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "m4a", "aiff", "flac", "ogg", "wma", "aac", "opus", "amr", "mp4", "webm", "mov",
    "mkv", "avi", "wmv", "flv", "3gp",
];
const GLOB_PATTERN_CHARS: &[char] = &['*', '?', '['];

#[derive(Debug, Args)]
#[command(
    about = "Transcribe local audio or video files with offline models",
    after_help = "Examples:\n  sona transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo\n  sona transcribe ./sample.mp4 --config ./sona.toml --output ./sample.srt\n  sona transcribe --input-dir ./media --output-dir ./transcripts --format srt --recursive\n  sona transcribe ./sample.wav --model-id sherpa-onnx-funasr-nano-int8-2025-12-30 --punctuation-model-id sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"
)]
pub struct TranscribeArgs {
    /// Input audio/video files or glob patterns to transcribe when not using --input-dir.
    #[arg(
        help = "Local audio/video file path or glob pattern",
        required_unless_present = "input_dir",
        conflicts_with = "input_dir",
        num_args = 1..
    )]
    input: Vec<PathBuf>,
    /// Directory containing media files to transcribe.
    #[arg(
        long,
        value_name = "DIR",
        help = "Transcribe supported media files from this directory"
    )]
    input_dir: Option<PathBuf>,
    /// Path to a TOML config file.
    #[arg(
        short = 'c',
        long,
        help = "Load default options from a TOML config file"
    )]
    config: Option<PathBuf>,
    /// Output file path. If omitted, JSON is written to stdout.
    #[arg(
        long,
        conflicts_with = "output_dir",
        help = "Write output to a file instead of stdout"
    )]
    output: Option<PathBuf>,
    /// Output directory for batch directory transcription.
    #[arg(
        long,
        value_name = "DIR",
        conflicts_with = "output",
        help = "Write one transcript per input file into this directory"
    )]
    output_dir: Option<PathBuf>,
    /// Recursively scan the input directory.
    #[arg(
        long,
        requires = "input_dir",
        help = "Scan input directory recursively"
    )]
    recursive: bool,
    /// Maximum number of files to transcribe at once in directory mode.
    #[arg(
        long,
        value_name = "N",
        help = "Maximum concurrent batch file jobs, default 1"
    )]
    jobs: Option<usize>,
    /// Explicit export format.
    #[arg(
        long,
        value_name = "FORMAT",
        help = "Override export format, for example json, srt, txt, vtt, md"
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
        help = "VAD companion model id, default silero-vad when required"
    )]
    vad_model_id: Option<String>,
    /// Punctuation preset model id.
    #[arg(
        long,
        value_name = "MODEL_ID",
        help = "Punctuation companion model id, default CT Transformer when required"
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
    /// Recognizer GPU acceleration provider.
    #[arg(
        long,
        value_name = "PROVIDER",
        help = "Recognizer GPU acceleration provider: auto, cpu, cuda, coreml, or directml"
    )]
    gpu_acceleration: Option<String>,
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
    /// Allows overwriting existing output files.
    #[arg(long, help = "Overwrite existing output files")]
    force: bool,
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
    pub gpu_acceleration: Option<String>,
    pub vad_buffer: Option<f32>,
    pub save_wav: Option<PathBuf>,
    pub quiet: bool,
    pub force: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchOutputPlan {
    input_path: PathBuf,
    output_path: PathBuf,
}

#[derive(Debug, Clone)]
struct BatchTranscribePlan {
    input_path: PathBuf,
    output_path: PathBuf,
    resolved: ResolvedTranscribeOptions,
}

#[derive(Debug, Clone)]
struct BatchInputSource {
    inputs: Vec<PathBuf>,
    base_dir: PathBuf,
    preserve_relative_paths: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchTranscribeSummary {
    processed: usize,
    succeeded: usize,
    failed: usize,
    results: Vec<BatchTranscribeFileSummary>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchTranscribeFileSummary {
    input_path: String,
    output_path: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Resolves CLI arguments, config-file values, and defaults into a concrete request.
pub fn resolve_transcribe_options(
    cli: TranscribeCliOptions,
    config: Option<CliConfigFile>,
) -> Result<ResolvedTranscribeOptions, CliError> {
    resolve_transcribe_options_with_install_checker(cli, config, PresetModel::is_installed_at)
}

fn resolve_transcribe_options_with_install_checker(
    cli: TranscribeCliOptions,
    config: Option<CliConfigFile>,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<ResolvedTranscribeOptions, CliError> {
    let config = config.unwrap_or_default();
    let output_target = resolve_output_target(cli.output.clone());
    let export_format = resolve_export_format(
        cli.format.as_deref().or(config.format.as_deref()),
        match &output_target {
            OutputTarget::Stdout => None,
            OutputTarget::File(path) => Some(path.as_path()),
        },
    )
    .map_err(CliError::Validation)?;
    let gpu_acceleration =
        resolve_cli_gpu_acceleration(cli.gpu_acceleration.or(config.gpu_acceleration))?;

    ensure_input_file_exists(&cli.input)?;
    ensure_output_can_be_written(&output_target, cli.force)?;
    let models_dir = resolve_models_dir(cli.models_dir.or(config.models_dir))?;
    let model_id = cli.model_id.or(config.model_id).ok_or_else(|| {
        CliError::Validation(
            "Missing required offline model. Pass --model-id or set model_id in --config."
                .to_string(),
        )
    })?;
    let model = resolve_offline_model(&model_id)?;
    let rules = model.resolved_rules();

    let vad_model_id = cli.vad_model_id.or(config.vad_model_id);
    let punctuation_model_id = cli.punctuation_model_id.or(config.punctuation_model_id);

    let enable_itn = cli.enable_itn.or(config.enable_itn).unwrap_or(false);
    let threads = cli.threads.or(config.threads).unwrap_or(DEFAULT_THREADS);
    if threads <= 0 {
        return Err(CliError::Validation(
            "threads must be greater than 0".to_string(),
        ));
    }

    let vad_buffer = cli
        .vad_buffer
        .or(config.vad_buffer_size)
        .unwrap_or(DEFAULT_VAD_BUFFER_SIZE);
    if vad_buffer <= 0.0 {
        return Err(CliError::Validation(
            "vad_buffer must be greater than 0".to_string(),
        ));
    }

    let language = cli
        .language
        .or(config.language)
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string());
    let model_path = require_installed_model(model, &models_dir, is_installed)?;
    let vad_model = if rules.requires_vad {
        let companion_id = vad_model_id.unwrap_or_else(|| DEFAULT_SILERO_VAD_MODEL_ID.to_string());
        Some(require_installed_companion(
            &companion_id,
            &models_dir,
            is_installed,
        )?)
    } else {
        optional_installed_companion(vad_model_id.as_deref(), &models_dir, is_installed)?
    };
    let punctuation_model = if rules.requires_punctuation {
        let companion_id =
            punctuation_model_id.unwrap_or_else(|| DEFAULT_PUNCTUATION_MODEL_ID.to_string());
        Some(require_installed_companion(
            &companion_id,
            &models_dir,
            is_installed,
        )?)
    } else {
        optional_installed_companion(punctuation_model_id.as_deref(), &models_dir, is_installed)?
    };

    Ok(ResolvedTranscribeOptions {
        export_format,
        output_target,
        quiet: cli.quiet || config.quiet.unwrap_or(false),
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
            batch_segmentation_mode: crate::integrations::asr::BatchSegmentationMode::Vad,
            model_type: model.model_type.clone(),
            file_config: model.file_config.clone(),
            hotwords: cli.hotwords.or(config.hotwords),
            speaker_processing: None,
            normalization_options:
                crate::integrations::asr::TranscriptNormalizationOptions::default(),
            postprocessor: crate::integrations::asr::TranscriptPostprocessor::compile(
                crate::integrations::asr::TranscriptPostprocessOptions::default(),
            )
            .map_err(|e| CliError::Other(e.to_string()))?,
            gpu_acceleration,
        },
    })
}

pub async fn run_transcribe(args: TranscribeArgs) -> CliResult<()> {
    let config = match args.config.as_deref() {
        Some(path) => Some(load_config_file(path)?),
        None => None,
    };

    if args.input_dir.is_some() || should_run_path_batch(&args.input) {
        return run_batch_transcribe(args, config).await;
    }

    run_single_transcribe(args, config).await
}

async fn run_single_transcribe(
    args: TranscribeArgs,
    config: Option<CliConfigFile>,
) -> CliResult<()> {
    let enable_itn = resolve_enable_itn(&args);

    let resolved = resolve_transcribe_options(
        TranscribeCliOptions {
            input: args
                .input
                .into_iter()
                .next()
                .ok_or_else(|| CliError::Validation("Missing input file path.".to_string()))?,
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
            gpu_acceleration: args.gpu_acceleration,
            vad_buffer: args.vad_buffer,
            save_wav: args.save_wav,
            quiet: args.quiet,
            force: args.force,
        },
        config,
    )?;

    let quiet = resolved.quiet;
    let export_format = resolved.export_format;
    let output_target = resolved.output_target.clone();
    let request = resolved.request.clone();
    let stderr_is_terminal = io::stderr().is_terminal();

    let mut reporter = CliProgressReporter::single(quiet, stderr_is_terminal);
    let segments = transcribe_with_cli_gpu_fallback(&request, None, |event| match event {
        CliTranscribeEvent::Progress(progress) => reporter.write(progress),
        CliTranscribeEvent::DirectMlRetry { error } => {
            reporter.finish_write();
            if !quiet {
                eprintln!("DirectML transcription failed, retrying with CPU: {error}");
            }
        }
    })
    .await;
    reporter.finish_write();
    let segments = segments.map_err(CliError::Other)?;
    let content = export_segments(&segments, export_format).map_err(CliError::Other)?;
    write_output(&output_target, &content)
}

async fn run_batch_transcribe(
    args: TranscribeArgs,
    config: Option<CliConfigFile>,
) -> CliResult<()> {
    if args.save_wav.is_some() {
        return Err(CliError::Validation(
            "Batch directory transcription does not support --save-wav.".to_string(),
        ));
    }

    let output_dir = args.output_dir.clone().ok_or_else(|| {
        CliError::Validation(
            "Batch transcription requires --output-dir so each file has a destination.".to_string(),
        )
    })?;
    let config = config.unwrap_or_default();
    let jobs = resolve_batch_jobs(args.jobs.or(config.jobs))?;
    let quiet = args.quiet || config.quiet.unwrap_or(false);
    let input_source = resolve_batch_input_source(&args)?;
    let export_format =
        resolve_export_format(args.format.as_deref().or(config.format.as_deref()), None)
            .map_err(CliError::Validation)?;
    let inputs = input_source.inputs;

    if inputs.is_empty() {
        return Err(CliError::Validation(format!(
            "No supported media files found in {}.",
            input_source.base_dir.display()
        )));
    }

    let output_plans = plan_batch_output_files(
        &inputs,
        &input_source.base_dir,
        &output_dir,
        export_format,
        input_source.preserve_relative_paths,
        args.force,
    )?;
    let mut plans = Vec::with_capacity(output_plans.len());
    let format = Some(export_format_name(export_format).to_string());
    let enable_itn = resolve_enable_itn(&args);
    let first_plan = output_plans.first().ok_or_else(|| {
        CliError::Validation("No supported media files found for batch transcription.".to_string())
    })?;
    let resolved_template = resolve_transcribe_options(
        TranscribeCliOptions {
            input: first_plan.input_path.clone(),
            output: Some(first_plan.output_path.clone()),
            format,
            language: args.language,
            model_id: args.model_id,
            models_dir: args.models_dir,
            vad_model_id: args.vad_model_id,
            punctuation_model_id: args.punctuation_model_id,
            threads: args.threads,
            enable_itn,
            hotwords: args.hotwords,
            gpu_acceleration: args.gpu_acceleration,
            vad_buffer: args.vad_buffer,
            save_wav: None,
            quiet,
            force: args.force,
        },
        Some(config),
    )?;

    for output_plan in output_plans {
        let resolved = retarget_resolved_transcribe_options(
            &resolved_template,
            output_plan.input_path.clone(),
            output_plan.output_path.clone(),
        );
        plans.push(BatchTranscribePlan {
            input_path: output_plan.input_path,
            output_path: output_plan.output_path,
            resolved,
        });
    }

    run_batch_transcribe_plans(plans, jobs).await
}

fn resolve_enable_itn(args: &TranscribeArgs) -> Option<bool> {
    if args.enable_itn {
        Some(true)
    } else if args.disable_itn {
        Some(false)
    } else {
        None
    }
}

fn retarget_resolved_transcribe_options(
    resolved: &ResolvedTranscribeOptions,
    input_path: PathBuf,
    output_path: PathBuf,
) -> ResolvedTranscribeOptions {
    let mut retargeted = resolved.clone();
    retargeted.output_target = OutputTarget::File(output_path);
    retargeted.request.file_path = input_path.to_string_lossy().to_string();
    retargeted
}

fn should_run_path_batch(inputs: &[PathBuf]) -> bool {
    inputs.len() > 1
        || inputs
            .iter()
            .any(|input| path_contains_glob_pattern(input.as_path()))
}

fn resolve_batch_input_source(args: &TranscribeArgs) -> Result<BatchInputSource, CliError> {
    if let Some(input_dir) = args.input_dir.as_deref() {
        return Ok(BatchInputSource {
            inputs: collect_batch_input_files(input_dir, args.recursive)?,
            base_dir: input_dir.to_path_buf(),
            preserve_relative_paths: args.recursive,
        });
    }

    let inputs = expand_input_patterns(&args.input)?;
    let base_dir = common_input_parent(&inputs)?;
    Ok(BatchInputSource {
        inputs,
        base_dir,
        preserve_relative_paths: false,
    })
}

fn expand_input_patterns(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, CliError> {
    let mut expanded = Vec::new();

    for input in inputs {
        if path_contains_glob_pattern(input) {
            let pattern = input.to_string_lossy().to_string();
            let mut matches = glob::glob(&pattern)
                .map_err(|error| {
                    CliError::Io(format!("Invalid glob pattern {}: {error}", input.display()))
                })?
                .map(|entry| {
                    entry.map_err(|error| {
                        CliError::Io(format!(
                            "Failed to read glob match for {}: {error}",
                            input.display()
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            matches.retain(|path| path.is_file());
            matches.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
            if matches.is_empty() {
                return Err(CliError::Validation(format!(
                    "No input files matched glob pattern: {}",
                    input.display()
                )));
            }
            expanded.extend(matches);
        } else {
            ensure_input_file_exists(input)?;
            expanded.push(input.clone());
        }
    }

    expanded.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    expanded.dedup_by(|left, right| {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    });
    Ok(expanded)
}

fn path_contains_glob_pattern(path: &Path) -> bool {
    path.to_string_lossy()
        .chars()
        .any(|character| GLOB_PATTERN_CHARS.contains(&character))
}

fn common_input_parent(inputs: &[PathBuf]) -> Result<PathBuf, CliError> {
    let first_parent = inputs
        .first()
        .and_then(|path| path.parent())
        .ok_or_else(|| CliError::Validation("Missing input file path.".to_string()))?;
    Ok(first_parent.to_path_buf())
}

async fn run_batch_transcribe_plans(plans: Vec<BatchTranscribePlan>, jobs: usize) -> CliResult<()> {
    let total = plans.len();
    if total == 0 {
        return Ok(());
    }

    // Preload the shared model recognizer using the first plan's options
    let first_plan = &plans[0];
    let request = &first_plan.resolved.request;

    let config_type = crate::integrations::asr::build_model_config(
        Path::new(&request.model_path),
        &request.model_type,
        &request.file_config,
        request.enable_itn,
        &request.language,
        request.hotwords.clone(),
    )
    .map_err(CliError::Model)?;

    let gpu_plan =
        crate::app::hardware::resolve_gpu_acceleration_plan(request.gpu_acceleration.as_deref())
            .await;

    let recognizer_result = crate::integrations::asr::create_recognizer_with_gpu_plan(
        config_type,
        request.num_threads,
        gpu_plan,
    )
    .map_err(CliError::Model)?;

    // Print fallback notice once at the start of the batch if applicable
    if let Some(notice) = recognizer_result.fallback_notice.as_ref() {
        if !first_plan.resolved.quiet {
            eprintln!(
                "DirectML transcription failed, retrying with CPU: {}",
                notice.error
            );
        }
    }

    let shared_recognizer = Arc::new(recognizer_result.recognizer);

    let mut results = if jobs == 1 {
        let mut results = Vec::with_capacity(total);
        for (index, plan) in plans.into_iter().enumerate() {
            results.push((
                index,
                run_batch_transcribe_plan(plan, shared_recognizer.clone(), index, total).await,
            ));
        }
        results
    } else {
        stream::iter(plans.into_iter().enumerate().map({
            let shared_recognizer = shared_recognizer.clone();
            move |(index, plan)| {
                let rec = shared_recognizer.clone();
                async move {
                    (
                        index,
                        run_batch_transcribe_plan(plan, rec, index, total).await,
                    )
                }
            }
        }))
        .buffer_unordered(jobs)
        .collect::<Vec<_>>()
        .await
    };

    results.sort_by_key(|(index, _)| *index);
    let file_results = results
        .into_iter()
        .map(|(_, result)| result)
        .collect::<Vec<_>>();
    let failed = file_results
        .iter()
        .filter(|result| result.error.is_some())
        .count();
    let summary = BatchTranscribeSummary {
        processed: total,
        succeeded: total.saturating_sub(failed),
        failed,
        results: file_results,
    };
    let content = serde_json::to_string_pretty(&summary)
        .map_err(|error| CliError::Other(format!("Failed to serialize batch summary: {error}")))?;
    write_output(&OutputTarget::Stdout, &content)?;

    if failed > 0 {
        Err(CliError::Other(format!(
            "{failed} of {total} batch files failed."
        )))
    } else {
        Ok(())
    }
}

async fn run_batch_transcribe_plan(
    plan: BatchTranscribePlan,
    shared_recognizer: Arc<Recognizer>,
    index: usize,
    total: usize,
) -> BatchTranscribeFileSummary {
    let input_label = plan.input_path.display().to_string();
    let output_label = plan.output_path.display().to_string();
    let result = async {
        ensure_output_parent(&plan.output_path)?;
        let quiet = plan.resolved.quiet;
        let export_format = plan.resolved.export_format;
        let output_target = plan.resolved.output_target.clone();
        let request = plan.resolved.request.clone();
        let stderr_is_terminal = io::stderr().is_terminal();
        let mut reporter = CliProgressReporter::batch(
            quiet,
            stderr_is_terminal,
            index,
            total,
            input_label.clone(),
        );
        let segments = transcribe_with_cli_gpu_fallback(
            &request,
            Some(shared_recognizer),
            |event| match event {
                CliTranscribeEvent::Progress(progress) => reporter.write(progress),
                CliTranscribeEvent::DirectMlRetry { error } => {
                    reporter.finish_write();
                    if !quiet {
                        eprintln!("DirectML transcription failed, retrying with CPU: {error}");
                    }
                }
            },
        )
        .await;
        reporter.finish_write();
        let segments = segments.map_err(CliError::Other)?;
        let content = export_segments(&segments, export_format).map_err(CliError::Other)?;
        write_output(&output_target, &content)
    }
    .await;

    match result {
        Ok(()) => BatchTranscribeFileSummary {
            input_path: input_label,
            output_path: output_label,
            status: "success".to_string(),
            error: None,
        },
        Err(error) => BatchTranscribeFileSummary {
            input_path: input_label,
            output_path: output_label,
            status: "error".to_string(),
            error: Some(error.to_string()),
        },
    }
}

async fn transcribe_with_cli_gpu_fallback<F>(
    request: &BatchTranscriptionRequest,
    preloaded_recognizer: Option<Arc<Recognizer>>,
    on_event: F,
) -> Result<Vec<crate::integrations::asr::TranscriptSegment>, String>
where
    F: FnMut(CliTranscribeEvent<'_>),
{
    let on_event = std::cell::RefCell::new(on_event);
    transcribe_batch_with_progress_and_fallback_notice(
        request,
        preloaded_recognizer,
        |progress| {
            let mut on_event = on_event.borrow_mut();
            (*on_event)(CliTranscribeEvent::Progress(progress));
        },
        |notice| {
            let mut on_event = on_event.borrow_mut();
            (*on_event)(CliTranscribeEvent::DirectMlRetry {
                error: &notice.error,
            });
        },
    )
    .await
}

enum CliTranscribeEvent<'a> {
    Progress(f32),
    DirectMlRetry { error: &'a str },
}

enum CliProgressLabel {
    Single,
    Batch {
        index: usize,
        total: usize,
        input_label: String,
    },
}

struct CliProgressReporter {
    quiet: bool,
    terminal: bool,
    last_reported_progress: Option<i32>,
    wrote_terminal_progress: bool,
    label: CliProgressLabel,
}

impl CliProgressReporter {
    fn single(quiet: bool, terminal: bool) -> Self {
        Self {
            quiet,
            terminal,
            last_reported_progress: None,
            wrote_terminal_progress: false,
            label: CliProgressLabel::Single,
        }
    }

    fn batch(quiet: bool, terminal: bool, index: usize, total: usize, input_label: String) -> Self {
        Self {
            quiet,
            terminal,
            last_reported_progress: None,
            wrote_terminal_progress: false,
            label: CliProgressLabel::Batch {
                index,
                total,
                input_label,
            },
        }
    }

    fn render(&mut self, progress: f32) -> Option<String> {
        if self.quiet {
            return None;
        }

        let rounded = progress.round().clamp(0.0, 100.0) as i32;
        if self.last_reported_progress == Some(rounded) {
            return None;
        }

        if !self.terminal {
            let should_emit = match self.last_reported_progress {
                None => true,
                Some(previous) => rounded == 100 || rounded / 10 > previous / 10,
            };
            if !should_emit {
                self.last_reported_progress = Some(rounded);
                return None;
            }
        }

        self.last_reported_progress = Some(rounded);
        if self.terminal {
            self.wrote_terminal_progress = true;
            Some(format!("\r{}", self.format_line(rounded)))
        } else {
            Some(format!("{}\n", self.format_line(rounded)))
        }
    }

    fn finish(&mut self) -> Option<String> {
        if self.wrote_terminal_progress {
            self.wrote_terminal_progress = false;
            Some("\n".to_string())
        } else {
            None
        }
    }

    fn write(&mut self, progress: f32) {
        if let Some(line) = self.render(progress) {
            eprint!("{line}");
            let _ = io::stderr().flush();
        }
    }

    fn finish_write(&mut self) {
        if let Some(line) = self.finish() {
            eprint!("{line}");
            let _ = io::stderr().flush();
        }
    }

    fn format_line(&self, progress: i32) -> String {
        match &self.label {
            CliProgressLabel::Single => format!("Progress: {progress}%"),
            CliProgressLabel::Batch {
                index,
                total,
                input_label,
            } => format!("[{}/{}] {}: {}%", index + 1, total, input_label, progress),
        }
    }
}

/// Writes CLI output to the selected destination.
pub fn write_output(target: &OutputTarget, content: &str) -> CliResult<()> {
    match target {
        OutputTarget::Stdout => {
            if content.ends_with('\n') {
                print!("{content}");
            } else {
                println!("{content}");
            }
            std::io::stdout()
                .flush()
                .map_err(|error| CliError::Io(format!("Failed to flush stdout: {error}")))?;
            Ok(())
        }
        OutputTarget::File(path) => fs::write(path, content).map_err(|error| {
            CliError::Io(format!(
                "Failed to write output file {}: {error}",
                path.display()
            ))
        }),
    }
}

fn ensure_output_can_be_written(target: &OutputTarget, force: bool) -> CliResult<()> {
    match target {
        OutputTarget::Stdout => Ok(()),
        OutputTarget::File(path) if force || !path.exists() => Ok(()),
        OutputTarget::File(path) => Err(CliError::Io(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            path.display()
        ))),
    }
}

fn ensure_output_parent(path: &Path) -> CliResult<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::Io(format!(
                "Failed to create output directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    Ok(())
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

fn resolve_output_target(output: Option<PathBuf>) -> OutputTarget {
    match output {
        Some(path) => OutputTarget::File(path),
        None => OutputTarget::Stdout,
    }
}

fn ensure_input_file_exists(input: &Path) -> CliResult<()> {
    if input.is_file() {
        Ok(())
    } else {
        Err(CliError::Validation(format!(
            "Input file must be an existing file: {}",
            input.display()
        )))
    }
}

fn export_format_name(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Json => "json",
        ExportFormat::Txt => "txt",
        ExportFormat::Srt => "srt",
        ExportFormat::Vtt => "vtt",
        ExportFormat::Md => "md",
    }
}

fn collect_batch_input_files(input_dir: &Path, recursive: bool) -> Result<Vec<PathBuf>, CliError> {
    if !input_dir.is_dir() {
        return Err(CliError::Validation(format!(
            "--input-dir must be an existing directory: {}",
            input_dir.display()
        )));
    }

    let walker = walkdir::WalkDir::new(input_dir)
        .min_depth(1)
        .max_depth(if recursive { usize::MAX } else { 1 });
    let mut files = Vec::new();

    for entry in walker {
        let entry = entry.map_err(|error| {
            CliError::Io(format!("Failed to read {}: {error}", input_dir.display()))
        })?;
        if entry.file_type().is_file() && is_supported_batch_media_path(entry.path()) {
            files.push(entry.path().to_path_buf());
        }
    }

    files.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    Ok(files)
}

fn is_supported_batch_media_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let normalized = extension.trim_start_matches('.').to_ascii_lowercase();
            SUPPORTED_BATCH_MEDIA_EXTENSIONS.contains(&normalized.as_str())
        })
        .unwrap_or(false)
}

fn plan_batch_output_files(
    inputs: &[PathBuf],
    input_dir: &Path,
    output_dir: &Path,
    format: ExportFormat,
    preserve_relative_paths: bool,
    force: bool,
) -> Result<Vec<BatchOutputPlan>, CliError> {
    let extension = export_format_name(format);
    let mut seen_outputs = HashSet::new();
    let mut plans = Vec::with_capacity(inputs.len());

    for input_path in inputs {
        let relative_output =
            batch_relative_output_path(input_path, input_dir, extension, preserve_relative_paths)
                .map_err(CliError::Validation)?;
        let output_path = output_dir.join(relative_output);
        let output_key = output_path.to_string_lossy().to_ascii_lowercase();
        if !seen_outputs.insert(output_key) {
            return Err(CliError::Io(format!(
                "Batch output path {} would overwrite another result. Use --recursive to preserve directories or remove duplicate input stems.",
                output_path.display()
            )));
        }
        if !force && output_path.exists() {
            return Err(CliError::Io(format!(
                "Output file already exists: {}. Use --force to overwrite.",
                output_path.display()
            )));
        }

        plans.push(BatchOutputPlan {
            input_path: input_path.clone(),
            output_path,
        });
    }

    Ok(plans)
}

fn batch_relative_output_path(
    input_path: &Path,
    input_dir: &Path,
    extension: &str,
    preserve_relative_paths: bool,
) -> Result<PathBuf, String> {
    if preserve_relative_paths {
        let relative = input_path.strip_prefix(input_dir).map_err(|_| {
            format!(
                "Input file {} is not inside --input-dir {}.",
                input_path.display(),
                input_dir.display()
            )
        })?;
        let mut output = relative.to_path_buf();
        output.set_extension(extension);
        return Ok(output);
    }

    let stem = input_path.file_stem().ok_or_else(|| {
        format!(
            "Unable to derive output file name from {}.",
            input_path.display()
        )
    })?;
    let mut output = PathBuf::from(stem);
    output.set_extension(extension);
    Ok(output)
}

fn resolve_batch_jobs(value: Option<usize>) -> Result<usize, CliError> {
    let jobs = value.unwrap_or(DEFAULT_BATCH_JOBS);
    if jobs == 0 {
        Err(CliError::Validation(
            "--jobs must be greater than 0.".to_string(),
        ))
    } else {
        Ok(jobs)
    }
}

pub fn resolve_cli_gpu_acceleration(value: Option<String>) -> Result<Option<String>, CliError> {
    let value = value.unwrap_or_else(|| DEFAULT_GPU_ACCELERATION.to_string());
    let normalized = value.trim().to_ascii_lowercase();

    if GPU_ACCELERATION_VALUES.contains(&normalized.as_str()) {
        Ok(Some(normalized))
    } else {
        Err(CliError::Validation(format!(
            "gpu_acceleration must be one of {}.",
            GPU_ACCELERATION_VALUES.join(", ")
        )))
    }
}

fn resolve_offline_model(model_id: &str) -> Result<&'static PresetModel, CliError> {
    let model = find_preset_model(model_id)
        .ok_or_else(|| CliError::Model(format!("Unknown model id: {model_id}")))?;
    if !model.supports_mode("offline") {
        return Err(CliError::Model(format!(
            "Model '{model_id}' does not support offline transcription."
        )));
    }
    Ok(model)
}

fn require_installed_model(
    model: &PresetModel,
    models_dir: &Path,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<String, CliError> {
    let path = model.resolve_install_path(models_dir);
    if !is_installed(model, models_dir) {
        return Err(CliError::Model(format!(
            "Model '{}' was not found at {}. Pass --models-dir explicitly if your desktop models live elsewhere.",
            model.id,
            path.display()
        )));
    }
    Ok(path.to_string_lossy().to_string())
}

fn require_installed_companion(
    model_id: &str,
    models_dir: &Path,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<String, CliError> {
    let model = find_preset_model(model_id)
        .ok_or_else(|| CliError::Model(format!("Unknown companion model id: {model_id}")))?;
    let path = model.resolve_install_path(models_dir);
    if !is_installed(model, models_dir) {
        return Err(CliError::Model(format!(
            "Companion model '{model_id}' was not found at {}. Pass --models-dir explicitly if your desktop models live elsewhere.",
            path.display()
        )));
    }
    Ok(path.to_string_lossy().to_string())
}

fn optional_installed_companion(
    model_id: Option<&str>,
    models_dir: &Path,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<Option<String>, CliError> {
    model_id
        .map(|id| require_installed_companion(id, models_dir, is_installed))
        .transpose()
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
    pub hotwords: Option<String>,
    pub quiet: Option<bool>,
    pub jobs: Option<usize>,
    pub vad_buffer_size: Option<f32>,
    pub format: Option<String>,
    pub gpu_acceleration: Option<String>,
}

/// Loads a TOML configuration file for the CLI.
pub fn load_config_file(path: &Path) -> Result<CliConfigFile, CliError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        CliError::Io(format!(
            "Failed to read config file {}: {error}",
            path.display()
        ))
    })?;
    toml::from_str(&contents).map_err(|error| {
        CliError::Validation(format!(
            "Failed to parse config file {}: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::export::ExportFormat;
    use std::fs;
    use tempfile::tempdir;

    fn test_model_exists(model: &PresetModel, models_dir: &Path) -> bool {
        model.resolve_install_path(models_dir).exists()
    }

    fn resolve_test_transcribe_options(
        cli: TranscribeCliOptions,
        config: Option<CliConfigFile>,
    ) -> Result<ResolvedTranscribeOptions, CliError> {
        resolve_transcribe_options_with_install_checker(cli, config, test_model_exists)
    }

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
            gpu_acceleration: None,
            vad_buffer: None,
            save_wav: None,
            quiet: false,
            force: false,
        }
    }

    fn installed_whisper_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("sample.wav");
        let models_dir = dir.path().join("models");
        fs::write(&input_path, "").unwrap();
        fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();
        fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();
        (dir, input_path, models_dir)
    }

    fn installed_funasr_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("sample.wav");
        let models_dir = dir.path().join("models");
        fs::write(&input_path, "").unwrap();
        fs::create_dir_all(models_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30")).unwrap();
        fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();
        fs::create_dir_all(
            models_dir.join("sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"),
        )
        .unwrap();
        (dir, input_path, models_dir)
    }

    #[test]
    fn config_file_is_loaded_from_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sona-cli.toml");
        fs::write(
            &path,
            "model_id = \"silero-vad\"\nthreads = 3\nenable_itn = true\nhotwords = \"Sona,ASR\"\nquiet = true\njobs = 2\n",
        )
        .unwrap();

        let config = load_config_file(&path).unwrap();
        assert_eq!(config.model_id.as_deref(), Some("silero-vad"));
        assert_eq!(config.threads, Some(3));
        assert_eq!(config.enable_itn, Some(true));
        assert_eq!(config.hotwords.as_deref(), Some("Sona,ASR"));
        assert_eq!(config.quiet, Some(true));
        assert_eq!(config.jobs, Some(2));
    }

    #[test]
    fn explicit_cli_values_override_config_file_values() {
        let (_dir, input_path, models_dir) = installed_whisper_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir.clone());
        cli.vad_model_id = Some("silero-vad".to_string());
        cli.threads = Some(8);
        cli.enable_itn = Some(true);
        cli.hotwords = Some("cli-term".to_string());

        let resolved = resolve_test_transcribe_options(
            cli,
            Some(CliConfigFile {
                threads: Some(2),
                enable_itn: Some(false),
                hotwords: Some("config-term".to_string()),
                model_id: Some("ignored".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.request.num_threads, 8);
        assert!(resolved.request.enable_itn);
        assert_eq!(resolved.request.hotwords.as_deref(), Some("cli-term"));
        assert_eq!(
            resolved.request.model_path,
            models_dir
                .join("sherpa-onnx-whisper-turbo")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn defaults_gpu_acceleration_to_auto() {
        let (_dir, input_path, models_dir) = installed_whisper_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved = resolve_test_transcribe_options(cli, None).unwrap();

        assert_eq!(resolved.request.gpu_acceleration.as_deref(), Some("auto"));
    }

    #[test]
    fn defaults_required_vad_model_id_when_omitted() {
        let (_dir, input_path, models_dir) = installed_whisper_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir.clone());

        let resolved = resolve_test_transcribe_options(cli, None).unwrap();

        assert_eq!(
            resolved.request.vad_model.as_deref(),
            Some(
                models_dir
                    .join("silero_vad.onnx")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[test]
    fn defaults_required_punctuation_model_id_when_omitted() {
        let (_dir, input_path, models_dir) = installed_funasr_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-funasr-nano-int8-2025-12-30".to_string());
        cli.models_dir = Some(models_dir.clone());

        let resolved = resolve_test_transcribe_options(cli, None).unwrap();

        assert_eq!(
            resolved.request.punctuation_model.as_deref(),
            Some(
                models_dir
                    .join("sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8")
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[test]
    fn explicit_required_companion_config_overrides_default_model_id() {
        let (_dir, input_path, models_dir) = installed_whisper_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);

        let error = resolve_test_transcribe_options(
            cli,
            Some(CliConfigFile {
                vad_model_id: Some("custom-vad".to_string()),
                ..Default::default()
            }),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Unknown companion model id: custom-vad")
        );
        assert!(!error.to_string().contains("silero-vad"));
    }

    #[test]
    fn config_file_gpu_acceleration_is_used() {
        let (dir, input_path, models_dir) = installed_whisper_fixture();
        let config_path = dir.path().join("sona-cli.toml");
        fs::write(&config_path, "gpu_acceleration = \"cpu\"\n").unwrap();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved =
            resolve_test_transcribe_options(cli, Some(load_config_file(&config_path).unwrap()))
                .unwrap();

        assert_eq!(resolved.request.gpu_acceleration.as_deref(), Some("cpu"));
    }

    #[test]
    fn config_file_hotwords_are_used_when_cli_omits_hotwords() {
        let (_dir, input_path, models_dir) = installed_whisper_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved = resolve_test_transcribe_options(
            cli,
            Some(CliConfigFile {
                hotwords: Some("config-hotword".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.request.hotwords.as_deref(), Some("config-hotword"));
    }

    #[test]
    fn explicit_cli_gpu_acceleration_overrides_config_file() {
        let (_dir, input_path, models_dir) = installed_whisper_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());
        cli.gpu_acceleration = Some("cuda".to_string());

        let resolved = resolve_test_transcribe_options(
            cli,
            Some(CliConfigFile {
                gpu_acceleration: Some("cpu".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.request.gpu_acceleration.as_deref(), Some("cuda"));
    }

    #[test]
    fn invalid_gpu_acceleration_fails_before_model_resolution() {
        let mut cli = temp_cli_options();
        cli.model_id = Some("not-a-real-model".to_string());
        cli.gpu_acceleration = Some("vulkan".to_string());

        let error = resolve_test_transcribe_options(cli, None).unwrap_err();

        assert!(error.to_string().contains("gpu_acceleration"));
        assert!(
            error
                .to_string()
                .contains("auto, cpu, cuda, coreml, directml")
        );
        assert!(!error.to_string().contains("Unknown model id"));
    }

    #[test]
    fn infers_export_format_from_output_path() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("sample.wav");
        let models_dir = dir.path().join("models");
        fs::write(&input_path, "").unwrap();
        fs::create_dir_all(models_dir.join("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25")).unwrap();
        fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.output = Some(PathBuf::from("out.srt"));
        cli.model_id = Some("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved = resolve_test_transcribe_options(cli, None).unwrap();
        assert_eq!(resolved.export_format, ExportFormat::Srt);
    }

    #[test]
    fn format_flag_overrides_output_extension() {
        let (_dir, input_path, models_dir) = installed_whisper_fixture();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.output = Some(PathBuf::from("out.txt"));
        cli.format = Some("json".to_string());
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved = resolve_test_transcribe_options(cli, None).unwrap();
        assert_eq!(resolved.export_format, ExportFormat::Json);
    }

    #[test]
    fn batch_input_collection_defaults_to_top_level_supported_media_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("meeting.wav"), "").unwrap();
        fs::write(dir.path().join("clip.MP4"), "").unwrap();
        fs::write(dir.path().join("notes.txt"), "").unwrap();
        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested").join("hidden.wav"), "").unwrap();

        let inputs = collect_batch_input_files(dir.path(), false).unwrap();
        let names = inputs
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["clip.MP4", "meeting.wav"]);
    }

    #[test]
    fn batch_input_collection_can_recurse_supported_media_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("meeting.wav"), "").unwrap();
        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested").join("call.mp3"), "").unwrap();

        let inputs = collect_batch_input_files(dir.path(), true).unwrap();
        let names = inputs
            .iter()
            .map(|path| {
                path.strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["meeting.wav", "nested/call.mp3"]);
    }

    #[test]
    fn batch_output_plans_preserve_relative_paths_when_recursive() {
        let dir = tempdir().unwrap();
        let input_dir = dir.path().join("input");
        let output_dir = dir.path().join("output");
        fs::create_dir_all(input_dir.join("nested")).unwrap();
        let meeting = input_dir.join("meeting.wav");
        let call = input_dir.join("nested").join("call.mp3");
        fs::write(&meeting, "").unwrap();
        fs::write(&call, "").unwrap();

        let plans = plan_batch_output_files(
            &[meeting, call],
            &input_dir,
            &output_dir,
            ExportFormat::Srt,
            true,
            false,
        )
        .unwrap();
        let outputs = plans
            .iter()
            .map(|plan| {
                plan.output_path
                    .strip_prefix(&output_dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();

        assert_eq!(outputs, vec!["meeting.srt", "nested/call.srt"]);
    }

    #[test]
    fn batch_output_plans_reject_duplicate_output_paths() {
        let dir = tempdir().unwrap();
        let input_dir = dir.path().join("input");
        let output_dir = dir.path().join("output");
        fs::create_dir_all(&input_dir).unwrap();
        let wav = input_dir.join("demo.wav");
        let mp4 = input_dir.join("demo.mp4");
        fs::write(&wav, "").unwrap();
        fs::write(&mp4, "").unwrap();

        let error = plan_batch_output_files(
            &[wav, mp4],
            &input_dir,
            &output_dir,
            ExportFormat::Json,
            false,
            false,
        )
        .unwrap_err();

        assert!(error.to_string().contains("would overwrite"));
        assert!(error.to_string().contains("demo.json"));
    }

    #[test]
    fn batch_output_plans_reject_existing_outputs_without_force() {
        let dir = tempdir().unwrap();
        let input_dir = dir.path().join("input");
        let output_dir = dir.path().join("output");
        fs::create_dir_all(&input_dir).unwrap();
        fs::create_dir_all(&output_dir).unwrap();
        let input = input_dir.join("demo.wav");
        let existing = output_dir.join("demo.json");
        fs::write(&input, "").unwrap();
        fs::write(&existing, "old").unwrap();

        let error = plan_batch_output_files(
            &[input],
            &input_dir,
            &output_dir,
            ExportFormat::Json,
            false,
            false,
        )
        .unwrap_err();

        assert!(error.to_string().contains("Output file already exists"));
        assert!(error.to_string().contains("demo.json"));
        assert!(error.to_string().contains("--force"));
    }

    #[test]
    fn batch_output_plans_allow_existing_outputs_with_force() {
        let dir = tempdir().unwrap();
        let input_dir = dir.path().join("input");
        let output_dir = dir.path().join("output");
        fs::create_dir_all(&input_dir).unwrap();
        fs::create_dir_all(&output_dir).unwrap();
        let input = input_dir.join("demo.wav");
        let existing = output_dir.join("demo.json");
        fs::write(&input, "").unwrap();
        fs::write(&existing, "old").unwrap();

        let plans = plan_batch_output_files(
            &[input],
            &input_dir,
            &output_dir,
            ExportFormat::Json,
            false,
            true,
        )
        .unwrap();

        assert_eq!(plans[0].output_path, existing);
    }

    #[test]
    fn glob_input_expansion_errors_when_pattern_matches_nothing() {
        let dir = tempdir().unwrap();
        let pattern = dir.path().join("*.wav");

        let error = expand_input_patterns(&[pattern]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("No input files matched glob pattern")
        );
    }

    #[test]
    fn glob_input_expansion_sorts_matches() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("a.wav");
        let second = dir.path().join("b.wav");
        fs::write(&second, "").unwrap();
        fs::write(&first, "").unwrap();
        let pattern = dir.path().join("*.wav");

        let inputs = expand_input_patterns(&[pattern]).unwrap();

        assert_eq!(inputs, vec![first, second]);
    }

    #[test]
    fn batch_jobs_default_to_one_and_must_be_positive() {
        assert_eq!(resolve_batch_jobs(None).unwrap(), 1);
        assert_eq!(resolve_batch_jobs(Some(2)).unwrap(), 2);
        assert!(
            resolve_batch_jobs(Some(0))
                .unwrap_err()
                .to_string()
                .contains("--jobs")
        );
    }

    #[test]
    fn retarget_resolved_transcribe_options_updates_input_and_output() {
        let (_dir, first_input, models_dir) = installed_whisper_fixture();
        let second_input = first_input.with_file_name("second.wav");
        fs::write(&second_input, "").unwrap();
        let first_output = first_input.with_extension("json");
        let second_output = second_input.with_extension("srt");

        let mut cli = temp_cli_options();
        cli.input = first_input.clone();
        cli.output = Some(first_output.clone());
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);
        cli.vad_model_id = Some("silero-vad".to_string());

        let resolved = resolve_test_transcribe_options(cli, None).unwrap();
        let retargeted = retarget_resolved_transcribe_options(
            &resolved,
            second_input.clone(),
            second_output.clone(),
        );

        assert_eq!(resolved.request.file_path, first_input.to_string_lossy());
        assert_eq!(resolved.output_target, OutputTarget::File(first_output));
        assert_eq!(retargeted.request.file_path, second_input.to_string_lossy());
        assert_eq!(retargeted.output_target, OutputTarget::File(second_output));
        assert_eq!(retargeted.request.model_path, resolved.request.model_path);
    }

    #[test]
    fn missing_default_companion_model_fails_fast() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("sample.wav");
        let models_dir = dir.path().join("models");
        fs::write(&input_path, "").unwrap();
        fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();

        let mut cli = temp_cli_options();
        cli.input = input_path;
        cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
        cli.models_dir = Some(models_dir);

        let error = resolve_test_transcribe_options(cli, None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Companion model 'silero-vad' was not found")
        );
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

    #[test]
    fn transcribe_progress_renderer_refreshes_in_terminal() {
        let mut reporter = CliProgressReporter::single(false, true);

        assert_eq!(reporter.render(12.2), Some("\rProgress: 12%".to_string()));
        assert_eq!(reporter.render(12.4), None);
        assert_eq!(reporter.render(13.0), Some("\rProgress: 13%".to_string()));
        assert_eq!(reporter.finish(), Some("\n".to_string()));
    }

    #[test]
    fn transcribe_progress_renderer_samples_non_terminal_output() {
        let mut reporter = CliProgressReporter::single(false, false);

        assert_eq!(reporter.render(3.0), Some("Progress: 3%\n".to_string()));
        assert_eq!(reporter.render(4.0), None);
        assert_eq!(reporter.render(10.0), Some("Progress: 10%\n".to_string()));
        assert_eq!(reporter.render(10.4), None);
        assert_eq!(reporter.render(21.0), Some("Progress: 21%\n".to_string()));
        assert_eq!(reporter.render(22.0), None);
        assert_eq!(reporter.render(100.0), Some("Progress: 100%\n".to_string()));
        assert_eq!(reporter.finish(), None);
    }

    #[test]
    fn transcribe_progress_renderer_honors_quiet() {
        let mut reporter = CliProgressReporter::single(true, true);

        assert_eq!(reporter.render(50.0), None);
        assert_eq!(reporter.finish(), None);
    }

    #[test]
    fn batch_progress_renderer_includes_file_position() {
        let mut reporter = CliProgressReporter::batch(false, true, 1, 3, "demo.wav".to_string());

        assert_eq!(
            reporter.render(99.6),
            Some("\r[2/3] demo.wav: 100%".to_string())
        );
        assert_eq!(reporter.finish(), Some("\n".to_string()));
    }
}
