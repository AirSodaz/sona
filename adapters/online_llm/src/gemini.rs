use async_trait::async_trait;
use reqwest::Client;
use rig_core::client::CompletionClient;
use rig_core::providers::gemini;
use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{
    GeminiGenerateContentRequestParts as CoreGeminiGenerateContentRequestParts,
    StandardLlmResponse, build_gemini_generate_content_request_parts, clean_gemini_base_url,
};
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::completion::{
    LlmAdapter, build_rig_completion_request, completion_input, extract_text_response,
    reasoning_budget_tokens, reasoning_level_label, structured_schema, token_usage_from_rig_usage,
};
use crate::transport::{LlmApiUrl, classify_llm_port_error, post_json_request};

pub fn build_gemini_payload_for_request(
    request: &LlmCompletionRequest,
) -> Result<Value, LlmPortError> {
    let mut generation_config = json!({
        "temperature": request.effective_temperature().unwrap_or(0.7),
    });
    if let Some(max_output_tokens) = request.options.max_output_tokens {
        generation_config["maxOutputTokens"] = json!(max_output_tokens);
    }
    if request.effective_reasoning_enabled() {
        let level = request.effective_reasoning_level();
        generation_config["thinkingConfig"] = if request.config.model.contains("gemini-2.5") {
            let budget = request
                .options
                .max_output_tokens
                .map(|limit| {
                    reasoning_budget_tokens(level).min(limit.min(u64::from(u32::MAX)) as u32)
                })
                .unwrap_or_else(|| reasoning_budget_tokens(level));
            json!({
                "thinkingBudget": budget,
                "includeThoughts": true,
            })
        } else {
            json!({
                "thinkingLevel": reasoning_level_label(level),
                "includeThoughts": true,
            })
        };
    }
    if matches!(
        &request.options.response_format,
        sona_core::llm::runtime::LlmResponseFormat::JsonObject
    ) {
        generation_config["responseMimeType"] = json!("application/json");
    } else if let Some(schema) = structured_schema(request)? {
        generation_config["responseMimeType"] = json!("application/json");
        generation_config["responseJsonSchema"] = schema;
    }

    let mut payload = json!({
        "contents": [{"parts": [{"text": completion_input(request)}]}],
        "generationConfig": generation_config,
    });
    if let Some(system_prompt) = request.system_prompt.as_deref() {
        payload["systemInstruction"] = json!({"parts": [{"text": system_prompt}]});
    }
    Ok(payload)
}

pub fn extract_gemini_visible_text(response: &Value) -> Option<String> {
    let text = response
        .pointer("/candidates/0/content/parts")?
        .as_array()?
        .iter()
        .filter(|part| part.get("thought").and_then(Value::as_bool) != Some(true))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

#[derive(Clone, Debug)]
pub struct GeminiGenerateContentRequestParts {
    pub url: LlmApiUrl,
    pub headers: Vec<(&'static str, String)>,
}

pub fn build_gemini_generate_content_request_parts_for_reqwest(
    base_url: &str,
    model: &str,
    api_key: &str,
    stream: bool,
) -> Result<GeminiGenerateContentRequestParts, LlmPortError> {
    let CoreGeminiGenerateContentRequestParts { url, headers } =
        build_gemini_generate_content_request_parts(base_url, model, api_key, stream)?;
    let url = LlmApiUrl::parse(&url)?;

    Ok(GeminiGenerateContentRequestParts { url, headers })
}

pub fn extract_gemini_usage(usage: &Value) -> Option<TokenUsage> {
    let prompt_tokens = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    if prompt_tokens == 0 && completion_tokens == 0 && total_tokens == 0 {
        return None;
    }
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cached_input_tokens: usage
            .get("cachedContentTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        reasoning_tokens: usage
            .get("thoughtsTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        ..TokenUsage::default()
    })
}

pub struct GeminiAdapter;

#[async_trait]
impl LlmAdapter for GeminiAdapter {
    async fn generate(
        &self,
        _client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        if request.effective_reasoning_enabled() {
            let request_parts = build_gemini_generate_content_request_parts_for_reqwest(
                &config.base_url,
                &config.model,
                &config.api_key,
                false,
            )?;

            let payload = build_gemini_payload_for_request(request)?;

            let response = post_json_request(
                &request_parts.url,
                request_parts.headers,
                payload,
                config.timeout_seconds,
            )
            .await?;

            let text = extract_gemini_visible_text(&response).ok_or_else(|| {
                LlmPortError::new(
                    LlmPortErrorKind::Protocol,
                    "Gemini response did not contain text output",
                )
            })?;

            let usage = response.get("usageMetadata").and_then(extract_gemini_usage);

            return Ok(StandardLlmResponse { text, usage });
        }

        let reqwest_client = LlmApiUrl::parse(&config.base_url)?.client(config.timeout_seconds)?;
        let client = gemini::Client::builder()
            .api_key(&config.api_key)
            .base_url(clean_gemini_base_url(&config.base_url))
            .http_client(reqwest_client)
            .build()
            .map_err(|error| classify_llm_port_error(error.to_string()))?;

        let response =
            build_rig_completion_request(client.completion_model(&config.model), request)?
                .send()
                .await
                .map_err(|error| classify_llm_port_error(error.to_string()))?;

        Ok(StandardLlmResponse {
            text: extract_text_response(&response.choice)?,
            usage: token_usage_from_rig_usage(Some(response.usage)),
        })
    }
}
