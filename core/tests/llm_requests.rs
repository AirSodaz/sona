use serde_json::json;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm_requests::{
    HistorySummaryPayload, LlmConfig, PolishSegmentsRequest, SummarizeTranscriptRequest,
    TranscriptLlmJobRequest, TranscriptSummaryRecordPayload, TranslateSegmentsRequest,
    validate_llm_config, validate_llm_generate_request, validate_polish_segments_request,
    validate_summarize_transcript_request, validate_task_request,
    validate_translate_segments_request,
};
use sona_core::llm_tasks::{
    LlmProviderStrategy, LlmSegmentInput, LlmTaskType, SummarySegmentInput, SummaryTemplateConfig,
};
use sona_core::transcript::TranscriptSegment;

fn sample_config() -> LlmConfig {
    LlmConfig {
        provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        strategy: LlmProviderStrategy::OpenAiCompatible,
        base_url: "https://api.example.com/v1".to_string(),
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        api_path: None,
        api_version: None,
        temperature: Some(0.2),
        reasoning_enabled: None,
        reasoning_level: None,
        timeout_seconds: Some(30),
    }
}

fn sample_transcript_segment() -> TranscriptSegment {
    TranscriptSegment {
        id: "segment-1".to_string(),
        text: "Hello world".to_string(),
        start: 0.0,
        end: 1.5,
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

#[test]
fn llm_config_infers_strategy_from_legacy_provider_only_payload() {
    let config: LlmConfig = serde_json::from_value(json!({
        "provider": "gemini",
        "baseUrl": "https://generativelanguage.googleapis.com",
        "apiKey": "gemini-key",
        "model": "gemini-2.5-flash"
    }))
    .expect("legacy provider-only config should deserialize");

    assert_eq!(
        config.provider,
        LlmProvider::Builtin(BuiltinLlmProvider::Gemini)
    );
    assert_eq!(config.strategy, LlmProviderStrategy::Gemini);
}

#[test]
fn llm_config_accepts_custom_provider_with_explicit_strategy() {
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
    assert_eq!(config.api_path.as_deref(), Some("/v1/responses"));
}

#[test]
fn llm_config_validation_rejects_empty_model_names() {
    let mut config = sample_config();
    config.model = "  ".to_string();

    let error = validate_llm_config(&config).unwrap_err();

    assert_eq!(error, "Model name cannot be empty");
}

#[test]
fn llm_config_validation_leaves_api_host_policy_to_online_adapter() {
    for base_url in [
        "https://api.example.com/v1",
        "http://api.example.com/v1",
        "http://localhost:1234/v1",
        "http://127.0.0.1:11434",
        "http://[::1]:11434",
        "not a url",
    ] {
        let mut config = sample_config();
        config.base_url = base_url.to_string();

        validate_llm_config(&config).unwrap_or_else(|error| {
            panic!("{base_url} should be accepted but failed with {error}")
        });
    }
}

#[test]
fn task_request_validation_requires_a_task_id() {
    let error = validate_task_request("  ", &sample_config()).unwrap_err();

    assert_eq!(error, "Task ID cannot be empty");
}

#[test]
fn generate_request_validation_requires_input_text() {
    let error = validate_llm_generate_request(&sona_core::llm_requests::LlmGenerateRequest {
        config: sample_config(),
        input: "  ".to_string(),
        source: None,
    })
    .unwrap_err();

    assert_eq!(error, "Input cannot be empty");
}

#[test]
fn polish_request_validation_rejects_google_translate_strategy() {
    let mut config = sample_config();
    config.strategy = LlmProviderStrategy::GoogleTranslateFree;
    let request = PolishSegmentsRequest {
        task_id: "task-polish".to_string(),
        config,
        segments: vec![],
        chunk_size: None,
        context: None,
        keywords: None,
    };

    let error = validate_polish_segments_request(&request).unwrap_err();

    assert_eq!(
        error,
        "Google Translate does not support transcript polishing"
    );
}

#[test]
fn translate_request_validation_requires_target_language() {
    let request = TranslateSegmentsRequest {
        task_id: "task-translate".to_string(),
        config: sample_config(),
        segments: vec![],
        chunk_size: None,
        target_language: "  ".to_string(),
        target_language_name: None,
    };

    let error = validate_translate_segments_request(&request).unwrap_err();

    assert_eq!(error, "Target language cannot be empty");
}

#[test]
fn summary_request_validation_rejects_google_translate_strategy() {
    let mut config = sample_config();
    config.strategy = LlmProviderStrategy::GoogleTranslate;
    let request = SummarizeTranscriptRequest {
        task_id: "task-summary".to_string(),
        config,
        template: SummaryTemplateConfig {
            id: "meeting".to_string(),
            name: "Meeting".to_string(),
            instructions: "Summarize decisions.".to_string(),
        },
        segments: vec![],
        chunk_char_budget: None,
    };

    let error = validate_summarize_transcript_request(&request).unwrap_err();

    assert_eq!(
        error,
        "Google Translate does not support transcript summaries"
    );
}

#[test]
fn segment_request_contracts_keep_camel_case_transport_shape() {
    let polish = PolishSegmentsRequest {
        task_id: "task-polish".to_string(),
        config: sample_config(),
        segments: vec![LlmSegmentInput {
            id: "segment-1".to_string(),
            text: "hello".to_string(),
        }],
        chunk_size: Some(12),
        context: Some("meeting notes".to_string()),
        keywords: None,
    };

    let value = serde_json::to_value(polish).expect("polish request should serialize");

    assert_eq!(value["taskId"], "task-polish");
    assert_eq!(value["chunkSize"], 12);
    assert_eq!(value["config"]["baseUrl"], "https://api.example.com/v1");
    assert!(value.get("task_id").is_none());
    assert!(value.get("chunk_size").is_none());

    let translate: TranslateSegmentsRequest = serde_json::from_value(json!({
        "taskId": "task-translate",
        "config": sample_config(),
        "segments": [{ "id": "segment-1", "text": "hello" }],
        "targetLanguage": "ja",
        "targetLanguageName": "Japanese"
    }))
    .expect("translate request should deserialize");

    assert_eq!(translate.task_id, "task-translate");
    assert_eq!(translate.target_language, "ja");
    assert_eq!(translate.target_language_name.as_deref(), Some("Japanese"));
}

#[test]
fn summary_request_contract_uses_core_summary_inputs() {
    let request: SummarizeTranscriptRequest = serde_json::from_value(json!({
        "taskId": "summary-task",
        "config": sample_config(),
        "template": {
            "id": "meeting",
            "name": "Meeting",
            "instructions": "Summarize decisions."
        },
        "segments": [{
            "id": "segment-1",
            "text": "The team approved the plan.",
            "start": 0.0,
            "end": 12.0,
            "isFinal": true
        }],
        "chunkCharBudget": 2400
    }))
    .expect("summary request should deserialize");

    assert_eq!(request.task_id, "summary-task");
    assert_eq!(request.template.id, "meeting");
    assert_eq!(request.segments[0].id, "segment-1");
    assert_eq!(request.chunk_char_budget, Some(2400));
}

#[test]
fn transcript_job_request_uses_core_transcript_segments() {
    let request = TranscriptLlmJobRequest {
        task_id: "job-1".to_string(),
        task_type: LlmTaskType::Translate,
        job_history_id: Some("history-1".to_string()),
        config: sample_config(),
        segments: vec![sample_transcript_segment()],
        target_language: Some("fr".to_string()),
        target_language_name: Some("French".to_string()),
        context: None,
        keywords: None,
        template: Some(SummaryTemplateConfig {
            id: "meeting".to_string(),
            name: "Meeting".to_string(),
            instructions: "Summarize decisions.".to_string(),
        }),
        chunk_size: Some(8),
        chunk_char_budget: Some(2400),
    };

    let value = serde_json::to_value(&request).expect("job request should serialize");
    let decoded: TranscriptLlmJobRequest =
        serde_json::from_value(value).expect("job request should deserialize");

    assert_eq!(decoded.task_id, "job-1");
    assert_eq!(decoded.task_type, LlmTaskType::Translate);
    assert_eq!(decoded.segments[0], sample_transcript_segment());
    assert_eq!(decoded.target_language_name.as_deref(), Some("French"));
}

#[test]
fn history_summary_payload_omits_absent_record() {
    let empty_payload = HistorySummaryPayload {
        active_template_id: "meeting".to_string(),
        record: None,
    };

    let value = serde_json::to_value(empty_payload).expect("summary payload should serialize");

    assert_eq!(value["activeTemplateId"], "meeting");
    assert!(value.get("record").is_none());

    let payload_with_record = HistorySummaryPayload {
        active_template_id: "meeting".to_string(),
        record: Some(TranscriptSummaryRecordPayload {
            template_id: "meeting".to_string(),
            content: "Final summary".to_string(),
            generated_at: "2026-07-08T00:00:00Z".to_string(),
            source_fingerprint: "fingerprint".to_string(),
        }),
    };

    let value =
        serde_json::to_value(payload_with_record).expect("summary record payload should serialize");

    assert_eq!(value["record"]["templateId"], "meeting");
    assert_eq!(value["record"]["sourceFingerprint"], "fingerprint");
}

#[test]
fn summary_segment_input_stays_available_for_request_construction() {
    let segment = SummarySegmentInput {
        id: "segment-1".to_string(),
        text: "Ship the beta.".to_string(),
        start: 1.0,
        end: 4.0,
        is_final: true,
    };

    assert_eq!(segment.id, "segment-1");
}
