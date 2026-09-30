use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{CliError, CliOutput, CliResult};
use sona_api_server::{
    ApiServerServiceParts, ApiServerStartError, DefaultApiServerPlatform, RunningApiServer,
    start_api_server_runtime,
};
use sona_core::runtime::config::ServeConfigSection;
use sona_core::runtime::serve::{ServeRuntimeArgs, resolve_serve_runtime_options};

#[derive(Debug, Args)]
#[command(
    about = "Run the shared local HTTP API server",
    after_help = "Examples:\n  sona-cli serve\n  sona-cli serve --host 127.0.0.1 --port 14200\n  sona-cli serve --config ./sona-cli.toml"
)]
pub struct ServeArgs {
    /// Optional config file, usually sona-cli.toml.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    /// Host/IP address to bind.
    #[arg(long)]
    host: Option<String>,
    /// TCP port to bind.
    #[arg(long)]
    port: Option<u16>,
    /// Bearer token required for private endpoints.
    #[arg(long = "api-key")]
    api_key: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long = "models-dir")]
    models_dir: Option<PathBuf>,
    /// Comma-separated IP whitelist, for example localhost,192.168.1.0/24.
    #[arg(long = "ip-whitelist")]
    ip_whitelist: Option<String>,
    /// Maximum concurrent streaming sessions.
    #[arg(long = "max-streaming")]
    max_streaming: Option<usize>,
    /// Maximum concurrent transcription jobs.
    #[arg(long = "max-concurrent")]
    max_concurrent: Option<usize>,
    /// Maximum queued transcription jobs; 0 means effectively unbounded.
    #[arg(long = "max-queue-size")]
    max_queue_size: Option<usize>,
    /// Maximum upload size in MiB; 0 disables the request body limit.
    #[arg(long = "max-upload-size-mb")]
    max_upload_size_mb: Option<usize>,
    /// Completed job retention window in minutes; 0 disables cleanup.
    #[arg(long = "job-ttl-minutes")]
    job_ttl_minutes: Option<u64>,
    /// GPU acceleration mode.
    #[arg(long = "gpu-acceleration")]
    gpu_acceleration: Option<String>,
    /// VAD model id override.
    #[arg(long = "vad-model-id")]
    vad_model_id: Option<String>,
    /// Punctuation model id override.
    #[arg(long = "punctuation-model-id")]
    punctuation_model_id: Option<String>,
}

