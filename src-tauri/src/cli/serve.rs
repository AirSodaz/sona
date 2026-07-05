use crate::cli::{CliError, CliResult};
use clap::Args;
use sona_core::cli_runtime::{
    ResolvedServeRuntimeOptions, ServeRuntimeArgs, load_serve_config_file,
    resolve_serve_runtime_options,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Path to a TOML config file.
    #[arg(
        short = 'c',
        long,
        help = "Load default options from a TOML config file"
    )]
    config: Option<PathBuf>,
    #[arg(long, help = "TCP port for the HTTP API server")]
    port: Option<u16>,
    #[arg(long, help = "Bind address for the HTTP API server")]
    host: Option<String>,
    #[arg(long, help = "Bearer API key required by HTTP requests")]
    api_key: Option<String>,
    /// Models directory containing installed presets.
    #[arg(
        long,
        help = "Override the models directory used to resolve installed models"
    )]
    models_dir: Option<PathBuf>,
    #[arg(
        long,
        help = "Allowed client IP rules: localhost, exact IP, CIDR, *, or IPv4 wildcards"
    )]
    ip_whitelist: Option<String>,
    /// Maximum concurrent streaming WebSocket connections.
    #[arg(long)]
    max_streaming: Option<usize>,
    /// Maximum concurrent batch transcription jobs.
    #[arg(long)]
    max_concurrent: Option<usize>,
    /// Maximum queued batch transcription jobs. 0 means effectively unlimited.
    #[arg(long)]
    max_queue_size: Option<usize>,
    /// Maximum upload size in MB. 0 disables the upload size limit.
    #[arg(long)]
    max_upload_size_mb: Option<usize>,
    /// Minutes to keep completed or failed jobs. 0 disables cleanup.
    #[arg(long)]
    job_ttl_minutes: Option<u64>,
    /// Recognizer GPU acceleration provider.
    #[arg(
        long,
        value_name = "PROVIDER",
        help = "Recognizer GPU acceleration provider: auto, cpu, cuda, coreml, or directml"
    )]
    gpu_acceleration: Option<String>,
    /// VAD companion model id used by API server jobs.
    #[arg(long, value_name = "MODEL_ID")]
    vad_model_id: Option<String>,
    /// Punctuation companion model id used by API server jobs.
    #[arg(long, value_name = "MODEL_ID")]
    punctuation_model_id: Option<String>,
}

pub async fn run_serve(args: ServeArgs) -> CliResult<()> {
    let config = match args.config.as_deref() {
        Some(path) => Some(load_serve_config_file(path).map_err(CliError::Validation)?),
        None => None,
    };
    let resolved = resolve_serve_options(args, config)?;

    if resolved.api_key.is_empty() && resolved.host != "127.0.0.1" && resolved.host != "localhost" {
        eprintln!(
            "WARNING: API server is binding to {} without an API key. \
             Any client that can reach this address may submit requests. \
             Pass --api-key to require authentication, or --host 127.0.0.1 to restrict access.",
            resolved.host
        );
    }

    let temp_dir = std::env::temp_dir().join(format!("sona_api_{}", std::process::id()));
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Spawn signal handler task for graceful shutdown
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let sigint = signal(SignalKind::interrupt());
            let sigterm = signal(SignalKind::terminate());

            match (sigint, sigterm) {
                (Ok(mut sigint), Ok(mut sigterm)) => {
                    tokio::select! {
                        _ = sigint.recv() => {
                            eprintln!("\nReceived SIGINT (Ctrl+C), starting graceful shutdown...");
                        }
                        _ = sigterm.recv() => {
                            eprintln!("\nReceived SIGTERM, starting graceful shutdown...");
                        }
                    }
                    let _ = tx.send(());
                }
                (sigint_res, sigterm_res) => {
                    if let Err(e) = sigint_res {
                        eprintln!("Failed to install SIGINT handler: {e}");
                    }
                    if let Err(e) = sigterm_res {
                        eprintln!("Failed to install SIGTERM handler: {e}");
                    }
                    // Keep the task alive indefinitely so `tx` is not dropped,
                    // allowing the server to run without the signal handlers.
                    std::future::pending::<()>().await;
                }
            }
        }

        #[cfg(not(unix))]
        {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    eprintln!("\nReceived Ctrl+C, starting graceful shutdown...");
                    let _ = tx.send(());
                }
                Err(e) => {
                    eprintln!("Failed to install Ctrl+C handler: {e}");
                    // Keep the task alive indefinitely so `tx` is not dropped,
                    // allowing the server to run without the signal handlers.
                    std::future::pending::<()>().await;
                }
            }
        }
    });
    let parsed_whitelist = match crate::app::server::parse_ip_whitelist(&resolved.ip_whitelist) {
        Ok(nets) => nets,
        Err(e) => {
            return Err(CliError::Validation(format!(
                "Failed to parse IP whitelist: {e}"
            )));
        }
    };
    let parsed_arc = Arc::new(parsed_whitelist);

    crate::app::server::run_server(crate::app::server::ApiServerRuntimeConfig {
        app: None,
        host: resolved.host,
        port: resolved.port,
        api_key: resolved.api_key,
        temp_dir,
        models_dir: resolved.models_dir,
        max_concurrent: resolved.max_concurrent,
        max_queue_size: resolved.max_queue_size,
        max_upload_size_mb: resolved.max_upload_size_mb,
        job_ttl_minutes: resolved.job_ttl_minutes,
        max_streaming: resolved.max_streaming,
        ip_whitelist: parsed_arc,
        online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        transcription_defaults: crate::app::server::ApiServerTranscriptionDefaults {
            gpu_acceleration: resolved.transcription_defaults.gpu_acceleration,
            vad_model_id: resolved.transcription_defaults.vad_model_id,
            punctuation_model_id: resolved.transcription_defaults.punctuation_model_id,
        },
        shutdown_rx: rx,
        bind_tx: None,
    })
    .await
    .map_err(CliError::Other)?;

    Ok(())
}

