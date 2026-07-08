use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{CliError, CliOutput, CliResult};
use sona_api_server::{
    ApiServerRuntimeParts, DefaultApiServerPlatform, prepare_runtime_config, run_server,
};
use sona_core::runtime_config::ServeConfigSection;
use sona_core::serve_runtime::{ServeRuntimeArgs, resolve_serve_runtime_options};

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

pub fn run_serve(args: ServeArgs) -> CliResult<CliOutput> {
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
        },
        config,
    )
    .map_err(CliError::Validation)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Io(format!("Failed to create async runtime: {error}")))?;

    runtime.block_on(async move {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (bind_tx, bind_rx) = tokio::sync::oneshot::channel();
        let host = resolved.host.clone();
        let port = resolved.port;
        let prepared = prepare_runtime_config(ApiServerRuntimeParts {
            resolved,
            temp_dir,
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            shutdown_rx,
            bind_tx: Some(bind_tx),
        })
        .map_err(CliError::Validation)?;
        let normalized_whitelist = prepared.normalized_ip_whitelist.clone();
        let server = run_server(prepared.config);
        let mut server = tokio::spawn(server);

        match bind_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = shutdown_tx.send(());
                let _ = server.await;
                return Err(CliError::Network(error));
            }
            Err(_) => {
                let _ = shutdown_tx.send(());
                let _ = server.await;
                return Err(CliError::Other(
                    "API server failed to start: task terminated prematurely".to_string(),
                ));
            }
        }

        eprintln!(
            "Serving Sona API on http://{}:{} (allowed clients: {})",
            host, port, normalized_whitelist
        );

        tokio::select! {
            ctrl_c = tokio::signal::ctrl_c() => {
                ctrl_c.map_err(|error| CliError::Io(format!("Failed to wait for Ctrl+C: {error}")))?;
                let _ = shutdown_tx.send(());
                server
                    .await
                    .map_err(|error| CliError::Other(format!("API server task failed: {error}")))?
                    .map_err(CliError::Other)?;
                Ok(CliOutput::stderr("Stopped Sona API server".to_string()))
            }
            result = &mut server => {
                result
                    .map_err(|error| CliError::Other(format!("API server task failed: {error}")))?
                    .map_err(CliError::Other)?;
                Ok(CliOutput::stderr("API server stopped".to_string()))
            }
        }
    })
}

fn load_config(path: Option<&PathBuf>) -> CliResult<Option<ServeConfigSection>> {
    let Some(path) = path else {
        return Ok(None);
    };
    sona_runtime_fs::load_serve_config_file(path)
        .map(Some)
        .map_err(CliError::Validation)
}

fn default_temp_dir() -> PathBuf {
    std::env::temp_dir().join("sona-api-server")
}
