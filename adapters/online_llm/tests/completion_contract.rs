use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use serde_json::json;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::{
    LlmCompletionOptions, LlmCompletionRequest, LlmPromptCachePolicy, LlmResponseFormat,
    LlmStreamDelta,
};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmCompletionPort, LlmPortErrorKind, LlmStreamingPort};
use sona_online_llm::Usage;
use sona_online_llm::{
    LlmApiUrl, OnlineLlmAdapter, build_anthropic_payload_for_request,
    build_gemini_payload_for_request, build_openai_chat_payload_for_request,
    build_openai_responses_payload, extract_anthropic_stream_usage, extract_gemini_usage,
    extract_gemini_visible_text, extract_openai_responses_stream_usage, post_json_request,
    token_usage_from_rig_usage,
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
fn openai_responses_payload_suppresses_temperature_for_reasoning_models() {
    let mut request = request();
    request.config.model = "o3-mini".into();
    request.options.temperature = Some(0.7);
    request.options.reasoning_enabled = Some(true);
    request.options.reasoning_level = Some("high".into());
    let payload = build_openai_responses_payload(&request, false);

    assert!(payload.get("temperature").is_none());
    assert_eq!(payload["reasoning"]["effort"], "high");
}

#[test]
fn anthropic_payload_uses_custom_budget_tokens() {
    let mut request = request();
    request.config.strategy = LlmProviderStrategy::Anthropic;
    request.config.model = "claude-3-7-sonnet-20250219".into();
    request.options.reasoning_enabled = Some(true);
    request.options.reasoning_level = Some("12000".into());
    request.options.max_output_tokens = Some(16384);
    let payload = build_anthropic_payload_for_request(&request, false).unwrap();

    assert_eq!(payload["thinking"]["budget_tokens"], 12000);
    assert_eq!(payload["temperature"], 1.0);
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
fn custom_reasoning_payloads_preserve_options_and_visible_output() {
    let mut anthropic = request();
    anthropic.config.strategy = LlmProviderStrategy::Anthropic;
    anthropic.options.reasoning_enabled = Some(true);
    anthropic.options.prompt_cache = LlmPromptCachePolicy::Automatic;
    anthropic.options.max_output_tokens = Some(8192);
    let anthropic_payload = build_anthropic_payload_for_request(&anthropic, true).unwrap();

    assert_eq!(
        anthropic_payload["system"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert_eq!(
        anthropic_payload["output_config"]["format"]["type"],
        "json_schema"
    );
    assert!(
        anthropic_payload["thinking"]["budget_tokens"]
            .as_u64()
            .unwrap()
            < 8192
    );

    let mut gemini = request();
    gemini.config.strategy = LlmProviderStrategy::Gemini;
    gemini.config.model = "gemini-2.5-flash".into();
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
    assert_eq!(
        gemini_payload["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        256
    );

    anthropic.options.max_output_tokens = Some(1024);
    assert_eq!(
        build_anthropic_payload_for_request(&anthropic, false)
            .unwrap_err()
            .kind,
        LlmPortErrorKind::InvalidRequest
    );
    assert_eq!(
        extract_gemini_visible_text(&json!({
            "candidates": [{"content": {"parts": [
                {"thought": true, "text": "analysis"}, {"text": "answer"}
            ]}}]
        })),
        Some("answer".into())
    );
}

#[test]
fn payload_builders_preserve_invalid_schema_category() {
    let mut request = request();
    request.options.response_format = LlmResponseFormat::JsonSchema {
        name: "invalid".into(),
        schema: json!(42),
    };

    for error in [
        build_openai_chat_payload_for_request(&request, false, false).unwrap_err(),
        build_anthropic_payload_for_request(&request, false).unwrap_err(),
        build_gemini_payload_for_request(&request).unwrap_err(),
    ] {
        assert_eq!(error.kind, LlmPortErrorKind::InvalidRequest);
        assert!(error.message.contains("JSON Schema"));
    }
}

#[test]
fn json_object_mode_does_not_send_a_native_schema() {
    let mut request = request();
    request.options.response_format = LlmResponseFormat::JsonObject;

    let chat = build_openai_chat_payload_for_request(&request, false, false).unwrap();
    let responses = build_openai_responses_payload(&request, false);
    let anthropic = build_anthropic_payload_for_request(&request, false).unwrap();
    let gemini = build_gemini_payload_for_request(&request).unwrap();
    assert_eq!(
        json!({
            "chat": chat["response_format"],
            "responses": responses["text"]["format"],
            "anthropicSchema": anthropic["output_config"],
            "geminiMime": gemini["generationConfig"]["responseMimeType"],
            "geminiSchema": gemini["generationConfig"]["responseJsonSchema"],
        }),
        json!({
            "chat": {"type": "json_object"},
            "responses": {"type": "json_object"},
            "anthropicSchema": null,
            "geminiMime": "application/json",
            "geminiSchema": null,
        })
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
fn native_provider_from_request_supports_azure_openai() {
    let mut azure_request = request();
    azure_request.config.strategy = LlmProviderStrategy::AzureOpenAi;
    azure_request.config.base_url = "https://example.openai.azure.com/".into();
    azure_request.config.api_key = "azure-key".into();
    azure_request.config.model = "gpt-4o".into();
    azure_request.config.api_version = Some("2024-10-21".into());

    let (url, headers) =
        sona_online_llm::native_completion::resolve_strategy_url_and_headers(&azure_request, false)
            .unwrap();
    assert_eq!(
        url.as_str(),
        "https://example.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-10-21"
    );
    assert_eq!(headers, vec![("api-key", "azure-key".to_string())]);
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

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut data = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = stream.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            let header_str = String::from_utf8_lossy(&data[..pos]);
            let content_length = header_str
                .lines()
                .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))
                .and_then(|line| line.split(':').nth(1))
                .and_then(|val| val.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if data.len() >= pos + 4 + content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&data).to_string()
}
#[tokio::test]
async fn transport_preserves_retryable_error_metadata() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut stream);
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
        // See the streaming twin below: a long budget keeps the assertion about
        // status mapping rather than about timing.
        Some(60),
    )
    .await
    .unwrap_err();
    server.join().unwrap();
    assert_eq!(
        (error.kind, error.retry_after_ms),
        (LlmPortErrorKind::RateLimited, Some(9_000))
    );
}

#[tokio::test]
async fn streaming_transport_preserves_retryable_status_metadata() {
    for (status, retry_after, expected_kind, expected_retry_after) in [
        (
            "429 Too Many Requests",
            Some("9"),
            LlmPortErrorKind::RateLimited,
            Some(9_000),
        ),
        (
            "529 Site Overloaded",
            None,
            LlmPortErrorKind::Unavailable,
            None,
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let retry_after = retry_after.map(str::to_string);
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_http_request(&mut stream);
            let retry_header = retry_after
                .map(|value| format!("Retry-After: {value}\r\n"))
                .unwrap_or_default();
            let response = format!(
                "HTTP/1.1 {status}\r\n{retry_header}Content-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let mut stream_request = request();
        stream_request.config.base_url = format!("http://{address}");
        // Generous on purpose. The subject here is the status-to-error-kind
        // mapping, not the timeout: the local server answers immediately, so a
        // long budget costs nothing on the passing path. A tight one made this
        // test flaky, because under a loaded parallel run the client could time
        // out first and report `Unavailable` instead of the mapped kind.
        stream_request.config.timeout_seconds = Some(60);
        let mut emit = |_delta: LlmStreamDelta| Ok(());
        let error = OnlineLlmAdapter
            .stream_completion(stream_request, &mut emit)
            .await
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(
            (error.kind, error.retry_after_ms),
            (expected_kind, expected_retry_after)
        );
    }
}

#[tokio::test]
async fn openai_adapter_posts_to_chat_completions_with_messages() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request_str = read_http_request(&mut stream);
        assert!(
            request_str.starts_with("POST /v1/chat/completions HTTP/1.1"),
            "expected POST /v1/chat/completions but got request line: {}",
            request_str.lines().next().unwrap_or_default()
        );
        let body = request_str.split("\r\n\r\n").nth(1).unwrap_or_default();
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert!(
            parsed.get("messages").is_some(),
            "expected 'messages' field in payload, got: {body}"
        );
        assert!(
            parsed.get("input").is_none(),
            "did not expect 'input' field in chat payload"
        );
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"id\":\"chatcmpl-test\",\"object\":\"chat.completion\",\"created\":12345,\"model\":\"test-model\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":\"test answer\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":2,\"total_tokens\":7}}";
        stream.write_all(response.as_bytes()).unwrap();
    });

    let mut completion_request = request();
    completion_request.config.base_url = format!("http://{address}");
    completion_request.options.reasoning_enabled = Some(false);
    completion_request.options.response_format = LlmResponseFormat::Text;

    let response = OnlineLlmAdapter.complete(completion_request).await.unwrap();
    server.join().unwrap();
    assert_eq!(response.text, "test answer");
}
#[tokio::test]
async fn native_rig_providers_post_to_expected_endpoint_paths() {
    let test_cases = [
        (
            LlmProviderStrategy::Cohere,
            "POST /v2/chat HTTP/1.1",
            r#"{"id":"cohere-1","finish_reason":"COMPLETE","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}"#,
        ),
        (
            LlmProviderStrategy::Together,
            "POST /v1/chat/completions HTTP/1.1",
            r#"{"id":"tgt-1","object":"chat.completion","created":123,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        ),
        (
            LlmProviderStrategy::Hyperbolic,
            "POST /v1/chat/completions HTTP/1.1",
            r#"{"id":"hyp-1","object":"chat.completion","created":123,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        ),
        (
            LlmProviderStrategy::Llamafile,
            "POST /v1/chat/completions HTTP/1.1",
            r#"{"id":"lf-1","object":"chat.completion","created":123,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        ),
        (
            LlmProviderStrategy::MistralAi,
            "POST /v1/chat/completions HTTP/1.1",
            r#"{"id":"mis-1","object":"chat.completion","created":123,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        ),
        (
            LlmProviderStrategy::Groq,
            "POST /chat/completions HTTP/1.1",
            r#"{"id":"grq-1","object":"chat.completion","created":123,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        ),
        (
            LlmProviderStrategy::MoonshotAi,
            "POST /v1/chat/completions HTTP/1.1",
            r#"{"id":"ms-1","object":"chat.completion","created":123,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]}"#,
        ),
        (
            LlmProviderStrategy::DeepSeek,
            "POST /chat/completions HTTP/1.1",
            r#"{"id":"ds-1","object":"chat.completion","created":123,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":1}}"#,
        ),
    ];

    for (strategy, expected_prefix, mock_response) in test_cases {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let expected_prefix = expected_prefix.to_string();
        let mock_response = mock_response.to_string();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request_str = read_http_request(&mut stream);
            assert!(
                request_str.starts_with(&expected_prefix),
                "strategy {:?} expected start with '{}', got: '{}'",
                strategy,
                expected_prefix,
                request_str.lines().next().unwrap_or_default()
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                mock_response.len(),
                mock_response
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let mut req = request();
        req.config.strategy = strategy;
        req.config.base_url = format!("http://{address}");
        req.options.reasoning_enabled = Some(false);
        req.options.response_format = LlmResponseFormat::Text;

        let res = OnlineLlmAdapter.complete(req).await;
        server.join().unwrap();
        assert!(
            res.is_ok(),
            "strategy {:?} failed: {:?}",
            strategy,
            res.err()
        );
        assert_eq!(res.unwrap().text, "ok");
    }
}

#[test]
fn native_providers_build_successfully_from_default_configurations() {
    use sona_online_llm::native_completion::resolve_strategy_url_and_headers;

    let strategies = [
        LlmProviderStrategy::OpenAi,
        LlmProviderStrategy::OpenAiResponses,
        LlmProviderStrategy::Anthropic,
        LlmProviderStrategy::Gemini,
        LlmProviderStrategy::Ollama,
        LlmProviderStrategy::Copilot,
        LlmProviderStrategy::Cohere,
        LlmProviderStrategy::DeepSeek,
        LlmProviderStrategy::Groq,
        LlmProviderStrategy::MistralAi,
        LlmProviderStrategy::MoonshotAi,
        LlmProviderStrategy::MoonshotCn,
        LlmProviderStrategy::Kimi,
        LlmProviderStrategy::OpenRouter,
        LlmProviderStrategy::Perplexity,
        LlmProviderStrategy::Together,
        LlmProviderStrategy::XAi,
        LlmProviderStrategy::Venice,
        LlmProviderStrategy::Hyperbolic,
        LlmProviderStrategy::Llamafile,
    ];

    for strategy in strategies {
        let mut req = request();
        req.config.strategy = strategy;
        req.config.base_url = String::new();
        let res = resolve_strategy_url_and_headers(&req, false);
        assert!(
            res.is_ok(),
            "resolve_strategy_url_and_headers failed for strategy {:?}: {:?}",
            strategy,
            res.err()
        );
    }
}

#[tokio::test]
async fn native_stream_separates_thought_and_content_from_sse() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _request_str = read_http_request(&mut stream);
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Thinking step 1... \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Thinking step 2...\\n\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Final result\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{sse_body}"
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    let mut deltas = Vec::new();
    let mut emit = |delta: LlmStreamDelta| {
        deltas.push(delta);
        Ok(())
    };

    let mut req = request();
    req.config.base_url = format!("http://{address}");
    let res = OnlineLlmAdapter
        .stream_completion(req, &mut emit)
        .await
        .unwrap();
    server.join().unwrap();

    assert_eq!(res.text, "Final result");
    assert_eq!(deltas.len(), 3);
    assert!(deltas[0].is_thought());
    assert_eq!(deltas[0].delta, "Thinking step 1... ");
    assert!(deltas[1].is_thought());
    assert_eq!(deltas[1].delta, "Thinking step 2...\n");
    assert!(!deltas[2].is_thought());
    assert_eq!(deltas[2].delta, "Final result");
}

#[tokio::test]
async fn native_stream_demuxes_inline_think_tags_from_sse() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _request_str = read_http_request(&mut stream);
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"<think>Local reasoning</think>Hello world\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{sse_body}"
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    let mut deltas = Vec::new();
    let mut emit = |delta: LlmStreamDelta| {
        deltas.push(delta);
        Ok(())
    };

    let mut req = request();
    req.config.base_url = format!("http://{address}");
    let res = OnlineLlmAdapter
        .stream_completion(req, &mut emit)
        .await
        .unwrap();
    server.join().unwrap();

    assert_eq!(res.text, "Hello world");
    assert_eq!(deltas.len(), 2);
    assert!(deltas[0].is_thought());
    assert_eq!(deltas[0].delta, "Local reasoning");
    assert!(!deltas[1].is_thought());
    assert_eq!(deltas[1].delta, "Hello world");
}

#[tokio::test]
async fn native_stream_handles_multibyte_utf8_split_across_chunks() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _request_str = read_http_request(&mut stream);
        let headers =
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        stream.write_all(headers.as_bytes()).unwrap();

        // Chinese character "好" is 3 bytes: 0xE5 0xA5 0xBD
        let part1 = "data: {\"choices\":[{\"delta\":{\"content\":\"\u{4f60}";
        stream.write_all(part1.as_bytes()).unwrap();
        stream.flush().unwrap();

        // Write first 2 bytes of "好"
        let hao_bytes = "好".as_bytes();
        stream.write_all(&hao_bytes[..2]).unwrap();
        stream.flush().unwrap();

        // Write remainder of "好" + closing json
        let mut part2 = Vec::new();
        part2.extend_from_slice(&hao_bytes[2..]);
        part2.extend_from_slice(b"\"}}]}\n\ndata: [DONE]\n\n");
        stream.write_all(&part2).unwrap();
        stream.flush().unwrap();
    });

    let mut deltas = Vec::new();
    let mut emit = |delta: LlmStreamDelta| {
        deltas.push(delta);
        Ok(())
    };

    let mut req = request();
    req.config.base_url = format!("http://{address}");
    let res = OnlineLlmAdapter
        .stream_completion(req, &mut emit)
        .await
        .unwrap();
    server.join().unwrap();

    assert_eq!(res.text, "你好");
}

#[tokio::test]
async fn native_stream_propagates_upstream_error_event() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _request_str = read_http_request(&mut stream);
        let sse_body = "data: {\"error\":{\"message\":\"Upstream context window exceeded\"}}\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{sse_body}"
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    let mut emit = |_delta: LlmStreamDelta| Ok(());

    let mut req = request();
    req.config.base_url = format!("http://{address}");
    let err = OnlineLlmAdapter
        .stream_completion(req, &mut emit)
        .await
        .unwrap_err();
    server.join().unwrap();

    assert!(
        err.message.contains("Upstream context window exceeded"),
        "expected error message to contain upstream error, got: {}",
        err.message
    );
}
