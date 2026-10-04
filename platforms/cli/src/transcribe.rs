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
    after_help = "Examples:\n  sona-cli transcribe ./sample.wav -m whisper-turbo\n  sona-cli transcribe ./sample.wav -m sensevoice -l zh -f txt\n  sona-cli transcribe ./sample.wav -m whisper-turbo -o ./out.srt\n  sona-cli transcribe --input-dir ./recordings --output-dir ./transcripts -f srt\n  sona-cli transcribe ./sample.wav --online-provider groq-whisper --output ./out.srt\n  sona-cli transcribe ./sample.wav --online-provider volcengine-doubao --api-key-env MY_ASR_KEY"
)]
pub struct TranscribeArgs {
    /// Input audio file(s), video file(s), or glob pattern(s). Required unless --input-dir is specified.
    #[arg(value_name = "INPUT")]
    inputs: Vec<PathBuf>,
    /// Directory containing input files for batch transcription.
    #[arg(long = "input-dir", value_name = "DIR")]
    input_dir: Option<PathBuf>,
    /// Directory to write transcript files for batch transcription.
    #[arg(long = "output-dir", value_name = "DIR")]
    output_dir: Option<PathBuf>,
    /// Recursively scan input directory.
    #[arg(long, default_value_t = false)]
    recursive: bool,
    /// Output transcript file. Defaults to stdout when omitted (single-input mode only).
    #[arg(short, long, value_name = "PATH", conflicts_with = "output_dir")]
    output: Option<PathBuf>,
    /// Export format: json, txt, srt, vtt, or md.
    #[arg(
        short,
        long,
        value_name = "FORMAT",
        value_parser = ["json", "txt", "srt", "vtt", "md"]
    )]
    format: Option<String>,
    /// Optional config file, usually sona-cli.toml.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// Override the language setting.
    #[arg(short = 'l', long)]
    language: Option<String>,
    /// Preset model id to use.
    #[arg(short = 'm', long = "model-id")]
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
    /// Number of recognition threads (default: system auto-configured).
    #[arg(long)]
    threads: Option<i32>,
    /// Enable inverse text normalization (convert spoken numbers/dates to digits, e.g. "一百二十" -> "120").
    #[arg(long, default_value_t = false)]
    enable_itn: bool,
    /// Optional hotwords string to enhance recognition, separated by newlines or commas.
    #[arg(long)]
    hotwords: Option<String>,
    /// GPU acceleration mode: auto, cpu, vulkan, metal, or cuda.
    #[arg(
        long = "gpu-acceleration",
        value_name = "MODE",
        value_parser = crate::runtime::gpu_acceleration_value_parser()
    )]
    gpu_acceleration: Option<String>,
    /// VAD buffer size in seconds, for example 0.5.
    #[arg(long = "vad-buffer")]
    vad_buffer: Option<f32>,
    /// Save the resampled WAV to a file.
    #[arg(long = "save-wav")]
    save_wav: Option<PathBuf>,
    /// Custom path to the ffmpeg executable.
    #[arg(long = "ffmpeg-path", value_name = "PATH")]
    ffmpeg_path: Option<String>,
    /// Suppress progress output.
    #[arg(short = 'q', long, default_value_t = false)]
    quiet: bool,
    /// List available online ASR providers and exit.
    #[arg(long, default_value_t = false)]
    pub list_providers: bool,
    /// Overwrite existing output files.
    #[arg(
        short = 'F',
        long,
        default_value_t = false,
        help = "Overwrite existing output files"
    )]
    force: bool,
    /// Number of batch transcription jobs (currently runs sequentially; concurrent jobs experimental).
    #[arg(long, value_name = "N")]
    jobs: Option<usize>,
    /// Text selection mode: original, translation, or bilingual.
    #[arg(
        long,
        value_name = "MODE",
        value_parser = ["original", "translation", "bilingual"],
        default_value = "original"
    )]
    mode: String,
}

