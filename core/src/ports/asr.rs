use crate::model_config::ModelFileConfig;
use crate::transcribe_runtime::OfflineTranscribePlan;
use crate::transcript::TranscriptSegment;
pub use crate::transcript_postprocess::{
    TranscriptNormalizationOptions, TranscriptPostprocessOptions, TranscriptTextReplacementRule,
    TranscriptTextReplacementRuleSet,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "specta")]
use specta::Type;
use std::sync::OnceLock;

const ONLINE_ASR_PROVIDERS_JSON: &str = include_str!("online-asr-providers.json");

pub const LOCAL_SHERPA_PROVIDER_ID: &str = "local_sherpa";
pub const VOLCENGINE_DOUBAO_PROVIDER_ID: &str = "volcengine-doubao";
pub const VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY: &str = "volcengineDoubao";
pub const GROQ_WHISPER_PROVIDER_ID: &str = "groq-whisper";
pub const MISTRAL_VOXTRAL_PROVIDER_ID: &str = "mistral-voxtral";

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
    Offline,
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
pub trait OfflineTranscriber: Send + Sync {
    async fn transcribe(
        &self,
        plan: OfflineTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, String>;
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
    pub speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,

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
        speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
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
            AsrMode::Offline,
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
