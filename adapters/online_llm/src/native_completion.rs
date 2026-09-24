use sona_core::llm::provider_protocol::StandardLlmResponse;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortError;

use crate::transport::LlmApiUrl;

pub async fn execute_native_completion(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    match request.config.strategy {
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree => {
            let url = LlmApiUrl::parse(&request.config.base_url)?;
            let client = url.client(request.config.timeout_seconds)?;
            crate::providers::GoogleTranslateAdapter
                .generate(&client, request)
                .await
        }
        _ => crate::aimux_adapter::execute_aimux_completion(request).await,
    }
}
