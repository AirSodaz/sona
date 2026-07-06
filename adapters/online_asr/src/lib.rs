use async_trait::async_trait;
use reqwest::multipart;
use serde_json::Value;
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID,
    OnlineBatchTranscriber, OnlineBatchTranscriptionOutput, OnlineBatchTranscriptionRequest,
    find_online_asr_provider,
};
use sona_core::transcript::TranscriptSegment;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhisperCompatibleProvider {
    GroqWhisper,
    MistralVoxtral,
}

impl WhisperCompatibleProvider {
    pub fn provider_id(self) -> &'static str {
        match self {
            Self::GroqWhisper => GROQ_WHISPER_PROVIDER_ID,
            Self::MistralVoxtral => MISTRAL_VOXTRAL_PROVIDER_ID,
        }
    }

    fn provider_name(self) -> &'static str {
        match self {
            Self::GroqWhisper => "Groq Whisper",
            Self::MistralVoxtral => "Mistral Voxtral",
        }
    }

    fn api_key_error(self) -> &'static str {
        match self {
            Self::GroqWhisper => "Groq API Key is not configured.",
            Self::MistralVoxtral => "Mistral API Key is not configured.",
        }
    }

    fn endpoint_error(self) -> &'static str {
        match self {
            Self::GroqWhisper => "Groq batch endpoint or model is not configured.",
            Self::MistralVoxtral => "Mistral batch endpoint or model is not configured.",
        }
    }

    fn offline_only_error(self) -> String {
        format!(
            "{} API can only be used in offline/batch mode.",
            self.provider_name()
        )
    }

    fn missing_request_error(self) -> String {
        format!(
            "Online ASR provider request is missing for {}.",
            self.provider_name()
        )
    }

    fn missing_manifest_error(self) -> String {
        format!("{} provider not found in manifest", self.provider_name())
    }

    fn network_error(self, error: reqwest::Error) -> String {
        format!("{} network request failed: {error}", self.provider_name())
    }

    fn status_error(self, status: reqwest::StatusCode, text: String) -> String {
        format!(
            "{} API returned error status {}: {}",
            self.provider_name(),
            status,
            text
        )
    }

    fn response_parse_error(self, error: reqwest::Error) -> String {
        format!("{} response parsing failed: {error}", self.provider_name())
    }

    fn missing_segments_error(self) -> String {
        format!(
            "{} response is missing 'segments' array.",
            self.provider_name()
        )
    }

    fn stage(self) -> &'static str {
        match self {
            Self::GroqWhisper => "groq_batch_complete",
            Self::MistralVoxtral => "mistral_batch_complete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperCompatibleConfigFields {
    pub api_key: String,
    pub batch_endpoint: String,
    pub model: String,
}

#[derive(Clone)]
pub struct WhisperCompatibleBatchTranscriber {
    provider: WhisperCompatibleProvider,
    client: reqwest::Client,
}

impl WhisperCompatibleBatchTranscriber {
    pub fn new(provider: WhisperCompatibleProvider) -> Self {
        Self {
            provider,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_client(provider: WhisperCompatibleProvider, client: reqwest::Client) -> Self {
        Self { provider, client }
    }
}

#[async_trait]
impl OnlineBatchTranscriber for WhisperCompatibleBatchTranscriber {
    async fn transcribe(
        &self,
        input: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, String> {
        if input.request.mode != AsrMode::Offline {
            return Err(self.provider.offline_only_error());
        }

        let config = resolve_whisper_config(&input.request, self.provider)?;
        let bytes = tokio::fs::read(&input.file_path)
            .await
            .map_err(|error| format!("Failed to read audio file: {error}"))?;

        let file_name = input
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio.wav")
            .to_string();

        let part = multipart::Part::bytes(bytes.clone())
            .file_name(file_name)
            .mime_str("audio/wav")
            .map_err(|error| format!("Failed to create multipart file: {error}"))?;

        let mut form = multipart::Form::new()
            .part("file", part)
            .text("model", config.model.clone())
            .text("response_format", "verbose_json");

        if let Some(hotwords) = input
            .request
            .hotwords
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            form = form.text("prompt", hotwords.replace('\n', ", "));
        }

        let response = self
            .client
            .post(&config.batch_endpoint)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|error| self.provider.network_error(error))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(self.provider.status_error(status, text));
        }

        let response_value = response
            .json::<Value>()
            .await
            .map_err(|error| self.provider.response_parse_error(error))?;

        let segments = segments_from_whisper_response(&response_value, self.provider)?;
        let audio_duration_ms = response_value
            .get("duration")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            * 1000.0;

        Ok(OnlineBatchTranscriptionOutput {
            segments,
            audio_duration_ms,
            buffered_samples: bytes.len() / 2,
            stage: self.provider.stage().to_string(),
        })
    }
}

#[derive(Clone)]
pub struct GroqWhisperBatchTranscriber {
    inner: WhisperCompatibleBatchTranscriber,
}

impl Default for GroqWhisperBatchTranscriber {
    fn default() -> Self {
        Self {
            inner: WhisperCompatibleBatchTranscriber::new(WhisperCompatibleProvider::GroqWhisper),
        }
    }
}

