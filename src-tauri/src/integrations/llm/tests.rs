use super::commands::list_llm_models_command;
use super::jobs::{
    compute_summary_source_fingerprint, merge_polished_items_into_segments,
    merge_translated_items_into_segments,
};
use super::network::LlmApiUrl;
use super::*;
use futures_util::future::BoxFuture;
use reqwest::{StatusCode, header::RETRY_AFTER};
use serde_json::json;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn sample_segments() -> Vec<LlmSegmentInput> {
    vec![
        LlmSegmentInput {
            id: "1".to_string(),
            text: "hello".to_string(),
        },
        LlmSegmentInput {
            id: "2".to_string(),
            text: "world".to_string(),
        },
        LlmSegmentInput {
            id: "3".to_string(),
            text: "again".to_string(),
        },
    ]
}

fn sample_summary_segments() -> Vec<SummarySegmentInput> {
    vec![
        SummarySegmentInput {
            id: "1".to_string(),
            text: "Opening discussion about the roadmap.".to_string(),
            start: 0.0,
            end: 12.0,
            is_final: true,
        },
        SummarySegmentInput {
            id: "2".to_string(),
            text: "The team agreed to ship the beta next month.".to_string(),
            start: 12.0,
            end: 26.0,
            is_final: true,
        },
        SummarySegmentInput {
            id: "3".to_string(),
            text: "Alice will prepare the onboarding checklist.".to_string(),
            start: 26.0,
            end: 39.0,
            is_final: true,
        },
    ]
}

fn sample_summary_template(id: &str, name: &str, instructions: &str) -> SummaryTemplateConfig {
    SummaryTemplateConfig {
        id: id.to_string(),
        name: name.to_string(),
        instructions: instructions.to_string(),
    }
}

fn sample_transcript_segment(id: &str, text: &str) -> crate::integrations::asr::TranscriptSegment {
    crate::integrations::asr::TranscriptSegment {
        id: id.to_string(),
        text: text.to_string(),
        start: 0.0,
        end: 1.0,
        is_final: true,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    }
}

fn sample_llm_config(base_url: &str) -> LlmConfig {
    LlmConfig {
        timeout_seconds: None,
        provider: sona_core::domain::LlmProvider::Builtin(
            sona_core::domain::BuiltinLlmProvider::OpenAi,
        ),
        strategy: LlmProviderStrategy::OpenAiCompatible,
        base_url: base_url.to_string(),
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        api_path: None,
        api_version: None,
        temperature: None,
        reasoning_enabled: None,
        reasoning_level: None,
    }
}

#[test]
fn validate_llm_config_rejects_remote_http_api_hosts() {
    let error = validate_llm_config(&sample_llm_config("http://api.example.com/v1")).unwrap_err();

    assert_eq!(
        error,
        "LLM API host must use https:// unless it points to localhost."
    );
}

#[test]
fn validate_llm_config_accepts_https_and_loopback_http_api_hosts() {
    assert!(validate_llm_config(&sample_llm_config("https://api.example.com/v1")).is_ok());
    assert!(validate_llm_config(&sample_llm_config("http://localhost:1234/v1")).is_ok());
    assert!(validate_llm_config(&sample_llm_config("http://127.0.0.1:11434")).is_ok());
    assert!(validate_llm_config(&sample_llm_config("http://[::1]:11434")).is_ok());
}

#[test]
fn llm_api_url_preserves_https_policy_when_joining_and_querying() {
    let root = LlmApiUrl::parse("https://api.example.com/v1").unwrap();

    assert_eq!(
        root.join("/v1/chat/completions").unwrap().as_str(),
        "https://api.example.com/v1/chat/completions"
    );
    assert_eq!(
        root.with_query("api-version=2024-10-21").unwrap().as_str(),
        "https://api.example.com/v1?api-version=2024-10-21"
    );
}

#[test]
fn llm_api_url_rejects_remote_http_when_joining_and_querying() {
    let error = LlmApiUrl::parse("http://api.example.com/v1").unwrap_err();

    assert_eq!(
        error,
        "LLM API host must use https:// unless it points to localhost."
    );
}

#[tokio::test]
async fn list_llm_models_rejects_remote_http_before_requesting_models() {
    let error = list_llm_models_command(LlmModelsRequest {
        provider: sona_core::domain::LlmProvider::Builtin(
            sona_core::domain::BuiltinLlmProvider::OpenAi,
        ),
        strategy: Some(LlmProviderStrategy::OpenAiCompatible),
        base_url: "http://api.example.com/v1".to_string(),
        api_key: "test-key".to_string(),
    })
    .await
    .unwrap_err();

    assert_eq!(
        error,
        "LLM API host must use https:// unless it points to localhost."
    );
}

#[test]
fn transcript_job_translation_merge_preserves_existing_segment_fields() {
    let mut first = sample_transcript_segment("1", "hello");
    first.speaker = Some(sona_core::transcript::SpeakerTag {
        id: "speaker-a".to_string(),
        label: "Alice".to_string(),
        kind: "identified".to_string(),
        score: Some(0.91),
    });
    let second = sample_transcript_segment("2", "world");

    let merged = merge_translated_items_into_segments(
        vec![first.clone(), second.clone()],
        &[TranslatedSegment {
            id: "1".to_string(),
            translation: "你好".to_string(),
        }],
    );

    assert_eq!(merged[0].text, "hello");
    assert_eq!(merged[0].translation.as_deref(), Some("你好"));
    assert_eq!(merged[0].speaker, first.speaker);
    assert_eq!(merged[1], second);
}

