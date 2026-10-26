use std::time::Duration;

use reqwest::Url;
use serde::Deserialize;
use sona_core::llm::provider_protocol::{
    LlmModality, LlmModelMetadataSource, LlmModelSummary, join_url,
};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::transport::{
    LlmApiUrl, http_status_port_error, parse_llm_api_host, reqwest_port_error,
    validate_llm_api_host,
};

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(8);

pub async fn discover_native_provider_models(
    strategy: LlmProviderStrategy,
    api_key: &str,
    base_url: &str,
) -> Option<Result<Vec<LlmModelSummary>, LlmPortError>> {
    match strategy {
        LlmProviderStrategy::Gemini => Some(discover_gemini_models(api_key, base_url).await),
        LlmProviderStrategy::Ollama => Some(discover_ollama_models(api_key, base_url).await),
        LlmProviderStrategy::OpenRouter => {
            Some(discover_openrouter_models(api_key, base_url).await)
        }
        LlmProviderStrategy::Anthropic => Some(discover_anthropic_models(api_key, base_url).await),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Gemini
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GeminiListResponse {
    models: Option<Vec<GeminiModelItem>>,
}

#[derive(Deserialize)]
struct GeminiModelItem {
    name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "inputTokenLimit")]
    input_token_limit: Option<u64>,
    #[serde(rename = "outputTokenLimit")]
    output_token_limit: Option<u64>,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

pub fn parse_gemini_models(body: &str) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let parsed: GeminiListResponse = serde_json::from_str(body).map_err(|error| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("failed to parse Gemini models response: {error}"),
        )
    })?;

    let models = parsed.models.unwrap_or_default();
    let summaries = models
        .into_iter()
        .map(|item| {
            let model = item
                .name
                .strip_prefix("models/")
                .unwrap_or(&item.name)
                .to_string();

            let has_generate = item
                .supported_generation_methods
                .as_ref()
                .is_some_and(|methods| methods.iter().any(|m| m == "generateContent"));

            let supports_tools = if has_generate { Some(true) } else { None };
            let supports_multimodal = if has_generate { Some(true) } else { None };

            LlmModelSummary {
                model,
                display_name: item.display_name,
                context_window: item.input_token_limit,
                max_output_tokens: item.output_token_limit,
                supports_tools,
                supports_multimodal,
                metadata_sources: vec![LlmModelMetadataSource::Provider],
                ..Default::default()
            }
        })
        .collect();

    Ok(summaries)
}

async fn discover_gemini_models(
    api_key: &str,
    base_url: &str,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    validate_llm_api_host(base_url)?;

    let path = if base_url.trim_end_matches('/').ends_with("/v1beta") {
        "models"
    } else {
        "v1beta/models"
    };
    let joined = join_url(base_url, path);
    let mut url = Url::parse(&joined).map_err(|err| {
        LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            format!("invalid Gemini URL: {err}"),
        )
    })?;

    if !api_key.is_empty() {
        url.query_pairs_mut().append_pair("key", api_key);
    }

    let api_url = LlmApiUrl::parse(base_url)?;
    let client = api_url.client(Some(DISCOVERY_TIMEOUT.as_secs()))?;

    let mut req = client.get(url);
    if !api_key.is_empty() {
        req = req.header("x-goog-api-key", api_key);
    }

    let response = req.send().await.map_err(reqwest_port_error)?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.text().await.map_err(reqwest_port_error)?;

    if !status.is_success() {
        return Err(http_status_port_error(status, &headers, body));
    }

    parse_gemini_models(&body)
}

// ---------------------------------------------------------------------------
// Ollama
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaTagItem>>,
}

#[derive(Deserialize)]
struct OllamaTagItem {
    name: String,
    details: Option<OllamaTagDetails>,
}

#[derive(Deserialize)]
struct OllamaTagDetails {
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

pub fn parse_ollama_models(body: &str) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let parsed: OllamaTagsResponse = serde_json::from_str(body).map_err(|error| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("failed to parse Ollama tags response: {error}"),
        )
    })?;

    let models = parsed.models.unwrap_or_default();
    let summaries = models
        .into_iter()
        .map(|item| {
            let display_name = match item.details {
                Some(OllamaTagDetails {
                    parameter_size: Some(param),
                    quantization_level: Some(quant),
                }) if !param.is_empty() && !quant.is_empty() => {
                    Some(format!("{} ({}, {})", item.name, param, quant))
                }
                Some(OllamaTagDetails {
                    parameter_size: Some(param),
                    ..
                }) if !param.is_empty() => Some(format!("{} ({})", item.name, param)),
                Some(OllamaTagDetails {
                    quantization_level: Some(quant),
                    ..
                }) if !quant.is_empty() => Some(format!("{} ({})", item.name, quant)),
                _ => None,
            };

            LlmModelSummary {
                model: item.name,
                display_name,
                metadata_sources: vec![LlmModelMetadataSource::Provider],
                ..Default::default()
            }
        })
        .collect();

    Ok(summaries)
}

async fn discover_ollama_models(
    api_key: &str,
    base_url: &str,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    validate_llm_api_host(base_url)?;

    let joined = join_url(base_url, "api/tags");
    let url = parse_llm_api_host(&joined)?;
    let api_url = LlmApiUrl::parse(base_url)?;
    let client = api_url.client(Some(DISCOVERY_TIMEOUT.as_secs()))?;

    let mut req = client.get(url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {api_key}"));
    }

    let response = req.send().await.map_err(reqwest_port_error)?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.text().await.map_err(reqwest_port_error)?;

    if !status.is_success() {
        return Err(http_status_port_error(status, &headers, body));
    }

    parse_ollama_models(&body)
}

