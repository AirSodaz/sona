use sona_core::llm::provider_protocol::{
    GeminiModel, MessageRole, OpenAiModel, StandardLlmRequest, StandardMessage,
    build_standard_input, clean_gemini_base_url, format_gemini_models_url,
    format_openai_models_urls, gemini_model_to_summary, join_url, normalize_token_usage,
    openai_model_to_summary, strategy_supports_model_listing, strategy_supports_structured_output,
    strategy_uses_openai_chat_payload, strip_and_extract_inline_thoughts,
};
use sona_core::llm::tasks::LlmProviderStrategy;

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
    assert_eq!(
        join_url("https://api.openai.com/v1", "/chat/completions"),
        "https://api.openai.com/v1/chat/completions"
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

    assert!(strategy_uses_openai_chat_payload(
        LlmProviderStrategy::OpenAi
    ));
    assert!(strategy_uses_openai_chat_payload(
        LlmProviderStrategy::OpenRouter
    ));
    assert!(strategy_uses_openai_chat_payload(
        LlmProviderStrategy::DeepSeek
    ));
    assert!(!strategy_uses_openai_chat_payload(
        LlmProviderStrategy::Anthropic
    ));
    assert!(!strategy_uses_openai_chat_payload(
        LlmProviderStrategy::Gemini
    ));

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
