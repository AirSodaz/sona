use sona_core::llm::provider_protocol::{
    MessageRole, StandardLlmRequest, StandardLlmResponse, build_standard_input,
    normalize_token_usage,
};
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::LlmPortError;

#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub tool_use_prompt_tokens: u64,
    pub reasoning_tokens: u64,
}

pub fn token_usage_from_rig_usage(usage: Option<Usage>) -> Option<TokenUsage> {
    usage.and_then(|usage| {
        let mut normalized =
            normalize_token_usage(usage.input_tokens, usage.output_tokens, usage.total_tokens)?;
        normalized.cached_input_tokens = usage.cached_input_tokens;
        normalized.cache_creation_input_tokens = usage.cache_creation_input_tokens;
        normalized.reasoning_tokens = usage.reasoning_tokens;
        Some(normalized)
    })
}

pub async fn complete_with_provider(
    request: LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    if matches!(
        request.config.strategy,
        sona_core::llm::tasks::LlmProviderStrategy::GoogleTranslate
            | sona_core::llm::tasks::LlmProviderStrategy::GoogleTranslateFree
    ) {
        crate::native_completion::execute_native_completion(&request).await
    } else {
        crate::aimux_adapter::execute_aimux_completion(&request).await
    }
}

pub fn build_standard_user_input(input: impl Into<String>, temperature: f32) -> String {
    build_standard_input(&StandardLlmRequest {
        messages: vec![sona_core::llm::provider_protocol::StandardMessage {
            role: MessageRole::User,
            content: input.into(),
        }],
        temperature,
    })
}
