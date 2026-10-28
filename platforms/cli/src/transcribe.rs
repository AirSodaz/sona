use crate::{CliError, CliOutput, CliResult};
use clap::Args;
use sona_core::ports::asr::{AsrMode, BatchTranscriberPort, BatchTranscriptionObserver};
use sona_core::runtime::config::TranscribeConfigSection;
use sona_core::transcription::runtime::{
    BatchOutputPlan, BatchTranscribeOptions, OutputTarget, resolve_export_format,
    resolve_output_target,
};
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

struct CliStreamBatchObserver {
    segments: std::sync::Mutex<Vec<TranscriptSegment>>,
    tx: tokio::sync::mpsc::UnboundedSender<TranscriptSegment>,
    cancelled: std::sync::atomic::AtomicBool,
}

impl BatchTranscriptionObserver for CliStreamBatchObserver {
    fn on_progress(&self, _progress: f32) {}

    fn on_transcript_update(&self, update: &TranscriptUpdate) {
        let mut segments = self.segments.lock().unwrap();
        if !update.remove_ids.is_empty() {
            segments.retain(|s| !update.remove_ids.contains(&s.id));
        }
        for seg in &update.upsert_segments {
            if let Some(existing) = segments.iter_mut().find(|s| s.id == seg.id) {
                *existing = seg.clone();
            } else {
                segments.push(seg.clone());
            }
            let _ = self.tx.send(seg.clone());
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
    }
}

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