#[test]
fn transcript_job_polish_merge_only_rewrites_text() {
    let mut segment = sample_transcript_segment("1", "hello");
    segment.translation = Some("你好".to_string());

    let merged = merge_polished_items_into_segments(
        vec![segment.clone()],
        &[PolishedSegment {
            id: "1".to_string(),
            text: "Hello.".to_string(),
        }],
    );

    assert_eq!(merged[0].text, "Hello.");
    assert_eq!(merged[0].translation, segment.translation);
    assert_eq!(merged[0].start, segment.start);
    assert_eq!(merged[0].end, segment.end);
}

#[test]
fn transcript_job_summary_fingerprint_matches_frontend_contract() {
    let mut segment = sample_transcript_segment("1", "Hello");
    segment.speaker = Some(sona_core::transcript::SpeakerTag {
        id: "speaker-a".to_string(),
        label: "Alice".to_string(),
        kind: "identified".to_string(),
        score: Some(0.91),
    });
    segment.translation = Some("Bonjour".to_string());

    assert_eq!(
        compute_summary_source_fingerprint(&[segment]),
        "1:Hello:0:1:true:speaker-a:Alice:identified:0.91"
    );
}

#[test]
fn openai_models_url_accepts_root_or_v1() {
    assert_eq!(
        format_openai_models_urls("https://api.openai.com", false),
        vec![
            "https://api.openai.com/v1/models".to_string(),
            "https://api.openai.com/models".to_string()
        ]
    );
    assert_eq!(
        format_openai_models_urls("https://api.openai.com/v1", false),
        vec!["https://api.openai.com/v1/models".to_string()]
    );
}

#[test]
fn gemini_base_url_is_cleaned() {
    assert_eq!(
        clean_gemini_base_url("https://generativelanguage.googleapis.com/v1beta/models"),
        "https://generativelanguage.googleapis.com"
    );
    assert_eq!(
        clean_gemini_base_url("https://generativelanguage.googleapis.com/v1beta/openai"),
        "https://generativelanguage.googleapis.com"
    );
    assert_eq!(
        clean_gemini_base_url("https://generativelanguage.googleapis.com/v1/openai/"),
        "https://generativelanguage.googleapis.com"
    );
    assert_eq!(
        clean_gemini_base_url("https://generativelanguage.googleapis.com"),
        "https://generativelanguage.googleapis.com"
    );
}

#[test]
fn gemini_models_url_accepts_common_inputs() {
    assert_eq!(
        format_gemini_models_url("https://generativelanguage.googleapis.com/v1beta/openai"),
        "https://generativelanguage.googleapis.com/v1beta/models"
    );
}

#[test]
fn gemini_generate_content_request_keeps_api_key_out_of_url() {
    let request = build_gemini_generate_content_request_parts(
        "https://generativelanguage.googleapis.com/v1beta/openai",
        "gemini-2.5-flash",
        "secret-gemini-key",
        false,
    )
    .expect("gemini request parts should build");

    assert_eq!(
        request.url.as_str(),
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent"
    );
    assert_eq!(
        request.headers,
        vec![("x-goog-api-key", "secret-gemini-key".to_string())]
    );
    assert!(!request.url.as_str().contains("secret-gemini-key"));
    assert!(!request.url.as_str().contains("key="));
}

#[test]
fn gemini_stream_generate_content_request_keeps_api_key_out_of_url() {
    let request = build_gemini_generate_content_request_parts(
        "https://generativelanguage.googleapis.com/v1beta/models",
        "gemini-2.5-pro",
        "secret-stream-key",
        true,
    )
    .expect("gemini stream request parts should build");

    assert_eq!(
        request.url.as_str(),
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
    );
    assert_eq!(
        request.headers,
        vec![("x-goog-api-key", "secret-stream-key".to_string())]
    );
    assert!(!request.url.as_str().contains("secret-stream-key"));
    assert!(!request.url.as_str().contains("key="));
}

#[test]
fn gemini_generate_content_request_errors_do_not_include_api_key() {
    let error = build_gemini_generate_content_request_parts(
        "http://generativelanguage.googleapis.com",
        "gemini-2.5-flash",
        "secret-gemini-key",
        false,
    )
    .expect_err("remote http should be rejected before request dispatch");

    assert_eq!(
        error,
        "LLM API host must use https:// unless it points to localhost."
    );
    assert!(!error.contains("secret-gemini-key"));
}

#[test]
fn gemini_model_filter_keeps_generate_content_models() {
    let text_model = GeminiModel {
        name: "models/gemini-2.5-flash".to_string(),
        supported_generation_methods: Some(vec!["generateContent".to_string()]),
        input_token_limit: None,
        output_token_limit: None,
    };
    let embedding_model = GeminiModel {
        name: "models/text-embedding-004".to_string(),
        supported_generation_methods: Some(vec!["embedContent".to_string()]),
        input_token_limit: None,
        output_token_limit: None,
    };
    let legacy_model = GeminiModel {
        name: "models/gemini-pro".to_string(),
        supported_generation_methods: None,
        input_token_limit: None,
        output_token_limit: None,
    };

    assert!(is_gemini_text_generation_model(&text_model));
    assert!(!is_gemini_text_generation_model(&embedding_model));
    assert!(is_gemini_text_generation_model(&legacy_model));
}

#[test]
fn gemini_model_summary_preserves_supported_capabilities() {
    let model = GeminiModel {
        name: "models/gemini-2.5-pro".to_string(),
        supported_generation_methods: Some(vec![
            "generateContent".to_string(),
            "countTokens".to_string(),
        ]),
        input_token_limit: Some(1_048_576),
        output_token_limit: Some(65_536),
    };

    let summary = gemini_model_to_summary(model).expect("gemini text model should be converted");

    assert_eq!(summary.model, "gemini-2.5-pro");
    assert_eq!(summary.context_window, Some(1_048_576));
    assert_eq!(summary.max_output_tokens, Some(65_536));
    assert_eq!(summary.supports_tools, Some(true));
    assert_eq!(summary.supports_multimodal, Some(true));
}

