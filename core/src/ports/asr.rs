use crate::models::config::ModelFileConfig;
use crate::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use crate::transcription::postprocess::TranscriptPostprocessor;
pub use crate::transcription::postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions, TranscriptTextReplacementRule,
    TranscriptTextReplacementRuleSet,
};
use crate::transcription::runtime::BatchTranscribePlan;
use crate::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "specta")]
use specta::Type;
use std::path::PathBuf;
use std::sync::OnceLock;

const ONLINE_ASR_PROVIDERS_JSON: &str = include_str!("online-asr-providers.json");

pub const LOCAL_SHERPA_PROVIDER_ID: &str = "local_sherpa";
pub const VOLCENGINE_DOUBAO_PROVIDER_ID: &str = "volcengine-doubao";
pub const VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY: &str = "volcengineDoubao";
pub const GROQ_WHISPER_PROVIDER_ID: &str = "groq-whisper";
pub const MISTRAL_VOXTRAL_PROVIDER_ID: &str = "mistral-voxtral";

#[derive(thiserror::Error, Debug)]
pub enum SherpaError {
    #[error("在线 ASR provider 配置缺失。")]
    OnlineProviderConfigMissing,

    #[error("不支持的在线 ASR provider：{provider_id}")]
    UnsupportedOnlineProvider { provider_id: String },

    #[error("在线 ASR session 未初始化。")]
    OnlineSessionNotInitialized,

    #[error("provider {provider_id} 不支持流式识别")]
    StreamingNotSupported { provider_id: String },

    #[error("火山 ASR API Key 未配置。")]
    VolcengineApiKeyMissing,

    #[error("火山实时 ASR endpoint 或 Resource ID 未配置。")]
    VolcengineStreamingConfigMissing,

    #[error("火山批量 ASR endpoint 或 Resource ID 未配置。")]
    VolcengineBatchConfigMissing,

    #[error("火山 ASR provider 配置缺失。")]
    VolcengineProviderConfigMissing,

    #[error("不支持的火山 ASR provider：{provider_id}")]
    UnsupportedVolcengineProvider { provider_id: String },

    #[error("火山 ASR provider 配置无效：{error}")]
    VolcengineProviderConfigInvalid { error: String },

    #[error("火山 ASR 响应帧过短。")]
    VolcengineFrameTooShort,

    #[error("火山 ASR 返回错误帧。")]
    VolcengineErrorFrame,

    #[error("火山错误码解析失败。")]
    VolcengineErrorCodeParseFailed,

    #[error("火山错误长度解析失败。")]
    VolcengineErrorLengthParseFailed,

    #[error("火山 ASR 返回错误：{code} {message}")]
    VolcengineApiError { code: u32, message: String },

    #[error("火山 ASR 响应 payload 长度缺失。")]
    VolcenginePayloadLengthMissing,

    #[error("火山 ASR 响应 payload 长度解析失败。")]
    VolcenginePayloadLengthParseFailed,

    #[error("火山 ASR 响应 payload 不完整。")]
    VolcenginePayloadIncomplete,

    #[error("火山 ASR 响应解析失败：{error}")]
    VolcengineResponseParseFailed { error: String },

    #[error("火山 ASR WebSocket endpoint 无效：{error}")]
    VolcengineEndpointInvalid { error: String },

    #[error("火山 ASR WebSocket 连接失败：{error}")]
    VolcengineConnectionFailed { error: String },

    #[error("火山 ASR 初始化帧发送失败：{error}")]
    VolcengineInitFrameSendFailed { error: String },

    #[error("火山 ASR WebSocket 尚未连接。")]
    VolcengineWebSocketNotConnected,

    #[error("火山 ASR 音频发送失败：{error}")]
    VolcengineAudioSendFailed { error: String },

    #[error("火山 ASR 结束帧发送失败：{error}")]
    VolcengineEndFrameSendFailed { error: String },

    #[error("读取音频文件失败：{error}")]
    AudioFileReadFailed { error: String },

    #[error("火山批量 ASR 网络请求失败：{error}")]
    VolcengineBatchRequestFailed { error: String },

    #[error("火山批量 ASR 响应解析失败：{error}")]
    VolcengineBatchResponseParseFailed { error: String },

    #[error("{message}")]
    VolcengineLocalFileBatchUnsupported { message: String },

