use sona_core::llm::provider_protocol::{
    MessageRole, StandardLlmRequest, StandardMessage, build_standard_input, join_url,
    normalize_token_usage, strategy_supports_model_listing, strategy_supports_structured_output,
    strip_and_extract_inline_thoughts,
};
use sona_core::llm::tasks::LlmProviderStrategy;

#[test]
fn join_url_accepts_common_shapes() {
    assert_eq!(
        join_url("https://api.openai.com/v1", "/chat/completions"),
        "https://api.openai.com/v1/chat/completions"
    );
}
#[test]
fn provider_strategy_and_standard_input_helpers_are_core_owned() {
    let input = build_standard_input(&StandardLlmRequest {
        messages: vec![
            StandardMessage {
                role: MessageRole::System,
                content: "system instructions".to_string(),
            },
            StandardMessage {
                role: MessageRole::User,
                content: "first question".to_string(),
            },
            StandardMessage {
                role: MessageRole::Assistant,
                content: "first answer".to_string(),
            },
            StandardMessage {
                role: MessageRole::User,
                content: "follow up".to_string(),
            },
        ],
        temperature: 0.2,
    });
    assert_eq!(input, "first question\nfollow up");

    assert_eq!(
        strategy_supports_structured_output(LlmProviderStrategy::OpenAi),
        None
    );
    assert_eq!(
        strategy_supports_structured_output(LlmProviderStrategy::DeepSeek),
        Some(false)
    );
    assert_eq!(
        strategy_supports_structured_output(LlmProviderStrategy::MoonshotAi),
        Some(false)
    );
    assert_eq!(
        strategy_supports_structured_output(LlmProviderStrategy::SiliconFlow),
        Some(false)
    );

    assert!(strategy_supports_model_listing(LlmProviderStrategy::OpenAi));
    assert!(strategy_supports_model_listing(
        LlmProviderStrategy::Anthropic
    ));
    assert!(strategy_supports_model_listing(LlmProviderStrategy::Gemini));
    assert!(strategy_supports_model_listing(LlmProviderStrategy::Cohere));
    assert!(!strategy_supports_model_listing(
        LlmProviderStrategy::GoogleTranslateFree
    ));
}

#[test]
fn normalize_token_usage_works() {
    let usage = normalize_token_usage(10, 20, 30).unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 20);
    assert_eq!(usage.total_tokens, 30);

    let usage_inferred_total = normalize_token_usage(10, 20, 0).unwrap();
    assert_eq!(usage_inferred_total.total_tokens, 30);

    assert!(normalize_token_usage(0, 0, 0).is_none());
}

#[test]
fn strip_and_extract_inline_thoughts_handles_unicode_length_changing_chars() {
    let input = "\u{212A} Kelvin <think>\nDeep thought about \u{0130}stanbul\n</think>\nResult";
    let (text, thought) = strip_and_extract_inline_thoughts(input);
    assert_eq!(text, "\u{212A} Kelvin Result");
    assert_eq!(
        thought.as_deref(),
        Some("Deep thought about \u{0130}stanbul")
    );
}
