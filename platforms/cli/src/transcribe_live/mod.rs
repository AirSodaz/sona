mod config;
mod session;

pub(crate) use config::TranscribeLiveArgs;
pub use session::{
    LiveSessionMetadata, run_live_session, run_live_session_with_save_audio,
    write_final_transcript, write_final_transcript_with_mode,
};

use config::{
    LiveInputSource, ResolvedLiveAsr, ResolvedLiveCommand, load_config, resolve_live_command,
    validate_direct_input_options,
};
use session::{CliStreamingObserver, spawn_stop_signal};

use std::io::Write;
use std::sync::Arc;

use sona_core::ports::asr::AsrRuntimeObserver;

use crate::live_audio::{
    default_microphone_device_name, microphone_device_names, spawn_stdin_reader,
    start_microphone_input,
};
use crate::live_output::{LiveOutputRenderer, LiveStopReason};
use crate::{CliError, CliIo, CliResult};

pub(crate) async fn run_transcribe_live(
    args: TranscribeLiveArgs,
    io: &mut dyn CliIo,
) -> CliResult<()> {
    if args.list_providers {
        let output = crate::transcribe::render_online_providers_table();
        io.stdout()
            .write_all(output.as_bytes())
            .map_err(|error| CliError::Io(format!("Failed to write online providers: {error}")))?;
        io.stdout()
            .flush()
            .map_err(|error| CliError::Io(format!("Failed to flush online providers: {error}")))?;
        return Ok(());
    }

    if args.list_input_devices {
        let devices = microphone_device_names().map_err(CliError::Io)?;
        let default_device = default_microphone_device_name();
        let output = format_input_device_list(&devices, default_device.as_deref());
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
    let stdin_is_terminal = io.stdin_is_terminal();
    let resolved = resolve_live_command(args, config, stdin_is_terminal)?;
    let stdout_is_terminal = io.stdout_is_terminal();
    let (status, saved_audio) = {
        let stdout = io.stdout();
        run_resolved_live_command(resolved, stdout_is_terminal, stdout).await?
    };
    if let Some(path) = saved_audio {
        writeln!(
            io.stderr(),
            "Saved live audio recording to {}",
            path.display()
        )
        .map_err(|error| CliError::Io(format!("Failed to write audio status: {error}")))?;
    }
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
) -> CliResult<(Option<String>, Option<std::path::PathBuf>)> {
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
            start_microphone_input(resolved.device.as_deref()).map_err(|err| {
                CliError::Io(format!(
                    "{err}. Run 'sona-cli devices' to see available microphones."
                ))
            })?
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
    let session_result = run_live_session_with_save_audio(
        session,
        &mut input,
        &mut update_receiver,
        &mut renderer,
        stdout,
        stop_receiver,
        metadata,
        resolved.save_audio.as_deref(),
    )
    .await;

    let (reason, session_err) = match session_result {
        Ok(reason) => (reason, None),
        Err(error) => (LiveStopReason::Eof, Some(error)),
    };

    let status = if let Some(path) = output_path.as_ref() {
        let format = export_format.ok_or_else(|| {
            CliError::Validation(
                "Live plan with output path must include an export format.".to_string(),
            )
        })?;
        match write_final_transcript_with_mode(
            path,
            format,
            renderer.segments(),
            resolved.export_mode,
        ) {
            Ok(status) => Some(status),
            Err(error) => {
                let _ = renderer.write_error(stdout, &error.to_string());
                return Err(error);
            }
        }
    } else {
        None
    };

    if let Some(error) = session_err {
        if let Some(saved) = status.as_ref() {
            let mut stderr = std::io::stderr();
            let _ = writeln!(stderr, "{saved}");
        }
        return Err(error);
    }
    renderer
        .write_stopped(stdout, reason)
        .map_err(CliError::Io)?;
    Ok((status, resolved.save_audio))
}

pub(crate) fn format_input_device_list(devices: &[String], default_device: Option<&str>) -> String {
    sona_audio_capture::format_device_list(devices, default_device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_input_device_list_annotates_default_device() {
        let devices = vec!["Mic A".to_string(), "Mic B".to_string()];
        let output = format_input_device_list(&devices, Some("Mic A"));
        assert_eq!(output, "[0] Mic A [default]\n[1] Mic B\n");
    }

    #[test]
    fn format_input_device_list_without_default() {
        let devices = vec!["Mic A".to_string(), "Mic B".to_string()];
        let output = format_input_device_list(&devices, None);
        assert_eq!(output, "[0] Mic A\n[1] Mic B\n");
    }

    #[test]
    fn format_input_device_list_empty() {
        let output = format_input_device_list(&[], Some("Mic A"));
        assert_eq!(output, "No audio input devices found.\n");
    }

    #[test]
    fn format_input_device_list_sanitizes_control_chars() {
        let devices = vec!["Mic\x1b[31mA\n".to_string()];
        let output = format_input_device_list(&devices, None);
        assert!(!output.contains('\x1b'));
        assert_eq!(output, "[0] Mic\\u{1b}[31mA\\n\n");
    }
}
