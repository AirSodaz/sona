use serde_json::json;
use sona_core::llm_provider_protocol::{
    GeminiModel, LlmModelSummary, MessageRole, OpenAiModel, StandardLlmRequest, StandardMessage,
    build_gemini_generate_content_request_parts, build_standard_input, clean_gemini_base_url,
    extract_text_from_json_response, extract_usage_from_json_response, format_gemini_models_url,
    format_openai_models_urls, gemini_model_to_summary, join_url, openai_model_to_summary,
    strategy_supports_model_listing, strategy_uses_openai_chat_payload,
};
use sona_core::llm_tasks::LlmProviderStrategy;
use sona_core::llm_usage::TokenUsage;

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
        gemini_summary,
        LlmModelSummary {
            model: "gemini-2.5-pro".to_string(),
            input_price: None,
            output_price: None,
            context_window: Some(1_048_576),
            max_output_tokens: Some(65_536),
            supports_multimodal: Some(true),
            supports_tools: Some(true),
            supports_reasoning: None,
        }
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
            "output_tokens": 5
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
        })
    );
    assert_eq!(
        join_url("https://api.openai.com/v1", "/v1/responses"),
        "https://api.openai.com/v1/responses"
    );
}
