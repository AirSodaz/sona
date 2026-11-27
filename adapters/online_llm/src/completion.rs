use sona_core::llm::provider_protocol::{
    MessageRole, StandardLlmRequest, StandardLlmResponse, build_standard_input,
};
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::ports::llm::LlmPortError;

pub async fn complete_with_provider(
    request: LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    if request.config.strategy.is_online_translation_api() {
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
