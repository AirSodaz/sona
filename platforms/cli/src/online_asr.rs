use clap::Args;
use clap::builder::PossibleValuesParser;
use serde_json::{Map, Value};
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrPortError, AsrPortErrorKind, AsrTranscriptionRequest,
    OnlineAsrProviderRequest, find_online_asr_provider, online_asr_provider_ids,
};
use sona_core::transcription::postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};
use std::path::{Path, PathBuf};

use crate::{CliError, CliResult};

fn online_provider_value_parser() -> PossibleValuesParser {
    PossibleValuesParser::new(online_asr_provider_ids())
}

#[derive(Clone, Debug, Args)]
pub(crate) struct OnlineAsrArgs {
    /// Use an online ASR provider instead of local Sherpa ASR.
    #[arg(long, value_name = "PROVIDER", value_parser = online_provider_value_parser())]
    pub(crate) online_provider: Option<String>,
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

    pub(crate) fn resolve_with_config(
        &self,
        config_provider: Option<String>,
        config_api_key_env: Option<String>,
        config_online_config: Option<PathBuf>,
    ) -> Self {
        let same_provider = match (&self.online_provider, &config_provider) {
            (Some(cli_p), Some(cfg_p)) => cli_p == cfg_p,
            (None, _) => true,
            _ => false,
        };

        Self {
            online_provider: self.online_provider.clone().or(config_provider),
            api_key_env: if same_provider {
                self.api_key_env.clone().or(config_api_key_env)
            } else {
                self.api_key_env.clone()
            },
            online_config: if same_provider {
                self.online_config.clone().or(config_online_config)
            } else {
                self.online_config.clone()
            },
        }
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
        let provider_id = self.online_provider.as_deref().ok_or_else(|| {
            CliError::Validation("Missing required --online-provider.".to_string())
        })?;
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
            merge_config_overrides(config_object, path)?;
        }

