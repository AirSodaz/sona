use reqwest::Client;
use sona_core::llm::provider_protocol::{
    GeminiModelsResponse, LlmModelSummary, OllamaTagsResponse, OpenAiModelsResponse,
    format_gemini_models_url, format_openai_models_urls, gemini_model_to_summary,
    ollama_model_to_summary, openai_model_to_summary, strategy_supports_model_listing,
};
use sona_core::llm::requests::LlmModelsRequest;
use sona_core::llm::tasks::LlmProviderStrategy;

use crate::models_dev::{
    default_models_dev_catalog, models_dev_provider_id, should_enrich_model_metadata,
};
use crate::transport::{LlmApiUrl, validate_llm_api_host};

pub fn build_gemini_models_url(base_url: &LlmApiUrl) -> Result<LlmApiUrl, String> {
    LlmApiUrl::parse(&format_gemini_models_url(base_url.as_str()))
}

pub fn build_openai_models_urls(
    base_url: &LlmApiUrl,
    is_ollama: bool,
) -> Result<Vec<LlmApiUrl>, String> {
    format_openai_models_urls(base_url.as_str(), is_ollama)
        .into_iter()
        .map(|url| LlmApiUrl::parse(&url))
        .collect()
}

pub async fn get_gemini_models(
    client: &Client,
    api_key: &str,
    base_url: &LlmApiUrl,
) -> Result<Vec<LlmModelSummary>, String> {
    let url = build_gemini_models_url(base_url)?;
    let mut request = client
        .get(url.reqwest_url())
        .header("Content-Type", "application/json");

    if !api_key.is_empty() {
        request = request.header("x-goog-api-key", api_key);
    }

    let res = request.send().await.map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Gemini API Error: {}", res.status()));
    }

    let response_body: GeminiModelsResponse = res.json().await.map_err(|e| e.to_string())?;

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
) -> Result<Vec<LlmModelSummary>, String> {
    for url in build_openai_models_urls(base_url, is_ollama)? {
        let mut req = client
            .get(url.reqwest_url())
            .header("Content-Type", "application/json");

        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        if let Ok(res) = req.send().await
            && res.status().is_success()
        {
            let text = res.text().await.unwrap_or_default();

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
        }
    }

    Err("Failed to fetch models from any known endpoint".to_string())
}

pub async fn list_models_with_provider(
    request: LlmModelsRequest,
) -> Result<Vec<LlmModelSummary>, String> {
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
