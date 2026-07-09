use clap::Args;
use std::path::PathBuf;

use crate::{CliError, CliOutput, CliResult};
use sona_core::ports::asr::BatchTranscriber;
use sona_core::runtime::config::TranscribeConfigSection;
use sona_core::transcription::runtime::{BatchTranscribeOptions, OutputTarget};

#[derive(Debug, Args)]
#[command(
    about = "Transcribe local audio or video files",
    after_help = "Examples:\n  sona-cli transcribe ./sample.wav --model-id sherpa-onnx-whisper-turbo\n  sona-cli transcribe ./sample.wav --config ./sona-cli.toml --output ./out.srt"
)]
pub struct TranscribeArgs {
    /// Input media file to transcribe.
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
        .map_err(CliError::Validation)?;
    let export_format = plan.export_format;
    let output_target = plan.output_target.clone();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Io(format!("Failed to create async runtime: {error}")))?;
    let transcriber = crate::asr_adapter::local_batch_transcriber();
    let segments = runtime
        .block_on(transcriber.transcribe(plan))
        .map_err(CliError::Other)?;

    let output = sona_core::export::export_segments_with_mode(
        &segments,
        export_format,
        sona_core::export::ExportMode::Original,
    )
    .map_err(CliError::Serialize)?;

    match output_target {
        OutputTarget::Stdout => Ok(CliOutput::stdout(output)),
        OutputTarget::File(path) => {
            sona_runtime_fs::write_transcript_output_file(&path, &output).map_err(CliError::Io)?;
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
        .map_err(CliError::Validation)
}