    #[error("火山实时 ASR 只能用于 streaming 槽位。")]
    VolcengineRealtimeOnlyForStreaming,

    #[error("火山批量 ASR 只能用于 batch 槽位。")]
    VolcengineBatchModeMismatch,

    #[error("{0}")]
    Generic(String),
}

impl Serialize for SherpaError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct ErrorData<'a> {
            code: &'a str,
            message: String,
        }

        let code = match self {
            Self::OnlineProviderConfigMissing => "ONLINE_PROVIDER_CONFIG_MISSING",
            Self::UnsupportedOnlineProvider { .. } => "UNSUPPORTED_ONLINE_PROVIDER",
            Self::OnlineSessionNotInitialized => "ONLINE_SESSION_NOT_INITIALIZED",
            Self::StreamingNotSupported { .. } => "STREAMING_NOT_SUPPORTED",
            Self::VolcengineApiKeyMissing => "VOLCENGINE_API_KEY_MISSING",
            Self::VolcengineStreamingConfigMissing => "VOLCENGINE_STREAMING_CONFIG_MISSING",
            Self::VolcengineBatchConfigMissing => "VOLCENGINE_BATCH_CONFIG_MISSING",
            Self::VolcengineProviderConfigMissing => "VOLCENGINE_PROVIDER_CONFIG_MISSING",
            Self::UnsupportedVolcengineProvider { .. } => "UNSUPPORTED_VOLCENGINE_PROVIDER",
            Self::VolcengineProviderConfigInvalid { .. } => "VOLCENGINE_PROVIDER_CONFIG_INVALID",
            Self::VolcengineFrameTooShort => "VOLCENGINE_FRAME_TOO_SHORT",
            Self::VolcengineErrorFrame => "VOLCENGINE_ERROR_FRAME",
            Self::VolcengineErrorCodeParseFailed => "VOLCENGINE_ERROR_CODE_PARSE_FAILED",
            Self::VolcengineErrorLengthParseFailed => "VOLCENGINE_ERROR_LENGTH_PARSE_FAILED",
            Self::VolcengineApiError { .. } => "VOLCENGINE_API_ERROR",
            Self::VolcenginePayloadLengthMissing => "VOLCENGINE_PAYLOAD_LENGTH_MISSING",
            Self::VolcenginePayloadLengthParseFailed => "VOLCENGINE_PAYLOAD_LENGTH_PARSE_FAILED",
            Self::VolcenginePayloadIncomplete => "VOLCENGINE_PAYLOAD_INCOMPLETE",
            Self::VolcengineResponseParseFailed { .. } => "VOLCENGINE_RESPONSE_PARSE_FAILED",
            Self::VolcengineEndpointInvalid { .. } => "VOLCENGINE_ENDPOINT_INVALID",
            Self::VolcengineConnectionFailed { .. } => "VOLCENGINE_CONNECTION_FAILED",
            Self::VolcengineInitFrameSendFailed { .. } => "VOLCENGINE_INIT_FRAME_SEND_FAILED",
            Self::VolcengineWebSocketNotConnected => "VOLCENGINE_WEB_SOCKET_NOT_CONNECTED",
            Self::VolcengineAudioSendFailed { .. } => "VOLCENGINE_AUDIO_SEND_FAILED",
            Self::VolcengineEndFrameSendFailed { .. } => "VOLCENGINE_END_FRAME_SEND_FAILED",
            Self::AudioFileReadFailed { .. } => "AUDIO_FILE_READ_FAILED",
            Self::VolcengineBatchRequestFailed { .. } => "VOLCENGINE_BATCH_REQUEST_FAILED",
            Self::VolcengineBatchResponseParseFailed { .. } => {
                "VOLCENGINE_BATCH_RESPONSE_PARSE_FAILED"
            }
            Self::VolcengineLocalFileBatchUnsupported { .. } => {
                "VOLCENGINE_LOCAL_FILE_BATCH_UNSUPPORTED"
            }
            Self::VolcengineRealtimeOnlyForStreaming => "VOLCENGINE_REALTIME_ONLY_FOR_STREAMING",
            Self::VolcengineBatchModeMismatch => "VOLCENGINE_BATCH_MODE_MISMATCH",
            Self::Generic(_) => "GENERIC_ERROR",
        };

        let data = ErrorData {
            code,
            message: self.to_string(),
        };

        data.serialize(serializer)
    }
}

