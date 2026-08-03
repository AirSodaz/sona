use clap::Args;
use std::path::PathBuf;

use crate::{CliError, CliOutput, CliResult};
use sona_core::ports::asr::{AsrMode, BatchTranscriberPort};
use sona_core::runtime::config::TranscribeConfigSection;
use sona_core::transcription::runtime::{
    BatchTranscribeOptions, OutputTarget, resolve_export_format, resolve_output_target,
};
use sona_core::transcription::transcript::TranscriptSegment;

#[derive(Debug, Args)]
#[command(
    about = "Transcribe audio with local or online ASR; local ASR also accepts video",
    after_help = "Examples:\n  sona-cli transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo\n  sona-cli transcribe ./sample.wav --online-provider groq-whisper --output ./out.srt\n  sona-cli transcribe ./sample.wav --online-provider volcengine-doubao --api-key-env MY_ASR_KEY"
)]
pub struct TranscribeArgs {
    /// Input audio file, or a video file when using local ASR.
    #[arg(value_name = "INPUT")]
    input: PathBuf,
    /// Output transcript file. Defaults to stdout when omitted.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Export format: json, txt, srt, vtt, or md.
    #[arg(short, long)]
    format: Option<String>,
    /// Optional config file, usually sona-cli.toml.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// Override the language setting.
    #[arg(long)]
    language: Option<String>,
    /// Preset model id to use.
    #[arg(long = "model-id")]
    model_id: Option<String>,
    #[command(flatten)]
    online: crate::online_asr::OnlineAsrArgs,
    /// Models directory containing installed presets.
    #[arg(long = "models-dir")]
    models_dir: Option<PathBuf>,
    /// VAD model id override.
    #[arg(long = "vad-model-id")]
    vad_model_id: Option<String>,
    /// Punctuation model id override.
    #[arg(long = "punctuation-model-id")]
    punctuation_model_id: Option<String>,
    /// Number of threads to use.
    #[arg(long)]
    threads: Option<i32>,
    /// Enable ITN.
    #[arg(long, default_value_t = false)]
    enable_itn: bool,
    /// Optional hotwords string.
    #[arg(long)]
    hotwords: Option<String>,
    /// GPU acceleration mode.
    #[arg(long = "gpu-acceleration")]
    gpu_acceleration: Option<String>,
    /// VAD buffer size in seconds.
    #[arg(long = "vad-buffer")]
    vad_buffer: Option<f32>,
    /// Save the resampled WAV to a file.
    #[arg(long = "save-wav")]
    save_wav: Option<PathBuf>,
    /// Suppress progress output.
    #[arg(long, default_value_t = false)]
    quiet: bool,
    /// Overwrite existing output files.
    #[arg(long, default_value_t = false)]
    force: bool,
}

pub fn run_transcribe(args: TranscribeArgs) -> CliResult<CliOutput> {
    let config = load_config(args.config.as_ref())?;
    if args.online.is_online() {
        return run_online_transcribe(&args, config.as_ref());
    }
    let options = BatchTranscribeOptions {
        input: args.input,
        output: args.output,
        format: args.format,
        language: args.language,
        model_id: args.model_id,
        models_dir: args.models_dir,
        default_models_dir: crate::desktop_paths::default_models_dir(),
        vad_model_id: args.vad_model_id,
        punctuation_model_id: args.punctuation_model_id,
        threads: args.threads,
        enable_itn: if args.enable_itn { Some(true) } else { None },
        hotwords: args.hotwords,
        gpu_acceleration: args.gpu_acceleration,
        vad_buffer: args.vad_buffer,
        save_wav: args.save_wav,
        quiet: args.quiet,
        force: args.force,
    };

    let plan =
        sona_runtime_fs::resolve_batch_transcribe_plan_with_runtime_paths_and_models_dir_status(
            options,
            config,
            crate::desktop_paths::models_dir_status,
        )
        .map_err(crate::map_runtime_fs_error)?;
    let export_format = plan.export_format;
    let output_target = plan.output_target.clone();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Io(format!("Failed to create async runtime: {error}")))?;
    let transcriber = crate::asr_adapter::local_batch_transcriber();
    let segments = runtime
        .block_on(transcriber.transcribe(plan))
        .map_err(|error| CliError::Other(error.to_string()))?;

    render_transcription(segments, export_format, output_target)
}

