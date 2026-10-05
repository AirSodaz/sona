use serde::Deserialize;
use std::path::PathBuf;

use super::error::RuntimeConfigError;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UnifiedConfigFile {
    #[serde(flatten)]
    pub shared: SharedConfig,

    pub transcribe: Option<TranscribeConfigSection>,
    #[serde(alias = "live")]
    pub transcribe_live: Option<TranscribeLiveConfigSection>,
    pub serve: Option<ServeConfigSection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SharedConfig {
    pub models_dir: Option<PathBuf>,
    pub gpu_acceleration: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
    pub online_provider: Option<String>,
    pub api_key_env: Option<String>,
    pub online_config: Option<PathBuf>,
    pub ffmpeg_path: Option<String>,

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
    pub online_provider: Option<String>,
    pub api_key_env: Option<String>,
    pub online_config: Option<PathBuf>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub hotwords: Option<String>,
    pub quiet: Option<bool>,
    pub jobs: Option<usize>,
    pub vad_buffer_size: Option<f32>,
    pub format: Option<String>,
    pub gpu_acceleration: Option<String>,
    pub ffmpeg_path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TranscribeLiveConfigSection {
    pub models_dir: Option<PathBuf>,
    pub model_id: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
    pub language: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub hotwords: Option<String>,
    pub vad_buffer_size: Option<f32>,
    pub online_provider: Option<String>,
    pub api_key_env: Option<String>,
    pub online_config: Option<PathBuf>,
    pub gpu_acceleration: Option<String>,
    pub input: Option<String>,
    pub device: Option<String>,
    pub duration_seconds: Option<f64>,
    pub stream_format: Option<String>,
    pub output_format: Option<String>,
    pub format: Option<String>,
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
    pub ffmpeg_path: Option<String>,
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
        config.online_provider = config.online_provider.or(self.shared.online_provider);
        config.api_key_env = config.api_key_env.or(self.shared.api_key_env);
        config.online_config = config.online_config.or(self.shared.online_config);
        config.threads = config.threads.or(self.shared.threads);
        config.enable_itn = config.enable_itn.or(self.shared.enable_itn);
        config.hotwords = config.hotwords.or(self.shared.hotwords);
        config.quiet = config.quiet.or(self.shared.quiet);
        config.jobs = config.jobs.or(self.shared.jobs);
        config.vad_buffer_size = config.vad_buffer_size.or(self.shared.vad_buffer_size);
        config.format = config.format.or(self.shared.format);
        config.gpu_acceleration = config.gpu_acceleration.or(self.shared.gpu_acceleration);
        config.ffmpeg_path = config.ffmpeg_path.or(self.shared.ffmpeg_path.clone());
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
        config.ffmpeg_path = config.ffmpeg_path.or(self.shared.ffmpeg_path);
        config
    }

    pub fn into_transcribe_live_config(self) -> TranscribeLiveConfigSection {
        let mut config = self.transcribe_live.unwrap_or_default();
        config.models_dir = config.models_dir.or(self.shared.models_dir);
        config.model_id = config.model_id.or(self.shared.model_id);
        config.vad_model_id = config.vad_model_id.or(self.shared.vad_model_id);
        config.punctuation_model_id = config
            .punctuation_model_id
            .or(self.shared.punctuation_model_id);
        config.language = config.language.or(self.shared.language);
        config.online_provider = config.online_provider.or(self.shared.online_provider);
        config.api_key_env = config.api_key_env.or(self.shared.api_key_env);
        config.online_config = config.online_config.or(self.shared.online_config);
        config.threads = config.threads.or(self.shared.threads);
        config.enable_itn = config.enable_itn.or(self.shared.enable_itn);
        config.hotwords = config.hotwords.or(self.shared.hotwords);
        config.vad_buffer_size = config.vad_buffer_size.or(self.shared.vad_buffer_size);
        config.gpu_acceleration = config.gpu_acceleration.or(self.shared.gpu_acceleration);
        config.stream_format = config.stream_format.or(config.output_format.clone());
        config.format = config.format.or(self.shared.format);
        config
    }
}

pub fn parse_unified_config_file(
    contents: &str,
    source_label: &str,
) -> Result<UnifiedConfigFile, RuntimeConfigError> {
    toml::from_str(contents).map_err(|error| RuntimeConfigError::Parse {
        source_label: source_label.to_string(),
        reason: error.to_string(),
    })
}

pub fn parse_transcribe_config_file(
    contents: &str,
    source_label: &str,
) -> Result<TranscribeConfigSection, RuntimeConfigError> {
    let unified = parse_unified_config_file(contents, source_label)?;
    Ok(unified.into_transcribe_config())
}

pub fn parse_transcribe_live_config_file(
    contents: &str,
    source_label: &str,
) -> Result<TranscribeLiveConfigSection, RuntimeConfigError> {
    let unified = parse_unified_config_file(contents, source_label)?;
    Ok(unified.into_transcribe_live_config())
}

pub fn parse_serve_config_file(
    contents: &str,
    source_label: &str,
) -> Result<ServeConfigSection, RuntimeConfigError> {
    let unified = parse_unified_config_file(contents, source_label)?;
    Ok(unified.into_serve_config())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_merges_online_asr_config() {
        let toml = r#"
online_provider = "shared-provider"
api_key_env = "SHARED_KEY"

[transcribe]
model_id = "test-model"

[transcribe_live]
online_provider = "live-provider"
online_config = "./live-config.json"
"#;
        let transcribe = parse_transcribe_config_file(toml, "test").unwrap();
        assert_eq!(
            transcribe.online_provider.as_deref(),
            Some("shared-provider")
        );
        assert_eq!(transcribe.api_key_env.as_deref(), Some("SHARED_KEY"));
        assert!(transcribe.online_config.is_none());

        let live = parse_transcribe_live_config_file(toml, "test").unwrap();
        assert_eq!(live.online_provider.as_deref(), Some("live-provider"));
        assert_eq!(live.api_key_env.as_deref(), Some("SHARED_KEY"));
        assert_eq!(
            live.online_config,
            Some(PathBuf::from("./live-config.json"))
        );
    }

    #[test]
    fn parses_live_config_section_alias() {
        let toml = r#"
model_id = "shared-model"

[live]
model_id = "test-live"
duration_seconds = 45.0
"#;
        let live = parse_transcribe_live_config_file(toml, "test").unwrap();
        assert_eq!(live.model_id.as_deref(), Some("test-live"));
        assert_eq!(live.duration_seconds, Some(45.0));
    }
}
