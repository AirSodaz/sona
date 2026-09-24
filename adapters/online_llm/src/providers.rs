use std::future::Future;
use std::time::Duration;

use crate::transport::{
    LlmApiUrl, classify_llm_port_error, http_status_port_error, post_json_request,
    reqwest_port_error,
};
use futures_util::{StreamExt, stream};
use log::{info, warn};
use reqwest::{
    Client, StatusCode,
    header::{ACCEPT, ACCEPT_LANGUAGE, RETRY_AFTER, USER_AGENT},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sona_core::llm::provider_protocol::StandardLlmResponse;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

const GOOGLE_TRANSLATE_FREE_MAX_RETRIES: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS: u64 = 5;
const GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS: [u64; GOOGLE_TRANSLATE_FREE_MAX_RETRIES] = [500, 1000];

pub const GOOGLE_TRANSLATE_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/139.0.0.0 Safari/537.36";

#[derive(Serialize)]
pub struct GoogleTranslateRequest {
    pub q: Vec<String>,
    pub target: String,
    pub format: String,
}

#[derive(Deserialize)]
pub struct GoogleTranslateTranslation {
    #[serde(rename = "translatedText")]
    pub translated_text: String,
}

#[derive(Deserialize)]
pub struct GoogleTranslateData {
    pub translations: Vec<GoogleTranslateTranslation>,
}

#[derive(Deserialize)]
pub struct GoogleTranslateResponse {
    pub data: GoogleTranslateData,
}

pub async fn execute_google_translate_request(
    client: &Client,
    url: &LlmApiUrl,
    api_key: &str,
    texts: Vec<String>,
    target_language: String,
) -> Result<String, LlmPortError> {
    let payload = GoogleTranslateRequest {
        q: texts,
        target: target_language,
        format: "text".to_string(),
    };

    let response = client
        .post(url.reqwest_url())
        .header("x-goog-api-key", api_key)
        .json(&payload)
        .send()
        .await
        .map_err(reqwest_port_error)?;

    let status = response.status();
    let headers = response.headers().clone();
    let text = response.text().await.map_err(reqwest_port_error)?;

    if !status.is_success() {
        return Err(http_status_port_error(status, &headers, text));
    }

    Ok(text)
}

#[derive(Debug, Clone)]
pub enum GoogleTranslateFreeAttemptError {
    HttpStatus {
        status: StatusCode,
        retry_after: Option<Duration>,
    },
    Message(String),
    Port(LlmPortError),
}

fn format_attempt_label(attempts: usize) -> &'static str {
    if attempts == 1 { "attempt" } else { "attempts" }
}

pub fn parse_google_translate_free_retry_after(
    headers: &reqwest::header::HeaderMap,
) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| clamp_google_translate_free_retry_after(Duration::from_secs(seconds)))
}

fn clamp_google_translate_free_retry_after(duration: Duration) -> Duration {
    duration.min(Duration::from_secs(
        GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS,
    ))
}

fn default_google_translate_free_retry_delay(failed_attempt: usize) -> Duration {
    let delay_ms = GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS
        .get(failed_attempt.saturating_sub(1))
        .copied()
        .unwrap_or(
            *GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS
                .last()
                .unwrap_or(&1000),
        );
    Duration::from_millis(delay_ms)
}

fn google_translate_free_retry_delay(
    error: &GoogleTranslateFreeAttemptError,
    failed_attempt: usize,
) -> Option<Duration> {
    match error {
        GoogleTranslateFreeAttemptError::HttpStatus {
            status,
            retry_after,
        } if *status == StatusCode::TOO_MANY_REQUESTS
            && failed_attempt <= GOOGLE_TRANSLATE_FREE_MAX_RETRIES =>
        {
            Some(
                retry_after
                    .map(clamp_google_translate_free_retry_after)
                    .unwrap_or_else(|| default_google_translate_free_retry_delay(failed_attempt)),
            )
        }
        _ => None,
    }
}

fn google_translate_free_error_summary(error: &GoogleTranslateFreeAttemptError) -> String {
    match error {
        GoogleTranslateFreeAttemptError::HttpStatus { status, .. } => {
            format!("Free API Error: {}", status)
        }
        GoogleTranslateFreeAttemptError::Message(message) => {
            format!("Free translation request failed: {}", message)
        }
        GoogleTranslateFreeAttemptError::Port(error) => {
            format!("Free translation request failed: {}", error.message)
        }
    }
}

