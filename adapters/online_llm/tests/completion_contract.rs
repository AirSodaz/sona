use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use rig_core::completion::Usage;
use serde_json::json;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::{
    LlmCompletionOptions, LlmCompletionRequest, LlmPromptCachePolicy, LlmResponseFormat,
};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmCompletionPort, LlmPortErrorKind};
use sona_online_llm::{
    LlmApiUrl, OnlineLlmAdapter, build_anthropic_payload_for_request,
    build_gemini_payload_for_request, build_openai_chat_payload_for_request,
    build_openai_responses_payload, extract_anthropic_stream_usage, extract_gemini_usage,
    extract_openai_responses_stream_usage, post_json_request, token_usage_from_rig_usage,
};

fn request() -> LlmCompletionRequest {
    LlmCompletionRequest {
        config: LlmConfig {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
            strategy: LlmProviderStrategy::OpenAi,
            base_url: "https://api.example.com".into(),
            api_key: "test-key".into(),
            model: "gpt-test".into(),
            api_path: None,
            api_version: None,
            temperature: None,
            reasoning_enabled: None,
            reasoning_level: None,
            timeout_seconds: None,
        },
        system_prompt: Some("Return a compact object.".into()),
        input: "hello".into(),
        options: LlmCompletionOptions {
            temperature: Some(0.2),
            max_output_tokens: Some(256),
            response_format: LlmResponseFormat::JsonSchema {
                name: "answer".into(),
                schema: json!({"type": "object"}),
            },
            ..LlmCompletionOptions::default()
        },
        source: None,
    }
}

#[test]
fn openai_payload_applies_shared_completion_options() {
    let mut request = request();
    request.options.reasoning_enabled = Some(true);
    request.options.reasoning_level = Some("high".into());
    let payload = build_openai_chat_payload_for_request(&request, false, false).unwrap();

    assert_eq!(
        payload["messages"][0],
        json!({
            "role": "system",
            "content": "Return a compact object."
        })
    );
    assert_eq!(payload["max_tokens"], 256);
    assert_eq!(payload["response_format"]["type"], "json_schema");
    assert_eq!(payload["response_format"]["json_schema"]["name"], "answer");
    assert_eq!(
        build_openai_responses_payload(&request, false)["reasoning"]["effort"],
        "high"
    );
}

#[test]
fn rig_usage_preserves_cache_and_reasoning_breakdown() {
    let usage = token_usage_from_rig_usage(Some(Usage {
        input_tokens: 10,
        output_tokens: 4,
        total_tokens: 14,
        cached_input_tokens: 6,
        cache_creation_input_tokens: 2,
        tool_use_prompt_tokens: 0,
        reasoning_tokens: 3,
    }))
    .unwrap();

    assert_eq!(
        (
            usage.cached_input_tokens,
            usage.cache_creation_input_tokens,
            usage.reasoning_tokens,
        ),
        (6, 2, 3)
    );
}

#[test]
fn online_adapter_implements_completion_port() {
    fn assert_port<T: LlmCompletionPort>() {}
    assert_port::<OnlineLlmAdapter>();
}

#[test]
fn custom_reasoning_payloads_keep_structured_and_cache_options() {
    let mut anthropic = request();
    anthropic.config.strategy = LlmProviderStrategy::Anthropic;
    anthropic.options.reasoning_enabled = Some(true);
    anthropic.options.prompt_cache = LlmPromptCachePolicy::Automatic;
    let anthropic_payload = build_anthropic_payload_for_request(&anthropic, true).unwrap();

    assert_eq!(
        anthropic_payload["system"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert_eq!(
        anthropic_payload["output_config"]["format"]["type"],
        "json_schema"
    );

    let mut gemini = request();
    gemini.config.strategy = LlmProviderStrategy::Gemini;
    gemini.options.reasoning_enabled = Some(true);
    let gemini_payload = build_gemini_payload_for_request(&gemini).unwrap();

    assert_eq!(
        gemini_payload["systemInstruction"]["parts"][0]["text"],
        "Return a compact object."
    );
    assert_eq!(
        gemini_payload["generationConfig"]["responseMimeType"],
        "application/json"
    );
    assert_eq!(
        gemini_payload["generationConfig"]["responseJsonSchema"]["type"],
        "object"
    );
}

#[test]
fn request_reasoning_option_overrides_legacy_config() {
    let mut disabled = request();
    disabled.config.reasoning_enabled = Some(true);
    disabled.options.reasoning_enabled = Some(false);
    assert!(!disabled.effective_reasoning_enabled());

    let mut enabled = request();
    enabled.config.reasoning_enabled = Some(false);
    enabled.options.reasoning_enabled = Some(true);
    assert!(enabled.effective_reasoning_enabled());
}

#[test]
fn gemini_usage_preserves_cache_and_reasoning_breakdown() {
    let usage = extract_gemini_usage(&json!({
        "promptTokenCount": 10,
        "candidatesTokenCount": 4,
        "totalTokenCount": 17,
        "cachedContentTokenCount": 6,
        "thoughtsTokenCount": 3
    }))
    .unwrap();

    assert_eq!(
        (
            usage.total_tokens,
            usage.cached_input_tokens,
            usage.reasoning_tokens
        ),
        (17, 6, 3)
    );
}

#[test]
fn streamed_protocol_usage_keeps_provider_breakdowns() {
    let anthropic = extract_anthropic_stream_usage(&[
        json!({"message": {"usage": {
            "input_tokens": 4,
            "cache_read_input_tokens": 6,
            "cache_creation_input_tokens": 2
        }}}),
        json!({"usage": {"output_tokens": 3}}),
    ])
    .unwrap();
    assert_eq!(
        (
            anthropic.total_tokens,
            anthropic.cached_input_tokens,
            anthropic.cache_creation_input_tokens,
        ),
        (15, 6, 2)
    );

    let responses = extract_openai_responses_stream_usage(&json!({
        "type": "response.completed",
        "response": {"usage": {
            "input_tokens": 10,
            "output_tokens": 5,
            "total_tokens": 15,
            "input_tokens_details": {"cached_tokens": 4},
            "output_tokens_details": {"reasoning_tokens": 2}
        }}
    }))
    .unwrap();
    assert_eq!(
        (responses.cached_input_tokens, responses.reasoning_tokens),
        (4, 2)
    );
}

#[tokio::test]
async fn transport_preserves_retryable_error_metadata() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.read(&mut [0; 1024]);
        stream
            .write_all(
                b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 9\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            )
            .unwrap();
    });
    let error = post_json_request(
        &LlmApiUrl::parse(&format!("http://{address}")).unwrap(),
        Vec::new(),
        json!({}),
        Some(2),
    )
    .await
    .unwrap_err();
    server.join().unwrap();
    assert_eq!(
        (error.kind, error.retry_after_ms),
        (LlmPortErrorKind::RateLimited, Some(9_000))
    );
}
