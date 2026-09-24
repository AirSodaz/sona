pub mod aimux_adapter;
mod completion;
pub mod demuxer;
mod model_discovery;
mod models_dev;
pub mod native_completion;
mod providers;
mod transport;

use async_trait::async_trait;
pub use completion::{build_standard_user_input, complete_with_provider};

pub use model_discovery::list_models_with_provider;
pub use models_dev::{
    ModelsDevCatalog, models_dev_provider_id, parse_models_dev_models, should_enrich_model_metadata,
};
pub use providers::{
    GOOGLE_TRANSLATE_USER_AGENT, GoogleTranslateAdapter, GoogleTranslateData,
    GoogleTranslateFreeAttemptError, GoogleTranslateRequest, GoogleTranslateResponse,
    GoogleTranslateTranslation, build_google_translate_free_candidate_urls,
    execute_google_translate_free_request, execute_google_translate_request,
    extract_google_translate_free_translation, fetch_google_translate_free_translation,
    parse_google_translate_free_retry_after, run_google_translate_free_requests_in_order,
};
use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use sona_core::llm::requests::{LlmConfig, LlmGenerateRequest, LlmModelsRequest};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmStreamDelta};
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelListerPort, LlmModelMetadataPort,
    LlmPortError, LlmStreamingPort, LlmTaskDelayPort, LlmTextGeneratorPort, LlmTranslationPort,
    LlmTranslationRequest,
};
pub use transport::{
    LlmApiUrl, is_local_or_lan_host, parse_llm_api_host, post_json_request, validate_llm_api_host,
};

use crate::models_dev::default_models_dev_catalog;

#[derive(Clone, Copy, Debug, Default)]
pub struct OnlineLlmAdapter;

#[async_trait]
impl LlmTextGeneratorPort for OnlineLlmAdapter {
    async fn generate_text(
        &self,
        request: LlmGenerateRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        complete_with_provider(request.into()).await
    }
}

#[async_trait]
impl LlmCompletionPort for OnlineLlmAdapter {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        complete_with_provider(request).await
    }
}

#[async_trait]
impl LlmStreamingPort for OnlineLlmAdapter {
    async fn stream_completion(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let mut dual = sona_core::llm::streaming_protocol::DualStreamAccumulator::new(emit_delta);
        let stream_result = crate::aimux_adapter::execute_aimux_stream(&request, &mut dual).await;
        let emitted_any = dual.emitted_any();
        drop(dual);
        match stream_result {
            Ok(response) => Ok(response),
            Err(error)
                if !emitted_any
                    && error.kind == sona_core::ports::llm::LlmPortErrorKind::Unsupported =>
            {
                complete_with_provider(request).await
            }
            Err(error) => Err(error),
        }
    }
}

#[async_trait]
impl LlmTaskDelayPort for OnlineLlmAdapter {
    async fn delay(&self, duration: std::time::Duration) {
        tokio::time::sleep(duration).await;
    }
}

#[async_trait]
impl LlmTranslationPort for OnlineLlmAdapter {
    async fn translate_batch(
        &self,
        request: LlmTranslationRequest,
    ) -> Result<Vec<String>, LlmPortError> {
        let config = request.config;
        let target_language = request.target_language;
        let base_url = LlmApiUrl::parse(&config.base_url)?;
        let client = base_url.client(config.timeout_seconds)?;

        match config.strategy {
            sona_core::llm::tasks::LlmProviderStrategy::GoogleTranslate => {
                let response = post_json_request(
                    &base_url,
                    vec![("x-goog-api-key", config.api_key.clone())],
                    serde_json::json!(GoogleTranslateRequest {
                        q: request.texts,
                        target: target_language,
                        format: "text".to_string(),
                    }),
                    config.timeout_seconds,
                )
                .await?;
                let response: GoogleTranslateResponse =
                    serde_json::from_value(response).map_err(|error| {
                        LlmPortError::new(
                            sona_core::ports::llm::LlmPortErrorKind::Protocol,
                            format!("Invalid Google Translate response: {error}"),
                        )
                    })?;
                Ok(response
                    .data
                    .translations
                    .into_iter()
                    .map(|translation| translation.translated_text)
                    .collect())
            }
            sona_core::llm::tasks::LlmProviderStrategy::GoogleTranslateFree => {
                let mut indexed = Vec::with_capacity(request.texts.len());
                for (index, text) in request.texts.into_iter().enumerate() {
                    let fetch_client = client.clone();
                    let base_url = base_url.clone();
                    let (_, translation) = execute_google_translate_free_request(
                        index,
                        text,
                        target_language.clone(),
                        move |text, target| {
                            let client = fetch_client.clone();
                            let base_url = base_url.clone();
                            async move {
                                fetch_google_translate_free_translation(
                                    &client, &base_url, &target, &text,
                                )
                                .await
                            }
                        },
                        tokio::time::sleep,
                    )
                    .await?;
                    indexed.push((index, translation));
                }
                indexed.sort_by_key(|(index, _)| *index);
                Ok(indexed
                    .into_iter()
                    .map(|(_, translation)| translation)
                    .collect())
            }
            _ => Err(LlmPortError::new(
                sona_core::ports::llm::LlmPortErrorKind::Unsupported,
                "Direct translation is only available for Google Translate providers",
            )),
        }
    }
}

#[async_trait]
impl LlmModelListerPort for OnlineLlmAdapter {
    async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
        list_models_with_provider(request).await
    }
}

pub async fn generate_text_with_provider(
    request: LlmGenerateRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    complete_with_provider(request.into()).await
}

#[async_trait]
impl LlmModelDiscoveryPort for OnlineLlmAdapter {
    async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
        list_models_with_provider(request).await
    }
}

#[async_trait]
impl LlmModelMetadataPort for OnlineLlmAdapter {
    async fn describe_model(
        &self,
        config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError> {
        if config.model.trim().is_empty() {
            return Ok(None);
        }
        let catalog_summary = if should_enrich_model_metadata(&config.provider, &config.base_url) {
            let provider_id = models_dev_provider_id(config.strategy).unwrap_or("");
            default_models_dev_catalog()
                .describe(provider_id, &config.model)
                .await
        } else {
            None
        };

        let caps = sona_core::llm::capabilities::LlmModelCapabilities::resolve(
            config.strategy,
            &config.model,
            &config.base_url,
            catalog_summary.as_ref(),
        );

        let mut summary = catalog_summary.unwrap_or_else(|| LlmModelSummary {
            model: config.model.clone(),
            ..Default::default()
        });

        summary.supports_reasoning = Some(caps.reasoning);
        summary.reasoning_mode = Some(caps.reasoning_mode);
        summary.supported_thinking_levels = caps.supported_thinking_levels;
        summary.supports_temperature = Some(caps.supports_temperature);
        summary.token_limit_key = Some(caps.token_limit_key.as_str().to_string());

        Ok(Some(summary))
    }
}
