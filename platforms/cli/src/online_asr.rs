use clap::{Args, ValueEnum};
use serde_json::{Map, Value};
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrPortError, AsrPortErrorKind, AsrTranscriptionRequest,
    GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID, OnlineAsrProviderRequest,
    VOLCENGINE_DOUBAO_PROVIDER_ID, find_online_asr_provider,
};
use sona_core::transcription::postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};
use std::path::{Path, PathBuf};

use crate::{CliError, CliResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum OnlineAsrProviderArg {
    VolcengineDoubao,
    GroqWhisper,
    MistralVoxtral,
}

impl OnlineAsrProviderArg {
    fn provider_id(self) -> &'static str {
        match self {
            Self::VolcengineDoubao => VOLCENGINE_DOUBAO_PROVIDER_ID,
            Self::GroqWhisper => GROQ_WHISPER_PROVIDER_ID,
            Self::MistralVoxtral => MISTRAL_VOXTRAL_PROVIDER_ID,
        }
    }

    fn default_api_key_env(self) -> &'static str {
        match self {
            Self::VolcengineDoubao => "SONA_VOLCENGINE_ASR_API_KEY",
            Self::GroqWhisper => "GROQ_API_KEY",
            Self::MistralVoxtral => "MISTRAL_API_KEY",
        }
    }
}

#[derive(Clone, Debug, Args)]
pub(crate) struct OnlineAsrArgs {
    /// Use an online ASR provider instead of local Sherpa ASR.
    #[arg(long, value_enum, value_name = "PROVIDER")]
    pub(crate) online_provider: Option<OnlineAsrProviderArg>,
    /// Environment variable containing the online ASR API key.
    #[arg(long, value_name = "NAME", requires = "online_provider")]
    api_key_env: Option<String>,
    /// JSON object overriding non-secret provider endpoint or model settings.
    #[arg(long, value_name = "FILE", requires = "online_provider")]
    online_config: Option<PathBuf>,
}

impl OnlineAsrArgs {
    pub(crate) fn is_online(&self) -> bool {
        self.online_provider.is_some()
    }

    pub(crate) fn build_request(
        &self,
        mode: AsrMode,
        language: String,
        enable_itn: bool,
        hotwords: Option<String>,
    ) -> CliResult<AsrTranscriptionRequest> {
        self.build_request_with(mode, language, enable_itn, hotwords, |name| {
            std::env::var(name).map_err(|_| ())
        })
    }

    fn build_request_with<F>(
        &self,
        mode: AsrMode,
        language: String,
        enable_itn: bool,
        hotwords: Option<String>,
        read_env: F,
    ) -> CliResult<AsrTranscriptionRequest>
    where
        F: FnOnce(&str) -> Result<String, ()>,
    {
        let provider = self.online_provider.ok_or_else(|| {
            CliError::Validation("Missing required --online-provider.".to_string())
        })?;
        let provider_id = provider.provider_id();
        let manifest = find_online_asr_provider(provider_id).ok_or_else(|| {
            CliError::Validation(format!(
                "Online ASR provider manifest is missing {provider_id}."
            ))
        })?;
        if mode == AsrMode::Streaming && !manifest.streaming.supported.unwrap_or(true) {
            return Err(CliError::Validation(format!(
                "Online ASR provider {provider_id} does not support streaming transcription."
            )));
        }

        let mut config = manifest.defaults.clone();
        let config_object = config.as_object_mut().ok_or_else(|| {
            CliError::Validation(format!(
                "Online ASR provider defaults for {provider_id} must be a JSON object."
            ))
        })?;
        if let Some(path) = self.online_config.as_deref() {
            merge_non_secret_config(config_object, path)?;
        }

        let env_name = self
            .api_key_env
            .as_deref()
            .unwrap_or_else(|| provider.default_api_key_env());
        if env_name.trim().is_empty() {
            return Err(CliError::Validation(
                "--api-key-env must not be empty.".to_string(),
            ));
        }
        let api_key = read_env(env_name).map_err(|()| {
            CliError::Validation(format!(
                "Online ASR API key environment variable {env_name} is not set or is not valid UTF-8."
            ))
        })?;
        if api_key.trim().is_empty() {
            return Err(CliError::Validation(format!(
                "Online ASR API key environment variable {env_name} is empty."
            )));
        }
        config_object.insert("apiKey".to_string(), Value::String(api_key));

        Ok(AsrTranscriptionRequest {
            mode,
            language,
            enable_itn,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.to_string(),
                    profile_id: manifest.profile_id.clone(),
                    config,
                },
            },
        })
    }
}

