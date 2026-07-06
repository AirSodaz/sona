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
