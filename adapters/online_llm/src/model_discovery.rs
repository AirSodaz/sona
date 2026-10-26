use sona_core::llm::provider_protocol::{
    LlmModelMetadataSource, LlmModelSummary, strategy_supports_model_listing,
};
use sona_core::llm::requests::LlmModelsRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortError;

use crate::aimux_adapter::{create_aimux_provider, map_aimux_error};
use crate::models_dev::{
    default_models_dev_catalog, models_dev_provider_id, should_enrich_model_metadata,
};
use crate::provider_discovery::discover_native_provider_models;
use crate::transport::validate_llm_api_host;
pub async fn list_models_with_provider(
    request: LlmModelsRequest,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let strategy = request
        .strategy
        .unwrap_or_else(|| LlmProviderStrategy::from_provider(&request.provider));
    if !strategy_supports_model_listing(strategy) {
        return Ok(vec![]);
    }
    validate_llm_api_host(&request.base_url)?;

    // 1. Attempt native proprietary provider discovery
    let discovered = match discover_native_provider_models(
        strategy,
        &request.api_key,
        &request.base_url,
    )
    .await
    {
        Some(Ok(models)) => models,
        Some(Err(err)) => return Err(err),
        None => {
            // 2. Fall back to standard aimux model listing
            let provider =
                create_aimux_provider(strategy, &request.api_key, &request.base_url, None)?;
            let runtime_models = provider.list_models().await.map_err(map_aimux_error)?;
            runtime_models
                .into_iter()
                .map(|m| LlmModelSummary {
                    model: m.id,
                    metadata_sources: vec![LlmModelMetadataSource::Provider],
                    ..LlmModelSummary::default()
                })
                .collect::<Vec<_>>()
        }
    };
    // 3. Complete metadata via models.dev catalog
    let enriched = if !should_enrich_model_metadata(&request.provider, &request.base_url) {
        discovered
    } else {
        let provider_id = models_dev_provider_id(strategy).unwrap_or("");
        default_models_dev_catalog()
            .enrich(provider_id, discovered)
            .await
    };

    // 4. Resolve capabilities via built-in rules
    let results = enriched
        .into_iter()
        .map(|mut summary| {
            let caps = sona_core::llm::capabilities::LlmModelCapabilities::resolve(
                strategy,
                &summary.model,
                &request.base_url,
                Some(&summary),
            );
            summary.supports_reasoning = Some(caps.reasoning);
            summary.reasoning_mode = Some(caps.reasoning_mode);
            summary.supported_thinking_levels = caps.supported_thinking_levels;
            summary.supports_temperature = Some(caps.supports_temperature);
            summary.token_limit_key = Some(caps.token_limit_key.as_str().to_string());
            summary
        })
        .collect();

    Ok(results)
}