        let env_name = self
            .api_key_env
            .as_deref()
            .or_else(|| manifest.default_api_key_env())
            .ok_or_else(|| {
                CliError::Validation(format!(
                    "Online ASR provider {provider_id} does not declare a default API key environment variable; specify one with --api-key-env."
                ))
            })?;
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

fn merge_config_overrides(target: &mut Map<String, Value>, path: &Path) -> CliResult<()> {
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
    use clap::Parser;
    use serde_json::json;
    use sona_core::ports::asr::{
        ASSEMBLYAI_PROVIDER_ID, DEEPGRAM_PROVIDER_ID, ELEVENLABS_PROVIDER_ID,
        GROQ_WHISPER_PROVIDER_ID, OPENAI_WHISPER_PROVIDER_ID, VOLCENGINE_DOUBAO_PROVIDER_ID,
        online_asr_providers,
    };
    use tempfile::tempdir;

    fn online_args(provider: impl Into<String>) -> OnlineAsrArgs {
        OnlineAsrArgs {
            online_provider: Some(provider.into()),
            api_key_env: None,
            online_config: None,
        }
    }

    #[test]
    fn builds_batch_request_without_persisting_the_secret() {
        let request = online_args(GROQ_WHISPER_PROVIDER_ID)
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
        let error = online_args(GROQ_WHISPER_PROVIDER_ID)
            .build_request_with(AsrMode::Streaming, "auto".to_string(), false, None, |_| {
                Ok("secret-value".to_string())
            })
            .unwrap_err();

        assert!(error.to_string().contains("does not support streaming"));
    }

    #[test]
    fn merges_config_overrides_and_rejects_api_keys() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("online.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({"model": "whisper-large-v3"})).unwrap(),
        )
        .unwrap();
        let args = OnlineAsrArgs {
            online_provider: Some(GROQ_WHISPER_PROVIDER_ID.to_string()),
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

    #[test]
    fn manifest_providers_have_correct_env_and_id() {
        for provider in online_asr_providers() {
            assert!(!provider.id.is_empty());
            assert!(
                provider.default_api_key_env().is_some(),
                "provider {} must declare a default api key env",
                provider.id
            );
        }
        assert_eq!(
            find_online_asr_provider(OPENAI_WHISPER_PROVIDER_ID)
                .unwrap()
                .default_api_key_env(),
            Some("OPENAI_API_KEY")
        );
        assert_eq!(
            find_online_asr_provider(DEEPGRAM_PROVIDER_ID)
                .unwrap()
                .default_api_key_env(),
            Some("DEEPGRAM_API_KEY")
        );
        assert_eq!(
            find_online_asr_provider(ASSEMBLYAI_PROVIDER_ID)
                .unwrap()
                .default_api_key_env(),
            Some("ASSEMBLYAI_API_KEY")
        );
        assert_eq!(
            find_online_asr_provider(ELEVENLABS_PROVIDER_ID)
                .unwrap()
                .default_api_key_env(),
            Some("ELEVENLABS_API_KEY")
        );
        assert_eq!(
            find_online_asr_provider(VOLCENGINE_DOUBAO_PROVIDER_ID)
                .unwrap()
                .default_api_key_env(),
            Some("SONA_VOLCENGINE_ASR_API_KEY")
        );
    }

    #[test]
    fn clap_accepts_all_manifest_providers_and_rejects_unknown() {
        #[derive(Parser, Debug)]
        struct TestCli {
            #[command(flatten)]
            online: OnlineAsrArgs,
        }

        for provider in online_asr_providers() {
            let cli = TestCli::try_parse_from(["test", "--online-provider", &provider.id]).unwrap();
            assert_eq!(
                cli.online.online_provider.as_deref(),
                Some(provider.id.as_str())
            );
        }

        let err =
            TestCli::try_parse_from(["test", "--online-provider", "unknown-provider"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn rejects_missing_manifest_provider_in_request_builder() {
        let error = online_args("unknown-provider")
            .build_request_with(AsrMode::Batch, "auto".to_string(), false, None, |_| {
                Ok("secret".to_string())
            })
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Online ASR provider manifest is missing unknown-provider")
        );
    }

    #[test]
    fn resolves_online_args_from_config_defaults() {
        let empty_args = OnlineAsrArgs {
            online_provider: None,
            api_key_env: None,
            online_config: None,
        };
        assert!(!empty_args.is_online());

        let resolved = empty_args.resolve_with_config(
            Some(VOLCENGINE_DOUBAO_PROVIDER_ID.to_string()),
            Some("MY_KEY_VAR".to_string()),
            Some(PathBuf::from("conf.json")),
        );

        assert!(resolved.is_online());
        assert_eq!(
            resolved.online_provider.as_deref(),
            Some(VOLCENGINE_DOUBAO_PROVIDER_ID)
        );
        assert_eq!(resolved.api_key_env.as_deref(), Some("MY_KEY_VAR"));
        assert_eq!(resolved.online_config, Some(PathBuf::from("conf.json")));

        // CLI flags should override config values
        let cli_override = OnlineAsrArgs {
            online_provider: Some(GROQ_WHISPER_PROVIDER_ID.to_string()),
            api_key_env: Some("CLI_KEY_VAR".to_string()),
            online_config: None,
        };
        let resolved_override = cli_override.resolve_with_config(
            Some(VOLCENGINE_DOUBAO_PROVIDER_ID.to_string()),
            Some("MY_KEY_VAR".to_string()),
            Some(PathBuf::from("conf.json")),
        );
        assert_eq!(
            resolved_override.online_provider.as_deref(),
            Some(GROQ_WHISPER_PROVIDER_ID)
        );
        assert_eq!(
            resolved_override.api_key_env.as_deref(),
            Some("CLI_KEY_VAR")
        );
        // Overridden provider does not inherit another provider's config
        assert!(resolved_override.online_config.is_none());
    }
}
