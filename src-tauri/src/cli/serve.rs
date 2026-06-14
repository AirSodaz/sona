use crate::cli::models::resolve_models_dir;
use crate::cli::{DEFAULT_GPU_ACCELERATION, resolve_cli_gpu_acceleration};
use clap::Args;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_SERVE_PORT: u16 = 14200;
const DEFAULT_SERVE_HOST: &str = "0.0.0.0";
const DEFAULT_SERVE_IP_WHITELIST: &str = "localhost";
const DEFAULT_MAX_CONCURRENT: usize = 2;
const DEFAULT_MAX_QUEUE_SIZE: usize = 100;
const DEFAULT_MAX_UPLOAD_SIZE_MB: usize = 50;
const DEFAULT_JOB_TTL_MINUTES: u64 = 60;
const DEFAULT_MAX_STREAMING: usize = 2;

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

pub async fn run_serve(args: ServeArgs) -> Result<(), String> {
    let config = match args.config.as_deref() {
        Some(path) => Some(load_serve_config_file(path)?),
        None => None,
    };
    let resolved = resolve_serve_options(args, config)?;
    let temp_dir = std::env::temp_dir().join("sona_api");
    let (_tx, rx) = tokio::sync::oneshot::channel();
    let parsed_whitelist = match crate::app::server::parse_ip_whitelist(&resolved.ip_whitelist) {
        Ok(nets) => nets,
        Err(e) => return Err(format!("Failed to parse IP whitelist: {e}")),
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
        transcription_defaults: resolved.transcription_defaults,
        shutdown_rx: rx,
    })
    .await
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ServeConfigFile {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub api_key: Option<String>,
    pub models_dir: Option<PathBuf>,
    pub ip_whitelist: Option<String>,
    pub max_streaming: Option<usize>,
    pub max_concurrent: Option<usize>,
    pub max_queue_size: Option<usize>,
    pub max_upload_size_mb: Option<usize>,
    pub job_ttl_minutes: Option<u64>,
    pub gpu_acceleration: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedServeOptions {
    pub host: String,
    pub port: u16,
    pub api_key: String,
    pub models_dir: PathBuf,
    pub ip_whitelist: String,
    pub max_streaming: usize,
    pub max_concurrent: usize,
    pub max_queue_size: usize,
    pub max_upload_size_mb: usize,
    pub job_ttl_minutes: u64,
    pub transcription_defaults: crate::app::server::ApiServerTranscriptionDefaults,
}

pub fn resolve_serve_options(
    args: ServeArgs,
    config: Option<ServeConfigFile>,
) -> Result<ResolvedServeOptions, String> {
    let config = config.unwrap_or_default();
    let gpu_acceleration = resolve_cli_gpu_acceleration(
        args.gpu_acceleration
            .or(config.gpu_acceleration)
            .or_else(|| Some(DEFAULT_GPU_ACCELERATION.to_string())),
    )?;

    Ok(ResolvedServeOptions {
        host: args
            .host
            .or(config.host)
            .unwrap_or_else(|| DEFAULT_SERVE_HOST.to_string()),
        port: args.port.or(config.port).unwrap_or(DEFAULT_SERVE_PORT),
        api_key: args.api_key.or(config.api_key).unwrap_or_default(),
        models_dir: resolve_models_dir(args.models_dir.or(config.models_dir))?,
        ip_whitelist: args
            .ip_whitelist
            .or(config.ip_whitelist)
            .unwrap_or_else(|| DEFAULT_SERVE_IP_WHITELIST.to_string()),
        max_streaming: args
            .max_streaming
            .or(config.max_streaming)
            .unwrap_or(DEFAULT_MAX_STREAMING),
        max_concurrent: args
            .max_concurrent
            .or(config.max_concurrent)
            .unwrap_or(DEFAULT_MAX_CONCURRENT),
        max_queue_size: args
            .max_queue_size
            .or(config.max_queue_size)
            .unwrap_or(DEFAULT_MAX_QUEUE_SIZE),
        max_upload_size_mb: args
            .max_upload_size_mb
            .or(config.max_upload_size_mb)
            .unwrap_or(DEFAULT_MAX_UPLOAD_SIZE_MB),
        job_ttl_minutes: args
            .job_ttl_minutes
            .or(config.job_ttl_minutes)
            .unwrap_or(DEFAULT_JOB_TTL_MINUTES),
        transcription_defaults: crate::app::server::ApiServerTranscriptionDefaults {
            gpu_acceleration,
            vad_model_id: args.vad_model_id.or(config.vad_model_id).or_else(|| {
                Some(crate::core::preset_models::DEFAULT_SILERO_VAD_MODEL_ID.to_string())
            }),
            punctuation_model_id: args
                .punctuation_model_id
                .or(config.punctuation_model_id)
                .or_else(|| {
                    Some(crate::core::preset_models::DEFAULT_PUNCTUATION_MODEL_ID.to_string())
                }),
        },
    })
}

pub fn load_serve_config_file(path: &Path) -> Result<ServeConfigFile, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read config file {}: {error}", path.display()))?;
    toml::from_str(&contents)
        .map_err(|error| format!("Failed to parse config file {}: {error}", path.display()))
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
            Some(ServeConfigFile {
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
            Some(ServeConfigFile {
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