pub fn resolve_serve_options(
    args: ServeArgs,
    config: Option<crate::cli::config::ServeConfigSection>,
) -> CliResult<ResolvedServeRuntimeOptions> {
    resolve_serve_runtime_options(
        ServeRuntimeArgs {
            host: args.host,
            port: args.port,
            api_key: args.api_key,
            models_dir: args.models_dir,
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
    .map_err(CliError::Validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn serve_args() -> ServeArgs {
        ServeArgs {
            config: None,
            port: None,
            host: None,
            api_key: None,
            models_dir: None,
            ip_whitelist: None,
            max_streaming: None,
            max_concurrent: None,
            max_queue_size: None,
            max_upload_size_mb: None,
            job_ttl_minutes: None,
            gpu_acceleration: None,
            vad_model_id: None,
            punctuation_model_id: None,
        }
    }

    #[test]
    fn serve_config_file_values_are_used() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();
        let mut args = serve_args();
        args.models_dir = Some(models_dir.clone());

        let resolved = resolve_serve_options(
            args,
            Some(crate::cli::config::ServeConfigSection {
                host: Some("127.0.0.1".to_string()),
                port: Some(15000),
                api_key: Some("secret".to_string()),
                ip_whitelist: Some("localhost,192.168.1.*".to_string()),
                max_streaming: Some(4),
                max_concurrent: Some(3),
                max_queue_size: Some(7),
                max_upload_size_mb: Some(88),
                job_ttl_minutes: Some(9),
                gpu_acceleration: Some("cuda".to_string()),
                vad_model_id: Some("silero-vad".to_string()),
                punctuation_model_id: Some("punct".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.host, "127.0.0.1");
        assert_eq!(resolved.port, 15000);
        assert_eq!(resolved.api_key, "secret");
        assert_eq!(resolved.models_dir, models_dir);
        assert_eq!(resolved.ip_whitelist, "localhost,192.168.1.*");
        assert_eq!(resolved.max_streaming, 4);
        assert_eq!(resolved.max_concurrent, 3);
        assert_eq!(resolved.max_queue_size, 7);
        assert_eq!(resolved.max_upload_size_mb, 88);
        assert_eq!(resolved.job_ttl_minutes, 9);
        assert_eq!(
            resolved.transcription_defaults.gpu_acceleration.as_deref(),
            Some("cuda")
        );
        assert_eq!(
            resolved.transcription_defaults.vad_model_id.as_deref(),
            Some("silero-vad")
        );
        assert_eq!(
            resolved
                .transcription_defaults
                .punctuation_model_id
                .as_deref(),
            Some("punct")
        );
    }

    #[test]
    fn serve_cli_values_override_config_file_values() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();
        let mut args = serve_args();
        args.models_dir = Some(models_dir.clone());
        args.host = Some("0.0.0.0".to_string());
        args.gpu_acceleration = Some("cpu".to_string());
        args.max_concurrent = Some(11);

        let resolved = resolve_serve_options(
            args,
            Some(crate::cli::config::ServeConfigSection {
                host: Some("127.0.0.1".to_string()),
                gpu_acceleration: Some("cuda".to_string()),
                max_concurrent: Some(2),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.host, "0.0.0.0");
        assert_eq!(resolved.max_concurrent, 11);
        assert_eq!(
            resolved.transcription_defaults.gpu_acceleration.as_deref(),
            Some("cpu")
        );
    }
}
