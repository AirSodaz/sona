use async_trait::async_trait;
use serde_json::json;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::provider_protocol::normalize_token_usage;
use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use sona_core::llm::requests::{LlmConfig, LlmModelsRequest};
use sona_core::llm::runtime::{
    LlmCapabilityPolicy, LlmCompletionOptions, LlmCompletionRequest, LlmResponseFormat,
    LlmResponseFormatKind, LlmRuntimeError, LlmRuntimeService, LlmStreamDelta,
};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelMetadataPort, LlmPortError, LlmStreamingPort,
};
use std::sync::Mutex;

#[test]
fn token_usage_deserializes_legacy_payloads() {
    let usage: sona_core::llm::usage::TokenUsage = serde_json::from_value(json!({
        "promptTokens": 3,
        "completionTokens": 5,
        "totalTokens": 8
    }))
    .unwrap();

    assert_eq!(
        (
            usage.cached_input_tokens,
            usage.cache_creation_input_tokens,
            usage.reasoning_tokens
        ),
        (0, 0, 0)
    );
}

#[test]
fn token_usage_preserves_values_larger_than_u32() {
    let usage = normalize_token_usage(u32::MAX as u64 + 1, 2, 0).unwrap();

    assert_eq!(usage.prompt_tokens, u32::MAX as u64 + 1);
    assert_eq!(usage.total_tokens, u32::MAX as u64 + 3);
}

fn config() -> LlmConfig {
    LlmConfig {
        provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        strategy: LlmProviderStrategy::OpenAi,
        base_url: "https://api.example.com".into(),
        api_key: "test-key".into(),
        model: "test-model".into(),
        api_path: None,
        api_version: None,
        temperature: Some(0.7),
        reasoning_enabled: None,
        reasoning_level: None,
        timeout_seconds: None,
    }
}

fn model(supports_structured_output: Option<bool>) -> LlmModelSummary {
    LlmModelSummary {
        model: "test-model".into(),
        supports_structured_output,
        ..LlmModelSummary::default()
    }
}

struct FakeCompletionPort {
    response: String,
    requests: Mutex<Vec<LlmCompletionRequest>>,
}

#[async_trait]
impl LlmCompletionPort for FakeCompletionPort {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        self.requests.lock().unwrap().push(request);
        Ok(StandardLlmResponse {
            text: self.response.clone(),
            usage: None,
        })
    }
}

#[async_trait]
impl LlmModelDiscoveryPort for FakeCompletionPort {
    async fn list_models(
        &self,
        _request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
        Ok(vec![model(Some(true))])
    }
}