#[test]
fn openai_model_summary_defaults_missing_metadata_to_none() {
    let summary = openai_model_to_summary(OpenAiModel {
        id: "gpt-4.1-mini".to_string(),
    });

    assert_eq!(summary.model, "gpt-4.1-mini");
    assert_eq!(summary.context_window, None);
    assert_eq!(summary.input_price, None);
    assert_eq!(summary.supports_reasoning, None);
}

#[test]
fn anthropic_listing_is_disabled() {
    assert!(!strategy_supports_model_listing(
        LlmProviderStrategy::Anthropic
    ));
    assert!(!strategy_supports_model_listing(
        LlmProviderStrategy::AzureOpenAi
    ));
    assert!(!strategy_supports_model_listing(
        LlmProviderStrategy::OpenAiCompatibleCustomPath
    ));
    assert!(strategy_supports_model_listing(
        LlmProviderStrategy::OpenAiCompatible
    ));
}

#[test]
fn llm_config_accepts_custom_provider_with_strategy() {
    let config: LlmConfig = serde_json::from_value(json!({
        "provider": "custom-private-gateway",
        "strategy": "openai_responses",
        "baseUrl": "https://gateway.example.com",
        "apiKey": "gateway-key",
        "model": "gpt-4o",
        "apiPath": "/v1/responses"
    }))
    .expect("custom provider config should deserialize");

    assert_eq!(
        config.provider,
        LlmProvider::Custom("custom-private-gateway".to_string())
    );
    assert_eq!(config.strategy, LlmProviderStrategy::OpenAiResponses);
}

#[test]
fn openai_chat_payload_keeps_temperature_when_reasoning_is_enabled() {
    let mut config = sample_llm_config("https://api.openai.com/v1");
    config.temperature = Some(0.35);
    config.reasoning_enabled = Some(true);
    config.reasoning_level = Some("high".to_string());

    let payload = build_openai_chat_payload(&config, "hello", true);

    assert_eq!(payload["model"], "test-model");
    assert_eq!(payload["stream"], true);
    assert_eq!(payload["reasoning_effort"], "high");
    let temperature = payload["temperature"]
        .as_f64()
        .expect("temperature should be numeric");
    assert!((temperature - 0.35).abs() < 0.000_001);
}

#[test]
fn provider_strategy_uses_legacy_provider_when_strategy_is_missing() {
    let config: LlmConfig = serde_json::from_value(json!({
        "provider": "gemini",
        "baseUrl": "https://generativelanguage.googleapis.com",
        "apiKey": "gemini-key",
        "model": "gemini-2.5-flash"
    }))
    .expect("legacy provider-only config should deserialize");

    assert_eq!(
        config.provider,
        LlmProvider::Builtin(sona_core::domain::BuiltinLlmProvider::Gemini)
    );
    assert_eq!(config.strategy, LlmProviderStrategy::Gemini);
}

fn strategy_key(strategy: LlmProviderStrategy) -> String {
    serde_json::to_value(strategy)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

#[test]
fn join_url_trims_duplicate_slashes() {
    assert_eq!(
        join_url("https://api.openai.com/", "/v1/responses"),
        "https://api.openai.com/v1/responses"
    );
    assert_eq!(
        join_url(
            "https://ark.cn-beijing.volces.com",
            "api/v3/chat/completions"
        ),
        "https://ark.cn-beijing.volces.com/api/v3/chat/completions"
    );
}

#[test]
fn extract_text_from_chat_completions_response() {
    let response = json!({
        "choices": [
            {
                "message": {
                    "content": "Hello from chat completions"
                }
            }
        ]
    });

    assert_eq!(
        extract_text_from_json_response(&response).unwrap(),
        "Hello from chat completions"
    );
}

#[test]
fn extract_text_from_responses_api_payload() {
    let response = json!({
        "output": [
            {
                "content": [
                    {
                        "type": "output_text",
                        "text": "Hello from responses"
                    }
                ]
            }
        ]
    });

    assert_eq!(
        extract_text_from_json_response(&response).unwrap(),
        "Hello from responses"
    );
}

#[test]
fn clean_json_response_removes_markdown_fences() {
    assert_eq!(
        clean_json_response("```json\n[{\"id\":\"1\",\"text\":\"Hello\"}]\n```"),
        "[{\"id\":\"1\",\"text\":\"Hello\"}]"
    );
}

#[test]
fn parse_polish_chunk_rejects_length_mismatch() {
    let err = parse_polish_chunk(r#"[{"id":"1","text":"Hello"}]"#, &sample_segments()[..2], 1)
        .expect_err("length mismatch should fail");

    assert!(err.contains("polish chunk 1 failed"));
    assert!(err.contains("expected 2 objects but received 1"));
}

#[test]
fn parse_translate_chunk_rejects_id_order_mismatch() {
    let err = parse_translate_chunk(
        r#"[{"id":"2","translation":"B"},{"id":"1","translation":"A"}]"#,
        &sample_segments()[..2],
        2,
    )
    .expect_err("id order mismatch should fail");

    assert!(err.contains("translate chunk 2 failed"));
    assert!(err.contains("expected id '1'"));
}

#[test]
fn build_polish_prompt_contains_context_and_keywords() {
    let prompt = build_polish_prompt(
        &sample_segments()[..2],
        Some("combined context"),
        Some("keyword-a, keyword-b"),
    );

    assert!(prompt.contains("[User Context]"));
    assert!(prompt.contains("combined context"));
    assert!(prompt.contains("[User Keywords]"));
    assert!(prompt.contains("keyword-a, keyword-b"));
}

#[test]
fn build_translate_prompt_contains_language_name() {
    let prompt =
        build_translate_prompt(&sample_segments()[..2], "zh", Some("Chinese (Simplified)"));

    assert!(prompt.contains("Chinese (Simplified)"));
    assert!(prompt.contains("replace 'text' with 'translation'"));
}

#[test]
fn build_summary_chunk_prompt_requires_same_language_and_structure() {
    let template = sample_summary_template(
        "meeting",
        "Meeting",
        "1. Meeting overview.\n2. Decisions made.\n3. Action items with owners when the transcript names them.",
    );
    let prompt = build_summary_chunk_prompt(&template, &sample_summary_segments()[..2], 1, 2);

    assert!(prompt.contains("Use the same language as the transcript."));
    assert!(prompt.contains("Meeting overview."));
    assert!(prompt.contains("Action items with owners"));
}

#[test]
fn build_summary_finalize_prompt_requires_same_language_and_structure() {
    let template = sample_summary_template(
        "lecture",
        "Lecture",
        "1. Lecture overview.\n2. Core concepts or arguments.\n3. Important examples, evidence, or explanations.",
    );
    let prompt = build_summary_finalize_prompt(
        &template,
        &["Chunk 1 summary".to_string(), "Chunk 2 summary".to_string()],
    );

    assert!(prompt.contains("Use the same language as the transcript."));
    assert!(prompt.contains("Core concepts or arguments."));
    assert!(prompt.contains("[Chunk 1]"));
}

#[test]
fn split_summary_segments_uses_char_budget() {
    let chunks = split_summary_segments(&sample_summary_segments(), 70);

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0][0].id, "1");
    assert_eq!(chunks[1][0].id, "2");
    assert_eq!(chunks[2][0].id, "3");
}

