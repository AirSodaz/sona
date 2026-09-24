use sona_core::llm::provider_protocol::{LlmModelSummary, strategy_supports_model_listing};
use sona_core::llm::requests::LlmModelsRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortError;

use crate::aimux_adapter::{create_aimux_provider, map_aimux_error};
use crate::models_dev::{
    default_models_dev_catalog, models_dev_provider_id, should_enrich_model_metadata,
};
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

    let provider = create_aimux_provider(strategy, &request.api_key, &request.base_url, None)?;

    let runtime_models = provider.list_models().await.map_err(map_aimux_error)?;

    let discovered = runtime_models
        .into_iter()
        .map(|m| LlmModelSummary {
            model: m.id,
            ..LlmModelSummary::default()
        })
        .collect::<Vec<_>>();
    if !should_enrich_model_metadata(&request.provider, &request.base_url) {
        return Ok(discovered);
    }
    let provider_id = models_dev_provider_id(strategy).unwrap_or("");
    Ok(default_models_dev_catalog()
        .enrich(provider_id, discovered)
        .await)
}
