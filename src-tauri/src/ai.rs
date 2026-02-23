use serde::{Deserialize, Serialize};
use reqwest::Client;

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
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
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

#[tauri::command]
pub async fn call_ai_model(
    api_key: String,
    base_url: String,
    model_name: String,
    input: String,
    api_format: String,
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
             let error_text = res.text().await.unwrap_or_default();
            return Err(format!("Anthropic API Error: {} - {}", res.status(), error_text));
        }

        let response_body: AnthropicResponse = res.json().await.map_err(|e| e.to_string())?;
        if let Some(content) = response_body.content.first() {
            Ok(content.text.clone())
        } else {
            Err("No content in Anthropic response".to_string())
        }

    } else if api_format == "gemini" {
        // Handle Google Gemini
        // Base URL expectation: https://generativelanguage.googleapis.com/v1beta/models
        // Or full URL provided?
        // Let's assume user provides base like https://generativelanguage.googleapis.com

        // Construct URL: {base_url}/v1beta/models/{model_name}:generateContent?key={api_key}
        // But if user puts full path, we need to be careful.
        // Let's assume standard base_url is "https://generativelanguage.googleapis.com"

        let url = if base_url.contains("generateContent") {
             // User provided full URL, just append key if missing?
             // Safer to construct it properly if we can.
             format!("{}?key={}", base_url, api_key)
        } else {
             format!("{}/v1beta/models/{}:generateContent?key={}", base_url.trim_end_matches('/'), model_name, api_key)
        };

        let request_body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: input }],
            }],
        };

        let res = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
             let error_text = res.text().await.unwrap_or_default();
            return Err(format!("Gemini API Error: {} - {}", res.status(), error_text));
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
             format!("{}/v1/chat/completions", base_url.trim_end_matches('/'))
        };

        let request_body = OpenAIRequest {
            model: model_name,
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: input,
            }],
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
             let error_text = res.text().await.unwrap_or_default();
            return Err(format!("OpenAI API Error: {} - {}", res.status(), error_text));
        }

        let response_body: OpenAIResponse = res.json().await.map_err(|e| e.to_string())?;
        if let Some(choice) = response_body.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err("No choices in OpenAI response".to_string())
        }
    }
}
