use crate::cli::models::resolve_models_dir;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, default_value = "14200")]
    port: u16,
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value = "")]
    api_key: String,
    /// Models directory containing installed presets.
    #[arg(
        long,
        help = "Override the models directory used to resolve installed models"
    )]
    models_dir: Option<PathBuf>,
    #[arg(long, default_value = "localhost")]
    ip_whitelist: String,
    /// Maximum concurrent streaming WebSocket connections.
    #[arg(long, default_value = "2")]
    max_streaming: usize,
}

pub async fn run_serve(args: ServeArgs) -> Result<(), String> {
    let models_dir = resolve_models_dir(args.models_dir)?;
    let temp_dir = std::env::temp_dir().join("sona_api");
    let (_tx, rx) = tokio::sync::oneshot::channel();
    let parsed_whitelist = match crate::app::server::parse_ip_whitelist(&args.ip_whitelist) {
        Ok(nets) => nets,
        Err(e) => {
            eprintln!("Failed to parse IP whitelist: {e}");
            std::process::exit(1);
        }
    };
    let parsed_arc = std::sync::Arc::new(parsed_whitelist);

    crate::app::server::run_server(
        None,
        &args.host,
        args.port,
        &args.api_key,
        temp_dir,
        models_dir,
        2,   // max_concurrent
        100, // max_queue_size
        50,  // max_upload_size_mb
        60,  // job_ttl_minutes
        args.max_streaming,
        parsed_arc,
        std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        rx,
    )
    .await
}