#[async_trait]
impl LlmStreamingPort for FakeCompletionPort {
    async fn stream_completion(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError> {
        self.requests.lock().unwrap().push(request);
        emit_delta(LlmStreamDelta {
            text: self.response.clone(),
            delta: self.response.clone(),
        })?;
        Ok(StandardLlmResponse {
            text: self.response.clone(),
            usage: None,
        })
    }
}

struct FakeMetadataPort(Option<LlmModelSummary>);

#[async_trait]
impl LlmModelMetadataPort for FakeMetadataPort {
    async fn describe_model(
        &self,
        _config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError> {
        Ok(self.0.clone())
    }
}

fn request(response_format: LlmResponseFormat) -> LlmCompletionRequest {
    LlmCompletionRequest {
        config: config(),
        system_prompt: Some("Return one object.".into()),
        input: "hello".into(),
        options: LlmCompletionOptions {
            temperature: Some(0.2),
            response_format,
            ..LlmCompletionOptions::default()
        },
        source: None,
    }
}

#[tokio::test]
async fn runtime_normalizes_options_and_parses_json_output() {
    let completion = FakeCompletionPort {
        response: r#"{"answer":"ok"}"#.into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(Some(model(Some(true)))));

    let response = service
        .complete(request(LlmResponseFormat::JsonObject))
        .await
        .unwrap();

    assert_eq!(response.json, Some(json!({"answer": "ok"})));
    assert_eq!(
        response.execution.applied_format,
        LlmResponseFormatKind::JsonObject
    );
    assert_eq!(
        completion.requests.lock().unwrap()[0].options.temperature,
        Some(0.2)
    );
}

#[tokio::test]
async fn strict_schema_mode_rejects_known_unsupported_model() {
    let completion = FakeCompletionPort {
        response: "unused".into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(Some(model(Some(false)))));
    let mut request = request(LlmResponseFormat::JsonSchema {
        name: "answer".into(),
        schema: json!({"type": "object"}),
    });
    request.options.capability_policy = LlmCapabilityPolicy::Strict;

    let error = service.complete(request).await.unwrap_err();

    assert!(matches!(
        error,
        LlmRuntimeError::UnsupportedCapability { .. }
    ));
    assert!(completion.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn compatible_schema_mode_downgrades_and_reports_warning() {
    let completion = FakeCompletionPort {
        response: r#"{"answer":"ok"}"#.into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(Some(model(Some(false)))));

    let response = service
        .complete(request(LlmResponseFormat::JsonSchema {
            name: "answer".into(),
            schema: json!({"type": "object"}),
        }))
        .await
        .unwrap();

    assert_eq!(
        response.execution.applied_format,
        LlmResponseFormatKind::JsonObject
    );
    assert_eq!(response.execution.warnings.len(), 1);
    assert!(matches!(
        completion.requests.lock().unwrap()[0]
            .options
            .response_format,
        LlmResponseFormat::JsonObject
    ));
}

#[tokio::test]
async fn compatible_schema_downgrade_still_validates_the_requested_schema() {
    let completion = FakeCompletionPort {
        response: r#"{"answer":1}"#.into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(Some(model(Some(false)))));
    let error = service
        .complete(request(LlmResponseFormat::JsonSchema {
            name: "answer".into(),
            schema: json!({"properties": {"answer": {"type": "string"}}}),
        }))
        .await
        .unwrap_err();

    assert!(matches!(error, LlmRuntimeError::InvalidResponse { .. }));
}

#[tokio::test]
async fn invalid_schema_is_rejected_before_adapter_delegation() {
    let completion = FakeCompletionPort {
        response: "unused".into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(None));
    let error = service
        .complete(request(LlmResponseFormat::JsonSchema {
            name: "answer".into(),
            schema: json!({"type": 42}),
        }))
        .await
        .unwrap_err();

    assert!(matches!(error, LlmRuntimeError::InvalidRequest { .. }));
    assert!(completion.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn json_object_mode_rejects_non_object_output() {
    let completion = FakeCompletionPort {
        response: "[]".into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(None));

    let error = service
        .complete(request(LlmResponseFormat::JsonObject))
        .await
        .unwrap_err();

    assert!(matches!(error, LlmRuntimeError::InvalidResponse { .. }));
}

#[tokio::test]
async fn schema_mode_validates_the_parsed_output() {
    let completion = FakeCompletionPort {
        response: r#"{"answer":1}"#.into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(Some(model(Some(true)))));

    let error = service
        .complete(request(LlmResponseFormat::JsonSchema {
            name: "answer".into(),
            schema: json!({
                "type": "object",
                "required": ["answer"],
                "properties": {"answer": {"type": "string"}}
            }),
        }))
        .await
        .unwrap_err();

    assert!(matches!(error, LlmRuntimeError::InvalidResponse { .. }));
}

#[tokio::test]
async fn runtime_delegates_model_discovery_through_the_typed_port() {
    let completion = FakeCompletionPort {
        response: "unused".into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(None));

    let models = service
        .list_models(LlmModelsRequest {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
            strategy: Some(LlmProviderStrategy::OpenAi),
            base_url: "https://api.example.com".into(),
            api_key: "test-key".into(),
        })
        .await
        .unwrap();

    assert_eq!(models[0].model, "test-model");
}

#[tokio::test]
async fn runtime_streams_deltas_and_validates_the_final_output() {
    let completion = FakeCompletionPort {
        response: r#"{"answer":"ok"}"#.into(),
        requests: Mutex::new(Vec::new()),
    };
    let service = LlmRuntimeService::new(&completion, FakeMetadataPort(Some(model(Some(true)))));
    let mut deltas = Vec::new();

    let response = service
        .stream(request(LlmResponseFormat::JsonObject), &mut |event| {
            deltas.push(event.delta);
            Ok(())
        })
        .await
        .unwrap();

    assert_eq!(deltas, vec![r#"{"answer":"ok"}"#]);
    assert_eq!(response.json, Some(json!({"answer": "ok"})));
}