fn merge_non_secret_config(target: &mut Map<String, Value>, path: &Path) -> CliResult<()> {
    let bytes = std::fs::read(path).map_err(|error| {
        CliError::Io(format!(
            "Failed to read online ASR config {}: {error}",
            path.display()
        ))
    })?;
    let overrides: Value = serde_json::from_slice(&bytes).map_err(|error| {
        CliError::Validation(format!(
            "Invalid online ASR config JSON in {}: {error}",
            path.display()
        ))
    })?;
    let overrides = overrides.as_object().ok_or_else(|| {
        CliError::Validation(format!(
            "Online ASR config {} must contain a JSON object.",
            path.display()
        ))
    })?;
    for (key, value) in overrides {
        let normalized = key
            .chars()
            .filter(|character| *character != '_' && *character != '-')
            .collect::<String>()
            .to_ascii_lowercase();
        if normalized == "apikey" {
            return Err(CliError::Validation(format!(
                "Online ASR config {} must not contain an API key; use --api-key-env instead.",
                path.display()
            )));
        }
        target.insert(key.clone(), value.clone());
    }
    Ok(())
}

pub(crate) fn map_asr_error(error: AsrPortError) -> CliError {
    match error.kind {
        AsrPortErrorKind::InvalidRequest | AsrPortErrorKind::Unsupported => {
            CliError::Validation(error.to_string())
        }
        AsrPortErrorKind::FileSystem => CliError::Io(error.to_string()),
        AsrPortErrorKind::Model => CliError::Model(error.to_string()),
        AsrPortErrorKind::Authentication
        | AsrPortErrorKind::RateLimited
        | AsrPortErrorKind::Timeout
        | AsrPortErrorKind::Network
        | AsrPortErrorKind::Protocol
        | AsrPortErrorKind::Unavailable => CliError::Network(error.to_string()),
        AsrPortErrorKind::Runtime => CliError::Other(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn online_args(provider: OnlineAsrProviderArg) -> OnlineAsrArgs {
        OnlineAsrArgs {
            online_provider: Some(provider),
            api_key_env: None,
            online_config: None,
        }
    }

    #[test]
    fn builds_batch_request_without_persisting_the_secret() {
        let request = online_args(OnlineAsrProviderArg::GroqWhisper)
            .build_request_with(AsrMode::Batch, "en".to_string(), false, None, |name| {
                assert_eq!(name, "GROQ_API_KEY");
                Ok("secret-value".to_string())
            })
            .unwrap();

        assert_eq!(request.provider_id(), GROQ_WHISPER_PROVIDER_ID);
        let AsrEngineConfig::Online { provider } = request.engine_config else {
            panic!("expected online request");
        };
        assert_eq!(provider.config["apiKey"], "secret-value");
    }

    #[test]
    fn rejects_batch_only_provider_for_streaming() {
        let error = online_args(OnlineAsrProviderArg::MistralVoxtral)
            .build_request_with(AsrMode::Streaming, "auto".to_string(), false, None, |_| {
                Ok("secret-value".to_string())
            })
            .unwrap_err();

        assert!(error.to_string().contains("does not support streaming"));
    }

    #[test]
    fn merges_non_secret_overrides_and_rejects_api_keys() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("online.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({"model": "whisper-large-v3"})).unwrap(),
        )
        .unwrap();
        let args = OnlineAsrArgs {
            online_provider: Some(OnlineAsrProviderArg::GroqWhisper),
            api_key_env: Some("CUSTOM_ASR_KEY".to_string()),
            online_config: Some(config_path.clone()),
        };
        let request = args
            .build_request_with(AsrMode::Batch, "auto".to_string(), false, None, |name| {
                assert_eq!(name, "CUSTOM_ASR_KEY");
                Ok("secret-value".to_string())
            })
            .unwrap();
        let AsrEngineConfig::Online { provider } = request.engine_config else {
            panic!("expected online request");
        };
        assert_eq!(provider.config["model"], "whisper-large-v3");

        std::fs::write(&config_path, br#"{"api_key":"must-not-be-stored"}"#).unwrap();
        let error = args
            .build_request_with(AsrMode::Batch, "auto".to_string(), false, None, |_| {
                Ok("secret-value".to_string())
            })
            .unwrap_err();
        assert!(error.to_string().contains("must not contain an API key"));
    }
}
