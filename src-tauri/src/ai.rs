use reqwest::Client;
use serde::{Deserialize, Serialize};

// --- OpenAI / Generic Compatible Types ---
#[derive(Serialize, Deserialize)]
pub struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
pub struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Deserialize)]
struct OpenAIModel {
    id: String,
}

#[derive(Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

// --- Ollama Tags Types ---
#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

// --- Anthropic Types ---
#[derive(Serialize, Deserialize)]
pub struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
pub struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

// --- Gemini Types ---
#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiCandidateContent,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize)]
struct GeminiModel {
    name: String,
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModel>>,
}

#[tauri::command]
pub async fn call_ai_model(
    api_key: String,
    base_url: String,
    model_name: String,
    input: String,
    api_format: String,
    temperature: Option<f32>,
) -> Result<String, String> {
    let client = Client::new();

    if api_format == "anthropic" {
        // Handle Anthropic
        let url = if base_url.ends_with("/messages") {
            base_url
        } else {
            format!("{}/v1/messages", base_url.trim_end_matches('/'))
        };

        let request_body = AnthropicRequest {
            model: model_name,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: input,
            }],
            max_tokens: 1024,
            temperature,
        };

        let res = client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text().await.unwrap_or_default();
            return Err(format!("Anthropic API Error: {} - {}", status, error_text));
        }

        let response_body: AnthropicResponse = res.json().await.map_err(|e| e.to_string())?;
        if let Some(content) = response_body.content.first() {
            Ok(content.text.clone())
        } else {
            Err("No content in Anthropic response".to_string())
        }
    } else if api_format == "gemini" {
        if model_name.trim().is_empty() {
            return Err("Model name cannot be empty for Gemini API".to_string());
        }

        let base = base_url.trim_end_matches('/');
        let url = if base.contains("generateContent") {
            format!("{}?key={}", base, api_key)
        } else {
            let cleaned_base = if base.ends_with("/v1beta/models") {
                base.strip_suffix("/v1beta/models").unwrap_or(base)
            } else if base.ends_with("/models") {
                base.strip_suffix("/models").unwrap_or(base)
            } else if base.ends_with("/v1beta") {
                base.strip_suffix("/v1beta").unwrap_or(base)
            } else if base.ends_with("/v1") {
                base.strip_suffix("/v1").unwrap_or(base)
            } else {
                base
            };
            format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                cleaned_base, model_name, api_key
            )
        };

        let request_body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: input }],
            }],
            generation_config: temperature.map(|t| GeminiGenerationConfig { temperature: Some(t) }),
        };

        let res = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text().await.unwrap_or_default();
            return Err(format!("Gemini API Error: {} - {}", status, error_text));
        }

        let response_body: GeminiResponse = res.json().await.map_err(|e| e.to_string())?;

        if let Some(candidates) = response_body.candidates {
            if let Some(candidate) = candidates.first() {
                if let Some(part) = candidate.content.parts.first() {
                    return Ok(part.text.clone());
                }
            }
        }
        Err("No content in Gemini response".to_string())
    } else {
        // Default: OpenAI Compatible (works for OpenAI, Ollama, Groq, DeepSeek, etc.)
        let url = if base_url.contains("chat/completions") {
            base_url
        } else {
            let base = base_url.trim_end_matches('/');
            if base.ends_with("/v1") {
                format!("{}/chat/completions", base)
            } else {
                format!("{}/v1/chat/completions", base)
            }
        };

        let request_body = OpenAIRequest {
            model: model_name,
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: input,
            }],
            temperature,
        };

        let mut req = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Only add Authorization header if api_key is not empty (Ollama often doesn't need it)
        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let res = req.send().await.map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            let status = res.status();
            let error_text = res.text().await.unwrap_or_default();
            return Err(format!("OpenAI API Error: {} - {}", status, error_text));
        }

        let response_body: OpenAIResponse = res.json().await.map_err(|e| e.to_string())?;
        if let Some(choice) = response_body.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err("No choices in OpenAI response".to_string())
        }
    }
}

#[tauri::command]
pub async fn get_ai_models(
    api_key: String,
    base_url: String,
    api_format: String,
) -> Result<Vec<String>, String> {
    let client = Client::new();
    let base = base_url.trim_end_matches('/');

    if api_format == "anthropic" {
        Ok(vec![])
    } else if api_format == "gemini" {
        let cleaned_base = if base.ends_with("/v1beta/models") {
            base.strip_suffix("/v1beta/models").unwrap_or(base)
        } else if base.ends_with("/models") {
            base.strip_suffix("/models").unwrap_or(base)
        } else if base.ends_with("/v1beta") {
            base.strip_suffix("/v1beta").unwrap_or(base)
        } else if base.ends_with("/v1") {
            base.strip_suffix("/v1").unwrap_or(base)
        } else {
            base
        };
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

        if let Some(models) = response_body.models {
            let names = models
                .into_iter()
                .map(|m| m.name.trim_start_matches("models/").to_string())
                .collect();
            Ok(names)
        } else {
            Ok(vec![])
        }
    } else {
        // OpenAI / Ollama
        let mut urls_to_try = Vec::new();

        if api_format == "ollama" {
            urls_to_try.push(format!("{}/api/tags", base));
            urls_to_try.push(format!("{}/v1/models", base));
        } else if base.ends_with("/v1") {
            urls_to_try.push(format!("{}/models", base));
        } else {
            urls_to_try.push(format!("{}/v1/models", base));
            urls_to_try.push(format!("{}/models", base));
        }

        for url in urls_to_try {
            let mut req = client.get(&url).header("Content-Type", "application/json");

            if !api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", api_key));
            }

            match req.send().await {
                Ok(res) => {
                    if res.status().is_success() {
                        let text = res.text().await.unwrap_or_default();

                        // Try OpenAI format
                        if let Ok(response_body) =
                            serde_json::from_str::<OpenAIModelsResponse>(&text)
                        {
                            return Ok(response_body.data.into_iter().map(|m| m.id).collect());
                        }

                        // Try Ollama format
                        if let Ok(response_body) = serde_json::from_str::<OllamaTagsResponse>(&text)
                        {
                            return Ok(response_body.models.into_iter().map(|m| m.name).collect());
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        Err("Failed to fetch models from any known endpoint".to_string())
    }
}
