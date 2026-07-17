use crate::mapper::transcript_segment_to_ffi;
use crate::{FfiTranscriptSegment, SonaCoreBindingError, SonaCoreBindingResult};
use serde_json::Value;
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrTranscriptionRequest, GROQ_WHISPER_PROVIDER_ID,
    MISTRAL_VOXTRAL_PROVIDER_ID, OnlineAsrProviderRequest, OnlineBatchTranscriptionOutput,
    OnlineBatchTranscriptionRequest, VOLCENGINE_DOUBAO_PROVIDER_ID, find_online_asr_provider,
};
use sona_core::transcription::postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions,
};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiOnlineAsrBatchProvider {
    VolcengineDoubao,
    GroqWhisper,
    MistralVoxtral,
}

impl FfiOnlineAsrBatchProvider {
    fn provider_id(self) -> &'static str {
        match self {
            Self::VolcengineDoubao => VOLCENGINE_DOUBAO_PROVIDER_ID,
            Self::GroqWhisper => GROQ_WHISPER_PROVIDER_ID,
            Self::MistralVoxtral => MISTRAL_VOXTRAL_PROVIDER_ID,
        }
    }
}

#[derive(uniffi::Object)]
pub struct FfiOnlineAsrApiKey {
    value: String,
}

impl fmt::Debug for FfiOnlineAsrApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FfiOnlineAsrApiKey")
            .field("value", &"<redacted>")
            .finish()
    }
}

