use serde::Deserialize;
use std::path::PathBuf;

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

    // Transcribe legacy flat keys
    pub model_id: Option<String>,
    pub language: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub hotwords: Option<String>,
    pub quiet: Option<bool>,
    pub jobs: Option<usize>,
    pub vad_buffer_size: Option<f32>,
    pub format: Option<String>,

    // Serve legacy flat keys
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_fields_act_as_fallback_for_transcribe() {
        let toml_str = r#"
        models_dir = "/shared/models"
        gpu_acceleration = "cuda"

        [transcribe]
        model_id = "test-model"
        # gpu_acceleration is omitted, should fall back
        "#;

        let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();
        let config = unified.into_transcribe_config();

        assert_eq!(config.models_dir, Some(PathBuf::from("/shared/models")));
        assert_eq!(config.gpu_acceleration, Some("cuda".to_string()));
        assert_eq!(config.model_id, Some("test-model".to_string()));
    }

    #[test]
    fn local_fields_override_shared_for_serve() {
        let toml_str = r#"
        port = 14200
        gpu_acceleration = "cuda"

        [serve]
        port = 15000
        gpu_acceleration = "coreml"
        "#;

        let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();
        let config = unified.into_serve_config();

        assert_eq!(config.port, Some(15000));
        assert_eq!(config.gpu_acceleration, Some("coreml".to_string()));
    }

    #[test]
    fn flat_config_maintains_backward_compatibility() {
        let toml_str = r#"
        models_dir = "/legacy/models"
        model_id = "legacy-model"
        host = "0.0.0.0"
        port = 8080
        "#;

        let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();

        let transcribe = unified.clone().into_transcribe_config();
        assert_eq!(transcribe.models_dir, Some(PathBuf::from("/legacy/models")));
        assert_eq!(transcribe.model_id, Some("legacy-model".to_string()));

        let serve = unified.into_serve_config();
        assert_eq!(serve.models_dir, Some(PathBuf::from("/legacy/models")));
        assert_eq!(serve.host, Some("0.0.0.0".to_string()));
        assert_eq!(serve.port, Some(8080));
    }
}
