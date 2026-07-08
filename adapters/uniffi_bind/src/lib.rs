use sona_core::export::ExportFormat;
use sona_core::llm::requests::{
    LlmConfig, PolishSegmentsRequest, SummarizeTranscriptRequest, TranslateSegmentsRequest,
};
use sona_core::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, find_preset_model,
};
use sona_core::ports::asr::{
    BatchSegmentationMode, OnlineAsrProviderRequest, VolcengineDoubaoAsrConfig,
};
use sona_runtime_fs::resolve_runtime_path_status;

mod mapper;
pub use mapper::{
    FfiAsrEngine, FfiAsrMode, FfiBatchSegmentationMode, FfiLlmConfig, FfiLlmProviderStrategy,
    FfiLlmSegmentInput, FfiOnlineAsrProviderRequest, FfiPolishSegmentsRequest, FfiRuntimePathKind,
    FfiRuntimePathStatus, FfiSummarizeTranscriptRequest, FfiSummarySegmentInput,
    FfiSummaryTemplateConfig, FfiTranslateSegmentsRequest, FfiVolcengineDoubaoAsrConfig,
};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SonaCoreBindingError {
    #[error("{message}")]
    InvalidInput { message: String },
}

pub type SonaCoreBindingResult<T> = Result<T, SonaCoreBindingError>;

/// Rust facade used by tests and by the top-level UniFFI exports.
pub struct SonaCoreFacade;

impl SonaCoreFacade {
    pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
        let format = ExportFormat::parse(&value)
            .map_err(|message| SonaCoreBindingError::InvalidInput { message })?;
        Ok(match format {
            ExportFormat::Json => "json",
            ExportFormat::Txt => "txt",
            ExportFormat::Srt => "srt",
            ExportFormat::Vtt => "vtt",
            ExportFormat::Md => "md",
        }
        .to_string())
    }

    pub fn default_vad_model_id() -> String {
        DEFAULT_SILERO_VAD_MODEL_ID.to_string()
    }

    pub fn default_punctuation_model_id() -> String {
        DEFAULT_PUNCTUATION_MODEL_ID.to_string()
    }

    pub fn preset_model_name(model_id: String) -> Option<String> {
        find_preset_model(&model_id).map(|model| model.name.clone())
    }

    pub fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
        mapper::runtime_path_status_to_ffi(resolve_runtime_path_status(&path))
    }

    pub fn default_batch_segmentation_mode() -> FfiBatchSegmentationMode {
        mapper::batch_segmentation_mode_to_ffi(BatchSegmentationMode::default())
    }

    pub fn online_asr_provider_request(
        provider_id: String,
        profile_id: String,
        config_json: String,
    ) -> SonaCoreBindingResult<FfiOnlineAsrProviderRequest> {
        let config = serde_json::from_str(&config_json).map_err(|error| {
            SonaCoreBindingError::InvalidInput {
                message: format!("Invalid ASR provider config JSON: {error}"),
            }
        })?;

        Ok(mapper::online_asr_provider_request_to_ffi(
            OnlineAsrProviderRequest {
                provider_id,
                profile_id,
                config,
            },
        ))
    }

    pub fn volcengine_doubao_asr_config_from_json(
        config_json: String,
    ) -> SonaCoreBindingResult<FfiVolcengineDoubaoAsrConfig> {
        let config: VolcengineDoubaoAsrConfig =
            serde_json::from_str(&config_json).map_err(|error| {
                SonaCoreBindingError::InvalidInput {
                    message: format!("Invalid Volcengine Doubao ASR config JSON: {error}"),
                }
            })?;

        Ok(mapper::volcengine_doubao_asr_config_to_ffi(config))
    }

    pub fn llm_config_from_json(config_json: String) -> SonaCoreBindingResult<FfiLlmConfig> {
        let config: LlmConfig = parse_core_json(&config_json, "LLM config")?;
        Ok(mapper::llm_config_to_ffi(config))
    }

    pub fn polish_segments_request_from_json(
        request_json: String,
    ) -> SonaCoreBindingResult<FfiPolishSegmentsRequest> {
        let request: PolishSegmentsRequest =
            parse_core_json(&request_json, "polish segments request")?;
        Ok(mapper::polish_segments_request_to_ffi(request))
    }

    pub fn translate_segments_request_from_json(
        request_json: String,
    ) -> SonaCoreBindingResult<FfiTranslateSegmentsRequest> {
        let request: TranslateSegmentsRequest =
            parse_core_json(&request_json, "translate segments request")?;
        Ok(mapper::translate_segments_request_to_ffi(request))
    }

    pub fn summarize_transcript_request_from_json(
        request_json: String,
    ) -> SonaCoreBindingResult<FfiSummarizeTranscriptRequest> {
        let request: SummarizeTranscriptRequest =
            parse_core_json(&request_json, "summarize transcript request")?;
        Ok(mapper::summarize_transcript_request_to_ffi(request))
    }
}