fn run_online_transcribe(
    args: &TranscribeArgs,
    config: Option<&TranscribeConfigSection>,
) -> CliResult<CliOutput> {
    reject_online_local_options(args)?;
    validate_online_paths(&args.input, args.output.as_ref(), args.force)?;
    let language = args
        .language
        .clone()
        .or_else(|| config.and_then(|config| config.language.clone()))
        .unwrap_or_else(|| sona_core::transcription::runtime::DEFAULT_LANGUAGE.to_string());
    let enable_itn =
        args.enable_itn || config.and_then(|config| config.enable_itn).unwrap_or(false);
    let hotwords = args
        .hotwords
        .clone()
        .or_else(|| config.and_then(|config| config.hotwords.clone()));
    let request = args
        .online
        .build_request(AsrMode::Batch, language, enable_itn, hotwords)?;
    let export_format = resolve_export_format(
        args.format
            .as_deref()
            .or_else(|| config.and_then(|config| config.format.as_deref())),
        args.output.as_deref(),
    )
    .map_err(|error| CliError::Validation(error.to_string()))?;
    let output_target = resolve_output_target(args.output.clone());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Io(format!("Failed to create async runtime: {error}")))?;
    let segments = runtime
        .block_on(crate::asr_adapter::online_batch_transcribe(
            args.input.clone(),
            request,
        ))
        .map_err(crate::online_asr::map_asr_error)?;
    render_transcription(segments, export_format, output_target)
}

fn reject_online_local_options(args: &TranscribeArgs) -> CliResult<()> {
    let local_option = [
        (args.model_id.is_some(), "--model-id"),
        (args.models_dir.is_some(), "--models-dir"),
        (args.vad_model_id.is_some(), "--vad-model-id"),
        (
            args.punctuation_model_id.is_some(),
            "--punctuation-model-id",
        ),
        (args.threads.is_some(), "--threads"),
        (args.gpu_acceleration.is_some(), "--gpu-acceleration"),
        (args.vad_buffer.is_some(), "--vad-buffer"),
        (args.save_wav.is_some(), "--save-wav"),
    ]
    .into_iter()
    .find_map(|(present, option)| present.then_some(option));
    if let Some(option) = local_option {
        return Err(CliError::Validation(format!(
            "{option} can only be used with local ASR."
        )));
    }
    Ok(())
}

fn validate_online_paths(input: &PathBuf, output: Option<&PathBuf>, force: bool) -> CliResult<()> {
    match std::fs::metadata(input) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) | Err(_) => {
            return Err(CliError::Validation(format!(
                "Input file must be an existing file: {}",
                input.display()
            )));
        }
    }
    if let Some(output) = output
        && output.exists()
        && !force
    {
        return Err(CliError::Io(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            output.display()
        )));
    }
    Ok(())
}

fn render_transcription(
    segments: Vec<TranscriptSegment>,
    export_format: sona_core::export::ExportFormat,
    output_target: OutputTarget,
) -> CliResult<CliOutput> {
    let output = sona_core::export::export_segments_with_mode(
        &segments,
        export_format,
        sona_core::export::ExportMode::Original,
    )
    .map_err(|error| CliError::Serialize(error.to_string()))?;
    match output_target {
        OutputTarget::Stdout => Ok(CliOutput::stdout(output)),
        OutputTarget::File(path) => {
            sona_runtime_fs::write_transcript_output_file(&path, &output)
                .map_err(|error| CliError::Io(error.to_string()))?;
            Ok(CliOutput::stderr(format!(
                "Wrote transcript to {}",
                path.display()
            )))
        }
    }
}

fn load_config(path: Option<&PathBuf>) -> CliResult<Option<TranscribeConfigSection>> {
    let Some(path) = path else {
        return Ok(None);
    };
    sona_runtime_fs::load_transcribe_config_file(path)
        .map(Some)
        .map_err(|error| CliError::Validation(error.to_string()))
}