#[async_trait]
impl OnlineBatchTranscriber for GroqWhisperBatchTranscriber {
    async fn transcribe(
        &self,
        request: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, String> {
        self.inner.transcribe(request).await
    }
}

#[derive(Clone)]
pub struct MistralVoxtralBatchTranscriber {
    inner: WhisperCompatibleBatchTranscriber,
}

impl Default for MistralVoxtralBatchTranscriber {
    fn default() -> Self {
        Self {
            inner: WhisperCompatibleBatchTranscriber::new(
                WhisperCompatibleProvider::MistralVoxtral,
            ),
        }
    }
}

#[async_trait]
impl OnlineBatchTranscriber for MistralVoxtralBatchTranscriber {
    async fn transcribe(
        &self,
        request: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, String> {
        self.inner.transcribe(request).await
    }
}

pub fn resolve_whisper_config(
    request: &sona_core::ports::asr::AsrTranscriptionRequest,
    provider: WhisperCompatibleProvider,
) -> Result<WhisperCompatibleConfigFields, String> {
    let provider_request = if let AsrEngineConfig::Online { provider } = &request.engine_config {
        provider
    } else {
        return Err(provider.missing_request_error());
    };

    let get_string = |key: &str, default_val: &str| -> String {
        provider_request
            .config
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(default_val)
            .to_string()
    };

    let manifest = find_online_asr_provider(provider.provider_id())
        .ok_or_else(|| provider.missing_manifest_error())?;
    let defaults = manifest.defaults.as_object().ok_or_else(|| {
        format!(
            "{} provider defaults should be an object",
            provider.provider_name()
        )
    })?;

    let fields = WhisperCompatibleConfigFields {
        api_key: get_string(
            "apiKey",
            defaults.get("apiKey").and_then(Value::as_str).unwrap_or(""),
        ),
        batch_endpoint: get_string(
            "batchEndpoint",
            defaults
                .get("batchEndpoint")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
        model: get_string(
            "model",
            defaults.get("model").and_then(Value::as_str).unwrap_or(""),
        ),
    };

    if fields.api_key.is_empty() {
        return Err(provider.api_key_error().to_string());
    }
    if fields.batch_endpoint.is_empty() || fields.model.is_empty() {
        return Err(provider.endpoint_error().to_string());
    }

    Ok(fields)
}

pub fn segments_from_whisper_response(
    response: &Value,
    provider: WhisperCompatibleProvider,
) -> Result<Vec<TranscriptSegment>, String> {
    let segments_array = response
        .get("segments")
        .and_then(Value::as_array)
        .ok_or_else(|| provider.missing_segments_error())?;

    let mut segments = Vec::with_capacity(segments_array.len());
    for segment in segments_array {
        let text = segment
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let start = segment.get("start").and_then(Value::as_f64).unwrap_or(0.0);
        let end = segment.get("end").and_then(Value::as_f64).unwrap_or(0.0);

        segments.push(TranscriptSegment {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            start,
            end,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        });
    }

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, AsrTranscriptionRequest, GROQ_WHISPER_PROVIDER_ID,
        MISTRAL_VOXTRAL_PROVIDER_ID, OnlineAsrProviderRequest,
    };
    use sona_core::transcript_postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };

    use crate::{
        WhisperCompatibleProvider, resolve_whisper_config, segments_from_whisper_response,
    };

    fn online_request(provider_id: &str, config: serde_json::Value) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode: AsrMode::Offline,
            language: "auto".to_string(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.to_string(),
                    profile_id: format!("{provider_id}-default"),
                    config,
                },
            },
        }
    }

    #[test]
    fn resolves_groq_config_from_request_with_manifest_defaults() {
        let request = online_request(
            GROQ_WHISPER_PROVIDER_ID,
            json!({
                "apiKey": " groq-key ",
                "model": " custom-whisper "
            }),
        );

        let config =
            resolve_whisper_config(&request, WhisperCompatibleProvider::GroqWhisper).unwrap();

        assert_eq!(config.api_key, "groq-key");
        assert_eq!(config.model, "custom-whisper");
        assert_eq!(
            config.batch_endpoint,
            "https://api.groq.com/openai/v1/audio/transcriptions"
        );
    }

    #[test]
    fn resolves_mistral_config_from_request_with_manifest_defaults() {
        let request = online_request(
            MISTRAL_VOXTRAL_PROVIDER_ID,
            json!({
                "apiKey": " mistral-key "
            }),
        );

        let config =
            resolve_whisper_config(&request, WhisperCompatibleProvider::MistralVoxtral).unwrap();

        assert_eq!(config.api_key, "mistral-key");
        assert_eq!(config.model, "mistral-small-latest");
        assert_eq!(
            config.batch_endpoint,
            "https://api.mistral.ai/v1/audio/transcriptions"
        );
    }

    #[test]
    fn parses_whisper_compatible_verbose_json_segments() {
        let response = json!({
            "duration": 1.25,
            "segments": [
                { "text": " hello ", "start": 0.0, "end": 0.5 },
                { "text": "world", "start": 0.5, "end": 1.25 }
            ]
        });

        let segments =
            segments_from_whisper_response(&response, WhisperCompatibleProvider::GroqWhisper)
                .unwrap();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments[0].end, 0.5);
        assert!(segments[0].is_final);
        assert_eq!(segments[1].text, "world");
    }
}