fn parse_core_json<T>(json: &str, label: &str) -> SonaCoreBindingResult<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(json).map_err(|error| SonaCoreBindingError::InvalidInput {
        message: format!("Invalid {label} JSON: {error}"),
    })
}

#[uniffi::export]
pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::normalize_export_format(value)
}

#[uniffi::export]
pub fn default_vad_model_id() -> String {
    SonaCoreFacade::default_vad_model_id()
}

#[uniffi::export]
pub fn default_punctuation_model_id() -> String {
    SonaCoreFacade::default_punctuation_model_id()
}

#[uniffi::export]
pub fn preset_model_name(model_id: String) -> Option<String> {
    SonaCoreFacade::preset_model_name(model_id)
}

#[uniffi::export]
pub fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
    SonaCoreFacade::runtime_path_status(path)
}

#[uniffi::export]
pub fn default_batch_segmentation_mode() -> FfiBatchSegmentationMode {
    SonaCoreFacade::default_batch_segmentation_mode()
}

#[uniffi::export]
pub fn online_asr_provider_request(
    provider_id: String,
    profile_id: String,
    config_json: String,
) -> SonaCoreBindingResult<FfiOnlineAsrProviderRequest> {
    SonaCoreFacade::online_asr_provider_request(provider_id, profile_id, config_json)
}

#[uniffi::export]
pub fn volcengine_doubao_asr_config_from_json(
    config_json: String,
) -> SonaCoreBindingResult<FfiVolcengineDoubaoAsrConfig> {
    SonaCoreFacade::volcengine_doubao_asr_config_from_json(config_json)
}

#[uniffi::export]
pub fn llm_config_from_json(config_json: String) -> SonaCoreBindingResult<FfiLlmConfig> {
    SonaCoreFacade::llm_config_from_json(config_json)
}

#[uniffi::export]
pub fn polish_segments_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiPolishSegmentsRequest> {
    SonaCoreFacade::polish_segments_request_from_json(request_json)
}

#[uniffi::export]
pub fn translate_segments_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiTranslateSegmentsRequest> {
    SonaCoreFacade::translate_segments_request_from_json(request_json)
}

