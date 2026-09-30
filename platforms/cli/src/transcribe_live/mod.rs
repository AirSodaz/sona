mod config;
mod session;

pub(crate) use config::TranscribeLiveArgs;
pub use session::{LiveSessionMetadata, run_live_session, write_final_transcript};

use config::{
    LiveInputSource, ResolvedLiveAsr, ResolvedLiveCommand, load_config, resolve_live_command,
    validate_direct_input_options,
};
use session::{CliStreamingObserver, spawn_stop_signal};

use std::io::Write;
use std::sync::Arc;

use sona_core::ports::asr::AsrRuntimeObserver;

use crate::live_audio::{microphone_device_names, spawn_stdin_reader, start_microphone_input};
use crate::live_output::LiveOutputRenderer;
use crate::{CliError, CliIo, CliResult};

pub(crate) async fn run_transcribe_live(
    args: TranscribeLiveArgs,
    io: &mut dyn CliIo,
) -> CliResult<()> {
    if args.list_input_devices {
        let devices = microphone_device_names().map_err(CliError::Io)?;
        let output = if devices.is_empty() {
            String::new()
        } else {
            format!("{}\n", devices.join("\n"))
        };
        io.stdout()
            .write_all(output.as_bytes())
            .map_err(|error| CliError::Io(format!("Failed to write input devices: {error}")))?;
        io.stdout()
            .flush()
            .map_err(|error| CliError::Io(format!("Failed to flush input devices: {error}")))?;
        return Ok(());
    }

    validate_direct_input_options(args.input, args.device.as_deref(), args.duration)?;
    let config = load_config(args.config.as_ref())?;
    let resolved = resolve_live_command(args, config)?;
    let stdout_is_terminal = io.stdout_is_terminal();
    let status = {
        let stdout = io.stdout();
        run_resolved_live_command(resolved, stdout_is_terminal, stdout).await?
    };
    if let Some(status) = status {
        writeln!(io.stderr(), "{status}")
            .map_err(|error| CliError::Io(format!("Failed to write live status: {error}")))?;
    }
    Ok(())
}

async fn run_resolved_live_command(
    resolved: ResolvedLiveCommand,
    stdout_is_terminal: bool,
    stdout: &mut (dyn Write + Send),
) -> CliResult<Option<String>> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let (update_sender, mut update_receiver) = tokio::sync::mpsc::unbounded_channel();
    let observer: Arc<dyn AsrRuntimeObserver> = Arc::new(CliStreamingObserver {
        sender: update_sender,
    });
    let (spec, model_id, output_path, export_format) = match resolved.asr {
        ResolvedLiveAsr::Local(plan) => {
            let spec = plan
                .to_streaming_spec()
                .map_err(crate::online_asr::map_asr_error)?;
            (spec, plan.model_id, plan.output_path, plan.export_format)
        }
        ResolvedLiveAsr::Online(plan) => {
            let spec = sona_core::ports::asr::StreamingInferenceSpec::from_request(&plan.request)
                .map_err(crate::online_asr::map_asr_error)?;
            (spec, plan.provider_id, plan.output_path, plan.export_format)
        }
    };
    let transcriber = crate::asr_adapter::streaming_transcriber();
    let session = transcriber
        .create(&session_id, &spec, observer)
        .await
        .map_err(crate::online_asr::map_asr_error)?;
    let mut input = match resolved.input {
        LiveInputSource::Microphone => {
            start_microphone_input(resolved.device.as_deref()).map_err(CliError::Io)?
        }
        LiveInputSource::Stdin => spawn_stdin_reader(std::io::stdin()),
    };
    let stop_receiver = spawn_stop_signal(resolved.duration);
    let metadata = LiveSessionMetadata {
        source: resolved.input.label().to_string(),
        device_name: input.device_name.clone(),
        model_id,
    };
    let mut renderer =
        LiveOutputRenderer::new(resolved.output_format, stdout_is_terminal, &session_id);
    let reason = run_live_session(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        stdout,
        stop_receiver,
        metadata,
    )
    .await?;

    let status = if let Some(path) = output_path.as_ref() {
        let format =
            export_format.expect("live plan with output path must include an export format");
        match write_final_transcript(path, format, renderer.segments()) {
            Ok(status) => Some(status),
            Err(error) => {
                let _ = renderer.write_error(stdout, &error.to_string());
                return Err(error);
            }
        }
    } else {
        None
    };
    renderer
        .write_stopped(stdout, reason)
        .map_err(CliError::Io)?;
    Ok(status)
}