pub async fn run_serve(
    args: ServeArgs,
    io: &mut (dyn crate::CliIo + Send),
) -> CliResult<CliOutput> {
    let config = load_config(args.config.as_ref())?;
    let temp_dir = default_temp_dir();
    let resolved = resolve_serve_runtime_options(
        ServeRuntimeArgs {
            host: args.host,
            port: args.port,
            api_key: args.api_key,
            models_dir: args.models_dir,
            default_models_dir: crate::desktop_paths::default_models_dir(),
            ip_whitelist: args.ip_whitelist,
            max_streaming: args.max_streaming,
            max_concurrent: args.max_concurrent,
            max_queue_size: args.max_queue_size,
            max_upload_size_mb: args.max_upload_size_mb,
            job_ttl_minutes: args.job_ttl_minutes,
            gpu_acceleration: args.gpu_acceleration,
            vad_model_id: args.vad_model_id,
            punctuation_model_id: args.punctuation_model_id,
            ffmpeg_path: None,
        },
        config,
    )
    .map_err(|error| CliError::Validation(error.to_string()))?;

    let host = resolved.host.clone();
    let port = resolved.port;
    let RunningApiServer {
        normalized_ip_whitelist,
        mut shutdown_tx,
        mut join_handle,
        dashboard,
        ..
    } = start_api_server_runtime(ApiServerServiceParts {
        resolved,
        temp_dir,
        online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        batch_transcriber: Arc::new(crate::asr_adapter::local_batch_transcriber()),
        media_validator: Arc::new(sona_media_detector::MagicNumberMediaFileValidator),
        gpu_availability: Arc::new(sona_sherpa_onnx::gpu::LocalGpuAvailabilityProvider),
        model_catalog: Arc::new(sona_runtime_fs::RuntimeModelCatalogProvider),
        batch_plan_resolver: Arc::new(sona_runtime_fs::RuntimeBatchTranscribePlanResolver),
        platform: Arc::new(DefaultApiServerPlatform),
        streaming_transcriber: Some(crate::asr_adapter::streaming_transcriber()),
        web_dist_dir: None,
    })
    .await
    .map_err(|error| match error {
        ApiServerStartError::Configuration(error) => CliError::Validation(error.to_string()),
        ApiServerStartError::Runtime(error) => CliError::Network(error.to_string()),
    })?;

    writeln!(
        io.stderr(),
        "Serving Sona API on http://{}:{} (allowed clients: {})",
        host,
        port,
        normalized_ip_whitelist
    )
    .map_err(|error| CliError::Io(format!("Failed to write banner: {error}")))?;

    loop {
        tokio::select! {
            ctrl_c = tokio::signal::ctrl_c() => {
                ctrl_c.map_err(|error| CliError::Io(format!("Failed to wait for Ctrl+C: {error}")))?;
                let (processing, pending) = dashboard.active_job_count().await;
                let total_active = processing + pending;
                if total_active > 0 && io.stdin_is_terminal() {
                    write!(
                        io.stderr(),
                        "\nWarning: There are {} active/pending transcription task(s) (processing: {}, pending: {}).\nAre you sure you want to exit? [y/N]: ",
                        total_active, processing, pending
                    )
                    .map_err(|error| CliError::Io(format!("Failed to write confirmation prompt: {error}")))?;
                    io.stderr()
                        .flush()
                        .map_err(|error| CliError::Io(format!("Failed to flush confirmation prompt: {error}")))?;

                    let outcome = {
                        let mut prompt_fut = io.prompt_line_async();
                        tokio::select! {
                            read_res = &mut prompt_fut => {
                                PromptOutcome::Read(read_res)
                            }
                            ctrl_c_again = tokio::signal::ctrl_c() => {
                                ctrl_c_again.map_err(|error| CliError::Io(format!("Failed to wait for Ctrl+C: {error}")))?;
                                PromptOutcome::Forced
                            }
                        }
                    };

                    match evaluate_exit_confirmation(outcome, io.stderr())? {
                        ExitConfirmation::Proceed => {}
                        ExitConfirmation::Cancelled => continue,
                    }
                } else if total_active > 0 {
                    writeln!(
                        io.stderr(),
                        "\nShutting down Sona API server with {} active/pending task(s)...",
                        total_active
                    )
                    .map_err(|error| CliError::Io(format!("Failed to write shutdown notice: {error}")))?;
                }
                if let Some(sender) = shutdown_tx.take() {
                    let _ = sender.send(());
                }
                join_handle
                    .await
                    .map_err(|error| CliError::Other(format!("API server task failed: {error}")))?
                    .map_err(|error| CliError::Other(error.to_string()))?;
                return Ok(CliOutput::stderr("Stopped Sona API server".to_string()));
            }
            result = &mut join_handle => {
                result
                    .map_err(|error| CliError::Other(format!("API server task failed: {error}")))?
                    .map_err(|error| CliError::Other(error.to_string()))?;
                return Ok(CliOutput::stderr("API server stopped".to_string()));
            }
        }
    }
}

fn load_config(path: Option<&PathBuf>) -> CliResult<Option<ServeConfigSection>> {
    let Some(path) = path else {
        return Ok(None);
    };
    sona_runtime_fs::load_serve_config_file(path)
        .map(Some)
        .map_err(|error| CliError::Validation(error.to_string()))
}

fn default_temp_dir() -> PathBuf {
    std::env::temp_dir().join("sona-api-server")
}

