use crate::models::paths::ModelsDirStatus;
use crate::models::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use crate::runtime::config::ServeConfigSection;
use crate::runtime::gpu::resolve_gpu_acceleration;
use serde_json::Value;
use std::collections::HashMap;
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
    pub default_models_dir: Option<PathBuf>,
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

#[derive(Debug, Clone, Default)]
pub struct ServeStartupSettings {
    pub enabled: bool,
    pub config: ServeConfigSection,
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
    let max_concurrent = args
        .max_concurrent
        .or(config.max_concurrent)
        .unwrap_or(DEFAULT_MAX_CONCURRENT);
    if max_concurrent == 0 {
        return Err("max_concurrent must be greater than 0".to_string());
    }

    Ok(ResolvedServeRuntimeOptions {
        host: args
            .host
            .or(config.host)
            .unwrap_or_else(|| DEFAULT_SERVE_HOST.to_string()),
        port: args.port.or(config.port).unwrap_or(DEFAULT_SERVE_PORT),
        api_key: args.api_key.or(config.api_key).unwrap_or_default(),
        models_dir: crate::models::paths::resolve_models_dir(
            args.models_dir.or(config.models_dir),
            args.default_models_dir,
            |_| ModelsDirStatus::Missing,
        )?,
        ip_whitelist: args
            .ip_whitelist
            .or(config.ip_whitelist)
            .unwrap_or_else(|| DEFAULT_SERVE_IP_WHITELIST.to_string()),
        max_streaming: args
            .max_streaming
            .or(config.max_streaming)
            .unwrap_or(DEFAULT_MAX_STREAMING),
        max_concurrent,
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

pub fn app_config_payload(value: &Value) -> &Value {
    value
        .get("sona-config")
        .filter(|value| value.is_object())
        .or_else(|| value.get("sona_config"))
        .filter(|value| value.is_object())
        .or_else(|| value.get("config"))
        .filter(|value| value.is_object())
        .unwrap_or(value)
}

pub fn app_config_payload_owned(value: Value) -> Value {
    app_config_payload(&value).clone()
}

pub fn online_asr_config_from_app_config(value: &Value) -> HashMap<String, Value> {
    app_config_payload(value)
        .get("asr")
        .and_then(|value| value.get("providers"))
        .and_then(|value| value.get("online"))
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

pub fn serve_startup_settings_from_app_config(value: &Value) -> ServeStartupSettings {
    let config = app_config_payload(value);
    ServeStartupSettings {
        enabled: config
            .get("httpServerEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        config: ServeConfigSection {
            host: string_field(config, "httpServerHost"),
            port: u16_field(config, "httpServerPort"),
            api_key: string_field(config, "httpServerApiKey"),
            models_dir: None,
            ip_whitelist: string_field(config, "httpServerIpWhitelist"),
            max_streaming: usize_field(config, "httpServerMaxStreaming"),
            max_concurrent: usize_field(config, "httpServerMaxConcurrent"),
            max_queue_size: usize_field(config, "httpServerMaxQueueSize"),
            max_upload_size_mb: usize_field(config, "httpServerMaxUploadSizeMB"),
            job_ttl_minutes: u64_field(config, "httpServerJobTtlMinutes"),
            gpu_acceleration: string_field(config, "gpuAcceleration"),
            vad_model_id: None,
            punctuation_model_id: None,
        },
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn u16_field(value: &Value, key: &str) -> Option<u16> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
}

fn usize_field(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}
