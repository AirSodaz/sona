use serde_json::json;
use sona_core::llm::streaming_protocol::{
    OpenAiChatPayloadConfig, OpenAiStreamUrlConfig, SseEventBuffer, StreamTextAccumulator,
    StreamingLineBuffer, build_openai_chat_payload, build_openai_stream_url,
};
use sona_core::llm::tasks::LlmProviderStrategy;
use std::convert::Infallible;

#[test]
fn stream_text_accumulator_emits_full_text_and_delta() {
    let mut emitted = Vec::new();
    let mut emit_delta = |text: &str, delta: &str| {
        emitted.push((text.to_string(), delta.to_string()));
        Ok::<(), Infallible>(())
    };
    let mut accumulator = StreamTextAccumulator::new(&mut emit_delta);

    accumulator.push("").expect("empty delta should be ignored");
    accumulator.push("Hel").expect("first delta should emit");
    accumulator.push("lo").expect("second delta should emit");

    drop(accumulator);
    assert_eq!(
        emitted,
        vec![
            ("Hel".to_string(), "Hel".to_string()),
            ("Hello".to_string(), "lo".to_string()),
        ]
    );
}

#[derive(Debug, PartialEq, Eq)]
struct EmitFailure(&'static str);

#[test]
fn stream_text_accumulator_preserves_callback_error_type() {
    let mut emit_delta = |_text: &str, _delta: &str| Err(EmitFailure("observer closed"));
    let mut accumulator = StreamTextAccumulator::new(&mut emit_delta);

    assert_eq!(
        accumulator.push("hello").unwrap_err(),
        EmitFailure("observer closed")
    );
}

#[test]
fn streaming_line_buffer_reassembles_partial_lines() {
    let mut buffer = StreamingLineBuffer::default();

    assert!(buffer.process("data: {\"a\"").is_empty());
    assert_eq!(buffer.process(":1}\nnext"), vec!["data: {\"a\":1}"]);
    assert_eq!(buffer.flush(), vec!["next"]);
    assert!(buffer.flush().is_empty());
}

#[test]
fn sse_event_buffer_joins_data_lines_until_blank_separator() {
    let mut buffer = SseEventBuffer::default();

    assert!(buffer.process("event: delta\ndata: {\"a\":").is_empty());
    assert_eq!(
        buffer.process("1}\ndata: {\"b\":2}\n\n"),
        vec!["{\"a\":1}\n{\"b\":2}"]
    );
    assert_eq!(buffer.process("data: [DONE]"), Vec::<String>::new());
    assert_eq!(buffer.flush(), vec!["[DONE]"]);
}

#[test]
fn openai_stream_url_is_provider_strategy_aware() {
    assert_eq!(
        build_openai_stream_url(OpenAiStreamUrlConfig {
            strategy: LlmProviderStrategy::AzureOpenAi,
            base_url: "https://azure.example.com/openai",
            model: "deployment-a",
            api_path: None,
            api_version: None,
        }),
        "https://azure.example.com/openai/openai/deployments/deployment-a/chat/completions?api-version=2024-10-21"
    );
    assert_eq!(
        build_openai_stream_url(OpenAiStreamUrlConfig {
            strategy: LlmProviderStrategy::Perplexity,
            base_url: "https://api.perplexity.ai/",
            model: "sonar",
            api_path: Some("/chat/completions"),
            api_version: None,
        }),
        "https://api.perplexity.ai/chat/completions"
    );
    assert_eq!(
        build_openai_stream_url(OpenAiStreamUrlConfig {
            strategy: LlmProviderStrategy::OpenAi,
            base_url: "https://api.openai.com/v1",
            model: "gpt-4.1-mini",
            api_path: Some("/v1/chat/completions"),
            api_version: None,
        }),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[test]
fn openai_chat_payload_preserves_stream_and_reasoning_options() {
    let payload = build_openai_chat_payload(
        OpenAiChatPayloadConfig {
            strategy: LlmProviderStrategy::OpenAi,
            model: "gpt-4.1-mini",
            temperature: Some(0.35),
            reasoning_enabled: true,
            reasoning_level: Some("high"),
        },
        "hello",
        true,
    );

    assert_eq!(payload["model"], "gpt-4.1-mini");
    assert_eq!(payload["stream"], true);
    assert_eq!(payload["stream_options"], json!({"include_usage": true}));
    assert_eq!(payload["reasoning_effort"], "high");
    let temperature = payload["temperature"]
        .as_f64()
        .expect("temperature should be numeric");
    assert!((temperature - 0.35).abs() < 0.000_001);

    let azure_payload = build_openai_chat_payload(
        OpenAiChatPayloadConfig {
            strategy: LlmProviderStrategy::AzureOpenAi,
            model: "deployment-a",
            temperature: None,
            reasoning_enabled: false,
            reasoning_level: None,
        },
        "hello",
        true,
    );

    assert!(azure_payload.get("model").is_none());
    assert_eq!(azure_payload["stream"], true);
    assert!(azure_payload.get("stream_options").is_none());
    let azure_temperature = azure_payload["temperature"]
        .as_f64()
        .expect("temperature should be numeric");
    assert!((azure_temperature - 0.7).abs() < 0.000_001);
}

#[test]
fn openai_chat_payload_omits_temperature_for_prohibited_models() {
    for model in [
        "o1",
        "o1-mini",
        "o1-preview",
        "o3-mini",
        "deepseek-reasoner",
        "openai/o1-preview",
        "openai/o3-mini",
        "deepseek/deepseek-r1",
    ] {
        let payload = build_openai_chat_payload(
            OpenAiChatPayloadConfig {
                strategy: LlmProviderStrategy::OpenAi,
                model,
                temperature: Some(0.5),
                reasoning_enabled: true,
                reasoning_level: Some("high"),
            },
            "hello",
            true,
        );
        assert!(
            payload.get("temperature").is_none(),
            "model {model} must not include temperature"
        );
        assert_eq!(payload["reasoning_effort"], "high");
    }

    // When reasoning_level is "auto" or budget tokens, do not send invalid reasoning_effort to OpenAI
    let payload = build_openai_chat_payload(
        OpenAiChatPayloadConfig {
            strategy: LlmProviderStrategy::OpenAi,
            model: "o3-mini",
            temperature: Some(0.5),
            reasoning_enabled: true,
            reasoning_level: Some("auto"),
        },
        "hello",
        true,
    );
    assert!(payload.get("reasoning_effort").is_none());
}

#[test]
fn dual_stream_accumulator_separates_thought_and_content() {
    use sona_core::llm::runtime::LlmStreamDelta;
    use sona_core::llm::streaming_protocol::DualStreamAccumulator;

    let mut events = Vec::new();
    let mut emit = |delta: LlmStreamDelta| {
        events.push(delta);
        Ok::<(), ()>(())
    };
    let mut accumulator = DualStreamAccumulator::new(&mut emit);

    accumulator.push_thought("Thinking step 1...").unwrap();
    accumulator.push_thought("Thinking step 2...").unwrap();
    accumulator.push_content("Final ").unwrap();
    accumulator.push_content("Answer").unwrap();

    assert_eq!(
        accumulator.thought_text(),
        "Thinking step 1...Thinking step 2..."
    );
    assert_eq!(accumulator.content_text(), "Final Answer");
    assert_eq!(events.len(), 4);
    assert!(events[0].is_thought());
    assert_eq!(events[0].delta, "Thinking step 1...");
    assert!(events[1].is_thought());
    assert_eq!(events[1].delta, "Thinking step 2...");
    assert!(!events[2].is_thought());
    assert_eq!(events[2].delta, "Final ");
    assert!(!events[3].is_thought());
    assert_eq!(events[3].delta, "Answer");
}

#[test]
fn openai_chat_payload_omits_reasoning_effort_when_level_is_none() {
    let payload = build_openai_chat_payload(
        OpenAiChatPayloadConfig {
            strategy: LlmProviderStrategy::OpenAi,
            model: "o3-mini",
            temperature: Some(0.5),
            reasoning_enabled: true,
            reasoning_level: Some("none"),
        },
        "hello",
        true,
    );
    assert!(
        payload.get("reasoning_effort").is_none(),
        "reasoning_effort must not be set to 'none'"
    );
}

#[test]
fn stream_options_only_added_for_whitelisted_strategies() {
    use sona_core::llm::streaming_protocol::strategy_supports_stream_options;

    assert!(strategy_supports_stream_options(
        LlmProviderStrategy::OpenAi
    ));
    assert!(strategy_supports_stream_options(
        LlmProviderStrategy::DeepSeek
    ));
    assert!(strategy_supports_stream_options(LlmProviderStrategy::Groq));
    assert!(strategy_supports_stream_options(
        LlmProviderStrategy::OpenRouter
    ));

    assert!(!strategy_supports_stream_options(
        LlmProviderStrategy::Ollama
    ));
    assert!(!strategy_supports_stream_options(
        LlmProviderStrategy::Llamafile
    ));
    assert!(!strategy_supports_stream_options(
        LlmProviderStrategy::Local
    ));
    assert!(!strategy_supports_stream_options(
        LlmProviderStrategy::OpenAiCompatible
    ));
    assert!(!strategy_supports_stream_options(
        LlmProviderStrategy::AzureOpenAi
    ));
}