#[test]
fn parse_google_translate_free_retry_after_clamps_seconds() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(RETRY_AFTER, reqwest::header::HeaderValue::from_static("9"));

    assert_eq!(
        parse_google_translate_free_retry_after(&headers),
        Some(Duration::from_secs(5))
    );
}

#[test]
fn plan_segment_task_chunks_uses_dynamic_prompt_budget() {
    let segments = vec![
        LlmSegmentInput {
            id: "1".to_string(),
            text: "AAAAAA".to_string(),
        },
        LlmSegmentInput {
            id: "2".to_string(),
            text: "BBBBBB".to_string(),
        },
        LlmSegmentInput {
            id: "3".to_string(),
            text: "CCCCCC".to_string(),
        },
    ];
    let mut build_prompt = |chunk: &[LlmSegmentInput]| {
        chunk
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join("|")
    };

    let planned = plan_segment_task_chunks(
        "task-dynamic-1",
        LlmTaskType::Polish,
        &segments,
        None,
        19,
        &mut build_prompt,
    );

    assert_eq!(
        planned
            .iter()
            .map(|chunk| (chunk.start, chunk.end))
            .collect::<Vec<_>>(),
        vec![(0, 2), (2, 3)]
    );
    assert_eq!(planned[0].prompt, "AAAAAA|BBBBBB");
    assert_eq!(planned[1].prompt, "CCCCCC");
}

#[test]
fn plan_segment_task_chunks_context_and_keywords_reduce_capacity() {
    let segments = sample_segments();
    let context = "Project roadmap context. ".repeat(3);
    let keywords = "Sona, roadmap, launch. ".repeat(3);
    let without_context_two = prompt_char_count(&build_polish_prompt(&segments[..2], None, None));
    let with_context_one = prompt_char_count(&build_polish_prompt(
        &segments[..1],
        Some(&context),
        Some(&keywords),
    ));
    let with_context_two = prompt_char_count(&build_polish_prompt(
        &segments[..2],
        Some(&context),
        Some(&keywords),
    ));
    let budget = with_context_one.max(without_context_two);

    assert!(budget < with_context_two);

    let without_context = plan_segment_task_chunks(
        "task-dynamic-2",
        LlmTaskType::Polish,
        &segments,
        None,
        budget,
        &mut |chunk| build_polish_prompt(chunk, None, None),
    );
    let with_context = plan_segment_task_chunks(
        "task-dynamic-3",
        LlmTaskType::Polish,
        &segments,
        None,
        budget,
        &mut |chunk| build_polish_prompt(chunk, Some(&context), Some(&keywords)),
    );

    assert!(without_context[0].end - without_context[0].start >= 2);
    assert_eq!(with_context[0].end - with_context[0].start, 1);
}

#[test]
fn summary_provider_rejects_google_translate() {
    let err = validate_summary_strategy(LlmProviderStrategy::GoogleTranslate)
        .expect_err("google translate should be rejected");

    assert!(err.contains("does not support transcript summaries"));
}

#[test]
fn chunk_payload_serializes_with_camel_case() {
    let payload = LlmTaskChunkPayload {
        task_id: "task-1".to_string(),
        task_type: LlmTaskType::Polish,
        chunk_index: 1,
        total_chunks: 2,
        items: vec![PolishedSegment {
            id: "1".to_string(),
            text: "Hello".to_string(),
        }],
    };

    let json = serde_json::to_value(payload).expect("payload should serialize");

    assert_eq!(json["taskId"], "task-1");
    assert_eq!(json["taskType"], "polish");
    assert_eq!(json["chunkIndex"], 1);
    assert_eq!(json["totalChunks"], 2);
    assert_eq!(json["items"][0]["id"], "1");
    assert_eq!(json["items"][0]["text"], "Hello");
}