impl From<String> for SherpaError {
    fn from(s: String) -> Self {
        Self::Generic(s)
    }
}

impl From<&str> for SherpaError {
    fn from(s: &str) -> Self {
        Self::Generic(s.to_string())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "kebab-case")]
pub enum AsrEngine {
    LocalSherpa,
    Online,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum AsrMode {
    Streaming,
    #[serde(alias = "offline")]
    Batch,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum BatchSegmentationMode {
    #[default]
    Vad,
    Whole,
}

#[async_trait]
pub trait BatchTranscriber: Send + Sync {
    async fn transcribe(&self, plan: BatchTranscribePlan)
    -> Result<Vec<TranscriptSegment>, String>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsrTranscriptUpdateEvent {
    pub instance_id: String,
    pub stage: String,
    pub update: TranscriptUpdate,
}

pub trait AsrRuntimeObserver: Send + Sync {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent);
    fn on_model_load(&self, metric: &AsrModelLoadMetric);
    fn on_live_inference(&self, metric: &AsrInferenceMetric);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopAsrRuntimeObserver;

impl AsrRuntimeObserver for NoopAsrRuntimeObserver {
    fn on_transcript_update(&self, _event: &AsrTranscriptUpdateEvent) {}

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
}

#[async_trait]
pub trait AsrStreamingSession: Send + Sync {
    async fn start(&self) -> Result<(), SherpaError>;
    async fn stop(&self) -> Result<(), SherpaError>;
    async fn flush(&self) -> Result<(), SherpaError>;
    async fn feed_audio_chunk(&self, samples: Vec<u8>) -> Result<(), SherpaError>;
    async fn feed_audio_samples(&self, samples: &[f32]) -> Result<(), SherpaError>;
}

#[derive(Debug, Clone)]
pub struct BatchTranscriptionRequest {
    pub instance_id: Option<String>,
    pub file_path: PathBuf,
    pub save_to_path: Option<PathBuf>,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub batch_segmentation_mode: BatchSegmentationMode,
    pub model_type: String,
    pub file_config: Option<ModelFileConfig>,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocessor: TranscriptPostprocessor,
    pub gpu_acceleration: Option<String>,
}

impl BatchTranscriptionRequest {
    pub fn from_local_sherpa_request(
        file_path: PathBuf,
        save_to_path: Option<PathBuf>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,
        instance_id: Option<String>,
    ) -> Result<Self, String> {
        let AsrTranscriptionRequest {
            language,
            enable_itn,
            normalization_options,
            postprocess_options,
            hotwords,
            engine_config,
            ..
        } = request;

        match engine_config {
            AsrEngineConfig::LocalSherpa {
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config,
                gpu_acceleration,
                ..
            } => Ok(Self {
                instance_id,
                file_path,
                save_to_path,
                model_path,
                num_threads,
                enable_itn,
                language,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config: *file_config,
                hotwords,
                speaker_processing,
                normalization_options,
                postprocessor: TranscriptPostprocessor::compile(postprocess_options)?,
                gpu_acceleration,
            }),
            _ => Err("Expected LocalSherpa engine config".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalSherpaStreamingRequest {
    pub instance_id: String,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub model_type: String,
    pub file_config: Option<ModelFileConfig>,
    pub hotwords: Option<String>,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocess_options: TranscriptPostprocessOptions,
    pub gpu_acceleration: Option<String>,
}

impl LocalSherpaStreamingRequest {
    pub fn from_local_sherpa_request(
        instance_id: String,
        request: AsrTranscriptionRequest,
    ) -> Result<Self, String> {
        validate_local_sherpa_mode(&request, AsrMode::Streaming)?;

        let AsrTranscriptionRequest {
            language,
            enable_itn,
            normalization_options,
            postprocess_options,
            hotwords,
            engine_config,
            ..
        } = request;

        match engine_config {
            AsrEngineConfig::LocalSherpa {
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                model_type,
                file_config,
                gpu_acceleration,
                ..
            } => Ok(Self {
                instance_id,
                model_path,
                num_threads,
                enable_itn,
                language,
                punctuation_model,
                vad_model,
                vad_buffer,
                model_type,
                file_config: *file_config,
                hotwords,
                normalization_options,
                postprocess_options,
                gpu_acceleration,
            }),
            _ => Err("Expected LocalSherpa engine config".to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnlineBatchTranscriptionRequest {
    pub file_path: PathBuf,
    pub request: AsrTranscriptionRequest,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnlineBatchTranscriptionOutput {
    pub segments: Vec<TranscriptSegment>,
    pub audio_duration_ms: f64,
    pub buffered_samples: usize,
    pub stage: String,
}

#[async_trait]
pub trait OnlineBatchTranscriber: Send + Sync {
    async fn transcribe(
        &self,
        request: OnlineBatchTranscriptionRequest,
    ) -> Result<OnlineBatchTranscriptionOutput, String>;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AsrTranscriptionRequest {
    pub mode: AsrMode,
    pub language: String,
    pub enable_itn: bool,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocess_options: TranscriptPostprocessOptions,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,

    #[serde(flatten)]
    pub engine_config: AsrEngineConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(tag = "engine")]
pub enum AsrEngineConfig {
    #[serde(rename = "local-sherpa", rename_all = "camelCase")]
    LocalSherpa {
        #[serde(default)]
        model_id: Option<String>,
        model_path: String,
        num_threads: i32,
        #[serde(default)]
        punctuation_model: Option<String>,
        #[serde(default)]
        vad_model: Option<String>,
        vad_buffer: f32,
        #[serde(default)]
        batch_segmentation_mode: BatchSegmentationMode,
        model_type: String,
        #[serde(default)]
        file_config: Box<Option<ModelFileConfig>>,
        #[serde(default)]
        gpu_acceleration: Option<String>,
    },
    #[serde(rename = "online", rename_all = "camelCase")]
    Online {
        #[serde(rename = "onlineProvider")]
        provider: OnlineAsrProviderRequest,
    },
}

impl AsrTranscriptionRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn local_sherpa(
        mode: AsrMode,
        model_path: String,
        num_threads: i32,
        enable_itn: bool,
        language: String,
        punctuation_model: Option<String>,
        vad_model: Option<String>,
        vad_buffer: f32,
        model_type: String,
        file_config: Option<ModelFileConfig>,
        hotwords: Option<String>,
        normalization_options: TranscriptNormalizationOptions,
        postprocess_options: TranscriptPostprocessOptions,
        speaker_processing: Option<crate::transcription::speaker::SpeakerProcessingConfig>,
        gpu_acceleration: Option<String>,
    ) -> Self {
        Self {
            mode,
            language,
            enable_itn,
            normalization_options,
            postprocess_options,
            hotwords,
            speaker_processing,
            engine_config: AsrEngineConfig::LocalSherpa {
                model_id: None,
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode: BatchSegmentationMode::Vad,
                model_type,
                file_config: Box::new(file_config),
                gpu_acceleration,
            },
        }
    }

    pub fn engine(&self) -> AsrEngine {
        match &self.engine_config {
            AsrEngineConfig::LocalSherpa { .. } => AsrEngine::LocalSherpa,
            AsrEngineConfig::Online { .. } => AsrEngine::Online,
        }
    }

    pub fn provider_id(&self) -> &str {
        match &self.engine_config {
            AsrEngineConfig::LocalSherpa { .. } => LOCAL_SHERPA_PROVIDER_ID,
            AsrEngineConfig::Online { provider } => provider.provider_id.as_str(),
        }
    }
}

pub fn validate_local_sherpa_mode(
    request: &AsrTranscriptionRequest,
    expected: AsrMode,
) -> Result<(), String> {
    if request.engine() != AsrEngine::LocalSherpa {
        return Err("Unsupported ASR engine for local Sherpa adapter".to_string());
    }
    if request.mode != expected {
        return Err(format!(
            "ASR request mode mismatch: expected {:?}, got {:?}",
            expected, request.mode
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProviderRequest {
    pub provider_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct VolcengineDoubaoAsrConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub streaming_endpoint: String,
    #[serde(default)]
    pub streaming_resource_id: String,
    #[serde(default)]
    pub batch_endpoint: String,
    #[serde(default)]
    pub batch_resource_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
struct OnlineAsrProviderManifest {
    schema_version: u32,
    providers: Vec<OnlineAsrProvider>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProvider {
    pub id: String,
    pub profile_id: String,
    pub defaults: Value,
    pub streaming: OnlineAsrCapability,
    pub batch: OnlineAsrBatchCapability,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrCapability {
    pub supported: Option<bool>,
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrBatchCapability {
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
    pub local_file_mode: OnlineAsrLocalFileBatchMode,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrLocalFileBatchMode {
    pub supported: bool,
    pub endpoint: String,
    pub resource_id: String,
    pub unsupported_message: String,
}

static ONLINE_ASR_PROVIDER_MANIFEST: OnceLock<OnlineAsrProviderManifest> = OnceLock::new();

fn online_asr_provider_manifest() -> &'static OnlineAsrProviderManifest {
    ONLINE_ASR_PROVIDER_MANIFEST.get_or_init(|| {
        let manifest: OnlineAsrProviderManifest = serde_json::from_str(ONLINE_ASR_PROVIDERS_JSON)
            .expect("shared online ASR providers JSON should be valid");
        assert_eq!(
            manifest.schema_version, 1,
            "shared online ASR providers schema version should be supported"
        );
        assert!(
            manifest
                .providers
                .iter()
                .any(|provider| provider.id == VOLCENGINE_DOUBAO_PROVIDER_ID),
            "shared online ASR providers JSON should include Volcengine Doubao"
        );
        for provider in &manifest.providers {
            assert!(
                !provider.profile_id.trim().is_empty(),
                "online ASR provider profile id should not be empty"
            );
            validate_capability_config_fields(
                &provider.id,
                "streaming",
                &provider.streaming.required_config_fields,
            );
            validate_capability_config_fields(
                &provider.id,
                "batch",
                &provider.batch.required_config_fields,
            );
            if provider.streaming.requires_api_key || provider.batch.requires_api_key {
                assert!(
                    provider.defaults.get("apiKey").is_some()
                        || provider
                            .streaming
                            .required_config_fields
                            .iter()
                            .chain(provider.batch.required_config_fields.iter())
                            .any(|field| field == "apiKey"),
                    "online ASR provider requiring an API key should declare apiKey"
                );
            }
            if provider.batch.local_file_mode.supported {
                assert!(
                    !provider.batch.local_file_mode.endpoint.trim().is_empty(),
                    "online ASR local file mode endpoint should not be empty"
                );
            } else {
                assert!(
                    !provider
                        .batch
                        .local_file_mode
                        .unsupported_message
                        .trim()
                        .is_empty(),
                    "online ASR local file mode unsupported message should not be empty"
                );
            }
        }
        manifest
    })
}

fn validate_capability_config_fields(provider_id: &str, label: &str, fields: &[String]) {
    for field in fields {
        assert!(
            !field.trim().is_empty(),
            "online ASR provider {provider_id} {label} config field should not be empty"
        );
    }
}

pub fn online_asr_providers() -> &'static [OnlineAsrProvider] {
    online_asr_provider_manifest().providers.as_slice()
}

pub fn find_online_asr_provider(provider_id: &str) -> Option<&'static OnlineAsrProvider> {
    online_asr_providers()
        .iter()
        .find(|provider| provider.id == provider_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_sherpa_request_builder_sets_defaults() {
        let request = AsrTranscriptionRequest::local_sherpa(
            AsrMode::Batch,
            "model".to_string(),
            4,
            true,
            "auto".to_string(),
            None,
            None,
            5.0,
            "whisper".to_string(),
            None,
            None,
            TranscriptNormalizationOptions::default(),
            TranscriptPostprocessOptions::default(),
            None,
            None,
        );

        assert_eq!(request.engine(), AsrEngine::LocalSherpa);
        assert!(matches!(
            request.engine_config,
            AsrEngineConfig::LocalSherpa {
                batch_segmentation_mode: BatchSegmentationMode::Vad,
                ..
            }
        ));
    }

    #[test]
    fn online_request_serializes_in_camel_case() {
        let request = AsrTranscriptionRequest {
            mode: AsrMode::Streaming,
            language: "auto".to_string(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: "volcengine".to_string(),
                    profile_id: "default".to_string(),
                    config: serde_json::json!({"apiKey":"secret"}),
                },
            },
        };

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["engine"], "online");
        assert_eq!(json["mode"], "streaming");
        assert_eq!(json["onlineProvider"]["providerId"], "volcengine");
    }
}
