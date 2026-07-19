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