#[test]
fn text_payload_serializes_with_camel_case() {
    let payload = LlmTaskTextPayload {
        task_id: "summary-task-1".to_string(),
        task_type: LlmTaskType::Summary,
        text: "Hello world".to_string(),
        delta: "world".to_string(),
    };

    let json = serde_json::to_value(payload).expect("payload should serialize");

    assert_eq!(json["taskId"], "summary-task-1");
    assert_eq!(json["taskType"], "summary");
    assert_eq!(json["text"], "Hello world");
    assert_eq!(json["delta"], "world");
}

#[test]
fn parse_polish_chunk_accepts_ndjson() {
    let segments = sample_segments();
    let response = concat!(
        "{\"id\":\"1\",\"text\":\"Hello\"}\n",
        "{\"id\":\"2\",\"text\":\"World\"}\n"
    );

    let parsed = parse_polish_chunk(response, &segments[..2], 1).expect("ndjson should parse");

    assert_eq!(
        parsed,
        vec![
            PolishedSegment {
                id: "1".to_string(),
                text: "Hello".to_string(),
            },
            PolishedSegment {
                id: "2".to_string(),
                text: "World".to_string(),
            },
        ]
    );
}

#[tokio::test]
async fn try_stream_text_skips_google_translate_providers() {
    for strategy in [
        LlmProviderStrategy::GoogleTranslate,
        LlmProviderStrategy::GoogleTranslateFree,
    ] {
        let request = LlmGenerateRequest {
            config: LlmConfig {
                timeout_seconds: None,
                provider: LlmProvider::Custom(strategy_key(strategy)),
                strategy,
                base_url: "https://example.com".to_string(),
                api_key: "test-key".to_string(),
                model: "test-model".to_string(),
                api_path: None,
                api_version: None,
                temperature: Some(0.2),
                reasoning_enabled: None,
                reasoning_level: None,
            },
            input: "hello".to_string(),
            source: None,
        };

        let mut emit_delta = |_text: &str, _delta: &str| Ok(());
        let mut accumulator = StreamTextAccumulator::new(&mut emit_delta);

        let streamed = try_stream_text(&request, &mut accumulator)
            .await
            .expect("google translate should skip streaming");

        assert_eq!(streamed, None);
        assert_eq!(accumulator.text(), "");
    }
}

#[tokio::test]
async fn run_segment_task_aggregates_chunks_and_emits_progress() {
    let segments = sample_segments();
    let mut chunk_events = Vec::new();
    let mut progress_events = Vec::new();
    #[derive(Debug, PartialEq, Eq)]
    enum SegmentEvent {
        Chunk(usize),
        Progress(usize),
    }
    let ordered_events = std::cell::RefCell::new(Vec::new());

    let result = run_segment_task(
        SegmentTaskContext {
            task_id: "task-1",
            task_type: LlmTaskType::Polish,
            segments: &segments,
            chunk_size: Some(2),
            prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
        },
        BufferedSegmentTaskConfig {
            build_prompt: |chunk: &[LlmSegmentInput]| serde_json::to_string(chunk).unwrap(),
            parse_chunk: parse_polish_chunk,
            generate_text: {
                let mut call_count = 0usize;
                move |_prompt: String| {
                    call_count += 1;
                    let response = match call_count {
                        1 => r#"[{"id":"1","text":"Hello"},{"id":"2","text":"World"}]"#,
                        2 => r#"[{"id":"3","text":"Again"}]"#,
                        _ => unreachable!(),
                    }
                    .to_string();

                    let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                        Box::pin(async move {
                            Ok(StandardLlmResponse {
                                text: response,
                                usage: None,
                            })
                        });
                    future
                }
            },
            on_success: |_response: &StandardLlmResponse| {},
            emit_chunk: |payload: LlmTaskChunkPayload<PolishedSegment>| {
                ordered_events
                    .borrow_mut()
                    .push(SegmentEvent::Chunk(payload.chunk_index));
                chunk_events.push(payload);
                Ok(())
            },
            emit_progress: |payload: LlmTaskProgressPayload| {
                ordered_events
                    .borrow_mut()
                    .push(SegmentEvent::Progress(payload.completed_chunks));
                progress_events.push(payload);
                Ok(())
            },
        },
    )
    .await
    .expect("task should succeed");

    assert_eq!(
        result,
        vec![
            PolishedSegment {
                id: "1".to_string(),
                text: "Hello".to_string()
            },
            PolishedSegment {
                id: "2".to_string(),
                text: "World".to_string()
            },
            PolishedSegment {
                id: "3".to_string(),
                text: "Again".to_string()
            },
        ]
    );
    assert_eq!(
        chunk_events,
        vec![
            LlmTaskChunkPayload {
                task_id: "task-1".to_string(),
                task_type: LlmTaskType::Polish,
                chunk_index: 1,
                total_chunks: 2,
                items: vec![
                    PolishedSegment {
                        id: "1".to_string(),
                        text: "Hello".to_string()
                    },
                    PolishedSegment {
                        id: "2".to_string(),
                        text: "World".to_string()
                    },
                ],
            },
            LlmTaskChunkPayload {
                task_id: "task-1".to_string(),
                task_type: LlmTaskType::Polish,
                chunk_index: 2,
                total_chunks: 2,
                items: vec![PolishedSegment {
                    id: "3".to_string(),
                    text: "Again".to_string()
                }],
            },
        ]
    );
    assert_eq!(
        progress_events,
        vec![
            LlmTaskProgressPayload {
                task_id: "task-1".to_string(),
                task_type: LlmTaskType::Polish,
                completed_chunks: 1,
                total_chunks: 2,
            },
            LlmTaskProgressPayload {
                task_id: "task-1".to_string(),
                task_type: LlmTaskType::Polish,
                completed_chunks: 2,
                total_chunks: 2,
            },
        ]
    );
    assert_eq!(
        ordered_events.into_inner(),
        vec![
            SegmentEvent::Chunk(1),
            SegmentEvent::Progress(1),
            SegmentEvent::Chunk(2),
            SegmentEvent::Progress(2),
        ]
    );
}

