use serde_json::Value;
use sona_core::llm::provider_protocol::{StandardLlmResponse, extract_usage_from_json_response};
use sona_core::llm::requests::LlmGenerateRequest;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::streaming_protocol::StreamTextAccumulator;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::LlmPortError;

pub async fn try_stream_completion_with_provider<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn, LlmPortError>,
) -> Result<Option<StandardLlmResponse>, LlmPortError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), LlmPortError> + Send + ?Sized,
{
    match request.config.strategy {
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree => Ok(None),
        _ => crate::rig_adapter::execute_rig_stream(request, accumulator)
            .await
            .map(Some),
    }
}

pub async fn try_stream_text_with_provider<EmitFn>(
    request: &LlmGenerateRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn, LlmPortError>,
) -> Result<Option<StandardLlmResponse>, LlmPortError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), LlmPortError> + Send + ?Sized,
{
    let completion_request = request.clone().into();
    try_stream_completion_with_provider(&completion_request, accumulator).await
}

fn update_anthropic_stream_usage(usage: &mut TokenUsage, event: &Value) {
    let source = event
        .get("message")
        .and_then(|m| m.get("usage"))
        .or_else(|| event.get("usage"));
    let Some(source) = source else {
        return;
    };

    if let Some(value) = source.get("input_tokens").and_then(Value::as_u64) {
        usage.prompt_tokens = value;
    }
    if let Some(value) = source.get("output_tokens").and_then(Value::as_u64) {
        usage.completion_tokens = value;
    }
    if let Some(value) = source
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
    {
        usage.cached_input_tokens = value;
    }
    if let Some(value) = source
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
    {
        usage.cache_creation_input_tokens = value;
    }
}

fn finish_anthropic_stream_usage(mut usage: TokenUsage) -> Option<TokenUsage> {
    usage.total_tokens = usage
        .prompt_tokens
        .saturating_add(usage.completion_tokens)
        .saturating_add(usage.cached_input_tokens)
        .saturating_add(usage.cache_creation_input_tokens);
    (usage.total_tokens > 0).then_some(usage)
}

pub fn extract_anthropic_stream_usage(events: &[Value]) -> Option<TokenUsage> {
    let mut usage = TokenUsage::default();
    for event in events {
        update_anthropic_stream_usage(&mut usage, event);
    }
    finish_anthropic_stream_usage(usage)
}

pub fn extract_openai_responses_stream_usage(event: &Value) -> Option<TokenUsage> {
    event
        .get("response")
        .and_then(extract_usage_from_json_response)
        .or_else(|| extract_usage_from_json_response(event))
}
