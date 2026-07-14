use sona_core::llm::requests::{LlmConfig, LlmModelsRequest};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmRuntimeError, LlmRuntimeService};
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelMetadataPort, LlmPortErrorKind,
};
use sona_online_llm::OnlineLlmAdapter;

use crate::json_bridge::parse_core_json;
use crate::mapper::{
    FfiLlmCompletionResponse, FfiLlmModelSummary, llm_completion_response_to_ffi,
    llm_model_summary_to_ffi,
};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};

pub(crate) async fn complete_llm_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiLlmCompletionResponse> {
    complete_with_port(&request_json, OnlineLlmAdapter).await
}

pub(crate) async fn list_llm_models_json(
    request_json: String,
) -> SonaCoreBindingResult<Vec<FfiLlmModelSummary>> {
    list_models_with_port(&request_json, OnlineLlmAdapter).await
}

pub(crate) async fn describe_llm_model_json(
    config_json: String,
) -> SonaCoreBindingResult<Option<FfiLlmModelSummary>> {
    describe_model_with_port(&config_json, OnlineLlmAdapter).await
}

async fn complete_with_port<P>(
    request_json: &str,
    port: P,
) -> SonaCoreBindingResult<FfiLlmCompletionResponse>
where
    P: Clone + LlmCompletionPort + LlmModelMetadataPort,
{
    let request: LlmCompletionRequest = parse_core_json(request_json, "LLM completion request")?;
    let response = LlmRuntimeService::new(&port, port.clone())
        .complete(request)
        .await
        .map_err(map_runtime_error)?;
    Ok(llm_completion_response_to_ffi(response))
}

async fn list_models_with_port<P>(
    request_json: &str,
    port: P,
) -> SonaCoreBindingResult<Vec<FfiLlmModelSummary>>
where
    P: Clone + LlmCompletionPort + LlmModelDiscoveryPort + LlmModelMetadataPort,
{
    let request: LlmModelsRequest = parse_core_json(request_json, "LLM models request")?;
    LlmRuntimeService::new(&port, port.clone())
        .list_models(request)
        .await
        .map_err(map_runtime_error)
        .map(|models| models.into_iter().map(llm_model_summary_to_ffi).collect())
}

async fn describe_model_with_port<P>(
    config_json: &str,
    port: P,
) -> SonaCoreBindingResult<Option<FfiLlmModelSummary>>
where
    P: Clone + LlmCompletionPort + LlmModelMetadataPort,
{
    let config: LlmConfig = parse_core_json(config_json, "LLM config")?;
    LlmRuntimeService::new(&port, port.clone())
        .describe_model(&config)
        .await
        .map_err(map_runtime_error)
        .map(|model| model.map(llm_model_summary_to_ffi))
}

fn map_runtime_error(error: LlmRuntimeError) -> SonaCoreBindingError {
    let reason = error.to_string();
    let (code, retry_after_ms) = match &error {
        LlmRuntimeError::InvalidRequest { .. } => ("invalid_request", None),
        LlmRuntimeError::UnsupportedCapability { .. } => ("unsupported_capability", None),
        LlmRuntimeError::InvalidResponse { .. } => ("invalid_response", None),
        LlmRuntimeError::Adapter {
            kind,
            retry_after_ms,
            ..
        } => (port_error_code(*kind), *retry_after_ms),
    };
    SonaCoreBindingError::LlmRuntime {
        code: code.to_string(),
        reason,
        retry_after_ms,
    }
}