#[uniffi::export]
impl FfiOnlineAsrApiKey {
    #[uniffi::constructor]
    pub fn new(value: String) -> Arc<Self> {
        Arc::new(Self { value })
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct FfiOnlineAsrBatchRequest {
    pub audio_path: String,
    pub provider: FfiOnlineAsrBatchProvider,
    pub api_key: Arc<FfiOnlineAsrApiKey>,
    pub language: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiOnlineAsrBatchResult {
    pub segments: Vec<FfiTranscriptSegment>,
    pub audio_duration_ms: f64,
    pub buffered_samples: u64,
    pub stage: String,
}

pub(crate) fn validate_request(request: &FfiOnlineAsrBatchRequest) -> SonaCoreBindingResult<()> {
    if request.audio_path.trim().is_empty() {
        return Err(invalid_input("Online ASR audio path must not be empty."));
    }
    if request.language.trim().is_empty() {
        return Err(invalid_input("Online ASR language must not be empty."));
    }
    if request.api_key.value.trim().is_empty() {
        return Err(invalid_input("Online ASR API Key must not be empty."));
    }
    Ok(())
}

pub(crate) fn build_core_request(
    request: &FfiOnlineAsrBatchRequest,
) -> SonaCoreBindingResult<OnlineBatchTranscriptionRequest> {
    validate_request(request)?;

    let provider_id = request.provider.provider_id();
    let manifest = find_online_asr_provider(provider_id).ok_or_else(|| {
        invalid_input(format!(
            "Online ASR provider manifest is missing {provider_id}."
        ))
    })?;
    let mut config = manifest.defaults.clone();
    let config_object = config.as_object_mut().ok_or_else(|| {
        invalid_input(format!(
            "Online ASR provider defaults for {provider_id} must be an object."
        ))
    })?;
    config_object.insert(
        "apiKey".to_string(),
        Value::String(request.api_key.value.trim().to_string()),
    );

    Ok(OnlineBatchTranscriptionRequest {
        file_path: PathBuf::from(request.audio_path.trim()),
        request: AsrTranscriptionRequest {
            mode: AsrMode::Batch,
            language: request.language.trim().to_string(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.to_string(),
                    profile_id: manifest.profile_id.clone(),
                    config,
                },
            },
        },
    })
}

pub(crate) fn output_to_ffi(output: OnlineBatchTranscriptionOutput) -> FfiOnlineAsrBatchResult {
    FfiOnlineAsrBatchResult {
        segments: output
            .segments
            .iter()
            .map(transcript_segment_to_ffi)
            .collect(),
        audio_duration_ms: output.audio_duration_ms,
        buffered_samples: u64::try_from(output.buffered_samples)
            .expect("usize fits u64 on supported UniFFI targets"),
        stage: output.stage,
    }
}

pub(crate) async fn transcribe_online_asr_batch(
    request: FfiOnlineAsrBatchRequest,
) -> SonaCoreBindingResult<FfiOnlineAsrBatchResult> {
    let request = build_core_request(&request)?;
    sona_online_asr::OnlineAsrAdapter
        .transcribe_batch(request)
        .await
        .map(output_to_ffi)
        .map_err(Into::into)
}

fn invalid_input(reason: impl Into<String>) -> SonaCoreBindingError {
    SonaCoreBindingError::InvalidInput {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, GROQ_WHISPER_PROVIDER_ID, MISTRAL_VOXTRAL_PROVIDER_ID,
        OnlineBatchTranscriptionOutput, VOLCENGINE_DOUBAO_PROVIDER_ID,
    };
    use sona_core::transcription::transcript::TranscriptSegment;

    fn request(provider: FfiOnlineAsrBatchProvider) -> FfiOnlineAsrBatchRequest {
        FfiOnlineAsrBatchRequest {
            audio_path: "recording.wav".to_string(),
            provider,
            api_key: FfiOnlineAsrApiKey::new("  temporary-secret  ".to_string()),
            language: "  auto  ".to_string(),
        }
    }

    #[test]
    fn builds_all_three_provider_requests_from_manifest_defaults() {
        let cases = [
            (
                FfiOnlineAsrBatchProvider::VolcengineDoubao,
                VOLCENGINE_DOUBAO_PROVIDER_ID,
                "volcengine-doubao-default",
                "batchResourceId",
                "volc.bigasr.auc_turbo",
            ),
            (
                FfiOnlineAsrBatchProvider::GroqWhisper,
                GROQ_WHISPER_PROVIDER_ID,
                "groq-whisper-default",
                "model",
                "whisper-large-v3-turbo",
            ),
            (
                FfiOnlineAsrBatchProvider::MistralVoxtral,
                MISTRAL_VOXTRAL_PROVIDER_ID,
                "mistral-voxtral-default",
                "model",
                "mistral-small-latest",
            ),
        ];

        for (provider, provider_id, profile_id, default_key, default_value) in cases {
            let core = build_core_request(&request(provider)).expect("core batch request");

            assert_eq!(core.file_path.to_string_lossy(), "recording.wav");
            assert_eq!(core.request.mode, AsrMode::Batch);
            assert_eq!(core.request.language, "auto");
            assert!(!core.request.enable_itn);
            let AsrEngineConfig::Online { provider } = core.request.engine_config else {
                panic!("batch request should use the online engine");
            };
            assert_eq!(provider.provider_id, provider_id);
            assert_eq!(provider.profile_id, profile_id);
            assert_eq!(provider.config["apiKey"], "temporary-secret");
            assert_eq!(provider.config[default_key], default_value);
        }
    }

    #[test]
    fn api_key_debug_output_is_redacted() {
        let key = FfiOnlineAsrApiKey::new("private-value".to_string());
        let debug = format!("{key:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("private-value"));
    }

    #[test]
    fn validation_rejects_invalid_inputs_before_dispatch() {
        let mut input = request(FfiOnlineAsrBatchProvider::GroqWhisper);
        input.audio_path.clear();
        let error = validate_request(&input).expect_err("empty paths must fail");
        assert!(error.to_string().contains("audio path"));
        assert!(!error.to_string().contains("temporary-secret"));

        input.audio_path = "recording.wav".to_string();
        input.language = "   ".to_string();
        let error = validate_request(&input).expect_err("empty language must fail");
        assert!(error.to_string().contains("language"));

        input.language = "auto".to_string();
        input.api_key = FfiOnlineAsrApiKey::new("   ".to_string());
        let error = validate_request(&input).expect_err("empty API keys must fail");
        assert!(error.to_string().contains("API Key"));
    }

    #[test]
    fn batch_output_mapping_preserves_segments_and_metrics() {
        let output = OnlineBatchTranscriptionOutput {
            segments: vec![TranscriptSegment {
                id: "segment-1".to_string(),
                text: "hello".to_string(),
                start: 0.25,
                end: 1.5,
                is_final: true,
                timing: None,
                tokens: Some(vec!["hello".to_string()]),
                timestamps: Some(vec![0.25]),
                durations: Some(vec![1.25]),
                translation: Some("hola".to_string()),
                speaker: None,
                speaker_attribution: None,
            }],
            audio_duration_ms: 1_500.0,
            buffered_samples: 24_000,
            stage: "groq_batch_complete".to_string(),
        };

        let mapped = output_to_ffi(output);

        assert_eq!(mapped.segments[0].id, "segment-1");
        assert_eq!(mapped.segments[0].translation.as_deref(), Some("hola"));
        assert_eq!(mapped.audio_duration_ms, 1_500.0);
        assert_eq!(mapped.buffered_samples, 24_000);
        assert_eq!(mapped.stage, "groq_batch_complete");
    }
}