// ---------------------------------------------------------------------------
// OpenRouter
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct OpenRouterListResponse {
    data: Option<Vec<OpenRouterModelItem>>,
}

#[derive(Deserialize)]
struct OpenRouterModelItem {
    id: String,
    name: Option<String>,
    context_length: Option<u64>,
    pricing: Option<OpenRouterPricing>,
    top_provider: Option<OpenRouterTopProvider>,
    architecture: Option<OpenRouterArchitecture>,
}

#[derive(Deserialize)]
struct OpenRouterPricing {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    prompt: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_f64_from_string_or_number"
    )]
    completion: Option<f64>,
}

fn deserialize_optional_f64_from_string_or_number<'de, D>(
    deserializer: D,
) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrFloat {
        Float(f64),
        String(String),
    }

    match Option::<StringOrFloat>::deserialize(deserializer)? {
        Some(StringOrFloat::Float(f)) => Ok(Some(f)),
        Some(StringOrFloat::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<f64>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
        None => Ok(None),
    }
}

#[derive(Deserialize)]
struct OpenRouterTopProvider {
    max_completion_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct OpenRouterArchitecture {
    modality: Option<String>,
}

pub fn parse_openrouter_models(body: &str) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let parsed: OpenRouterListResponse = serde_json::from_str(body).map_err(|error| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("failed to parse OpenRouter models response: {error}"),
        )
    })?;

    let models = parsed.data.unwrap_or_default();
    let summaries = models
        .into_iter()
        .map(|item| {
            let max_output_tokens = item
                .top_provider
                .as_ref()
                .and_then(|tp| tp.max_completion_tokens);

            let input_price = item.pricing.as_ref().and_then(|p| p.prompt).map(|val| {
                let converted = if val < 0.001 { val * 1_000_000.0 } else { val };
                (converted * 1_000_000.0).round() / 1_000_000.0
            });

            let output_price = item.pricing.as_ref().and_then(|p| p.completion).map(|val| {
                let converted = if val < 0.001 { val * 1_000_000.0 } else { val };
                (converted * 1_000_000.0).round() / 1_000_000.0
            });

            let mut input_modalities = Vec::new();
            let mut supports_multimodal = None;

            if let Some(modality) = item
                .architecture
                .as_ref()
                .and_then(|arch| arch.modality.as_ref())
            {
                let lower = modality.to_lowercase();
                if lower.contains("text") {
                    input_modalities.push(LlmModality::Text);
                }
                if lower.contains("image") {
                    input_modalities.push(LlmModality::Image);
                }
                if lower.contains("audio") {
                    input_modalities.push(LlmModality::Audio);
                }
                if lower.contains("video") {
                    input_modalities.push(LlmModality::Video);
                }
                if lower.contains("image")
                    || lower.contains("audio")
                    || lower.contains("multimodal")
                {
                    supports_multimodal = Some(true);
                }
            }

            LlmModelSummary {
                model: item.id,
                display_name: item.name,
                context_window: item.context_length,
                max_output_tokens,
                input_price,
                output_price,
                input_modalities,
                supports_multimodal,
                metadata_sources: vec![LlmModelMetadataSource::Provider],
                ..Default::default()
            }
        })
        .collect();

    Ok(summaries)
}

async fn discover_openrouter_models(
    api_key: &str,
    base_url: &str,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    validate_llm_api_host(base_url)?;

    let path = if base_url.trim_end_matches('/').ends_with("/v1") {
        "models"
    } else {
        "api/v1/models"
    };
    let joined = join_url(base_url, path);
    let url = parse_llm_api_host(&joined)?;
    let api_url = LlmApiUrl::parse(base_url)?;
    let client = api_url.client(Some(DISCOVERY_TIMEOUT.as_secs()))?;

    let mut req = client.get(url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {api_key}"));
    }

    let response = req.send().await.map_err(reqwest_port_error)?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.text().await.map_err(reqwest_port_error)?;

    if !status.is_success() {
        return Err(http_status_port_error(status, &headers, body));
    }

    parse_openrouter_models(&body)
}

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct AnthropicListResponse {
    data: Option<Vec<AnthropicModelItem>>,
}

#[derive(Deserialize)]
struct AnthropicModelItem {
    id: String,
    display_name: Option<String>,
    created_at: Option<String>,
}

pub fn parse_anthropic_models(body: &str) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let parsed: AnthropicListResponse = serde_json::from_str(body).map_err(|error| {
        LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("failed to parse Anthropic models response: {error}"),
        )
    })?;

    let models = parsed.data.unwrap_or_default();
    let summaries = models
        .into_iter()
        .map(|item| LlmModelSummary {
            model: item.id,
            display_name: item.display_name,
            release_date: item.created_at,
            metadata_sources: vec![LlmModelMetadataSource::Provider],
            ..Default::default()
        })
        .collect();

    Ok(summaries)
}

async fn discover_anthropic_models(
    api_key: &str,
    base_url: &str,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    validate_llm_api_host(base_url)?;

    let path = if base_url.trim_end_matches('/').ends_with("/v1") {
        "models"
    } else {
        "v1/models"
    };
    let joined = join_url(base_url, path);
    let url = parse_llm_api_host(&joined)?;
    let api_url = LlmApiUrl::parse(base_url)?;
    let client = api_url.client(Some(DISCOVERY_TIMEOUT.as_secs()))?;

    let mut req = client.get(url).header("anthropic-version", "2023-06-01");
    if !api_key.is_empty() {
        req = req.header("x-api-key", api_key);
    }

    let response = req.send().await.map_err(reqwest_port_error)?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.text().await.map_err(reqwest_port_error)?;

    if !status.is_success() {
        return Err(http_status_port_error(status, &headers, body));
    }

    parse_anthropic_models(&body)
}
