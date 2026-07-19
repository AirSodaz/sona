use serde_json::json;
use sona_core::llm::provider_protocol::{
    GeminiModel, MessageRole, OpenAiModel, StandardLlmRequest, StandardMessage,
    build_gemini_generate_content_request_parts, build_standard_input, clean_gemini_base_url,
    extract_anthropic_text_response, extract_text_from_json_response,
    extract_usage_from_json_response, format_gemini_models_url, format_openai_models_urls,
    gemini_model_to_summary, join_url, openai_model_to_summary, strategy_supports_model_listing,
    strategy_uses_openai_chat_payload,
};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::LlmPortErrorKind;

#[test]
fn provider_model_urls_accept_common_base_url_shapes() {
    assert_eq!(
        format_openai_models_urls("https://api.openai.com", false),
        vec![
            "https://api.openai.com/v1/models".to_string(),
            "https://api.openai.com/models".to_string(),
        ]
    );
    assert_eq!(
        format_openai_models_urls("https://api.openai.com/v1", false),
        vec!["https://api.openai.com/v1/models".to_string()]
    );
    assert_eq!(
        clean_gemini_base_url("https://generativelanguage.googleapis.com/v1beta/openai"),
        "https://generativelanguage.googleapis.com"
    );
    assert_eq!(
        format_gemini_models_url("https://generativelanguage.googleapis.com/v1beta/openai"),
        "https://generativelanguage.googleapis.com/v1beta/models"
    );
}

#[test]
fn gemini_generate_content_request_keeps_api_key_in_headers() {
    let request = build_gemini_generate_content_request_parts(
        "https://generativelanguage.googleapis.com/v1beta/models",
        "models/gemini-2.5-pro",
        "secret-stream-key",
        true,
    )
    .expect("request parts should be built");

    assert_eq!(
        request.url,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
    );
    assert!(!request.url.contains("secret-stream-key"));
    assert_eq!(
        request.headers,
        vec![("x-goog-api-key", "secret-stream-key".to_string())]
    );
}

#[test]
fn provider_protocol_errors_preserve_invalid_request_and_protocol_kinds() {
    let invalid_model = build_gemini_generate_content_request_parts(
        "https://generativelanguage.googleapis.com",
        "models/  ",
        "secret-key",
        false,
    )
    .unwrap_err();
    assert_eq!(invalid_model.kind, LlmPortErrorKind::InvalidRequest);
    assert_eq!(invalid_model.message, "Gemini model cannot be empty");

    let missing_text = extract_text_from_json_response(&json!({"choices": []})).unwrap_err();
    assert_eq!(missing_text.kind, LlmPortErrorKind::Protocol);
    assert_eq!(
        missing_text.message,
        "LLM response did not contain text output"
    );

    let malformed_anthropic = extract_anthropic_text_response(&json!({})).unwrap_err();
    assert_eq!(malformed_anthropic.kind, LlmPortErrorKind::Protocol);
    assert_eq!(
        malformed_anthropic.message,
        "Anthropic response missing content array"
    );
}

#[test]
fn provider_model_summaries_are_core_owned() {
    let gemini_summary = gemini_model_to_summary(GeminiModel {
        name: "models/gemini-2.5-pro".to_string(),
        supported_generation_methods: Some(vec![
            "generateContent".to_string(),
            "embedContent".to_string(),
        ]),
        input_token_limit: Some(1_048_576),
        output_token_limit: Some(65_536),
    })
    .expect("gemini text model should be converted");
    let openai_summary = openai_model_to_summary(OpenAiModel {
        id: "gpt-4.1-mini".to_string(),
    });

    assert_eq!(
        (
            gemini_summary.model.as_str(),
            gemini_summary.context_window,
            gemini_summary.max_output_tokens,
            gemini_summary.supports_multimodal,
            gemini_summary.supports_tools,
        ),
        (
            "gemini-2.5-pro",
            Some(1_048_576),
            Some(65_536),
            Some(true),
            Some(true),
        )
    );
    assert_eq!(openai_summary.model, "gpt-4.1-mini");
    assert_eq!(openai_summary.context_window, None);
}

#[test]
fn provider_strategy_and_standard_input_helpers_are_core_owned() {
    assert!(!strategy_supports_model_listing(
        LlmProviderStrategy::Anthropic
    ));
    assert!(strategy_supports_model_listing(
        LlmProviderStrategy::OpenAiCompatible
    ));
    assert!(strategy_uses_openai_chat_payload(
        LlmProviderStrategy::OpenRouter
    ));
    assert!(!strategy_uses_openai_chat_payload(
        LlmProviderStrategy::Gemini
    ));

    let request = StandardLlmRequest {
        messages: vec![
            StandardMessage {
                role: MessageRole::System,
                content: "system".to_string(),
            },
            StandardMessage {
                role: MessageRole::User,
                content: "hello".to_string(),
            },
            StandardMessage {
                role: MessageRole::User,
                content: "world".to_string(),
            },
        ],
        temperature: 0.7,
    };

    assert_eq!(build_standard_input(&request), "hello\nworld");
}

#[test]
fn response_text_and_usage_are_extracted_without_adapter_state() {
    let response = json!({
        "choices": [
            {
                "message": {
                    "content": "Hello from chat completions"
                }
            }
        ],
        "usage": {
            "input_tokens": 3,
            "output_tokens": 5,
            "prompt_tokens_details": {"cached_tokens": 2},
            "completion_tokens_details": {"reasoning_tokens": 1}
        }
    });

    assert_eq!(
        extract_text_from_json_response(&response).unwrap(),
        "Hello from chat completions"
    );
    assert_eq!(
        extract_usage_from_json_response(&response),
        Some(TokenUsage {
            prompt_tokens: 3,
            completion_tokens: 5,
            total_tokens: 8,
            cached_input_tokens: 2,
            reasoning_tokens: 1,
            ..TokenUsage::default()
        })
    );
    assert_eq!(
        join_url("https://api.openai.com/v1", "/v1/responses"),
        "https://api.openai.com/v1/responses"
    );
}

#[test]
fn anthropic_usage_preserves_prompt_cache_breakdown() {
    let (_, usage) = extract_anthropic_text_response(&json!({
        "content": [{"type": "text", "text": "ok"}],
        "usage": {
            "input_tokens": 10,
            "output_tokens": 4,
            "cache_read_input_tokens": 6,
            "cache_creation_input_tokens": 2
        }
    }))
    .unwrap();
    let usage = usage.unwrap();

    assert_eq!(
        (usage.cached_input_tokens, usage.cache_creation_input_tokens),
        (6, 2)
    );
}