#[tokio::test]
async fn run_segment_task_reports_second_chunk_failure() {
    let segments = sample_segments();

    let err = run_segment_task(
            SegmentTaskContext {
                task_id: "task-2",
                task_type: LlmTaskType::Translate,
                segments: &segments,
                chunk_size: Some(2),
                prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            },
            BufferedSegmentTaskConfig {
                build_prompt: |chunk: &[LlmSegmentInput]| serde_json::to_string(chunk).unwrap(),
                parse_chunk: parse_translate_chunk,
                generate_text: {
                    let mut call_count = 0usize;
                    move |_prompt: String| {
                        call_count += 1;
                        let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                            Box::pin(async move {
                                match call_count {
                                    1 => Ok(StandardLlmResponse {
                                        text: r#"[{"id":"1","translation":"A"},{"id":"2","translation":"B"}]"#
                                            .to_string(),
                                        usage: None,
                                    }),
                                    2 => Err("boom".to_string()),
                                    _ => unreachable!(),
                                }
                            });
                        future
                    }
                },
                on_success: |_response: &StandardLlmResponse| {},
                emit_chunk: |_payload: LlmTaskChunkPayload<TranslatedSegment>| Ok(()),
                emit_progress: |_payload: LlmTaskProgressPayload| Ok(()),
            },
        )
        .await
        .expect_err("second chunk should fail");

    assert_eq!(err, "translate chunk 2 failed: boom");
}

