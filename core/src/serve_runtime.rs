use crate::gpu::resolve_gpu_acceleration;
use crate::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use crate::runtime_config::ServeConfigSection;
use std::path::PathBuf;

pub const DEFAULT_SERVE_PORT: u16 = 14200;
pub const DEFAULT_SERVE_HOST: &str = "127.0.0.1";
pub const DEFAULT_SERVE_IP_WHITELIST: &str = "localhost";
pub const DEFAULT_MAX_CONCURRENT: usize = 2;
pub const DEFAULT_MAX_QUEUE_SIZE: usize = 100;
pub const DEFAULT_MAX_UPLOAD_SIZE_MB: usize = 50;
pub const DEFAULT_JOB_TTL_MINUTES: u64 = 60;
pub const DEFAULT_MAX_STREAMING: usize = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServeRuntimeArgs {
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServeTranscriptionDefaults {
    pub gpu_acceleration: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedServeRuntimeOptions {
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
    pub transcription_defaults: ServeTranscriptionDefaults,
}

pub fn resolve_serve_runtime_options(
    args: ServeRuntimeArgs,
    config: Option<ServeConfigSection>,
) -> Result<ResolvedServeRuntimeOptions, String> {
    let config = config.unwrap_or_default();
    let gpu_acceleration =
        resolve_gpu_acceleration(args.gpu_acceleration.or(config.gpu_acceleration))?;

    Ok(ResolvedServeRuntimeOptions {
        host: args
            .host
            .or(config.host)
            .unwrap_or_else(|| DEFAULT_SERVE_HOST.to_string()),
        port: args.port.or(config.port).unwrap_or(DEFAULT_SERVE_PORT),
        api_key: args.api_key.or(config.api_key).unwrap_or_default(),
        models_dir: crate::model_paths::resolve_models_dir(args.models_dir.or(config.models_dir))?,
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
        transcription_defaults: ServeTranscriptionDefaults {
            gpu_acceleration,
            vad_model_id: args
                .vad_model_id
                .or(config.vad_model_id)
                .or_else(|| Some(DEFAULT_SILERO_VAD_MODEL_ID.to_string())),
            punctuation_model_id: args
                .punctuation_model_id
                .or(config.punctuation_model_id)
                .or_else(|| Some(DEFAULT_PUNCTUATION_MODEL_ID.to_string())),
        },
    })
}