pub async fn run_transcribe(
    args: TranscribeArgs,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<CliOutput> {
    if args.list_providers {
        return Ok(CliOutput::stdout(render_online_providers_table()));
    }
    let config = load_config(args.config.as_ref())?;
    let resolved_online = if args.model_id.is_some() && args.online.online_provider.is_none() {
        // Explicit --model-id on CLI overrides config-file online provider
        args.online.validate_provider_presence()?;
        args.online.clone()
    } else {
        args.online.resolve_with_config(
            config.as_ref().and_then(|c| c.online_provider.clone()),
            config.as_ref().and_then(|c| c.api_key_env.clone()),
            config.as_ref().and_then(|c| c.online_config.clone()),
        )?
    };
    let resolved_jobs = sona_core::transcription::runtime::resolve_batch_jobs(
        args.jobs.or_else(|| config.as_ref().and_then(|c| c.jobs)),
    )
    .map_err(|error| CliError::Validation(error.to_string()))?;
    let export_mode = sona_core::export::ExportMode::parse(&args.mode)
        .map_err(|error| CliError::Validation(error.to_string()))?;

    let mut resolved_model_id = args.model_id.clone();
    if !resolved_online.is_online()
        && resolved_model_id.is_none()
        && config.as_ref().and_then(|c| c.model_id.as_ref()).is_none()
    {
        resolved_model_id = infer_single_batch_model(args.models_dir.as_ref()).map(|inferred| {
            if !args.quiet {
                eprintln!("Note: Automatically selected installed batch model '{inferred}'");
            }
            inferred
        });
    }

    let is_batch = args.input_dir.is_some()
        || args.output_dir.is_some()
        || sona_core::transcription::runtime::should_run_path_batch(&args.inputs);

    if is_batch {
        return run_batch_transcribe(
            &args,
            &resolved_online,
            config.as_ref(),
            resolved_jobs,
            resolved_model_id,
            export_mode,
        )
        .await;
    }

    if args.jobs.is_some() {
        return Err(CliError::Validation(
            "--jobs can only be used in batch transcription mode (multiple files or --input-dir)."
                .to_string(),
        ));
    }

    let single_input = args.inputs.first().cloned().ok_or_else(|| {
        CliError::Validation(
            "Missing input: specify at least one input file, stdin ('-'), or --input-dir."
                .to_string(),
        )
    })?;
    let (actual_single_input, _temp_guard): (PathBuf, Option<tempfile::NamedTempFile>) =
        if single_input.as_os_str() == "-" {
            if io.stdin_is_terminal() {
                return Err(CliError::Validation(
                "No audio input provided via stdin. Pipe audio/video into standard input, or specify an input file.".to_string(),
            ));
            }
            const MAX_STDIN_BYTES: usize = 512 * 1024 * 1024;
            let mut buf = Vec::new();
            io.read_bounded_stdin(&mut buf, MAX_STDIN_BYTES)
                .map_err(|error| {
                    CliError::Io(format!("Failed to read audio input from stdin: {error}"))
                })?;
            if buf.is_empty() {
                return Err(CliError::Validation(
                    "Standard input was empty; no audio data received.".to_string(),
                ));
            }
            if buf.len() > MAX_STDIN_BYTES {
                return Err(CliError::Validation(
                    "Audio input from stdin exceeds maximum supported size (512 MB).".to_string(),
                ));
            }
            let mut temp_file = tempfile::Builder::new()
                .prefix("sona_stdin_")
                .suffix(".tmp")
                .tempfile()
                .map_err(|e| {
                    CliError::Io(format!("Failed to create temporary file for stdin: {e}"))
                })?;
            std::io::Write::write_all(&mut temp_file, &buf).map_err(|e| {
                CliError::Io(format!("Failed to write stdin to temporary file: {e}"))
            })?;
            let path = temp_file.path().to_path_buf();
            (path, Some(temp_file))
        } else {
            (single_input, None)
        };

    if resolved_online.is_online() {
        return run_online_transcribe(
            &args,
            &actual_single_input,
            &resolved_online,
            config.as_ref(),
            export_mode,
        )
        .await;
    }
    let options = BatchTranscribeOptions {
        input: actual_single_input,
        output: args.output,
        format: args.format,
        language: args.language,
        model_id: resolved_model_id,
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
        ffmpeg_path: args.ffmpeg_path,
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
    let transcriber = crate::asr_adapter::local_batch_transcriber();
    let segments = transcriber
        .transcribe(plan)
        .await
        .map_err(crate::online_asr::map_asr_error)?;
    render_transcription(segments, export_format, output_target, export_mode)
}

async fn run_batch_transcribe(
    args: &TranscribeArgs,
    resolved_online: &crate::online_asr::OnlineAsrArgs,
    config: Option<&TranscribeConfigSection>,
    resolved_jobs: usize,
    resolved_model_id: Option<String>,
    export_mode: sona_core::export::ExportMode,
) -> CliResult<CliOutput> {
    let batch_source = sona_runtime_fs::resolve_batch_input_source(
        args.input_dir.as_deref(),
        &args.inputs,
        args.recursive,
    )
    .map_err(crate::map_runtime_fs_error)?;

    if batch_source.inputs.is_empty() {
        return Err(CliError::Validation(
            "No supported media files found for batch transcription.".to_string(),
        ));
    }

    if args.output.is_some() {
        return Err(CliError::Validation(
            "--output cannot be used in batch transcription mode; use --output-dir and --format instead."
                .to_string(),
        ));
    }
    if resolved_jobs > 1 {
        return Err(CliError::Validation(
            "Concurrent batch transcription (--jobs > 1) is not yet supported; batch jobs currently run sequentially.".to_string(),
        ));
    }

    let output_dir = if let Some(dir) = &args.output_dir {
        dir.clone()
    } else {
        batch_source.base_dir.clone()
    };

    let export_format = match args
        .format
        .as_deref()
        .or_else(|| config.and_then(|c| c.format.as_deref()))
    {
        Some(fmt) => sona_core::export::ExportFormat::parse(fmt)
            .map_err(|error| CliError::Validation(error.to_string()))?,
        None => sona_core::export::ExportFormat::Json,
    };

    let plans = sona_runtime_fs::plan_batch_output_files(
        &batch_source.inputs,
        &batch_source.base_dir,
        &output_dir,
        export_format,
        batch_source.preserve_relative_paths,
        args.force,
    )
    .map_err(crate::map_runtime_fs_error)?;

    let total = plans.len();
    if resolved_online.is_online() {
        reject_online_local_options(args)?;
        let language = args
            .language
            .clone()
            .or_else(|| config.and_then(|c| c.language.clone()))
            .unwrap_or_else(|| sona_core::transcription::runtime::DEFAULT_LANGUAGE.to_string());
        let enable_itn = args.enable_itn || config.and_then(|c| c.enable_itn).unwrap_or(false);
        let hotwords = args
            .hotwords
            .clone()
            .or_else(|| config.and_then(|c| c.hotwords.clone()));
        let request =
            resolved_online.build_request(AsrMode::Batch, language, enable_itn, hotwords)?;

        for (index, plan_item) in plans.iter().enumerate() {
            let segments = crate::asr_adapter::online_batch_transcribe(
                plan_item.input_path.clone(),
                request.clone(),
            )
            .await
            .map_err(crate::online_asr::map_asr_error)?;

            let content =
                sona_core::export::export_segments_with_mode(&segments, export_format, export_mode)
                    .map_err(|error| CliError::Serialize(error.to_string()))?;

            sona_runtime_fs::write_transcript_output_file(&plan_item.output_path, &content)
                .map_err(|error| CliError::Io(error.to_string()))?;

            if !args.quiet {
                eprintln!(
                    "[{}/{}] Transcribed {} -> {}",
                    index + 1,
                    total,
                    plan_item.input_path.display(),
                    plan_item.output_path.display()
                );
            }
        }
    } else {
        let transcriber = crate::asr_adapter::local_batch_transcriber();
        let format_name = match export_format {
            sona_core::export::ExportFormat::Json => "json",
            sona_core::export::ExportFormat::Txt => "txt",
            sona_core::export::ExportFormat::Srt => "srt",
            sona_core::export::ExportFormat::Vtt => "vtt",
            sona_core::export::ExportFormat::Md => "md",
        };
        for (index, plan_item) in plans.iter().enumerate() {
            let single_options = BatchTranscribeOptions {
                input: plan_item.input_path.clone(),
                output: Some(plan_item.output_path.clone()),
                format: Some(format_name.to_string()),
                language: args.language.clone(),
                model_id: resolved_model_id.clone(),
                models_dir: args.models_dir.clone(),
                default_models_dir: crate::desktop_paths::default_models_dir(),
                vad_model_id: args.vad_model_id.clone(),
                punctuation_model_id: args.punctuation_model_id.clone(),
                threads: args.threads,
                enable_itn: if args.enable_itn { Some(true) } else { None },
                hotwords: args.hotwords.clone(),
                gpu_acceleration: args.gpu_acceleration.clone(),
                vad_buffer: args.vad_buffer,
                save_wav: None,
                quiet: args.quiet,
                force: args.force,
                ffmpeg_path: args.ffmpeg_path.clone(),
            };

            let file_plan = sona_runtime_fs::resolve_batch_transcribe_plan_with_runtime_paths_and_models_dir_status(
                single_options,
                config.cloned(),
                crate::desktop_paths::models_dir_status,
            )
            .map_err(crate::map_runtime_fs_error)?;

            let segments = transcriber
                .transcribe(file_plan)
                .await
                .map_err(crate::online_asr::map_asr_error)?;

            let content =
                sona_core::export::export_segments_with_mode(&segments, export_format, export_mode)
                    .map_err(|error| CliError::Serialize(error.to_string()))?;

            sona_runtime_fs::write_transcript_output_file(&plan_item.output_path, &content)
                .map_err(|error| CliError::Io(error.to_string()))?;

            if !args.quiet {
                eprintln!(
                    "[{}/{}] Transcribed {} -> {}",
                    index + 1,
                    total,
                    plan_item.input_path.display(),
                    plan_item.output_path.display()
                );
            }
        }
    }

    Ok(CliOutput::stderr(format!(
        "Transcribed {total} file(s) into {}",
        output_dir.display()
    )))
}

async fn run_online_transcribe(
    args: &TranscribeArgs,
    input: &std::path::Path,
    online: &crate::online_asr::OnlineAsrArgs,
    config: Option<&TranscribeConfigSection>,
    export_mode: sona_core::export::ExportMode,
) -> CliResult<CliOutput> {
    reject_online_local_options(args)?;
    validate_online_paths(input, args.output.as_ref(), args.force)?;
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
    let request = online.build_request(AsrMode::Batch, language, enable_itn, hotwords)?;
    let export_format = resolve_export_format(
        args.format
            .as_deref()
            .or_else(|| config.and_then(|config| config.format.as_deref())),
        args.output.as_deref(),
    )
    .map_err(|error| CliError::Validation(error.to_string()))?;
    let output_target = resolve_output_target(args.output.clone());
    let segments = crate::asr_adapter::online_batch_transcribe(input.to_path_buf(), request)
        .await
        .map_err(crate::online_asr::map_asr_error)?;
    render_transcription(segments, export_format, output_target, export_mode)
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

fn validate_online_paths(
    input: &std::path::Path,
    output: Option<&PathBuf>,
    force: bool,
) -> CliResult<()> {
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
    export_mode: sona_core::export::ExportMode,
) -> CliResult<CliOutput> {
    let output =
        sona_core::export::export_segments_with_mode(&segments, export_format, export_mode)
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
    let resolved = crate::init_config::resolve_config_path(path);
    let Some(path) = resolved.as_ref() else {
        return Ok(None);
    };
    sona_runtime_fs::load_transcribe_config_file(path)
        .map(Some)
        .map_err(|error| CliError::Validation(error.to_string()))
}

pub(crate) fn render_online_providers_table() -> String {
    let providers = sona_core::ports::asr::online_asr_providers();
    let headers = ["PROVIDER", "DEFAULT_ENV_VAR", "MODES"];
    let mut rows = Vec::new();
    for p in providers {
        let env_var = p.default_api_key_env().unwrap_or("-");
        let mut modes = vec!["batch"];
        if p.streaming.supported.unwrap_or(false) {
            modes.push("streaming");
        }
        rows.push([p.id.clone(), env_var.to_string(), modes.join(", ")]);
    }
    let widths = [
        rows.iter()
            .map(|r| r[0].len())
            .max()
            .unwrap_or(8)
            .max(headers[0].len()),
        rows.iter()
            .map(|r| r[1].len())
            .max()
            .unwrap_or(15)
            .max(headers[1].len()),
        rows.iter()
            .map(|r| r[2].len())
            .max()
            .unwrap_or(5)
            .max(headers[2].len()),
    ];
    let mut out = String::new();
    crate::table::append_table_row(&mut out, &headers, &widths);
    crate::table::append_table_separator(&mut out, &widths);
    for row in rows {
        crate::table::append_table_row(&mut out, &[&row[0], &row[1], &row[2]], &widths);
    }
    out
}

fn infer_single_batch_model(models_dir: Option<&PathBuf>) -> Option<String> {
    let resolved_dir = sona_core::models::paths::resolve_models_dir(
        models_dir.cloned(),
        crate::desktop_paths::default_models_dir(),
        crate::desktop_paths::models_dir_status,
    )
    .ok()?;
    let all_models = sona_runtime_fs::list_models(&resolved_dir);
    let installed_batch: Vec<_> = all_models
        .into_iter()
        .filter(|m| m.installed && m.modes.iter().any(|mode| mode == "batch"))
        .collect();
    if installed_batch.len() == 1 {
        Some(installed_batch[0].id.clone())
    } else {
        None
    }
}