    #[command(flatten)]
    pub asr: crate::asr_args::CommonAsrArgs,
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
    /// Number of concurrent batch transcription jobs (default: 1).
    #[arg(long, value_name = "N", hide = true)]
    jobs: Option<usize>,
}
pub async fn run_transcribe(
    args: TranscribeArgs,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<CliOutput> {
    if args.list_providers {
        return Ok(CliOutput::stdout(
            crate::online_asr::render_online_providers_table(),
        ));
    }
    let config = load_config(args.config.as_ref())?;
    let mut resolved_online = args.asr.online.resolve_with_config(
        config.as_ref().and_then(|c| c.online_provider.clone()),
        config.as_ref().and_then(|c| c.api_key_env.clone()),
        config.as_ref().and_then(|c| c.online_config.clone()),
    )?;

    crate::asr_args::reconcile_online_model(&mut resolved_online, args.asr.model_id.as_deref())?;
    let resolved_jobs = sona_core::transcription::runtime::resolve_batch_jobs(
        args.jobs.or_else(|| config.as_ref().and_then(|c| c.jobs)),
    )
    .map_err(|error| CliError::Validation(error.to_string()))?;
    let export_mode = sona_core::export::ExportMode::parse(&args.mode)
        .map_err(|error| CliError::Validation(error.to_string()))?;

    let mut resolved_model_id = args.asr.model_id.clone();
    if !resolved_online.is_online()
        && resolved_model_id.is_none()
        && config.as_ref().and_then(|c| c.model_id.as_ref()).is_none()
    {
        match infer_batch_model(args.asr.models_dir.as_ref()) {
            BatchModelInference::AutoSelected(inferred) => {
                if !args.quiet {
                    writeln!(
                        io.stderr(),
                        "Note: Automatically selected installed batch model '{inferred}'"
                    )
                    .map_err(|e| CliError::Io(e.to_string()))?;
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
            io,
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
            let mut temp_file = tempfile::Builder::new()
                .prefix("sona_stdin_")
                .suffix(".tmp")
                .tempfile()
                .map_err(|e| {
                    CliError::Io(format!("Failed to create temporary file for stdin: {e}"))
                })?;
            let bytes_copied = io
                .copy_bounded_stdin_to_writer(&mut temp_file, MAX_STDIN_BYTES)
                .map_err(|error| {
                    CliError::Io(format!("Failed to read audio input from stdin: {error}"))
                })?;
            if bytes_copied == 0 {
                return Err(CliError::Validation(
                    "Standard input was empty; no audio data received.".to_string(),
                ));
            }
            if bytes_copied > MAX_STDIN_BYTES as u64 {
                return Err(CliError::Validation(
                    "Audio input from stdin exceeds maximum supported size (512 MB).".to_string(),
                ));
            }
            std::io::Write::flush(&mut temp_file).map_err(|e| {
                CliError::Io(format!("Failed to flush temporary file for stdin: {e}"))
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
    let is_network_stream =
        sona_core::history::is_network_url(&actual_single_input.to_string_lossy());
    let options = BatchTranscribeOptions {
        input: actual_single_input,
        output: args.output,
        format: single_format,
        language: args.asr.language,
        model_id: resolved_model_id,
        models_dir: args.asr.models_dir,
        default_models_dir: crate::desktop_paths::default_models_dir(),
        vad_model_id: args.asr.vad_model_id,
        punctuation_model_id: args.asr.punctuation_model_id,
        threads: args.asr.threads,
        enable_itn: if args.asr.enable_itn {
            Some(true)
        } else {
            None
        },
        hotwords: args.asr.hotwords,
        gpu_acceleration: args.asr.gpu_acceleration,
        vad_buffer: args.asr.vad_buffer,
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
    let segments = if is_network_stream {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let observer = Arc::new(CliStreamBatchObserver {
            segments: std::sync::Mutex::new(Vec::new()),
            tx,
            cancelled: std::sync::atomic::AtomicBool::new(false),
        });
        let transcribe_task = transcriber.transcribe_with_observer(
            plan,
            Arc::clone(&observer) as Arc<dyn BatchTranscriptionObserver>,
        );
        tokio::pin!(transcribe_task);

        loop {
            tokio::select! {
                res = &mut transcribe_task => {
                    if !args.quiet {
                        while let Ok(segment) = rx.try_recv() {
                            let _ = writeln!(io.stderr(), "{}", segment.text);
                        }
                    }
                    match res {
                        Ok(segs) => {
                            break segs;
                        }
                        Err(err) => {
                            let collected = observer.segments.lock().unwrap().clone();
                            if !collected.is_empty() {
                                break collected;
                            }
                            return Err(crate::online_asr::map_asr_error(err));
                        }
                    }
                }
                Some(segment) = rx.recv() => {
                    if !args.quiet {
                        let _ = writeln!(io.stderr(), "{}", segment.text);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    observer.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                    if !args.quiet {
                        while let Ok(segment) = rx.try_recv() {
                            let _ = writeln!(io.stderr(), "{}", segment.text);
                        }
                    }
                    let collected = observer.segments.lock().unwrap().clone();
                    if collected.is_empty() {
                        return Err(CliError::Cancelled("Transcription cancelled by user".to_string()));
                    } else {
                        break collected;
                    }
                }
            }
        }
    } else {
        tokio::select! {
            res = transcriber.transcribe(plan) => {
                res.map_err(crate::online_asr::map_asr_error)?
            }
            _ = tokio::signal::ctrl_c() => {
                return Err(CliError::Cancelled("Transcription cancelled by user".to_string()));
            }
        }
    };
    render_transcription(segments, export_format, output_target, export_mode)
}

enum BatchJobProgress {
    Success {
        index: usize,
        total: usize,
        input: std::path::PathBuf,
        output: std::path::PathBuf,
    },
    Failure {
        index: usize,
        total: usize,
        input: std::path::PathBuf,
        error: CliError,
    },
}

async fn run_batch_transcribe_dispatched<F, Fut>(
    args: &TranscribeArgs,
    plans: &[BatchOutputPlan],
    output_dir: &std::path::Path,
    resolved_jobs: usize,
    io: &mut (dyn crate::CliIo + Send),
    execute_item: F,
) -> CliResult<CliOutput>
where
    F: Fn(BatchOutputPlan) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), CliError>> + Send + 'static,
{
    let total = plans.len();
    let mut failures = Vec::new();
    let mut succeeded = 0;

    let concurrency = resolved_jobs.max(1);
    let (job_tx, job_rx) = tokio::sync::mpsc::channel::<(usize, BatchOutputPlan)>(concurrency);
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(concurrency * 2);
    let job_rx = Arc::new(tokio::sync::Mutex::new(job_rx));
    let mut worker_handles = Vec::with_capacity(concurrency + 1);
    let execute_item = Arc::new(execute_item);

    for _ in 0..concurrency {
        let rx = job_rx.clone();
        let tx = progress_tx.clone();
        let exec = execute_item.clone();

        let handle = tokio::spawn(async move {
            loop {
                let job = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };
                let Some((index, plan_item)) = job else {
                    break;
                };
                let input_path = plan_item.input_path.clone();
                let output_path = plan_item.output_path.clone();
                let res = exec(plan_item).await;

                let progress = match res {
                    Ok(()) => BatchJobProgress::Success {
                        index,
                        total,
                        input: input_path,
                        output: output_path,
                    },
                    Err(error) => BatchJobProgress::Failure {
                        index,
                        total,
                        input: input_path,
                        error,
                    },
                };
                if tx.send(progress).await.is_err() {
                    break;
                }
            }
        });
        worker_handles.push(handle);
    }

    let plans_items = plans.to_vec();
    let producer_handle = tokio::spawn(async move {
        for (index, plan_item) in plans_items.into_iter().enumerate() {
            if job_tx.send((index, plan_item)).await.is_err() {
                break;
            }
        }
    });
    worker_handles.push(producer_handle);
    drop(progress_tx);

    let mut ctrl_c = std::pin::pin!(tokio::signal::ctrl_c());
    let mut total_processed = 0;
    while total_processed < total {
        tokio::select! {
            biased;
            _ = &mut ctrl_c => {
                for handle in &worker_handles {
                    handle.abort();
                }
                return Err(CliError::Cancelled(format!(
                    "Batch transcription cancelled by user ({succeeded}/{total} completed)"
                )));
            }
            recv_opt = progress_rx.recv() => {
                match recv_opt {
                    Some(progress) => {
                        total_processed += 1;
                        match progress {
                            BatchJobProgress::Success { index, total, input, output } => {
                                succeeded += 1;
                                if !args.quiet {
                                    writeln!(
                                        io.stderr(),
                                        "[{}/{}] Transcribed {} -> {}",
                                        index + 1,
                                        total,
                                        input.display(),
                                        output.display()
                                    )
                                    .map_err(|e| {
                                        for handle in &worker_handles {
                                            handle.abort();
                                        }
                                        CliError::Io(e.to_string())
                                    })?;
                                }
                            }
                            BatchJobProgress::Failure { index, total, input, error } => {
                                if let Err(e) = writeln!(
                                    io.stderr(),
                                    "[{}/{}] [FAIL] {}: {error}",
                                    index + 1,
                                    total,
                                    input.display()
                                ) {
                                    for handle in &worker_handles {
                                        handle.abort();
                                    }
                                    return Err(CliError::Io(e.to_string()));
                                }
                                if args.continue_on_error {
                                    failures.push((input, error.to_string()));
                                } else {
                                    for handle in &worker_handles {
                                        handle.abort();
                                    }
                                    return Err(error);
                                }
                            }
                        }
                    }
                    None => {
                        for handle in &worker_handles {
                            handle.abort();
                        }
                        return Err(CliError::Other(format!(
                            "Batch transcription channel closed prematurely: completed {total_processed}/{total} tasks"
                        )));
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
        return Err(CliError::Other(msg));
    }

    Ok(CliOutput::stderr(format!(
        "Transcribed {total} file(s) into {}",
        output_dir.display()
    )))
}

async fn run_batch_transcribe(
    args: &TranscribeArgs,
    resolved_online: &crate::online_asr::OnlineAsrArgs,
    config: Option<&TranscribeConfigSection>,
    resolved_jobs: usize,
    resolved_model_id: Option<String>,
    export_mode: sona_core::export::ExportMode,
    io: &mut (dyn crate::CliIo + Send),
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

    if resolved_online.is_online() {
        reject_online_local_options(args)?;
        let language = args
            .asr
            .language
            .clone()
            .or_else(|| config.and_then(|c| c.language.clone()))
            .unwrap_or_else(|| sona_core::transcription::runtime::DEFAULT_LANGUAGE.to_string());
        let enable_itn = args.asr.enable_itn || config.and_then(|c| c.enable_itn).unwrap_or(false);
        let hotwords = args
            .asr
            .hotwords
            .clone()
            .or_else(|| config.and_then(|c| c.hotwords.clone()));
        let request =
            resolved_online.build_request(AsrMode::Batch, language, enable_itn, hotwords)?;

        run_batch_transcribe_dispatched(
            args,
            &plans,
            &output_dir,
            resolved_jobs,
            io,
            move |plan_item| {
                let request = request.clone();
                async move {
                    let segments = crate::asr_adapter::online_batch_transcribe(
                        plan_item.input_path.clone(),
                        request,
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
                }
            },
        )
        .await
    } else {
        let transcriber = Arc::new(crate::asr_adapter::local_batch_transcriber());
        let format_name = match export_format {
            sona_core::export::ExportFormat::Json => "json",
            sona_core::export::ExportFormat::Txt => "txt",
            sona_core::export::ExportFormat::Srt => "srt",
            sona_core::export::ExportFormat::Vtt => "vtt",
            sona_core::export::ExportFormat::Md => "md",
        };

        let resolved_model_id = resolved_model_id.clone();
        let config_clone = config.cloned();
        let args_language = args.asr.language.clone();
        let args_models_dir = args.asr.models_dir.clone();
        let args_vad_model_id = args.asr.vad_model_id.clone();
        let args_punctuation_model_id = args.asr.punctuation_model_id.clone();
        let args_threads = args.asr.threads;
        let args_enable_itn = if args.asr.enable_itn {
            Some(true)
        } else {
            None
        };
        let args_hotwords = args.asr.hotwords.clone();
        let args_gpu_acceleration = args.asr.gpu_acceleration.clone();
        let args_vad_buffer = args.asr.vad_buffer;
        let args_quiet = args.quiet;
        let args_force = args.force;
        let args_ffmpeg_path = args.ffmpeg_path.clone();

        run_batch_transcribe_dispatched(
            args,
            &plans,
            &output_dir,
            resolved_jobs,
            io,
            move |plan_item| {
                let transcriber = transcriber.clone();
                let single_options = BatchTranscribeOptions {
                    input: plan_item.input_path.clone(),
                    output: Some(plan_item.output_path.clone()),
                    format: Some(format_name.to_string()),
                    language: args_language.clone(),
                    model_id: resolved_model_id.clone(),
                    models_dir: args_models_dir.clone(),
                    default_models_dir: crate::desktop_paths::default_models_dir(),
                    vad_model_id: args_vad_model_id.clone(),
                    punctuation_model_id: args_punctuation_model_id.clone(),
                    threads: args_threads,
                    enable_itn: args_enable_itn,
                    hotwords: args_hotwords.clone(),
                    gpu_acceleration: args_gpu_acceleration.clone(),
                    vad_buffer: args_vad_buffer,
                    save_wav: None,
                    quiet: args_quiet,
                    force: args_force,
                    ffmpeg_path: args_ffmpeg_path.clone(),
                };
                let config_clone = config_clone.clone();

                async move {
                    let file_plan = sona_runtime_fs::resolve_batch_transcribe_plan_with_runtime_paths_and_models_dir_status(
                        single_options,
                        config_clone,
                        crate::desktop_paths::models_dir_status,
                    )
                    .map_err(crate::map_runtime_fs_error)?;

                    let segments = transcriber
                        .transcribe(file_plan)
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
                }
            },
        )
        .await
    }
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
        .asr
        .language
        .clone()
        .or_else(|| config.and_then(|config| config.language.clone()))
        .unwrap_or_else(|| sona_core::transcription::runtime::DEFAULT_LANGUAGE.to_string());
    let enable_itn =
        args.asr.enable_itn || config.and_then(|config| config.enable_itn).unwrap_or(false);
    let hotwords = args
        .asr
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
    let segments = tokio::select! {
        res = crate::asr_adapter::online_batch_transcribe(input.to_path_buf(), request) => {
            res.map_err(crate::online_asr::map_asr_error)?
        }
        _ = tokio::signal::ctrl_c() => {
            return Err(CliError::Cancelled("Online transcription cancelled by user".to_string()));
        }
    };
    render_transcription(segments, export_format, output_target, export_mode)
}

fn reject_online_local_options(args: &TranscribeArgs) -> CliResult<()> {
    crate::asr_args::reject_online_local_options(
        &args.asr,
        &[(args.save_wav.is_some(), "--save-wav")],
    )
}

fn validate_online_paths(
    input: &std::path::Path,
    output: Option<&PathBuf>,
    force: bool,
) -> CliResult<()> {
    let input_str = input.to_string_lossy();
    if sona_core::history::is_network_url(&input_str) {
        let trimmed = input_str.trim().to_ascii_lowercase();
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            return Err(CliError::Validation(
                "Online ASR only supports HTTP/HTTPS audio streams".into(),
            ));
        }
    } else {
        match std::fs::metadata(input) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) | Err(_) => {
                return Err(CliError::Validation(format!(
                    "Input file must be an existing file: {}",
                    input.display()
                )));
            }
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

    #[test]
    fn single_transcribe_cancelled_returns_exit_code_130() {
        let local_cancel = CliError::Cancelled("Transcription cancelled by user".to_string());
        assert_eq!(local_cancel.exit_code(), 130);
        assert_eq!(
            local_cancel.to_string(),
            "Cancelled: Transcription cancelled by user"
        );

        let online_cancel =
            CliError::Cancelled("Online transcription cancelled by user".to_string());
        assert_eq!(online_cancel.exit_code(), 130);
        assert_eq!(
            online_cancel.to_string(),
            "Cancelled: Online transcription cancelled by user"
        );
    }
    #[tokio::test]
    async fn single_transcribe_select_simulated_cancellation() {
        // Verify the tokio::select! cancellation branch pattern
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        cancel_tx.send(()).unwrap();

        let result: Result<(), CliError> = tokio::select! {
            _ = async {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                Ok::<(), CliError>(())
            } => Ok(()),
            _ = cancel_rx => {
                Err(CliError::Cancelled("Transcription cancelled by user".to_string()))
            }
        };

        let err = result.unwrap_err();
        assert_eq!(err.exit_code(), 130);
        assert_eq!(
            err.to_string(),
            "Cancelled: Transcription cancelled by user"
        );
    }
    #[tokio::test]
    async fn test_batch_transcribe_concurrency_and_io_isolation() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let temp_dir = tempfile::tempdir().unwrap();
        let file1 = temp_dir.path().join("audio1.wav");
        let file2 = temp_dir.path().join("audio2.wav");
        std::fs::write(&file1, b"test audio 1").unwrap();
        std::fs::write(&file2, b"test audio 2").unwrap();
        let out_dir = temp_dir.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let plans = vec![
            BatchOutputPlan {
                input_path: file1.clone(),
                output_path: out_dir.join("audio1.json"),
            },
            BatchOutputPlan {
                input_path: file2.clone(),
                output_path: out_dir.join("audio2.json"),
            },
        ];

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let active_clone = active.clone();
        let peak_clone = peak.clone();

        let args = TranscribeArgs {
            inputs: vec![file1, file2],
            input_dir: None,
            output_dir: Some(out_dir.clone()),
            recursive: false,
            output: None,
            format: None,
            mode: "source".to_string(),
            asr: crate::asr_args::CommonAsrArgs::default(),
            save_wav: None,
            quiet: false,
            force: true,
            continue_on_error: false,
            ffmpeg_path: None,
            config: None,
            list_providers: false,
            jobs: Some(2),
        };

        let mut io = crate::MemoryCliIo::default();
        let result =
            run_batch_transcribe_dispatched(&args, &plans, &out_dir, 2, &mut io, move |_plan| {
                let active = active_clone.clone();
                let peak = peak_clone.clone();
                async move {
                    let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(cur, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .await;

        assert!(
            result.is_ok(),
            "Batch transcribe should succeed: {result:?}"
        );
        let output = result.unwrap();
        let captured = io.into_output();

        // Standard out should be empty (strict IO isolation)
        assert_eq!(captured.stdout, "", "Nothing should leak onto stdout");
        assert_eq!(output.stdout, "", "CliOutput stdout must be empty");

        // Stderr should contain progress for both jobs
        assert!(
            captured.stderr.contains("[1/2] Transcribed"),
            "Stderr must report job 1: {}",
            captured.stderr
        );
        assert!(
            captured.stderr.contains("[2/2] Transcribed"),
            "Stderr must report job 2: {}",
            captured.stderr
        );
        assert!(
            output.stderr.contains("Transcribed 2 file(s) into"),
            "Output stderr must report summary: {}",
            output.stderr
        );

        // Peak concurrency with --jobs 2 must reach 2
        assert_eq!(
            peak.load(Ordering::SeqCst),
            2,
            "Concurrent execution with jobs=2 must execute 2 tasks in parallel"
        );
    }

    #[tokio::test]
    async fn test_batch_transcribe_sequential_with_jobs_1() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let temp_dir = tempfile::tempdir().unwrap();
        let file1 = temp_dir.path().join("audio1.wav");
        let file2 = temp_dir.path().join("audio2.wav");
        std::fs::write(&file1, b"test audio 1").unwrap();
        std::fs::write(&file2, b"test audio 2").unwrap();
        let out_dir = temp_dir.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let plans = vec![
            BatchOutputPlan {
                input_path: file1.clone(),
                output_path: out_dir.join("audio1.json"),
            },
            BatchOutputPlan {
                input_path: file2.clone(),
                output_path: out_dir.join("audio2.json"),
            },
        ];

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let active_clone = active.clone();
        let peak_clone = peak.clone();

        let args = TranscribeArgs {
            inputs: vec![file1, file2],
            input_dir: None,
            output_dir: Some(out_dir.clone()),
            recursive: false,
            output: None,
            format: None,
            mode: "source".to_string(),
            asr: crate::asr_args::CommonAsrArgs::default(),
            save_wav: None,
            quiet: false,
            force: true,
            continue_on_error: false,
            ffmpeg_path: None,
            config: None,
            list_providers: false,
            jobs: Some(1),
        };

        let mut io = crate::MemoryCliIo::default();
        let result =
            run_batch_transcribe_dispatched(&args, &plans, &out_dir, 1, &mut io, move |_plan| {
                let active = active_clone.clone();
                let peak = peak_clone.clone();
                async move {
                    let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(cur, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(
            peak.load(Ordering::SeqCst),
            1,
            "Execution with jobs=1 must be strictly sequential"
        );
    }

    #[tokio::test]
    async fn test_batch_transcribe_partial_failure_with_continue_on_error_returns_cli_error_other()
    {
        let temp_dir = tempfile::tempdir().unwrap();
        let file1 = temp_dir.path().join("audio1.wav");
        let file2 = temp_dir.path().join("audio2.wav");
        std::fs::write(&file1, b"test audio 1").unwrap();
        std::fs::write(&file2, b"test audio 2").unwrap();
        let out_dir = temp_dir.path().join("out");
        std::fs::create_dir_all(&out_dir).unwrap();

        let plans = vec![
            BatchOutputPlan {
                input_path: file1.clone(),
                output_path: out_dir.join("audio1.json"),
            },
            BatchOutputPlan {
                input_path: file2.clone(),
                output_path: out_dir.join("audio2.json"),
            },
        ];

        let args = TranscribeArgs {
            inputs: vec![file1.clone(), file2.clone()],
            input_dir: None,
            output_dir: Some(out_dir.clone()),
            recursive: false,
            output: None,
            format: None,
            mode: "source".to_string(),
            asr: crate::asr_args::CommonAsrArgs::default(),
            save_wav: None,
            quiet: false,
            force: true,
            continue_on_error: true,
            ffmpeg_path: None,
            config: None,
            list_providers: false,
            jobs: Some(1),
        };

        let mut io = crate::MemoryCliIo::default();
        let result = run_batch_transcribe_dispatched(
            &args,
            &plans,
            &out_dir,
            1,
            &mut io,
            move |plan| async move {
                if plan.input_path.ends_with("audio1.wav") {
                    Ok(())
                } else {
                    Err(CliError::Other("corrupt audio frames".to_string()))
                }
            },
        )
        .await;

        assert!(
            result.is_err(),
            "Batch transcription with failures should return Err"
        );
        let error = result.unwrap_err();
        assert!(
            matches!(error, CliError::Other(_)),
            "Partial failure must return CliError::Other, got: {error:?}"
        );
        assert_eq!(
            error.exit_code(),
            1,
            "Exit code must be 1 (runtime failure), not 2 (validation)"
        );
        assert!(
            error.to_string().contains("Transcribed 1/2 file(s)"),
            "Error message must report succeeded count: {error}"
        );
        assert!(
            error.to_string().contains("1 file(s) failed"),
            "Error message must report failed count: {error}"
        );

        let captured = io.into_output();
        assert!(captured.stderr.contains("[1/2] Transcribed"));
        assert!(captured.stderr.contains("[2/2] [FAIL]"));
    }

    #[test]
    fn test_validate_online_paths_network_url() {
        use std::path::Path;
        // HTTP URL succeeds
        assert!(
            validate_online_paths(Path::new("http://example.com/audio.mp3"), None, false).is_ok()
        );
        assert!(
            validate_online_paths(Path::new("https://example.com/stream"), None, false).is_ok()
        );

        // RTMP/RTSP fails
        let err_rtmp = validate_online_paths(Path::new("rtmp://live.example.com/app"), None, false)
            .unwrap_err();
        assert!(
            err_rtmp
                .to_string()
                .contains("Online ASR only supports HTTP/HTTPS audio streams")
        );
        let err_rtsp =
            validate_online_paths(Path::new("rtsp://192.168.1.1/live"), None, false).unwrap_err();
        assert!(
            err_rtsp
                .to_string()
                .contains("Online ASR only supports HTTP/HTTPS audio streams")
        );

        // Non-existent local file fails
        let err_missing =
            validate_online_paths(Path::new("non_existent_file.wav"), None, false).unwrap_err();
        assert!(
            err_missing
                .to_string()
                .contains("Input file must be an existing file")
        );
    }
}