fn port_error_code(kind: LlmPortErrorKind) -> &'static str {
    match kind {
        LlmPortErrorKind::InvalidRequest => "invalid_request",
        LlmPortErrorKind::Authentication => "authentication",
        LlmPortErrorKind::Permission => "permission",
        LlmPortErrorKind::RateLimited => "rate_limited",
        LlmPortErrorKind::Timeout => "timeout",
        LlmPortErrorKind::Unavailable => "unavailable",
        LlmPortErrorKind::Unsupported => "unsupported",
        LlmPortErrorKind::Protocol => "protocol",
        LlmPortErrorKind::Network => "network",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
    use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
    use sona_core::llm::runtime::{LlmCompletionOptions, LlmPromptCachePolicy, LlmResponseFormat};
    use sona_core::llm::tasks::LlmProviderStrategy;
    use sona_core::llm::usage::TokenUsage;
    use sona_core::ports::llm::LlmPortError;

    use super::*;
    use crate::mapper::FfiLlmResponseFormatKind;

    #[derive(Clone, Default)]
    struct FakeLlm {
        completed_model: Arc<Mutex<Option<String>>>,
        listed: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl LlmCompletionPort for FakeLlm {
        async fn complete(
            &self,
            request: LlmCompletionRequest,
        ) -> Result<StandardLlmResponse, LlmPortError> {
            *self.completed_model.lock().unwrap() = Some(request.config.model);
            Ok(StandardLlmResponse {
                text: r#"{"ok":true}"#.to_string(),
                usage: Some(TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 3,
                    total_tokens: 13,
                    cached_input_tokens: 4,
                    reasoning_tokens: 2,
                    ..TokenUsage::default()
                }),
            })
        }
    }

    #[async_trait]
    impl LlmModelMetadataPort for FakeLlm {
        async fn describe_model(
            &self,
            config: &LlmConfig,
        ) -> Result<Option<LlmModelSummary>, LlmPortError> {
            Ok(Some(LlmModelSummary {
                model: config.model.clone(),
                display_name: Some("Test Model".to_string()),
                max_output_tokens: Some(4096),
                supports_structured_output: Some(true),
                ..LlmModelSummary::default()
            }))
        }
    }

    #[async_trait]
    impl LlmModelDiscoveryPort for FakeLlm {
        async fn list_models(
            &self,
            _request: LlmModelsRequest,
        ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
            *self.listed.lock().unwrap() = true;
            Ok(vec![LlmModelSummary {
                model: "test-model".to_string(),
                max_output_tokens: Some(4096),
                ..LlmModelSummary::default()
            }])
        }
    }

    fn config() -> LlmConfig {
        LlmConfig {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
            strategy: LlmProviderStrategy::OpenAi,
            base_url: "https://api.example.com".to_string(),
            api_key: "secret".to_string(),
            model: "test-model".to_string(),
            api_path: None,
            api_version: None,
            temperature: None,
            reasoning_enabled: None,
            reasoning_level: None,
            timeout_seconds: None,
        }
    }

    #[tokio::test]
    async fn completion_maps_json_usage_and_execution_metadata() {
        let fake = FakeLlm::default();
        let request = LlmCompletionRequest {
            config: config(),
            system_prompt: Some("stable".to_string()),
            input: "hello".to_string(),
            options: LlmCompletionOptions {
                response_format: LlmResponseFormat::JsonObject,
                prompt_cache: LlmPromptCachePolicy::Automatic,
                ..LlmCompletionOptions::default()
            },
            source: None,
        };

        let response = complete_with_port(&serde_json::to_string(&request).unwrap(), fake.clone())
            .await
            .unwrap();

        assert_eq!(response.json.as_deref(), Some(r#"{"ok":true}"#));
        assert_eq!(response.usage.unwrap().cached_input_tokens, 4);
        assert_eq!(
            response.execution.applied_format,
            FfiLlmResponseFormatKind::JsonObject
        );
        assert_eq!(
            fake.completed_model.lock().unwrap().as_deref(),
            Some("test-model")
        );
    }

    #[tokio::test]
    async fn model_apis_delegate_and_keep_rich_metadata() {
        let fake = FakeLlm::default();
        let list_request = LlmModelsRequest {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
            strategy: Some(LlmProviderStrategy::OpenAi),
            base_url: "https://api.example.com".to_string(),
            api_key: "secret".to_string(),
        };
        let listed =
            list_models_with_port(&serde_json::to_string(&list_request).unwrap(), fake.clone())
                .await
                .unwrap();
        let described =
            describe_model_with_port(&serde_json::to_string(&config()).unwrap(), fake.clone())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(listed[0].max_output_tokens, Some(4096));
        assert_eq!(
            (
                described.display_name.as_deref(),
                described.supports_structured_output
            ),
            (Some("Test Model"), Some(true))
        );
        assert!(*fake.listed.lock().unwrap());
    }
}