pub fn extract_google_translate_free_translation(
    body: &Value,
) -> Result<String, GoogleTranslateFreeAttemptError> {
    let mut translated = String::new();

    if let Some(outer_arr) = body.as_array()
        && let Some(first) = outer_arr.first()
    {
        if let Some(inner_arr) = first.as_array() {
            if let Some(first_elem) = inner_arr.first() {
                if first_elem.is_array() {
                    // Format A: [[["translated text", "orig", ...], ...], ...]
                    for part in inner_arr {
                        if let Some(text) = part.get(0).and_then(|value| value.as_str()) {
                            translated.push_str(text);
                        }
                    }
                } else if let Some(text) = first_elem.as_str() {
                    // Format B: [["translated text", "src_lang"]]
                    translated.push_str(text);
                }
            }
        } else if first.is_string() {
            // Format C: ["translated text 1", "translated text 2"]
            for item in outer_arr {
                if let Some(text) = item.as_str() {
                    translated.push_str(text);
                }
            }
        }
    }

    if translated.is_empty() {
        return Err(GoogleTranslateFreeAttemptError::Message(
            "Empty translation".to_string(),
        ));
    }

    Ok(translated)
}

pub fn build_google_translate_free_candidate_urls(
    base_url: &LlmApiUrl,
    target_language: &str,
    text: &str,
) -> Vec<LlmApiUrl> {
    let encoded = urlencoding::encode(text);
    let base_str = base_url.as_str();
    let mut candidates = Vec::with_capacity(4);

    if base_str.contains("translate_a/t") {
        if let Ok(url) = base_url.with_query(&format!(
            "client=dict-chrome-ex&sl=auto&tl={target_language}&q={encoded}"
        )) {
            candidates.push(url);
        }
        if let Ok(url) = LlmApiUrl::parse(&format!(
            "https://translate.googleapis.com/translate_a/single?client=at&sl=auto&tl={target_language}&dt=t&q={encoded}"
        )) {
            candidates.push(url);
        }
    } else {
        // Standard endpoint candidate chain:
        // 1. client=at on base_url (solves 429 on client=gtx)
        if let Ok(url) = base_url.with_query(&format!(
            "client=at&sl=auto&tl={target_language}&dt=t&q={encoded}"
        )) {
            candidates.push(url);
        }
        // 2. clients5.google.com Chrome extension fallback
        if let Ok(url) = LlmApiUrl::parse(&format!(
            "https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=auto&tl={target_language}&q={encoded}"
        )) {
            candidates.push(url);
        }
        // 3. translate.google.com fallback with client=at
        if let Ok(url) = LlmApiUrl::parse(&format!(
            "https://translate.google.com/translate_a/single?client=at&sl=auto&tl={target_language}&dt=t&q={encoded}"
        )) {
            candidates.push(url);
        }
        // 4. legacy client=gtx on base_url
        if let Ok(url) = base_url.with_query(&format!(
            "client=gtx&sl=auto&tl={target_language}&dt=t&q={encoded}"
        )) {
            candidates.push(url);
        }
    }

    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.as_str().to_string()));
    candidates
}

pub async fn fetch_google_translate_free_translation(
    client: &Client,
    base_url: &LlmApiUrl,
    target_language: &str,
    text: &str,
) -> Result<String, GoogleTranslateFreeAttemptError> {
    let candidates = build_google_translate_free_candidate_urls(base_url, target_language, text);
    if candidates.is_empty() {
        return Err(GoogleTranslateFreeAttemptError::Message(
            "No valid translation endpoints".to_string(),
        ));
    }

    let mut last_error = None;

    for candidate_url in &candidates {
        let response = match client
            .get(candidate_url.reqwest_url())
            .header(USER_AGENT, GOOGLE_TRANSLATE_USER_AGENT)
            .header(ACCEPT, "*/*")
            .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                log::debug!(
                    "[LLM] google_translate_free request to {} failed: {}",
                    candidate_url.as_str(),
                    err
                );
                last_error = Some(GoogleTranslateFreeAttemptError::Port(reqwest_port_error(
                    err,
                )));
                continue;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let retry_after = parse_google_translate_free_retry_after(response.headers());
            log::info!(
                "[LLM] google_translate_free endpoint {} returned status {}, trying next candidate if available",
                candidate_url.as_str(),
                status
            );
            last_error = Some(GoogleTranslateFreeAttemptError::HttpStatus {
                status,
                retry_after,
            });
            continue;
        }

        let body: Value = match response.json().await {
            Ok(b) => b,
            Err(err) => {
                last_error = Some(GoogleTranslateFreeAttemptError::Port(reqwest_port_error(
                    err,
                )));
                continue;
            }
        };

        match extract_google_translate_free_translation(&body) {
            Ok(translation) => return Ok(translation),
            Err(err) => {
                last_error = Some(err);
                continue;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        GoogleTranslateFreeAttemptError::Message("All translation endpoints failed".to_string())
    }))
}

