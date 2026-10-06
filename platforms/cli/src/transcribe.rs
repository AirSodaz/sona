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
    #[arg(value_name = "INPUT", help_heading = "Input/Output")]
    inputs: Vec<PathBuf>,
    /// Directory containing input files for batch transcription.
    #[arg(long = "input-dir", value_name = "DIR", help_heading = "Input/Output")]
    input_dir: Option<PathBuf>,
    /// Directory to write transcript files for batch transcription.
    #[arg(long = "output-dir", value_name = "DIR", help_heading = "Input/Output")]
    output_dir: Option<PathBuf>,
    /// Recursively scan input directory.
    #[arg(long, default_value_t = false, help_heading = "Input/Output")]
    recursive: bool,
    /// Output transcript file. Defaults to stdout when omitted (single-input mode only).
    #[arg(
        short,
        long,
        value_name = "PATH",
        conflicts_with = "output_dir",
        help_heading = "Input/Output"
    )]
    output: Option<PathBuf>,
    /// Export format: json, txt, srt, vtt, or md.
    #[arg(
        short,
        long,
        value_name = "FORMAT",
        value_parser = ["json", "txt", "srt", "vtt", "md"],
        help_heading = "Input/Output"
    )]
    format: Option<String>,
    /// Text selection mode: original, translation, or bilingual.
    #[arg(
        long,
        value_name = "MODE",
        value_parser = ["original", "translation", "bilingual"],
        default_value = "original",
        help_heading = "Input/Output"
    )]
    mode: String,
    /// Overwrite existing output files.
    #[arg(
        short = 'F',
        long,
        default_value_t = false,
        help = "Overwrite existing output files",
        help_heading = "Input/Output"
    )]
    force: bool,
    /// Continue processing remaining files if an error occurs during batch transcription.
    #[arg(
        long = "continue-on-error",
        alias = "skip-errors",
        default_value_t = false,
        help_heading = "Input/Output"
    )]
    continue_on_error: bool,

    /// Preset model id to use, or online model name when an online provider is selected.
    #[arg(
        short = 'm',
        long = "model",
        alias = "model-id",
        help_heading = "Model Options"
    )]
    model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long = "models-dir", help_heading = "Model Options")]
    models_dir: Option<PathBuf>,
    /// Override the language setting.
    #[arg(short = 'l', long, help_heading = "Model Options")]
    language: Option<String>,
    /// VAD model id override.
    #[arg(long = "vad-model-id", help_heading = "Model Options")]
    vad_model_id: Option<String>,
    /// Punctuation model id override.
    #[arg(long = "punctuation-model-id", help_heading = "Model Options")]
    punctuation_model_id: Option<String>,

    #[command(flatten)]
    online: crate::online_asr::OnlineAsrArgs,

    /// Number of recognition threads (default: system auto-configured).
    #[arg(long, help_heading = "Audio & Performance")]
    threads: Option<i32>,
    /// Enable inverse text normalization (convert spoken numbers/dates to digits, e.g. "一百二十" -> "120").
    #[arg(long, default_value_t = false, help_heading = "Audio & Performance")]
    enable_itn: bool,
    /// Optional hotwords string to enhance recognition, separated by newlines or commas.
    #[arg(long, help_heading = "Audio & Performance")]
    hotwords: Option<String>,
    /// GPU acceleration mode: auto, cpu, vulkan, metal, or cuda.
    #[arg(
        long = "gpu-acceleration",
        value_name = "MODE",
        value_parser = crate::runtime::gpu_acceleration_value_parser(),
        help_heading = "Audio & Performance"
    )]
    gpu_acceleration: Option<String>,
    /// VAD buffer size in seconds, for example 0.5.
    #[arg(long = "vad-buffer", help_heading = "Audio & Performance")]
    vad_buffer: Option<f32>,
    /// Save the resampled WAV to a file.
    #[arg(
        long = "save-wav",
        visible_alias = "save-audio",
        help_heading = "Audio & Performance"
    )]
    save_wav: Option<PathBuf>,
    /// Custom path to the ffmpeg executable.
    #[arg(
        long = "ffmpeg-path",
        value_name = "PATH",
        help_heading = "Audio & Performance"
    )]
    ffmpeg_path: Option<String>,

    /// Optional config file, usually sona-cli.toml.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// Suppress progress output.
    #[arg(short = 'q', long, default_value_t = false)]
    quiet: bool,

    /// List available online ASR providers and exit (deprecated: use 'sona-cli providers' instead).
    #[arg(long, default_value_t = false, hide = true)]
    pub list_providers: bool,
    /// Number of batch transcription jobs (experimental; current execution is sequential).
    #[arg(long, value_name = "N", hide = true)]
    jobs: Option<usize>,
}
pub async fn run_transcribe(
    args: TranscribeArgs,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<CliOutput> {
    if args.list_providers {
        return Ok(CliOutput::stdout(render_online_providers_table()));
    }
    let config = load_config(args.config.as_ref())?;
    let mut resolved_online = args.online.resolve_with_config(
        config.as_ref().and_then(|c| c.online_provider.clone()),
        config.as_ref().and_then(|c| c.api_key_env.clone()),
        config.as_ref().and_then(|c| c.online_config.clone()),
    )?;

    if let (true, Some(cli_model)) = (resolved_online.is_online(), &args.model_id) {
        if let Some(existing_online_model) = &resolved_online.online_model {
            if existing_online_model != cli_model {
                return Err(CliError::Validation(format!(
                    "Conflicting model names specified: -m/--model '{cli_model}' vs --online-model '{existing_online_model}'."
                )));
            }
        } else {
            resolved_online.online_model = Some(cli_model.clone());
        }
    }
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
        match infer_batch_model(args.models_dir.as_ref()) {
            BatchModelInference::AutoSelected(inferred) => {
                if !args.quiet {
                    eprintln!("Note: Automatically selected installed batch model '{inferred}'");
                }
                resolved_model_id = Some(inferred);
            }
            BatchModelInference::MultipleInstalled(names) => {
                return Err(CliError::Validation(format!(
                    "Multiple batch models installed ([{}]). Specify one with -m/--model <MODEL_ID>, or configure model_id in sona-cli.toml.",
                    names.join(", ")
                )));
            }
            BatchModelInference::NoneInstalled => {}
        }
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

    let single_input = args.inputs.first().cloned();
    let single_input = match single_input {
        Some(input) => input,
        None if !io.stdin_is_terminal() => PathBuf::from("-"),
        None => {
            return Err(CliError::Validation(
                "Missing input: specify at least one input file, stdin ('-'), or --input-dir."
                    .to_string(),
            ));
        }
    };
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

    let single_format = resolve_single_transcribe_format(
        args.format.clone(),
        args.output.as_deref(),
        config.as_ref().and_then(|c| c.format.as_deref()),
        io.stdout_is_terminal(),
    );

    if resolved_online.is_online() {
        return run_online_transcribe(
            &args,
            &actual_single_input,
            &resolved_online,
            config.as_ref(),
            export_mode,
            single_format.as_deref(),
        )
        .await;
    }
    let options = BatchTranscribeOptions {
        input: actual_single_input,
        output: args.output,
        format: single_format,
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
            "--output cannot be used with multiple input files. Specify --format <FORMAT> (and optionally --output-dir <DIR>) to export each transcript individually."
                .to_string(),
        ));
    }
    if resolved_jobs > 1 {
        return Err(CliError::Validation(
            "Concurrent batch transcription (--jobs > 1) is experimental; current execution is sequential.".to_string(),
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
    let mut failures = Vec::new();
    let mut succeeded = 0;

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

        let mut ctrl_c = std::pin::pin!(tokio::signal::ctrl_c());
        for (index, plan_item) in plans.iter().enumerate() {
            let res: Result<(), CliError> = tokio::select! {
                biased;
                _ = &mut ctrl_c => {
                    return Err(CliError::Cancelled("Batch transcription cancelled by user".to_string()));
                }
                item_res = async {
                    let segments = crate::asr_adapter::online_batch_transcribe(
                        plan_item.input_path.clone(),
                        request.clone(),
                    )
                    .await
                    .map_err(crate::online_asr::map_asr_error)?;

                    let content = sona_core::export::export_segments_with_mode(
                        &segments,
                        export_format,
                        export_mode,
                    )
                    .map_err(|error| CliError::Serialize(error.to_string()))?;

                    sona_runtime_fs::write_transcript_output_file(&plan_item.output_path, &content)
                        .map_err(|error| CliError::Io(error.to_string()))?;
                    Ok(())
                } => item_res,
            };

            match res {
                Ok(()) => {
                    succeeded += 1;
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
                Err(err) => {
                    eprintln!(
                        "[{}/{}] [FAIL] {}: {err}",
                        index + 1,
                        total,
                        plan_item.input_path.display()
                    );
                    if args.continue_on_error {
                        failures.push((plan_item.input_path.clone(), err.to_string()));
                    } else {
                        return Err(err);
                    }
                }
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
        let mut ctrl_c = std::pin::pin!(tokio::signal::ctrl_c());
        for (index, plan_item) in plans.iter().enumerate() {
            let res: Result<(), CliError> = tokio::select! {
                biased;
                _ = &mut ctrl_c => {
                    return Err(CliError::Cancelled("Batch transcription cancelled by user".to_string()));
                }
                item_res = async {
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
                    Ok(())
                } => item_res,
            };
            match res {
                Ok(()) => {
                    succeeded += 1;
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
                Err(err) => {
                    eprintln!(
                        "[{}/{}] [FAIL] {}: {err}",
                        index + 1,
                        total,
                        plan_item.input_path.display()
                    );
                    if args.continue_on_error {
                        failures.push((plan_item.input_path.clone(), err.to_string()));
                    } else {
                        return Err(err);
                    }
                }
            }
        }
    }

    if !failures.is_empty() {
        let msg = format!(
            "Transcribed {succeeded}/{total} file(s) into {}. {} file(s) failed.",
            output_dir.display(),
            failures.len()
        );
        return Err(CliError::Validation(msg));
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
    resolved_format: Option<&str>,
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
        resolved_format.or_else(|| {
            args.format
                .as_deref()
                .or_else(|| config.and_then(|config| config.format.as_deref()))
        }),
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
    let headers = ["PROVIDER", "DEFAULT_ENV_VAR", "STATUS", "MODES"];
    let mut rows = Vec::new();
    for p in providers {
        let env_var = p.default_api_key_env().unwrap_or("-");
        let is_configured = p
            .default_api_key_env()
            .is_some_and(|var| std::env::var_os(var).is_some_and(|val| !val.is_empty()));
        let status = if is_configured {
            "configured"
        } else {
            "not set"
        };
        let mut modes = vec!["batch"];
        if p.streaming.supported.unwrap_or(false) {
            modes.push("streaming");
        }
        rows.push([
            p.id.clone(),
            env_var.to_string(),
            status.to_string(),
            modes.join(", "),
        ]);
    }
    let widths = crate::table::column_widths(&headers, &rows);
    let mut out = String::new();
    crate::table::append_table_row(&mut out, &headers, &widths);
    crate::table::append_table_separator(&mut out, &widths);
    for row in rows {
        crate::table::append_table_row(&mut out, &[&row[0], &row[1], &row[2], &row[3]], &widths);
    }
    out
}

pub(crate) fn render_online_providers_with_models() -> String {
    let providers = sona_core::ports::asr::online_asr_providers();
    let mut out = String::new();
    out.push_str("Supported Online ASR Providers & Models:\n\n");

    for p in providers {
        let env_var = p.default_api_key_env().unwrap_or("-");
        let is_configured = p
            .default_api_key_env()
            .is_some_and(|var| std::env::var_os(var).is_some_and(|val| !val.is_empty()));
        let status = if is_configured {
            "configured"
        } else {
            "not set"
        };
        let mut modes = vec!["batch"];
        if p.streaming.supported.unwrap_or(false) {
            modes.push("streaming");
        }

        out.push_str(&format!(
            "{} (Status: {}, Env: {}, Modes: {})\n",
            p.id,
            status,
            env_var,
            modes.join(", ")
        ));
        if p.models.is_empty() {
            out.push_str("  (no curated models listed; pass custom model via --online-model)\n");
        } else {
            for m in &p.models {
                let default_tag = if m.is_default.unwrap_or(false) {
                    " [default]"
                } else {
                    ""
                };
                let modes_tag = if m.modes.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", m.modes.join(", "))
                };
                let desc = m
                    .description
                    .as_deref()
                    .map(|d| format!(" - {d}"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  * {}{}{}: {}{}\n",
                    m.id, default_tag, modes_tag, m.name, desc
                ));
            }
        }
        out.push('\n');
    }
    out.push_str("Run 'sona-cli providers <PROVIDER>' for details on a specific provider.\n");
    out
}

pub(crate) fn render_online_provider_detail(provider_id: &str) -> CliResult<String> {
    let provider = sona_core::ports::asr::find_online_asr_provider(provider_id).ok_or_else(|| {
        CliError::Validation(format!(
            "Unknown online ASR provider '{provider_id}'. Run 'sona-cli providers' to see available providers."
        ))
    })?;

    let env_var = provider.default_api_key_env().unwrap_or("-");
    let is_configured = provider
        .default_api_key_env()
        .is_some_and(|var| std::env::var_os(var).is_some_and(|val| !val.is_empty()));
    let status = if is_configured {
        "configured"
    } else {
        "not set"
    };
    let mut modes = vec!["batch"];
    if provider.streaming.supported.unwrap_or(false) {
        modes.push("streaming");
    }

    let mut out = String::new();
    out.push_str(&format!("Provider: {}\n", provider.id));
    out.push_str(&format!("Status:   {} (Env: {})\n", status, env_var));
    out.push_str(&format!("Modes:    {}\n", modes.join(", ")));
    if !provider.languages.is_empty() {
        let sample_len = provider.languages.len().min(12);
        let sample = provider.languages[..sample_len].join(", ");
        if provider.languages.len() > sample_len {
            out.push_str(&format!(
                "Languages ({}): {}, ...\n",
                provider.languages.len(),
                sample
            ));
        } else {
            out.push_str(&format!(
                "Languages ({}): {}\n",
                provider.languages.len(),
                sample
            ));
        }
    }
    out.push('\n');
    out.push_str("Supported Models:\n");

    let headers = ["MODEL ID", "NAME", "MODES", "DEFAULT", "DESCRIPTION"];
    let rows: Vec<[String; 5]> = provider
        .models
        .iter()
        .map(|m| {
            [
                m.id.clone(),
                m.name.clone(),
                m.modes.join(", "),
                if m.is_default.unwrap_or(false) {
                    "yes"
                } else {
                    "-"
                }
                .to_string(),
                m.description.clone().unwrap_or_default(),
            ]
        })
        .collect();

    let widths = crate::table::column_widths(&headers, &rows);
    crate::table::append_table_row(&mut out, &headers, &widths);
    crate::table::append_table_separator(&mut out, &widths);
    for row in &rows {
        crate::table::append_table_row(
            &mut out,
            &[&row[0], &row[1], &row[2], &row[3], &row[4]],
            &widths,
        );
    }

    out.push_str("\nUsage Examples:\n");
    let sample_model = provider
        .models
        .iter()
        .find(|m| m.is_default.unwrap_or(false))
        .or_else(|| provider.models.first())
        .map(|m| m.id.as_str())
        .unwrap_or("model-name");
    out.push_str(&format!(
        "  sona-cli transcribe ./audio.wav --online-provider {} --online-model {}\n",
        provider.id, sample_model
    ));
    if provider.streaming.supported.unwrap_or(false) {
        out.push_str(&format!(
            "  sona-cli live --online-provider {} --stream ndjson\n",
            provider.id
        ));
    }

    Ok(out)
}

enum BatchModelInference {
    AutoSelected(String),
    MultipleInstalled(Vec<String>),
    NoneInstalled,
}

fn infer_batch_model(models_dir: Option<&PathBuf>) -> BatchModelInference {
    let Ok(resolved_dir) = sona_core::models::paths::resolve_models_dir(
        models_dir.cloned(),
        crate::desktop_paths::default_models_dir(),
        crate::desktop_paths::models_dir_status,
    ) else {
        return BatchModelInference::NoneInstalled;
    };
    let all_models = sona_runtime_fs::list_models(&resolved_dir);
    let installed_batch: Vec<_> = all_models
        .into_iter()
        .filter(|m| m.installed && m.modes.iter().any(|mode| mode == "batch"))
        .collect();

    if installed_batch.is_empty() {
        BatchModelInference::NoneInstalled
    } else if installed_batch.len() == 1 {
        BatchModelInference::AutoSelected(installed_batch[0].id.clone())
    } else {
        use sona_core::models::preset_models::DEFAULT_WHISPER_TURBO_MODEL_ID;
        if let Some(preferred) = installed_batch
            .iter()
            .find(|m| m.id == DEFAULT_WHISPER_TURBO_MODEL_ID)
        {
            BatchModelInference::AutoSelected(preferred.id.clone())
        } else {
            let names = installed_batch.into_iter().map(|m| m.id).collect();
            BatchModelInference::MultipleInstalled(names)
        }
    }
}

fn resolve_single_transcribe_format(
    cli_format: Option<String>,
    output: Option<&std::path::Path>,
    config_format: Option<&str>,
    stdout_is_terminal: bool,
) -> Option<String> {
    cli_format.or_else(|| {
        if output.is_none() && config_format.is_none() && stdout_is_terminal {
            Some("txt".to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_transcribe_format_defaults_to_txt_on_terminal_stdout() {
        assert_eq!(
            resolve_single_transcribe_format(None, None, None, true),
            Some("txt".to_string())
        );
    }

    #[test]
    fn single_transcribe_format_preserves_none_on_piped_stdout() {
        assert_eq!(
            resolve_single_transcribe_format(None, None, None, false),
            None
        );
    }

    #[test]
    fn single_transcribe_format_preserves_explicit_cli_format() {
        assert_eq!(
            resolve_single_transcribe_format(Some("json".to_string()), None, None, true),
            Some("json".to_string())
        );
    }

    #[test]
    fn single_transcribe_format_preserves_none_when_output_file_provided() {
        let path = std::path::Path::new("out.srt");
        assert_eq!(
            resolve_single_transcribe_format(None, Some(path), None, true),
            None
        );
    }

    #[test]
    fn single_transcribe_format_preserves_none_when_config_format_present() {
        assert_eq!(
            resolve_single_transcribe_format(None, None, Some("srt"), true),
            None
        );
    }
}
