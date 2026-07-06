use crate::paths::default_desktop_models_dir;
use crate::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub use crate::gpu::{
    DEFAULT_GPU_ACCELERATION, GPU_ACCELERATION_VALUES,
    resolve_gpu_acceleration as resolve_cli_gpu_acceleration,
};
pub const DEFAULT_SERVE_PORT: u16 = 14200;
pub const DEFAULT_SERVE_HOST: &str = "127.0.0.1";
pub const DEFAULT_SERVE_IP_WHITELIST: &str = "localhost";
pub const DEFAULT_MAX_CONCURRENT: usize = 2;
pub const DEFAULT_MAX_QUEUE_SIZE: usize = 100;
pub const DEFAULT_MAX_UPLOAD_SIZE_MB: usize = 50;
pub const DEFAULT_JOB_TTL_MINUTES: u64 = 60;
pub const DEFAULT_MAX_STREAMING: usize = 2;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UnifiedConfigFile {
    #[serde(flatten)]
    pub shared: SharedConfig,

    pub transcribe: Option<TranscribeConfigSection>,
    pub serve: Option<ServeConfigSection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SharedConfig {
    pub models_dir: Option<PathBuf>,
    pub gpu_acceleration: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,

    pub model_id: Option<String>,
    pub language: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub hotwords: Option<String>,
    pub quiet: Option<bool>,
    pub jobs: Option<usize>,
    pub vad_buffer_size: Option<f32>,
    pub format: Option<String>,

    pub host: Option<String>,
    pub port: Option<u16>,
    pub api_key: Option<String>,
    pub ip_whitelist: Option<String>,
    pub max_streaming: Option<usize>,
    pub max_concurrent: Option<usize>,
    pub max_queue_size: Option<usize>,
    pub max_upload_size_mb: Option<usize>,
    pub job_ttl_minutes: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TranscribeConfigSection {
    pub models_dir: Option<PathBuf>,
    pub model_id: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
    pub language: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub hotwords: Option<String>,
    pub quiet: Option<bool>,
    pub jobs: Option<usize>,
    pub vad_buffer_size: Option<f32>,
    pub format: Option<String>,
    pub gpu_acceleration: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServeConfigSection {
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

impl UnifiedConfigFile {
    pub fn into_transcribe_config(self) -> TranscribeConfigSection {
        let mut config = self.transcribe.unwrap_or_default();
        config.models_dir = config.models_dir.or(self.shared.models_dir);
        config.model_id = config.model_id.or(self.shared.model_id);
        config.vad_model_id = config.vad_model_id.or(self.shared.vad_model_id);
        config.punctuation_model_id = config
            .punctuation_model_id
            .or(self.shared.punctuation_model_id);
        config.language = config.language.or(self.shared.language);
        config.threads = config.threads.or(self.shared.threads);
        config.enable_itn = config.enable_itn.or(self.shared.enable_itn);
        config.hotwords = config.hotwords.or(self.shared.hotwords);
        config.quiet = config.quiet.or(self.shared.quiet);
        config.jobs = config.jobs.or(self.shared.jobs);
        config.vad_buffer_size = config.vad_buffer_size.or(self.shared.vad_buffer_size);
        config.format = config.format.or(self.shared.format);
        config.gpu_acceleration = config.gpu_acceleration.or(self.shared.gpu_acceleration);
        config
    }

    pub fn into_serve_config(self) -> ServeConfigSection {
        let mut config = self.serve.unwrap_or_default();
        config.host = config.host.or(self.shared.host);
        config.port = config.port.or(self.shared.port);
        config.api_key = config.api_key.or(self.shared.api_key);
        config.models_dir = config.models_dir.or(self.shared.models_dir);
        config.ip_whitelist = config.ip_whitelist.or(self.shared.ip_whitelist);
        config.max_streaming = config.max_streaming.or(self.shared.max_streaming);
        config.max_concurrent = config.max_concurrent.or(self.shared.max_concurrent);
        config.max_queue_size = config.max_queue_size.or(self.shared.max_queue_size);
        config.max_upload_size_mb = config.max_upload_size_mb.or(self.shared.max_upload_size_mb);
        config.job_ttl_minutes = config.job_ttl_minutes.or(self.shared.job_ttl_minutes);
        config.gpu_acceleration = config.gpu_acceleration.or(self.shared.gpu_acceleration);
        config.vad_model_id = config.vad_model_id.or(self.shared.vad_model_id);
        config.punctuation_model_id = config
            .punctuation_model_id
            .or(self.shared.punctuation_model_id);
        config
    }
}

pub fn resolve_cli_models_dir(configured: Option<PathBuf>) -> Result<PathBuf, String> {
    let path = if let Some(path) = configured {
        path
    } else {
        default_desktop_models_dir().ok_or_else(|| {
            "Unable to infer the desktop models directory. Pass --models-dir explicitly."
                .to_string()
        })?
    };

    if std::fs::metadata(&path)
        .map(|metadata| !metadata.is_dir())
        .unwrap_or(false)
    {
        return Err(format!(
            "Models directory '{}' exists but is not a directory.",
            path.display()
        ));
    }

    Ok(path)
}

pub fn load_serve_config_file(path: &Path) -> Result<ServeConfigSection, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read config file {}: {error}", path.display()))?;
    let unified: UnifiedConfigFile = toml::from_str(&contents)
        .map_err(|error| format!("Failed to parse config file {}: {error}", path.display()))?;
    Ok(unified.into_serve_config())
}

pub fn resolve_serve_runtime_options(
    args: ServeRuntimeArgs,
    config: Option<ServeConfigSection>,
) -> Result<ResolvedServeRuntimeOptions, String> {
    let config = config.unwrap_or_default();
    let gpu_acceleration =
        resolve_cli_gpu_acceleration(args.gpu_acceleration.or(config.gpu_acceleration))?;

    Ok(ResolvedServeRuntimeOptions {
        host: args
            .host
            .or(config.host)
            .unwrap_or_else(|| DEFAULT_SERVE_HOST.to_string()),
        port: args.port.or(config.port).unwrap_or(DEFAULT_SERVE_PORT),
        api_key: args.api_key.or(config.api_key).unwrap_or_default(),
        models_dir: resolve_cli_models_dir(args.models_dir.or(config.models_dir))?,
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