#[derive(Debug, PartialEq, Eq)]
enum ExitConfirmation {
    Proceed,
    Cancelled,
}

enum PromptOutcome {
    Read(std::io::Result<Option<String>>),
    Forced,
}

fn evaluate_exit_confirmation(
    outcome: PromptOutcome,
    stderr: &mut (dyn std::io::Write + Send),
) -> CliResult<ExitConfirmation> {
    match outcome {
        PromptOutcome::Read(Ok(Some(line))) => {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes") {
                Ok(ExitConfirmation::Proceed)
            } else {
                writeln!(stderr, "Exit cancelled. Continuing Sona API server...").map_err(
                    |error| CliError::Io(format!("Failed to write exit cancellation: {error}")),
                )?;
                Ok(ExitConfirmation::Cancelled)
            }
        }
        PromptOutcome::Read(Ok(None)) => Ok(ExitConfirmation::Proceed),
        PromptOutcome::Read(Err(error)) => Err(CliError::Io(format!(
            "Failed to read exit confirmation: {error}"
        ))),
        PromptOutcome::Forced => {
            writeln!(stderr, "\nForced exit.").map_err(|error| {
                CliError::Io(format!("Failed to write forced exit notice: {error}"))
            })?;
            Ok(ExitConfirmation::Proceed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn exit_confirmation_confirms_on_y_and_yes() {
        let mut stderr = Vec::new();
        let outcome = PromptOutcome::Read(Ok(Some("y\n".to_string())));
        assert_eq!(
            evaluate_exit_confirmation(outcome, &mut stderr).unwrap(),
            ExitConfirmation::Proceed
        );
        assert!(stderr.is_empty());

        let outcome = PromptOutcome::Read(Ok(Some("YES\n".to_string())));
        assert_eq!(
            evaluate_exit_confirmation(outcome, &mut stderr).unwrap(),
            ExitConfirmation::Proceed
        );
        assert!(stderr.is_empty());
    }

    #[test]
    fn exit_confirmation_cancels_on_n_or_other_input() {
        let mut stderr = Vec::new();
        let outcome = PromptOutcome::Read(Ok(Some("n\n".to_string())));
        assert_eq!(
            evaluate_exit_confirmation(outcome, &mut stderr).unwrap(),
            ExitConfirmation::Cancelled
        );
        assert!(String::from_utf8_lossy(&stderr).contains("Exit cancelled"));

        let mut stderr = Vec::new();
        let outcome = PromptOutcome::Read(Ok(Some("anything else\n".to_string())));
        assert_eq!(
            evaluate_exit_confirmation(outcome, &mut stderr).unwrap(),
            ExitConfirmation::Cancelled
        );
        assert!(String::from_utf8_lossy(&stderr).contains("Exit cancelled"));
    }

    #[test]
    fn exit_confirmation_proceeds_on_eof() {
        let mut stderr = Vec::new();
        let outcome = PromptOutcome::Read(Ok(None));
        assert_eq!(
            evaluate_exit_confirmation(outcome, &mut stderr).unwrap(),
            ExitConfirmation::Proceed
        );
        assert!(stderr.is_empty());
    }

    #[test]
    fn exit_confirmation_propagates_io_error() {
        let mut stderr = Vec::new();
        let outcome = PromptOutcome::Read(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "pipe broken",
        )));
        let error = evaluate_exit_confirmation(outcome, &mut stderr).unwrap_err();
        match error {
            CliError::Io(msg) => assert!(msg.contains("pipe broken")),
            other => panic!("expected CliError::Io, got {other:?}"),
        }
    }

    #[test]
    fn exit_confirmation_proceeds_on_forced_exit() {
        let mut stderr = Vec::new();
        let outcome = PromptOutcome::Forced;
        assert_eq!(
            evaluate_exit_confirmation(outcome, &mut stderr).unwrap(),
            ExitConfirmation::Proceed
        );
        assert!(String::from_utf8_lossy(&stderr).contains("Forced exit"));
    }
}
