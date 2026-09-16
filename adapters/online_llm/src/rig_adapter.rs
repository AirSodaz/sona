use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::{
    CompletionError, CompletionModel, CompletionRequest, CompletionResponse,
};
use rig_core::providers::{anthropic, azure, gemini, ollama, openai};
use rig_core::streaming::StreamingCompletionResponse;
use sona_core::llm::provider_protocol::StandardLlmResponse;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::streaming_protocol::StreamTextAccumulator;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortError;

use crate::completion::{
    build_rig_completion_request, extract_text_response, token_usage_from_rig_usage,
};
use crate::transport::{classify_llm_port_error, http_status_port_error};

#[derive(Clone)]
pub enum RigModel {
    OpenAi(openai::completion::CompletionModel),
    OpenAiResponses(openai::responses_api::ResponsesCompletionModel),
    Azure(azure::CompletionModel),
    Anthropic(anthropic::completion::CompletionModel),
    Gemini(gemini::completion::CompletionModel),
    Ollama(ollama::CompletionModel),
}

impl CompletionModel for RigModel {
    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, CompletionError> {
        match self {
            Self::OpenAi(m) => m.completion(request).await,
            Self::OpenAiResponses(m) => m.completion(request).await,
            Self::Azure(m) => m.completion(request).await,
            Self::Anthropic(m) => m.completion(request).await,
            Self::Gemini(m) => m.completion(request).await,
            Self::Ollama(m) => m.completion(request).await,
        }
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse, CompletionError> {
        match self {
            Self::OpenAi(m) => m.stream(request).await,
            Self::OpenAiResponses(m) => m.stream(request).await,
            Self::Azure(m) => m.stream(request).await,
            Self::Anthropic(m) => m.stream(request).await,
            Self::Gemini(m) => m.stream(request).await,
            Self::Ollama(m) => m.stream(request).await,
        }
    }
}

pub fn normalize_openai_base_url(base_url: &str, api_path: Option<&str>) -> String {
    let trimmed_base = base_url.trim().trim_end_matches('/');
    if trimmed_base.is_empty() {
        return String::new();
    }
    if let Some(path) = api_path {
        let trimmed_path = path.trim().trim_start_matches('/');
        if let Some(prefix) = trimmed_path.strip_suffix("chat/completions") {
            let prefix = prefix.trim_end_matches('/');
            if prefix.is_empty() {
                return trimmed_base.to_string();
            }
            if trimmed_base.ends_with(prefix) {
                return trimmed_base.to_string();
            }
            return format!("{trimmed_base}/{prefix}");
        }
    }
    if trimmed_base.ends_with("/v1") {
        trimmed_base.to_string()
    } else {
        format!("{trimmed_base}/v1")
    }
}

fn convert_http_headers(headers: &rig_core::http_client::HeaderMap) -> reqwest::header::HeaderMap {
    let mut reqwest_headers = reqwest::header::HeaderMap::new();
    for (key, val) in headers.iter() {
        if let (Ok(k), Ok(v)) = (
            reqwest::header::HeaderName::from_bytes(key.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(val.as_bytes()),
        ) {
            reqwest_headers.insert(k, v);
        }
    }
    reqwest_headers
}

pub fn classify_rig_completion_error(error: CompletionError) -> LlmPortError {
    match error {
        CompletionError::HttpError(
            rig_core::http_client::Error::InvalidStatusCodeWithDetails {
                status,
                body,
                headers,
            },
        ) => {
            let reqwest_status = reqwest::StatusCode::from_u16(status.as_u16())
                .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
            let reqwest_headers = convert_http_headers(&headers);
            http_status_port_error(reqwest_status, &reqwest_headers, body)
        }
        CompletionError::ProviderResponse(err) => {
            let reqwest_status = err
                .status
                .and_then(|s| reqwest::StatusCode::from_u16(s.as_u16()).ok())
                .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR);
            let reqwest_headers = err
                .headers
                .as_ref()
                .map(|h| convert_http_headers(h))
                .unwrap_or_default();
            http_status_port_error(reqwest_status, &reqwest_headers, err.body)
        }
        other => classify_llm_port_error(other.to_string()),
    }
}

pub(crate) fn resolve_http_client(
    base_url: &str,
    timeout_seconds: Option<u64>,
) -> Result<reqwest::Client, LlmPortError> {
    if !base_url.trim().is_empty() {
        let url = crate::transport::LlmApiUrl::parse(base_url)?;
        url.client(timeout_seconds)
    } else {
        use std::time::Duration;
        let mut builder = reqwest::Client::builder();
        if let Some(secs) = timeout_seconds {
            builder = builder.timeout(Duration::from_secs(secs));
        }
        builder
            .build()
            .map_err(crate::transport::reqwest_port_error)
    }
}

impl RigModel {
    pub fn from_request(request: &LlmCompletionRequest) -> Result<Self, LlmPortError> {
        let config = &request.config;
        let key = if config.api_key.is_empty() {
            "none"
        } else {
            config.api_key.as_str()
        };
        let http_client = resolve_http_client(&config.base_url, config.timeout_seconds)?;

        match config.strategy {
            LlmProviderStrategy::OpenAiResponses => {
                let mut builder = openai::Client::builder()
                    .api_key(key)
                    .http_client(http_client);
                if !config.base_url.is_empty() {
                    let base_url =
                        normalize_openai_base_url(&config.base_url, config.api_path.as_deref());
                    builder = builder.base_url(base_url);
                }
                let client = builder
                    .build()
                    .map_err(|e| classify_llm_port_error(e.to_string()))?;
                Ok(Self::OpenAiResponses(
                    client.completion_model(&config.model),
                ))
            }
            LlmProviderStrategy::AzureOpenAi => {
                let api_version = config.api_version.as_deref().unwrap_or("2024-10-21");
                let endpoint = config.base_url.trim().trim_end_matches('/').to_string();
                let client = azure::Client::builder()
                    .api_key(azure::AzureOpenAIAuth::ApiKey(config.api_key.clone()))
                    .azure_endpoint(endpoint)
                    .api_version(api_version)
                    .http_client(http_client)
                    .build()
                    .map_err(|e| classify_llm_port_error(e.to_string()))?;
                Ok(Self::Azure(client.completion_model(&config.model)))
            }
            LlmProviderStrategy::Anthropic => {
                let mut builder = anthropic::Client::builder()
                    .api_key(key)
                    .http_client(http_client);
                if !config.base_url.is_empty() {
                    builder = builder.base_url(&config.base_url);
                }
                let client = builder
                    .build()
                    .map_err(|e| classify_llm_port_error(e.to_string()))?;
                let mut model = client.completion_model(&config.model);
                if request.options.prompt_cache
                    == sona_core::llm::runtime::LlmPromptCachePolicy::Automatic
                {
                    model = model.with_automatic_caching();
                }
                Ok(Self::Anthropic(model))
            }
            LlmProviderStrategy::Gemini => {
                let mut builder = gemini::Client::builder()
                    .api_key(key)
                    .http_client(http_client);
                if !config.base_url.is_empty() {
                    let base_url =
                        sona_core::llm::provider_protocol::clean_gemini_base_url(&config.base_url);
                    builder = builder.base_url(base_url);
                }
                let client = builder
                    .build()
                    .map_err(|e| classify_llm_port_error(e.to_string()))?;
                Ok(Self::Gemini(client.completion_model(&config.model)))
            }
            LlmProviderStrategy::Ollama => {
                let mut builder = if config.api_key.is_empty() {
                    ollama::Client::builder().api_key(Nothing)
                } else {
                    ollama::Client::builder().api_key(config.api_key.as_str())
                }
                .http_client(http_client);
                if !config.base_url.is_empty() {
                    builder = builder.base_url(&config.base_url);
                }
                let client = builder
                    .build()
                    .map_err(|e| classify_llm_port_error(e.to_string()))?;
                Ok(Self::Ollama(client.completion_model(&config.model)))
            }
            _ => {
                // OpenAI Chat Completions compatible (works for standard OpenAI, DeepSeek, Moonshot, Qwen, vLLM, LiteLLM, New API, etc.)
                let mut builder = openai::Client::builder()
                    .api_key(key)
                    .http_client(http_client);
                if !config.base_url.is_empty() {
                    let base_url =
                        normalize_openai_base_url(&config.base_url, config.api_path.as_deref());
                    builder = builder.base_url(base_url);
                }
                let client = builder
                    .build()
                    .map_err(|e| classify_llm_port_error(e.to_string()))?;
                Ok(Self::OpenAi(
                    client.completions_api().completion_model(&config.model),
                ))
            }
        }
    }
}

pub async fn execute_rig_completion(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    let model = RigModel::from_request(request)?;
    let builder = build_rig_completion_request(model, request)?;
    let response = builder
        .send()
        .await
        .map_err(classify_rig_completion_error)?;

    Ok(StandardLlmResponse {
        text: extract_text_response(&response.choice)?,
        usage: token_usage_from_rig_usage(Some(response.usage)),
    })
}

pub async fn execute_rig_stream<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn, LlmPortError>,
) -> Result<StandardLlmResponse, LlmPortError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), LlmPortError> + Send + ?Sized,
{
    use futures_util::StreamExt;
    use rig_core::streaming::StreamedAssistantContent;

    let model = RigModel::from_request(request)?;
    let builder = build_rig_completion_request(model, request)?;
    let mut stream = builder
        .stream()
        .await
        .map_err(classify_rig_completion_error)?;
    while let Some(item) = stream.next().await {
        let event = item.map_err(classify_rig_completion_error)?;
        if let StreamedAssistantContent::Text(text) = event {
            accumulator.push(&text.text)?;
        }
    }

    let text = if accumulator.is_empty() {
        extract_text_response(&stream.choice)?
    } else {
        accumulator.text()
    };

    let usage = stream.usage();
    Ok(StandardLlmResponse {
        text,
        usage: token_usage_from_rig_usage(Some(usage)),
    })
}