#[tokio::test]
async fn run_segment_task_uses_dynamic_prompt_budget_when_chunk_size_is_missing() {
    let long_text = "A".repeat(7_000);
    let segments = vec![
        LlmSegmentInput {
            id: "1".to_string(),
            text: long_text.clone(),
        },
        LlmSegmentInput {
            id: "2".to_string(),
            text: long_text.clone(),
        },
        LlmSegmentInput {
            id: "3".to_string(),
            text: long_text,
        },
    ];
    let mut chunk_events = Vec::new();
    let mut progress_events = Vec::new();

    let result = run_segment_task(
        SegmentTaskContext {
            task_id: "task-3",
            task_type: LlmTaskType::Translate,
            segments: &segments,
            chunk_size: None,
            prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
        },
        BufferedSegmentTaskConfig {
            build_prompt: |chunk: &[LlmSegmentInput]| {
                chunk
                    .iter()
                    .map(|segment| format!("{}:{}", segment.id, segment.text))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            parse_chunk: parse_translate_chunk,
            generate_text: {
                let mut call_count = 0usize;
                move |_prompt: String| {
                    call_count += 1;
                    let response = match call_count {
                        1 => r#"[{"id":"1","translation":"One"},{"id":"2","translation":"Two"}]"#,
                        2 => r#"[{"id":"3","translation":"Three"}]"#,
                        _ => unreachable!(),
                    }
                    .to_string();

                    let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                        Box::pin(async move {
                            Ok(StandardLlmResponse {
                                text: response,
                                usage: None,
                            })
                        });
                    future
                }
            },
            on_success: |_response: &StandardLlmResponse| {},
            emit_chunk: |payload: LlmTaskChunkPayload<TranslatedSegment>| {
                chunk_events.push(payload);
                Ok(())
            },
            emit_progress: |payload: LlmTaskProgressPayload| {
                progress_events.push(payload);
                Ok(())
            },
        },
    )
    .await
    .expect("dynamic task should succeed");

    assert_eq!(
        result,
        vec![
            TranslatedSegment {
                id: "1".to_string(),
                translation: "One".to_string(),
            },
            TranslatedSegment {
                id: "2".to_string(),
                translation: "Two".to_string(),
            },
            TranslatedSegment {
                id: "3".to_string(),
                translation: "Three".to_string(),
            },
        ]
    );
    assert_eq!(
        chunk_events
            .iter()
            .map(|payload| {
                payload
                    .items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec!["1", "2"], vec!["3"]]
    );
    assert_eq!(
        progress_events,
        vec![
            LlmTaskProgressPayload {
                task_id: "task-3".to_string(),
                task_type: LlmTaskType::Translate,
                completed_chunks: 1,
                total_chunks: 2,
            },
            LlmTaskProgressPayload {
                task_id: "task-3".to_string(),
                task_type: LlmTaskType::Translate,
                completed_chunks: 2,
                total_chunks: 2,
            },
        ]
    );
}

#[tokio::test]
async fn run_segment_task_sends_single_over_budget_segment_alone() {
    let huge_text = "A".repeat(DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET + 100);
    let segments = vec![
        LlmSegmentInput {
            id: "1".to_string(),
            text: huge_text,
        },
        LlmSegmentInput {
            id: "2".to_string(),
            text: "small".to_string(),
        },
        LlmSegmentInput {
            id: "3".to_string(),
            text: "tiny".to_string(),
        },
    ];
    let mut chunk_events = Vec::new();
    let mut progress_events = Vec::new();

    let result = run_segment_task(
        SegmentTaskContext {
            task_id: "task-4",
            task_type: LlmTaskType::Translate,
            segments: &segments,
            chunk_size: None,
            prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
        },
        BufferedSegmentTaskConfig {
            build_prompt: |chunk: &[LlmSegmentInput]| {
                chunk
                    .iter()
                    .map(|segment| segment.text.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            },
            parse_chunk: parse_translate_chunk,
            generate_text: {
                let mut call_count = 0usize;
                move |_prompt: String| {
                    call_count += 1;
                    let response = match call_count {
                        1 => r#"[{"id":"1","translation":"Huge"}]"#,
                        2 => {
                            r#"[{"id":"2","translation":"Small"},{"id":"3","translation":"Tiny"}]"#
                        }
                        _ => unreachable!(),
                    }
                    .to_string();

                    let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                        Box::pin(async move {
                            Ok(StandardLlmResponse {
                                text: response,
                                usage: None,
                            })
                        });
                    future
                }
            },
            on_success: |_response: &StandardLlmResponse| {},
            emit_chunk: |payload: LlmTaskChunkPayload<TranslatedSegment>| {
                chunk_events.push(payload);
                Ok(())
            },
            emit_progress: |payload: LlmTaskProgressPayload| {
                progress_events.push(payload);
                Ok(())
            },
        },
    )
    .await
    .expect("single-over-budget task should succeed");

    assert_eq!(
        result,
        vec![
            TranslatedSegment {
                id: "1".to_string(),
                translation: "Huge".to_string(),
            },
            TranslatedSegment {
                id: "2".to_string(),
                translation: "Small".to_string(),
            },
            TranslatedSegment {
                id: "3".to_string(),
                translation: "Tiny".to_string(),
            },
        ]
    );
    assert_eq!(
        chunk_events
            .iter()
            .map(|payload| {
                payload
                    .items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![vec!["1"], vec!["2", "3"]]
    );
    assert_eq!(
        progress_events,
        vec![
            LlmTaskProgressPayload {
                task_id: "task-4".to_string(),
                task_type: LlmTaskType::Translate,
                completed_chunks: 1,
                total_chunks: 2,
            },
            LlmTaskProgressPayload {
                task_id: "task-4".to_string(),
                task_type: LlmTaskType::Translate,
                completed_chunks: 2,
                total_chunks: 2,
            },
        ]
    );
}

#[tokio::test]
async fn run_google_translate_free_requests_in_order_limits_concurrency() {
    let active = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let translations = run_google_translate_free_requests_in_order(
        vec![
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string(),
        ],
        2,
        {
            let active = active.clone();
            let max_seen = max_seen.clone();
            move |index, text| {
                let active = active.clone();
                let max_seen = max_seen.clone();
                async move {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_seen.fetch_max(current, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok((index, text.to_uppercase()))
                }
            }
        },
    )
    .await
    .expect("concurrency-limited run should succeed");

    assert_eq!(
        translations,
        vec![
            "ONE".to_string(),
            "TWO".to_string(),
            "THREE".to_string(),
            "FOUR".to_string(),
        ]
    );
    assert!(max_seen.load(Ordering::SeqCst) <= 2);
}

#[tokio::test]
async fn execute_google_translate_free_request_retries_429_then_succeeds() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let delays = Arc::new(Mutex::new(Vec::new()));

    let result = execute_google_translate_free_request(
        7,
        "hello".to_string(),
        "ja".to_string(),
        {
            let attempts = attempts.clone();
            move |_text, _target| {
                let attempts = attempts.clone();
                async move {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if attempt <= 2 {
                        Err(GoogleTranslateFreeAttemptError::HttpStatus {
                            status: StatusCode::TOO_MANY_REQUESTS,
                            retry_after: None,
                        })
                    } else {
                        Ok("\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}".to_string())
                    }
                }
            }
        },
        {
            let delays = delays.clone();
            move |delay| {
                let delays = delays.clone();
                async move {
                    delays.lock().unwrap().push(delay);
                }
            }
        },
    )
    .await
    .expect("request should succeed after retries");

    assert_eq!(
        result,
        (7, "\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}".to_string())
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(
        *delays.lock().unwrap(),
        vec![Duration::from_millis(500), Duration::from_millis(1000)]
    );
}

#[tokio::test]
async fn execute_google_translate_free_request_prefers_retry_after_and_clamps() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let delays = Arc::new(Mutex::new(Vec::new()));

    let result = execute_google_translate_free_request(
        1,
        "hello".to_string(),
        "fr".to_string(),
        {
            let attempts = attempts.clone();
            move |_text, _target| {
                let attempts = attempts.clone();
                async move {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if attempt == 1 {
                        Err(GoogleTranslateFreeAttemptError::HttpStatus {
                            status: StatusCode::TOO_MANY_REQUESTS,
                            retry_after: Some(Duration::from_secs(9)),
                        })
                    } else {
                        Ok("bonjour".to_string())
                    }
                }
            }
        },
        {
            let delays = delays.clone();
            move |delay| {
                let delays = delays.clone();
                async move {
                    delays.lock().unwrap().push(delay);
                }
            }
        },
    )
    .await
    .expect("request should succeed after honoring retry-after");

    assert_eq!(result, (1, "bonjour".to_string()));
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(*delays.lock().unwrap(), vec![Duration::from_secs(5)]);
}

#[tokio::test]
async fn execute_google_translate_free_request_does_not_retry_non_429() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let slept = Arc::new(AtomicUsize::new(0));

    let err = execute_google_translate_free_request(
        2,
        "hello".to_string(),
        "de".to_string(),
        {
            let attempts = attempts.clone();
            move |_text, _target| {
                let attempts = attempts.clone();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(GoogleTranslateFreeAttemptError::HttpStatus {
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        retry_after: None,
                    })
                }
            }
        },
        {
            let slept = slept.clone();
            move |_delay| {
                let slept = slept.clone();
                async move {
                    slept.fetch_add(1, Ordering::SeqCst);
                }
            }
        },
    )
    .await
    .expect_err("non-429 should fail immediately");

    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(slept.load(Ordering::SeqCst), 0);
    assert!(err.contains("500 Internal Server Error"));
    assert!(err.contains("after 1 attempt"));
}

#[tokio::test]
async fn run_google_translate_free_requests_in_order_fails_chunk_when_retries_exhaust() {
    let err = run_google_translate_free_requests_in_order(
        vec!["first".to_string(), "second".to_string()],
        2,
        move |index, text| async move {
            if index == 0 {
                execute_google_translate_free_request(
                    index,
                    text,
                    "es".to_string(),
                    |_text, _target| async {
                        Err(GoogleTranslateFreeAttemptError::HttpStatus {
                            status: StatusCode::TOO_MANY_REQUESTS,
                            retry_after: None,
                        })
                    },
                    |_delay| async {},
                )
                .await
            } else {
                Ok((index, text))
            }
        },
    )
    .await
    .expect_err("chunk should fail when a request exhausts retries");

    assert!(err.contains("429 Too Many Requests"));
    assert!(err.contains("after 3 attempts"));
}

#[tokio::test]
async fn run_summary_task_emits_chunk_and_final_progress() {
    let segments = vec![
        SummarySegmentInput {
            id: "1".to_string(),
            text: "A".repeat(700),
            start: 0.0,
            end: 12.0,
            is_final: true,
        },
        SummarySegmentInput {
            id: "2".to_string(),
            text: "B".repeat(700),
            start: 12.0,
            end: 24.0,
            is_final: true,
        },
        SummarySegmentInput {
            id: "3".to_string(),
            text: "C".repeat(700),
            start: 24.0,
            end: 36.0,
            is_final: true,
        },
    ];
    let mut progress_events = Vec::new();
    let template = sample_summary_template(
        "meeting",
        "Meeting",
        "1. Meeting overview.\n2. Decisions made.\n3. Action items with owners when the transcript names them.\n4. Open questions, blockers, or risks.",
    );
    let streamed_calls = Arc::new(AtomicUsize::new(0));

    let result = run_summary_task(
        "summary-task-1",
        &template,
        &segments,
        Some(1200),
        {
            let mut call_count = 0usize;
            move |_prompt| {
                call_count += 1;
                let response = match call_count {
                    1 => "Intermediate summary A".to_string(),
                    2 => "Intermediate summary B".to_string(),
                    3 => "Intermediate summary C".to_string(),
                    4 => "Final meeting summary".to_string(),
                    _ => unreachable!(),
                };

                Box::pin(async move { Ok(response) })
            }
        },
        {
            let streamed_calls = streamed_calls.clone();
            move |_prompt| {
                streamed_calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok("Final meeting summary".to_string()) })
            }
        },
        |payload| {
            progress_events.push(payload);
            Ok(())
        },
    )
    .await
    .expect("summary task should succeed");

    assert_eq!(
        result,
        TranscriptSummaryResult {
            template_id: "meeting".to_string(),
            content: "Final meeting summary".to_string(),
        }
    );
    assert_eq!(
        progress_events,
        vec![
            LlmTaskProgressPayload {
                task_id: "summary-task-1".to_string(),
                task_type: LlmTaskType::Summary,
                completed_chunks: 1,
                total_chunks: 4,
            },
            LlmTaskProgressPayload {
                task_id: "summary-task-1".to_string(),
                task_type: LlmTaskType::Summary,
                completed_chunks: 2,
                total_chunks: 4,
            },
            LlmTaskProgressPayload {
                task_id: "summary-task-1".to_string(),
                task_type: LlmTaskType::Summary,
                completed_chunks: 3,
                total_chunks: 4,
            },
            LlmTaskProgressPayload {
                task_id: "summary-task-1".to_string(),
                task_type: LlmTaskType::Summary,
                completed_chunks: 4,
                total_chunks: 4,
            },
        ]
    );
    assert_eq!(streamed_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn run_summary_task_streams_direct_single_chunk_without_intermediate_stage() {
    let segments = sample_summary_segments();
    let mut progress_events = Vec::new();
    let buffered_calls = Arc::new(AtomicUsize::new(0));
    let streamed_calls = Arc::new(AtomicUsize::new(0));
    let template = sample_summary_template(
        "direct",
        "Direct",
        "1. Main points.\n2. Decisions.\n3. Action items.",
    );

    let result = run_summary_task(
        "summary-task-direct",
        &template,
        &segments,
        Some(DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET),
        {
            let buffered_calls = buffered_calls.clone();
            move |_prompt| {
                buffered_calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok("unexpected intermediate summary".to_string()) })
            }
        },
        {
            let streamed_calls = streamed_calls.clone();
            move |_prompt| {
                streamed_calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok("Direct summary".to_string()) })
            }
        },
        |payload| {
            progress_events.push(payload);
            Ok(())
        },
    )
    .await
    .expect("single chunk summary should succeed");

    assert_eq!(
        result,
        TranscriptSummaryResult {
            template_id: "direct".to_string(),
            content: "Direct summary".to_string(),
        }
    );
    assert_eq!(buffered_calls.load(Ordering::SeqCst), 0);
    assert_eq!(streamed_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        progress_events,
        vec![LlmTaskProgressPayload {
            task_id: "summary-task-direct".to_string(),
            task_type: LlmTaskType::Summary,
            completed_chunks: 1,
            total_chunks: 1,
        }]
    );
}

#[test]
fn llm_api_url_client_builds_for_https_and_loopback_with_various_timeouts() {
    let cases = [
        ("https://api.example.com/v1", None),
        ("https://api.example.com/v1", Some(30)),
        ("http://localhost:1234/v1", Some(60)),
        ("http://127.0.0.1:11434", None),
    ];

    for (base_url, timeout_seconds) in cases {
        let url = LlmApiUrl::parse(base_url).unwrap_or_else(|_| panic!("{base_url} should parse"));

        // Two calls share a cache key; the second hits the cached clone. Both
        // must succeed for the construction + cache path to hold.
        assert!(url.client(timeout_seconds).is_ok());
        assert!(url.client(timeout_seconds).is_ok());
    }
}