#[uniffi::export]
pub fn summarize_transcript_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiSummarizeTranscriptRequest> {
    SonaCoreFacade::summarize_transcript_request_from_json(request_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_returns_owned_binding_safe_values_from_core() {
        assert_eq!(
            SonaCoreFacade::normalize_export_format("SRT".to_string()).unwrap(),
            "srt"
        );
        assert_eq!(SonaCoreFacade::default_vad_model_id(), "silero-vad");
        assert_eq!(
            SonaCoreFacade::preset_model_name("silero-vad".to_string()).as_deref(),
            find_preset_model("silero-vad").map(|model| model.name.as_str())
        );
    }

    #[test]
    fn facade_maps_core_errors_to_binding_errors() {
        let error = SonaCoreFacade::normalize_export_format("docx".to_string()).unwrap_err();
        assert_eq!(error.to_string(), "Unsupported export format: docx");
    }

    #[test]
    fn top_level_exports_delegate_to_core_facade() {
        assert_eq!(normalize_export_format("VTT".to_string()).unwrap(), "vtt");
        assert_eq!(default_punctuation_model_id(), DEFAULT_PUNCTUATION_MODEL_ID);
        assert_eq!(
            preset_model_name("missing-model".to_string()).as_deref(),
            None
        );
    }

    #[test]
    fn runtime_path_status_returns_binding_safe_record() {
        let missing_path = std::env::temp_dir()
            .join("sona-uniffi-bind-missing-runtime-path")
            .to_string_lossy()
            .into_owned();

        let status = SonaCoreFacade::runtime_path_status(missing_path.clone());

        assert_eq!(status.path, missing_path);
        assert_eq!(status.kind, FfiRuntimePathKind::Missing);
        assert_eq!(status.error, None);
    }

    #[test]
    fn facade_maps_asr_contract_values_to_binding_safe_records() {
        assert_eq!(
            SonaCoreFacade::default_batch_segmentation_mode(),
            FfiBatchSegmentationMode::Vad
        );

        let provider = SonaCoreFacade::online_asr_provider_request(
            "volcengine".to_string(),
            "default".to_string(),
            r#"{"apiKey":"secret"}"#.to_string(),
        )
        .unwrap();

        assert_eq!(provider.provider_id, "volcengine");
        assert_eq!(provider.profile_id, "default");
        assert_eq!(provider.config_json, r#"{"apiKey":"secret"}"#);
    }

    #[test]
    fn facade_maps_volcengine_asr_config_from_json() {
        let config = SonaCoreFacade::volcengine_doubao_asr_config_from_json(
            r#"{
                "apiKey": "secret",
                "streamingEndpoint": "wss://stream",
                "streamingResourceId": "stream-resource",
                "batchEndpoint": "https://batch",
                "batchResourceId": "batch-resource"
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(config.api_key, "secret");
        assert_eq!(config.streaming_endpoint, "wss://stream");
        assert_eq!(config.streaming_resource_id, "stream-resource");
        assert_eq!(config.batch_endpoint, "https://batch");
        assert_eq!(config.batch_resource_id, "batch-resource");
    }

    #[test]
    fn facade_rejects_invalid_asr_provider_config_json() {
        let error = SonaCoreFacade::online_asr_provider_request(
            "volcengine".to_string(),
            "default".to_string(),
            "{bad-json".to_string(),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Invalid ASR provider config JSON")
        );
    }

    #[test]
    fn facade_maps_llm_config_and_segment_task_requests_from_json() {
        let config = SonaCoreFacade::llm_config_from_json(
            r#"{
                "provider": "open_ai",
                "baseUrl": "https://api.openai.com",
                "apiKey": "secret",
                "model": "gpt-4o-mini",
                "temperature": 0.2,
                "timeoutSeconds": 30
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(config.provider_id, "open_ai");
        assert_eq!(config.strategy, FfiLlmProviderStrategy::OpenAi);
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.temperature, Some(0.2));
        assert_eq!(config.timeout_seconds, Some(30));

        let polish = SonaCoreFacade::polish_segments_request_from_json(
            r#"{
                "taskId": "polish-1",
                "config": {
                    "provider": "open_ai",
                    "baseUrl": "https://api.openai.com",
                    "apiKey": "secret",
                    "model": "gpt-4o-mini"
                },
                "segments": [{"id": "s1", "text": "hello"}],
                "chunkSize": 4,
                "context": "meeting",
                "keywords": "roadmap"
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(polish.task_id, "polish-1");
        assert_eq!(polish.segments[0].id, "s1");
        assert_eq!(polish.chunk_size, Some(4));
        assert_eq!(polish.context.as_deref(), Some("meeting"));

        let translate = SonaCoreFacade::translate_segments_request_from_json(
            r#"{
                "taskId": "translate-1",
                "config": {
                    "provider": "google_translate_free",
                    "baseUrl": "https://translate.googleapis.com/translate_a/single",
                    "apiKey": "",
                    "model": "translate"
                },
                "segments": [{"id": "s1", "text": "hello"}],
                "targetLanguage": "ja",
                "targetLanguageName": "Japanese"
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(
            translate.config.strategy,
            FfiLlmProviderStrategy::GoogleTranslateFree
        );
        assert_eq!(translate.target_language, "ja");
        assert_eq!(translate.target_language_name.as_deref(), Some("Japanese"));

        let summary = SonaCoreFacade::summarize_transcript_request_from_json(
            r#"{
                "taskId": "summary-1",
                "config": {
                    "provider": "open_ai",
                    "baseUrl": "https://api.openai.com",
                    "apiKey": "secret",
                    "model": "gpt-4o-mini"
                },
                "template": {
                    "id": "general",
                    "name": "General",
                    "instructions": "Summarize."
                },
                "segments": [{
                    "id": "s1",
                    "text": "hello",
                    "start": 0.0,
                    "end": 1.5,
                    "isFinal": true
                }],
                "chunkCharBudget": 1200
            }"#
            .to_string(),
        )
        .unwrap();

        assert_eq!(summary.template.id, "general");
        assert_eq!(summary.segments[0].end, 1.5);
        assert_eq!(summary.chunk_char_budget, Some(1200));
    }
}
