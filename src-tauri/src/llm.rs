use reqwest::Client;
use rig::client::CompletionClient;
use rig::completion::CompletionModel;
use rig::providers::{anthropic, deepseek, gemini, moonshot, ollama, openai};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    Kimi,
    SiliconFlow,
    OpenAiCompatible,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmGenerateRequest {
    pub config: LlmConfig,
    pub input: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelsRequest {
    pub provider: LlmProvider,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct GeminiModel {
    name: String,
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModel>>,
}

fn clean_gemini_base_url(base_url: &str) -> &str {
    let base = base_url.trim_end_matches('/');
    let suffixes = ["/v1beta/models", "/models", "/v1beta", "/v1"];

    for suffix in suffixes {
        if let Some(stripped) = base.strip_suffix(suffix) {
            return stripped;
        }
    }

    base
}

fn format_openai_models_urls(base_url: &str, is_ollama: bool) -> Vec<String> {
    let base = base_url.trim_end_matches('/');

    if is_ollama {
        return vec![format!("{}/api/tags", base), format!("{}/v1/models", base)];
    }

    if base.ends_with("/v1") {
        vec![format!("{}/models", base)]
    } else {
        vec![format!("{}/v1/models", base), format!("{}/models", base)]
    }
}

async fn get_gemini_models(client: &Client, api_key: &str, base_url: &str) -> Result<Vec<String>, String> {
    let cleaned_base = clean_gemini_base_url(base_url);
    let url = format!("{}/v1beta/models?key={}", cleaned_base, api_key);

    let res = client
        .get(&url)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Gemini API Error: {}", res.status()));
    }

    let response_body: GeminiModelsResponse = res.json().await.map_err(|e| e.to_string())?;

    Ok(response_body
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name.trim_start_matches("models/").to_string())
        .collect())
}

async fn get_openai_models(client: &Client, api_key: &str, base_url: &str, is_ollama: bool) -> Result<Vec<String>, String> {
    for url in format_openai_models_urls(base_url, is_ollama) {
        let mut req = client.get(&url).header("Content-Type", "application/json");

        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        if let Ok(res) = req.send().await {
            if res.status().is_success() {
                let text = res.text().await.unwrap_or_default();

                if let Ok(response_body) = serde_json::from_str::<OpenAiModelsResponse>(&text) {
                    return Ok(response_body.data.into_iter().map(|m| m.id).collect());
                }

                if let Ok(response_body) = serde_json::from_str::<OllamaTagsResponse>(&text) {
                    return Ok(response_body.models.into_iter().map(|m| m.name).collect());
                }
            }
        }
    }

    Err("Failed to fetch models from any known endpoint".to_string())
}

fn provider_supports_model_listing(provider: &LlmProvider) -> bool {
    !matches!(provider, LlmProvider::Anthropic)
}

fn extract_text_response(choice: &rig::OneOrMany<rig::completion::AssistantContent>) -> Result<String, String> {
    let parts = choice
        .iter()
        .filter_map(|content| match content {
            rig::completion::AssistantContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(parts.join("\n"))
}

async fn generate_with_openai_compatible(base_url: &str, api_key: &str, model: &str, input: &str, temperature: Option<f32>) -> Result<String, String> {
    let client = openai::Client::from_url(api_key, base_url);
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_anthropic(base_url: &str, api_key: &str, model: &str, input: &str, temperature: Option<f32>) -> Result<String, String> {
    let client = anthropic::ClientBuilder::new(api_key)
        .base_url(base_url)
        .build();
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_gemini(base_url: &str, api_key: &str, model: &str, input: &str, temperature: Option<f32>) -> Result<String, String> {
    let client = gemini::Client::from_url(api_key, clean_gemini_base_url(base_url));
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_ollama(base_url: &str, model: &str, input: &str, temperature: Option<f32>) -> Result<String, String> {
    let client = ollama::Client::from_url(base_url.trim_end_matches("/v1"));
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_deepseek(base_url: &str, api_key: &str, model: &str, input: &str, temperature: Option<f32>) -> Result<String, String> {
    let client = deepseek::Client::from_url(api_key, base_url);
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_kimi(base_url: &str, api_key: &str, model: &str, input: &str, temperature: Option<f32>) -> Result<String, String> {
    let client = moonshot::Client::from_url(api_key, base_url);
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_rig(request: LlmGenerateRequest) -> Result<String, String> {
    let config = request.config;

    match config.provider {
        LlmProvider::OpenAi | LlmProvider::OpenAiCompatible | LlmProvider::SiliconFlow => {
            generate_with_openai_compatible(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Anthropic => {
            generate_with_anthropic(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Gemini => {
            generate_with_gemini(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Ollama => {
            generate_with_ollama(
                &config.base_url,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::DeepSeek => {
            generate_with_deepseek(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Kimi => {
            generate_with_kimi(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
    }
}

#[tauri::command]
pub async fn generate_llm_text(request: LlmGenerateRequest) -> Result<String, String> {
    if request.config.model.trim().is_empty() {
        return Err("Model name cannot be empty".to_string());
    }

    if request.input.trim().is_empty() {
        return Err("Input cannot be empty".to_string());
    }

    generate_with_rig(request).await
}

#[tauri::command]
pub async fn list_llm_models(request: LlmModelsRequest) -> Result<Vec<String>, String> {
    if !provider_supports_model_listing(&request.provider) {
        return Ok(vec![]);
    }

    let client = Client::new();

    match request.provider {
        LlmProvider::Gemini => get_gemini_models(&client, &request.api_key, &request.base_url).await,
        LlmProvider::Ollama => get_openai_models(&client, &request.api_key, &request.base_url, true).await,
        _ => get_openai_models(&client, &request.api_key, &request.base_url, false).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn anthropic_listing_is_disabled() {
        assert!(!provider_supports_model_listing(&LlmProvider::Anthropic));
        assert!(provider_supports_model_listing(&LlmProvider::OpenAi));
    }
}