pub async fn execute_google_translate_free_request<FetchFn, FetchFuture, SleepFn, SleepFuture>(
    index: usize,
    text: String,
    target_language: String,
    mut fetch: FetchFn,
    mut sleep_fn: SleepFn,
) -> Result<(usize, String), LlmPortError>
where
    FetchFn: FnMut(String, String) -> FetchFuture,
    FetchFuture: Future<Output = Result<String, GoogleTranslateFreeAttemptError>>,
    SleepFn: FnMut(Duration) -> SleepFuture,
    SleepFuture: Future<Output = ()>,
{
    let mut attempts = 0usize;

    loop {
        attempts += 1;

        match fetch(text.clone(), target_language.clone()).await {
            Ok(translation) => return Ok((index, translation)),
            Err(error) => {
                if let Some(delay) = google_translate_free_retry_delay(&error, attempts) {
                    info!(
                        "[LLM] google_translate_free request hit 429 and will retry: index={} failed_attempt={} next_attempt={} delay_ms={}",
                        index,
                        attempts,
                        attempts + 1,
                        delay.as_millis()
                    );
                    sleep_fn(delay).await;
                    continue;
                }

                warn!(
                    "[LLM] google_translate_free request failed after retries: index={} attempts={} error={}",
                    index,
                    attempts,
                    google_translate_free_error_summary(&error)
                );
                let summary = google_translate_free_error_summary(&error);
                let mut port_error = google_translate_free_port_error(error);
                port_error.message = format!(
                    "{} after {} {}",
                    summary,
                    attempts,
                    format_attempt_label(attempts)
                );
                return Err(port_error);
            }
        }
    }
}

pub async fn run_google_translate_free_requests_in_order<RunFn, RunFuture>(
    texts: Vec<String>,
    max_concurrency: usize,
    mut run_request: RunFn,
) -> Result<Vec<String>, LlmPortError>
where
    RunFn: FnMut(usize, String) -> RunFuture,
    RunFuture: Future<Output = Result<(usize, String), LlmPortError>>,
{
    let mut indexed_translations = Vec::with_capacity(texts.len());
    let results = stream::iter(texts.into_iter().enumerate())
        .map(move |(index, text)| run_request(index, text))
        .buffer_unordered(max_concurrency.max(1))
        .collect::<Vec<_>>()
        .await;

    for result in results {
        indexed_translations.push(result?);
    }

    indexed_translations.sort_by_key(|(index, _)| *index);
    Ok(indexed_translations
        .into_iter()
        .map(|(_, translation)| translation)
        .collect())
}

pub struct GoogleTranslateAdapter;

impl GoogleTranslateAdapter {
    pub async fn generate(
        &self,
        client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        let input = request.input.clone();
        let base_url = LlmApiUrl::parse(&config.base_url)?;

        if config.strategy == LlmProviderStrategy::GoogleTranslateFree {
            let fetch_client = client.clone();

            let (_, text) = execute_google_translate_free_request(
                0,
                input,
                "en".to_string(),
                move |text, target| {
                    let client = fetch_client.clone();
                    let base_url = base_url.clone();
                    async move {
                        fetch_google_translate_free_translation(&client, &base_url, &target, &text)
                            .await
                    }
                },
                tokio::time::sleep,
            )
            .await?;

            return Ok(StandardLlmResponse {
                text,
                thought: None,
                usage: None,
            });
        }

        let payload = GoogleTranslateRequest {
            q: vec![input],
            target: "en".to_string(),
            format: "text".to_string(),
        };

        let response = post_json_request(
            &base_url,
            vec![("x-goog-api-key", config.api_key.clone())],
            json!(payload),
            config.timeout_seconds,
        )
        .await?;
        let parsed: GoogleTranslateResponse = serde_json::from_value(response).map_err(|e| {
            LlmPortError::new(
                LlmPortErrorKind::Protocol,
                format!("Google Translate response format error: {e}"),
            )
        })?;
        let text = parsed
            .data
            .translations
            .into_iter()
            .next()
            .map(|t| t.translated_text)
            .unwrap_or_default();
        Ok(StandardLlmResponse {
            text,
            thought: None,
            usage: None,
        })
    }
}

pub(crate) fn google_translate_free_port_error(
    error: GoogleTranslateFreeAttemptError,
) -> LlmPortError {
    match error {
        GoogleTranslateFreeAttemptError::HttpStatus {
            status,
            retry_after,
        } => {
            let kind = match status {
                StatusCode::UNAUTHORIZED => LlmPortErrorKind::Authentication,
                StatusCode::FORBIDDEN => LlmPortErrorKind::Permission,
                StatusCode::TOO_MANY_REQUESTS => LlmPortErrorKind::RateLimited,
                status if status.is_server_error() => LlmPortErrorKind::Unavailable,
                _ => LlmPortErrorKind::Protocol,
            };
            LlmPortError {
                kind,
                message: format!("Free API Error: {status}"),
                retry_after_ms: retry_after.map(|duration| duration.as_millis() as u64),
            }
        }
        GoogleTranslateFreeAttemptError::Message(message) => classify_llm_port_error(message),
        GoogleTranslateFreeAttemptError::Port(error) => error,
    }
}
