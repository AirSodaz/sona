use reqwest::Client;
use sona_core::llm::provider_protocol::{
    GeminiModelsResponse, LlmModelSummary, OllamaTagsResponse, OpenAiModelsResponse,
    format_gemini_models_url, format_openai_models_urls, gemini_model_to_summary,
    ollama_model_to_summary, openai_model_to_summary, strategy_supports_model_listing,
};
use sona_core::llm::requests::LlmModelsRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::models_dev::{
    default_models_dev_catalog, models_dev_provider_id, should_enrich_model_metadata,
};
use crate::transport::{
    LlmApiUrl, http_status_port_error, reqwest_port_error, validate_llm_api_host,
};

pub fn build_gemini_models_url(base_url: &LlmApiUrl) -> Result<LlmApiUrl, LlmPortError> {
    LlmApiUrl::parse(&format_gemini_models_url(base_url.as_str()))
}

pub fn build_openai_models_urls(
    base_url: &LlmApiUrl,
    is_ollama: bool,
) -> Result<Vec<LlmApiUrl>, LlmPortError> {
    format_openai_models_urls(base_url.as_str(), is_ollama)
        .into_iter()
        .map(|url| LlmApiUrl::parse(&url))
        .collect()
}

pub async fn get_gemini_models(
    client: &Client,
    api_key: &str,
    base_url: &LlmApiUrl,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let url = build_gemini_models_url(base_url)?;
    let mut request = client
        .get(url.reqwest_url())
        .header("Content-Type", "application/json");

    if !api_key.is_empty() {
        request = request.header("x-goog-api-key", api_key);
    }

    let res = request.send().await.map_err(reqwest_port_error)?;

    if !res.status().is_success() {
        let status = res.status();
        let headers = res.headers().clone();
        let body = res.text().await.unwrap_or_default();
        return Err(http_status_port_error(status, &headers, body));
    }

    let response_body: GeminiModelsResponse = res.json().await.map_err(reqwest_port_error)?;

    Ok(response_body
        .models
        .unwrap_or_default()
        .into_iter()
        .filter_map(gemini_model_to_summary)
        .collect())
}

pub async fn get_openai_models(
    client: &Client,
    api_key: &str,
    base_url: &LlmApiUrl,
    is_ollama: bool,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let mut last_error = None;
    for url in build_openai_models_urls(base_url, is_ollama)? {
        let mut req = client
            .get(url.reqwest_url())
            .header("Content-Type", "application/json");

        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let res = match req.send().await {
            Ok(res) => res,
            Err(error) => {
                last_error = Some(reqwest_port_error(error));
                continue;
            }
        };
        if !res.status().is_success() {
            let status = res.status();
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            last_error = Some(http_status_port_error(status, &headers, body));
            continue;
        }

        let text = match res.text().await {
            Ok(text) => text,
            Err(error) => {
                last_error = Some(reqwest_port_error(error));
                continue;
            }
        };

        if let Ok(response_body) = serde_json::from_str::<OpenAiModelsResponse>(&text) {
            return Ok(response_body
                .data
                .into_iter()
                .map(openai_model_to_summary)
                .collect());
        }

        if let Ok(response_body) = serde_json::from_str::<OllamaTagsResponse>(&text) {
            return Ok(response_body
                .models
                .into_iter()
                .map(ollama_model_to_summary)
                .collect());
        }

        last_error = Some(LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!(
                "model-list response from {} did not match a supported schema",
                url.as_str()
            ),
        ));
    }

    Err(last_error.unwrap_or_else(|| {
        LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            "no model-list endpoint could be derived from the LLM API host",
        )
    }))
}

pub async fn list_models_with_provider(
    request: LlmModelsRequest,
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let strategy = request
        .strategy
        .unwrap_or_else(|| LlmProviderStrategy::from_provider(&request.provider));
    if !strategy_supports_model_listing(strategy) {
        return Ok(vec![]);
    }
    validate_llm_api_host(&request.base_url)?;

    let base_url = LlmApiUrl::parse(&request.base_url)?;
    let client = base_url.client(None)?;

    let discovered = match strategy {
        LlmProviderStrategy::Gemini => {
            get_gemini_models(&client, &request.api_key, &base_url).await
        }
        LlmProviderStrategy::Ollama => {
            get_openai_models(&client, &request.api_key, &base_url, true).await
        }
        _ => get_openai_models(&client, &request.api_key, &base_url, false).await,
    }?;
    if !should_enrich_model_metadata(&request.provider, &request.base_url) {
        return Ok(discovered);
    }
    let Some(provider_id) = models_dev_provider_id(strategy) else {
        return Ok(discovered);
    };
    Ok(default_models_dev_catalog()
        .enrich(provider_id, discovered)
        .await)
}
