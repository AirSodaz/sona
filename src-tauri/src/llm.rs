use async_trait::async_trait;
use futures_util::{future::BoxFuture, stream, StreamExt};
use log::{info, warn};
use reqwest::{
    header::{CONTENT_TYPE, RETRY_AFTER},
    Client, StatusCode,
};
use rig::client::{CompletionClient, Nothing};
use rig::completion::{CompletionModel, GetTokenUsage};
use rig::providers::{anthropic, gemini, ollama, openai};
use rig::streaming::StreamedAssistantContent;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{future::Future, time::Duration};
use tauri::{AppHandle, Emitter};

const DEFAULT_SEGMENT_CHUNK_SIZE: usize = 30;
const DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET: usize = 32_000;
const DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET: usize = DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET / 2;
const GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRIES: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS: u64 = 5;
const GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS: [u64; GOOGLE_TRANSLATE_FREE_MAX_RETRIES] = [500, 1000];
const DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET: usize = 6000;
const MIN_SUMMARY_CHUNK_CHAR_BUDGET: usize = 1200;
const LLM_TASK_PROGRESS_EVENT: &str = "llm-task-progress";
const LLM_TASK_CHUNK_EVENT: &str = "llm-task-chunk";
const LLM_TASK_TEXT_EVENT: &str = "llm-task-text";
const LLM_USAGE_RECORDED_EVENT: &str = "llm-usage-recorded";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    OpenAi,
    OpenAiResponses,
    #[serde(rename = "azure_openai")]
    AzureOpenAi,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    Kimi,
    SiliconFlow,
    Qwen,
    QwenPortal,
    MinimaxGlobal,
    MinimaxCn,
    OpenRouter,
    LmStudio,
    Groq,
    XAi,
    MistralAi,
    Perplexity,
    Volcengine,
    Chatglm,
    #[serde(rename = "google_translate")]
    GoogleTranslate,
    #[serde(rename = "google_translate_free")]
    GoogleTranslateFree,
    OpenAiCompatible,
}

include!("llm/providers_impl.rs");

include!("llm/streaming_impl.rs");

include!("llm/tasks_impl.rs");

fn polished_segment_id(item: &PolishedSegment) -> &str {
    item.id.as_str()
}

fn translated_segment_id(item: &TranslatedSegment) -> &str {
    item.id.as_str()
}

#[tauri::command]
pub async fn generate_llm_text(
    app: AppHandle,
    request: LlmGenerateRequest,
) -> Result<String, String> {
    validate_llm_config(&request.config)?;

    if request.input.trim().is_empty() {
        return Err("Input cannot be empty".to_string());
    }

    let category = request.source.unwrap_or(LlmGenerateSource::Generic).into();
    let config = request.config.clone();
    let response = generate_with_rig(request).await?;
    emit_llm_usage_event(&app, &config, category, response.usage.clone());
    Ok(response.text)
}

#[tauri::command]
pub async fn polish_transcript_segments(
    app: AppHandle,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    validate_task_request(&request.task_id, &request.config)?;

    if request.config.provider == LlmProvider::GoogleTranslate {
        return Err("Google Translate does not support transcript polishing".to_string());
    }

    let config = request.config.clone();
    let usage_config = request.config.clone();
    let context = request.context.clone();
    let keywords = request.keywords.clone();
    let chunk_app = app.clone();
    let usage_app = app.clone();

    run_streaming_segment_task(
        SegmentTaskContext {
            task_id: &request.task_id,
            task_type: LlmTaskType::Polish,
            segments: &request.segments,
            chunk_size: request.chunk_size,
            prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
        },
        StreamingSegmentTaskConfig {
            build_prompt: move |chunk: &[LlmSegmentInput]| {
                build_polish_prompt(chunk, context.as_deref(), keywords.as_deref())
            },
            parse_chunk: parse_polish_chunk,
            build_request: move |prompt: String| LlmGenerateRequest {
                config: config.clone(),
                input: prompt,
                source: None,
            },
            get_output_id: polished_segment_id,
            on_success: move |response: &StandardLlmResponse| {
                emit_llm_usage_event(
                    &usage_app,
                    &usage_config,
                    LlmUsageCategory::Polish,
                    response.usage.clone(),
                );
            },
            emit_chunk: move |payload| {
                chunk_app
                    .emit(LLM_TASK_CHUNK_EVENT, payload)
                    .map_err(|error| error.to_string())
            },
            emit_progress: move |payload| {
                app.emit(LLM_TASK_PROGRESS_EVENT, payload)
                    .map_err(|error| error.to_string())
            },
        },
    )
    .await
}

#[tauri::command]
pub async fn translate_transcript_segments(
    app: AppHandle,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    validate_task_request(&request.task_id, &request.config)?;

    if request.target_language.trim().is_empty() {
        return Err("Target language cannot be empty".to_string());
    }

    let config = request.config.clone();
    let usage_config = request.config.clone();
    let target_language = request.target_language.clone();
    let chunk_app = app.clone();
    let usage_app = app.clone();

    if config.provider == LlmProvider::GoogleTranslate
        || config.provider == LlmProvider::GoogleTranslateFree
    {
        let client = Client::new();
        let translate_provider = config.provider;
        return run_segment_task(
            SegmentTaskContext {
                task_id: &request.task_id,
                task_type: LlmTaskType::Translate,
                segments: &request.segments,
                chunk_size: request.chunk_size,
                prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            },
            BufferedSegmentTaskConfig {
                build_prompt: move |chunk: &[LlmSegmentInput]| {
                    let texts: Vec<String> = chunk.iter().map(|s| s.text.clone()).collect();
                    serde_json::to_string(&texts).unwrap_or_default()
                },
                parse_chunk: move |response_text: &str,
                                   chunk: &[LlmSegmentInput],
                                   chunk_number: usize| {
                    if translate_provider == LlmProvider::GoogleTranslate {
                        let parsed: GoogleTranslateResponse =
                            serde_json::from_str(response_text).map_err(|error| {
                                chunk_error(
                                    LlmTaskType::Translate,
                                    chunk_number,
                                    format!("invalid JSON response: {error}"),
                                )
                            })?;

                        if parsed.data.translations.len() != chunk.len() {
                            return Err(chunk_error(
                                LlmTaskType::Translate,
                                chunk_number,
                                format!(
                                    "expected {} objects but received {}",
                                    chunk.len(),
                                    parsed.data.translations.len()
                                ),
                            ));
                        }

                        let mut translated_segments = Vec::with_capacity(chunk.len());
                        for (index, translation) in parsed.data.translations.into_iter().enumerate()
                        {
                            translated_segments.push(TranslatedSegment {
                                id: chunk[index].id.clone(),
                                translation: translation.translated_text,
                            });
                        }

                        Ok(translated_segments)
                    } else {
                        // GoogleTranslateFree returns raw JSON array of translations
                        let translations: Vec<String> =
                            serde_json::from_str(response_text).map_err(|e| {
                                chunk_error(
                                    LlmTaskType::Translate,
                                    chunk_number,
                                    format!("invalid free translation JSON: {e}"),
                                )
                            })?;

                        if translations.len() != chunk.len() {
                            return Err(chunk_error(
                                LlmTaskType::Translate,
                                chunk_number,
                                format!(
                                    "expected {} translations but received {}",
                                    chunk.len(),
                                    translations.len()
                                ),
                            ));
                        }

                        Ok(chunk
                            .iter()
                            .zip(translations)
                            .map(|(s, t)| TranslatedSegment {
                                id: s.id.clone(),
                                translation: t,
                            })
                            .collect())
                    }
                },
                generate_text: move |prompt: String| {
                    let config = config.clone();
                    let target_language = target_language.clone();
                    let client = client.clone();
                    let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                        Box::pin(async move {
                            let texts: Vec<String> =
                                serde_json::from_str(&prompt).unwrap_or_default();

                            if config.provider == LlmProvider::GoogleTranslate {
                                let payload = GoogleTranslateRequest {
                                    q: texts,
                                    target: target_language,
                                    format: "text".to_string(),
                                };

                                let url = format!(
                                    "{}?key={}",
                                    config.base_url.trim_end_matches('/'),
                                    config.api_key
                                );
                                let response = client
                                    .post(&url)
                                    .json(&payload)
                                    .send()
                                    .await
                                    .map_err(|e| e.to_string())?;

                                let status = response.status();
                                let text = response.text().await.unwrap_or_default();

                                if !status.is_success() {
                                    return Err(format!(
                                        "Google Translate API Error: {} {}",
                                        status, text
                                    ));
                                }

                                Ok(StandardLlmResponse { text, usage: None })
                            } else {
                                info!(
                                    "[LLM] google_translate_free chunk fan-out: items={} max_concurrency={}",
                                    texts.len(),
                                    GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY
                                );
                                let base_url = config.base_url.clone();
                                let translations = run_google_translate_free_requests_in_order(
                                    texts,
                                    GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY,
                                    move |index, text| {
                                        let client = client.clone();
                                        let base_url = base_url.clone();
                                        let target_language = target_language.clone();
                                        async move {
                                            execute_google_translate_free_request(
                                                index,
                                                text,
                                                target_language,
                                                move |text, target_language| {
                                                    let client = client.clone();
                                                    let base_url = base_url.clone();
                                                    async move {
                                                        fetch_google_translate_free_translation(
                                                            &client,
                                                            base_url.as_str(),
                                                            target_language.as_str(),
                                                            text.as_str(),
                                                        )
                                                        .await
                                                    }
                                                },
                                                |delay| async move {
                                                    tokio::time::sleep(delay).await;
                                                },
                                            )
                                            .await
                                        }
                                    },
                                )
                                .await?;
                                Ok(StandardLlmResponse {
                                    text: serde_json::to_string(&translations)
                                        .unwrap_or_default(),
                                    usage: None,
                                })
                            }
                        });
                    future
                },
                on_success: move |response: &StandardLlmResponse| {
                    emit_llm_usage_event(
                        &usage_app,
                        &usage_config,
                        LlmUsageCategory::Translation,
                        response.usage.clone(),
                    );
                },
                emit_chunk: move |payload| {
                    chunk_app
                        .emit(LLM_TASK_CHUNK_EVENT, payload)
                        .map_err(|error| error.to_string())
                },
                emit_progress: move |payload| {
                    app.emit(LLM_TASK_PROGRESS_EVENT, payload)
                        .map_err(|error| error.to_string())
                },
            },
        )
        .await;
    }

    run_streaming_segment_task(
        SegmentTaskContext {
            task_id: &request.task_id,
            task_type: LlmTaskType::Translate,
            segments: &request.segments,
            chunk_size: request.chunk_size,
            prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
        },
        StreamingSegmentTaskConfig {
            build_prompt: move |chunk: &[LlmSegmentInput]| {
                build_translate_prompt(chunk, &target_language)
            },
            parse_chunk: parse_translate_chunk,
            build_request: move |prompt: String| LlmGenerateRequest {
                config: config.clone(),
                input: prompt,
                source: None,
            },
            get_output_id: translated_segment_id,
            on_success: move |response: &StandardLlmResponse| {
                emit_llm_usage_event(
                    &usage_app,
                    &usage_config,
                    LlmUsageCategory::Translation,
                    response.usage.clone(),
                );
            },
            emit_chunk: move |payload| {
                chunk_app
                    .emit(LLM_TASK_CHUNK_EVENT, payload)
                    .map_err(|error| error.to_string())
            },
            emit_progress: move |payload| {
                app.emit(LLM_TASK_PROGRESS_EVENT, payload)
                    .map_err(|error| error.to_string())
            },
        },
    )
    .await
}

#[tauri::command]
pub async fn summarize_transcript(
    app: AppHandle,
    request: SummarizeTranscriptRequest,
) -> Result<TranscriptSummaryResult, String> {
    validate_task_request(&request.task_id, &request.config)?;
    validate_summary_provider(request.config.provider)?;

    let task_id = request.task_id.clone();
    let streamed_task_id = task_id.clone();
    let buffered_config = request.config.clone();
    let streamed_config = request.config.clone();
    let buffered_usage_config = request.config.clone();
    let streamed_usage_config = request.config.clone();
    let template = request.template;
    let progress_app = app.clone();
    let stream_app = app.clone();
    let buffered_usage_app = app.clone();
    let streamed_usage_app = app.clone();

    run_summary_task(
        &task_id,
        &template,
        &request.segments,
        request.chunk_char_budget,
        move |prompt| {
            let config = buffered_config.clone();
            let usage_config = buffered_usage_config.clone();
            let usage_app = buffered_usage_app.clone();
            Box::pin(async move {
                let response = generate_with_rig(LlmGenerateRequest {
                    config,
                    input: prompt,
                    source: None,
                })
                .await?;
                emit_llm_usage_event(
                    &usage_app,
                    &usage_config,
                    LlmUsageCategory::Summary,
                    response.usage.clone(),
                );
                Ok(response.text)
            })
        },
        move |prompt| {
            let config = streamed_config.clone();
            let task_id = streamed_task_id.clone();
            let app = stream_app.clone();
            let usage_config = streamed_usage_config.clone();
            let usage_app = streamed_usage_app.clone();
            Box::pin(async move {
                let response = generate_with_optional_streaming(
                    LlmGenerateRequest {
                        config,
                        input: prompt,
                        source: None,
                    },
                    &mut |text, delta| {
                        app.emit(
                            LLM_TASK_TEXT_EVENT,
                            LlmTaskTextPayload {
                                task_id: task_id.clone(),
                                task_type: LlmTaskType::Summary,
                                text: text.to_string(),
                                delta: delta.to_string(),
                            },
                        )
                        .map_err(|error| error.to_string())
                    },
                )
                .await?;
                emit_llm_usage_event(
                    &usage_app,
                    &usage_config,
                    LlmUsageCategory::Summary,
                    response.usage.clone(),
                );
                Ok(response.text)
            })
        },
        move |payload| {
            progress_app
                .emit(LLM_TASK_PROGRESS_EVENT, payload)
                .map_err(|error| error.to_string())
        },
    )
    .await
}

#[tauri::command]
pub async fn list_llm_models(request: LlmModelsRequest) -> Result<Vec<String>, String> {
    if !provider_supports_model_listing(&request.provider) {
        return Ok(vec![]);
    }

    let client = Client::new();

    match request.provider {
        LlmProvider::Gemini => {
            get_gemini_models(&client, &request.api_key, &request.base_url).await
        }
        LlmProvider::Ollama => {
            get_openai_models(&client, &request.api_key, &request.base_url, true).await
        }
        _ => get_openai_models(&client, &request.api_key, &request.base_url, false).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    fn sample_segments() -> Vec<LlmSegmentInput> {
        vec![
            LlmSegmentInput {
                id: "1".to_string(),
                text: "hello".to_string(),
            },
            LlmSegmentInput {
                id: "2".to_string(),
                text: "world".to_string(),
            },
            LlmSegmentInput {
                id: "3".to_string(),
                text: "again".to_string(),
            },
        ]
    }

    fn sample_summary_segments() -> Vec<SummarySegmentInput> {
        vec![
            SummarySegmentInput {
                id: "1".to_string(),
                text: "Opening discussion about the roadmap.".to_string(),
                start: 0.0,
                end: 12.0,
                is_final: true,
            },
            SummarySegmentInput {
                id: "2".to_string(),
                text: "The team agreed to ship the beta next month.".to_string(),
                start: 12.0,
                end: 26.0,
                is_final: true,
            },
            SummarySegmentInput {
                id: "3".to_string(),
                text: "Alice will prepare the onboarding checklist.".to_string(),
                start: 26.0,
                end: 39.0,
                is_final: true,
            },
        ]
    }

    fn sample_summary_template(id: &str, name: &str, instructions: &str) -> SummaryTemplateConfig {
        SummaryTemplateConfig {
            id: id.to_string(),
            name: name.to_string(),
            instructions: instructions.to_string(),
        }
    }

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
        assert_eq!(
            clean_gemini_base_url("https://generativelanguage.googleapis.com/v1beta/openai"),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            clean_gemini_base_url("https://generativelanguage.googleapis.com/v1/openai/"),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            clean_gemini_base_url("https://generativelanguage.googleapis.com"),
            "https://generativelanguage.googleapis.com"
        );
    }

    #[test]
    fn gemini_models_url_accepts_common_inputs() {
        assert_eq!(
            format_gemini_models_url(
                "https://generativelanguage.googleapis.com/v1beta/openai",
                "test-key"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models?key=test-key"
        );
    }

    #[test]
    fn gemini_model_filter_keeps_generate_content_models() {
        let text_model = GeminiModel {
            name: "models/gemini-2.5-flash".to_string(),
            supported_generation_methods: Some(vec!["generateContent".to_string()]),
        };
        let embedding_model = GeminiModel {
            name: "models/text-embedding-004".to_string(),
            supported_generation_methods: Some(vec!["embedContent".to_string()]),
        };
        let legacy_model = GeminiModel {
            name: "models/gemini-pro".to_string(),
            supported_generation_methods: None,
        };

        assert!(is_gemini_text_generation_model(&text_model));
        assert!(!is_gemini_text_generation_model(&embedding_model));
        assert!(is_gemini_text_generation_model(&legacy_model));
    }

    #[test]
    fn anthropic_listing_is_disabled() {
        assert!(!provider_supports_model_listing(&LlmProvider::Anthropic));
        assert!(!provider_supports_model_listing(&LlmProvider::AzureOpenAi));
        assert!(!provider_supports_model_listing(&LlmProvider::Volcengine));
        assert!(provider_supports_model_listing(&LlmProvider::OpenAi));
    }

    #[test]
    fn join_url_trims_duplicate_slashes() {
        assert_eq!(
            join_url("https://api.openai.com/", "/v1/responses"),
            "https://api.openai.com/v1/responses"
        );
        assert_eq!(
            join_url(
                "https://ark.cn-beijing.volces.com",
                "api/v3/chat/completions"
            ),
            "https://ark.cn-beijing.volces.com/api/v3/chat/completions"
        );
    }

    #[test]
    fn extract_text_from_chat_completions_response() {
        let response = json!({
            "choices": [
                {
                    "message": {
                        "content": "Hello from chat completions"
                    }
                }
            ]
        });

        assert_eq!(
            extract_text_from_json_response(&response).unwrap(),
            "Hello from chat completions"
        );
    }

    #[test]
    fn extract_text_from_responses_api_payload() {
        let response = json!({
            "output": [
                {
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello from responses"
                        }
                    ]
                }
            ]
        });

        assert_eq!(
            extract_text_from_json_response(&response).unwrap(),
            "Hello from responses"
        );
    }

    #[test]
    fn clean_json_response_removes_markdown_fences() {
        assert_eq!(
            clean_json_response("```json\n[{\"id\":\"1\",\"text\":\"Hello\"}]\n```"),
            "[{\"id\":\"1\",\"text\":\"Hello\"}]"
        );
    }

    #[test]
    fn parse_polish_chunk_rejects_length_mismatch() {
        let err = parse_polish_chunk(r#"[{"id":"1","text":"Hello"}]"#, &sample_segments()[..2], 1)
            .expect_err("length mismatch should fail");

        assert!(err.contains("polish chunk 1 failed"));
        assert!(err.contains("expected 2 objects but received 1"));
    }

    #[test]
    fn parse_translate_chunk_rejects_id_order_mismatch() {
        let err = parse_translate_chunk(
            r#"[{"id":"2","translation":"B"},{"id":"1","translation":"A"}]"#,
            &sample_segments()[..2],
            2,
        )
        .expect_err("id order mismatch should fail");

        assert!(err.contains("translate chunk 2 failed"));
        assert!(err.contains("expected id '1'"));
    }

    #[test]
    fn build_polish_prompt_contains_context_and_keywords() {
        let prompt = build_polish_prompt(
            &sample_segments()[..2],
            Some("combined context"),
            Some("keyword-a, keyword-b"),
        );

        assert!(prompt.contains("[User Context]"));
        assert!(prompt.contains("combined context"));
        assert!(prompt.contains("[User Keywords]"));
        assert!(prompt.contains("keyword-a, keyword-b"));
    }

    #[test]
    fn build_translate_prompt_contains_language_name() {
        let prompt = build_translate_prompt(&sample_segments()[..2], "zh");

        assert!(prompt.contains("Chinese (Simplified)"));
        assert!(prompt.contains("replace 'text' with 'translation'"));
    }

    #[test]
    fn build_summary_chunk_prompt_requires_same_language_and_structure() {
        let template = sample_summary_template(
            "meeting",
            "Meeting",
            "1. Meeting overview.\n2. Decisions made.\n3. Action items with owners when the transcript names them.",
        );
        let prompt = build_summary_chunk_prompt(&template, &sample_summary_segments()[..2], 1, 2);

        assert!(prompt.contains("Use the same language as the transcript."));
        assert!(prompt.contains("Meeting overview."));
        assert!(prompt.contains("Action items with owners"));
    }

    #[test]
    fn build_summary_finalize_prompt_requires_same_language_and_structure() {
        let template = sample_summary_template(
            "lecture",
            "Lecture",
            "1. Lecture overview.\n2. Core concepts or arguments.\n3. Important examples, evidence, or explanations.",
        );
        let prompt = build_summary_finalize_prompt(
            &template,
            &["Chunk 1 summary".to_string(), "Chunk 2 summary".to_string()],
        );

        assert!(prompt.contains("Use the same language as the transcript."));
        assert!(prompt.contains("Core concepts or arguments."));
        assert!(prompt.contains("[Chunk 1]"));
    }

    #[test]
    fn split_summary_segments_uses_char_budget() {
        let chunks = split_summary_segments(&sample_summary_segments(), 70);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0][0].id, "1");
        assert_eq!(chunks[1][0].id, "2");
        assert_eq!(chunks[2][0].id, "3");
    }

    #[test]
    fn parse_google_translate_free_retry_after_clamps_seconds() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(RETRY_AFTER, reqwest::header::HeaderValue::from_static("9"));

        assert_eq!(
            parse_google_translate_free_retry_after(&headers),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn plan_segment_task_chunks_uses_dynamic_prompt_budget() {
        let segments = vec![
            LlmSegmentInput {
                id: "1".to_string(),
                text: "AAAAAA".to_string(),
            },
            LlmSegmentInput {
                id: "2".to_string(),
                text: "BBBBBB".to_string(),
            },
            LlmSegmentInput {
                id: "3".to_string(),
                text: "CCCCCC".to_string(),
            },
        ];
        let mut build_prompt = |chunk: &[LlmSegmentInput]| {
            chunk
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join("|")
        };

        let planned = plan_segment_task_chunks(
            "task-dynamic-1",
            LlmTaskType::Polish,
            &segments,
            None,
            19,
            &mut build_prompt,
        );

        assert_eq!(
            planned
                .iter()
                .map(|chunk| (chunk.start, chunk.end))
                .collect::<Vec<_>>(),
            vec![(0, 2), (2, 3)]
        );
        assert_eq!(planned[0].prompt, "AAAAAA|BBBBBB");
        assert_eq!(planned[1].prompt, "CCCCCC");
    }

    #[test]
    fn plan_segment_task_chunks_context_and_keywords_reduce_capacity() {
        let segments = sample_segments();
        let context = "Project roadmap context. ".repeat(3);
        let keywords = "Sona, roadmap, launch. ".repeat(3);
        let without_context_two =
            prompt_char_count(&build_polish_prompt(&segments[..2], None, None));
        let with_context_one = prompt_char_count(&build_polish_prompt(
            &segments[..1],
            Some(&context),
            Some(&keywords),
        ));
        let with_context_two = prompt_char_count(&build_polish_prompt(
            &segments[..2],
            Some(&context),
            Some(&keywords),
        ));
        let budget = with_context_one.max(without_context_two);

        assert!(budget < with_context_two);

        let without_context = plan_segment_task_chunks(
            "task-dynamic-2",
            LlmTaskType::Polish,
            &segments,
            None,
            budget,
            &mut |chunk| build_polish_prompt(chunk, None, None),
        );
        let with_context = plan_segment_task_chunks(
            "task-dynamic-3",
            LlmTaskType::Polish,
            &segments,
            None,
            budget,
            &mut |chunk| build_polish_prompt(chunk, Some(&context), Some(&keywords)),
        );

        assert!(without_context[0].end - without_context[0].start >= 2);
        assert_eq!(with_context[0].end - with_context[0].start, 1);
    }

    #[test]
    fn summary_provider_rejects_google_translate() {
        let err = validate_summary_provider(LlmProvider::GoogleTranslate)
            .expect_err("google translate should be rejected");

        assert!(err.contains("does not support transcript summaries"));
    }

    #[test]
    fn chunk_payload_serializes_with_camel_case() {
        let payload = LlmTaskChunkPayload {
            task_id: "task-1".to_string(),
            task_type: LlmTaskType::Polish,
            chunk_index: 1,
            total_chunks: 2,
            items: vec![PolishedSegment {
                id: "1".to_string(),
                text: "Hello".to_string(),
            }],
        };

        let json = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(json["taskId"], "task-1");
        assert_eq!(json["taskType"], "polish");
        assert_eq!(json["chunkIndex"], 1);
        assert_eq!(json["totalChunks"], 2);
        assert_eq!(json["items"][0]["id"], "1");
        assert_eq!(json["items"][0]["text"], "Hello");
    }

    #[test]
    fn text_payload_serializes_with_camel_case() {
        let payload = LlmTaskTextPayload {
            task_id: "summary-task-1".to_string(),
            task_type: LlmTaskType::Summary,
            text: "Hello world".to_string(),
            delta: "world".to_string(),
        };

        let json = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(json["taskId"], "summary-task-1");
        assert_eq!(json["taskType"], "summary");
        assert_eq!(json["text"], "Hello world");
        assert_eq!(json["delta"], "world");
    }

    #[test]
    fn parse_polish_chunk_accepts_ndjson() {
        let segments = sample_segments();
        let response = concat!(
            "{\"id\":\"1\",\"text\":\"Hello\"}\n",
            "{\"id\":\"2\",\"text\":\"World\"}\n"
        );

        let parsed = parse_polish_chunk(response, &segments[..2], 1).expect("ndjson should parse");

        assert_eq!(
            parsed,
            vec![
                PolishedSegment {
                    id: "1".to_string(),
                    text: "Hello".to_string(),
                },
                PolishedSegment {
                    id: "2".to_string(),
                    text: "World".to_string(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn try_stream_text_skips_google_translate_providers() {
        for provider in [
            LlmProvider::GoogleTranslate,
            LlmProvider::GoogleTranslateFree,
        ] {
            let request = LlmGenerateRequest {
                config: LlmConfig {
                    provider,
                    base_url: "https://example.com".to_string(),
                    api_key: "test-key".to_string(),
                    model: "test-model".to_string(),
                    api_path: None,
                    api_version: None,
                    temperature: Some(0.2),
                },
                input: "hello".to_string(),
                source: None,
            };

            let mut emit_delta = |_text: &str, _delta: &str| Ok(());
            let mut accumulator = StreamTextAccumulator::new(&mut emit_delta);

            let streamed = try_stream_text(&request, &mut accumulator)
                .await
                .expect("google translate should skip streaming");

            assert_eq!(streamed, None);
            assert_eq!(accumulator.text(), "");
        }
    }

    #[tokio::test]
    async fn run_segment_task_aggregates_chunks_and_emits_progress() {
        let segments = sample_segments();
        let mut chunk_events = Vec::new();
        let mut progress_events = Vec::new();
        #[derive(Debug, PartialEq, Eq)]
        enum SegmentEvent {
            Chunk(usize),
            Progress(usize),
        }
        let ordered_events = std::cell::RefCell::new(Vec::new());

        let result = run_segment_task(
            SegmentTaskContext {
                task_id: "task-1",
                task_type: LlmTaskType::Polish,
                segments: &segments,
                chunk_size: Some(2),
                prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            },
            BufferedSegmentTaskConfig {
                build_prompt: |chunk: &[LlmSegmentInput]| serde_json::to_string(chunk).unwrap(),
                parse_chunk: parse_polish_chunk,
                generate_text: {
                    let mut call_count = 0usize;
                    move |_prompt: String| {
                        call_count += 1;
                        let response = match call_count {
                            1 => r#"[{"id":"1","text":"Hello"},{"id":"2","text":"World"}]"#,
                            2 => r#"[{"id":"3","text":"Again"}]"#,
                            _ => unreachable!(),
                        }
                        .to_string();

                        let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                            Box::pin(async move {
                                Ok(StandardLlmResponse {
                                    text: response,
                                    usage: None,
                                })
                            });
                        future
                    }
                },
                on_success: |_response: &StandardLlmResponse| {},
                emit_chunk: |payload: LlmTaskChunkPayload<PolishedSegment>| {
                    ordered_events
                        .borrow_mut()
                        .push(SegmentEvent::Chunk(payload.chunk_index));
                    chunk_events.push(payload);
                    Ok(())
                },
                emit_progress: |payload: LlmTaskProgressPayload| {
                    ordered_events
                        .borrow_mut()
                        .push(SegmentEvent::Progress(payload.completed_chunks));
                    progress_events.push(payload);
                    Ok(())
                },
            },
        )
        .await
        .expect("task should succeed");

        assert_eq!(
            result,
            vec![
                PolishedSegment {
                    id: "1".to_string(),
                    text: "Hello".to_string()
                },
                PolishedSegment {
                    id: "2".to_string(),
                    text: "World".to_string()
                },
                PolishedSegment {
                    id: "3".to_string(),
                    text: "Again".to_string()
                },
            ]
        );
        assert_eq!(
            chunk_events,
            vec![
                LlmTaskChunkPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    chunk_index: 1,
                    total_chunks: 2,
                    items: vec![
                        PolishedSegment {
                            id: "1".to_string(),
                            text: "Hello".to_string()
                        },
                        PolishedSegment {
                            id: "2".to_string(),
                            text: "World".to_string()
                        },
                    ],
                },
                LlmTaskChunkPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    chunk_index: 2,
                    total_chunks: 2,
                    items: vec![PolishedSegment {
                        id: "3".to_string(),
                        text: "Again".to_string()
                    }],
                },
            ]
        );
        assert_eq!(
            progress_events,
            vec![
                LlmTaskProgressPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    completed_chunks: 1,
                    total_chunks: 2,
                },
                LlmTaskProgressPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    completed_chunks: 2,
                    total_chunks: 2,
                },
            ]
        );
        assert_eq!(
            ordered_events.into_inner(),
            vec![
                SegmentEvent::Chunk(1),
                SegmentEvent::Progress(1),
                SegmentEvent::Chunk(2),
                SegmentEvent::Progress(2),
            ]
        );
    }

    #[tokio::test]
    async fn run_segment_task_reports_second_chunk_failure() {
        let segments = sample_segments();

        let err = run_segment_task(
            SegmentTaskContext {
                task_id: "task-2",
                task_type: LlmTaskType::Translate,
                segments: &segments,
                chunk_size: Some(2),
                prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            },
            BufferedSegmentTaskConfig {
                build_prompt: |chunk: &[LlmSegmentInput]| serde_json::to_string(chunk).unwrap(),
                parse_chunk: parse_translate_chunk,
                generate_text: {
                    let mut call_count = 0usize;
                    move |_prompt: String| {
                        call_count += 1;
                        let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                            Box::pin(async move {
                                match call_count {
                                    1 => Ok(StandardLlmResponse {
                                        text: r#"[{"id":"1","translation":"A"},{"id":"2","translation":"B"}]"#
                                            .to_string(),
                                        usage: None,
                                    }),
                                    2 => Err("boom".to_string()),
                                    _ => unreachable!(),
                                }
                            });
                        future
                    }
                },
                on_success: |_response: &StandardLlmResponse| {},
                emit_chunk: |_payload: LlmTaskChunkPayload<TranslatedSegment>| Ok(()),
                emit_progress: |_payload: LlmTaskProgressPayload| Ok(()),
            },
        )
        .await
        .expect_err("second chunk should fail");

        assert_eq!(err, "translate chunk 2 failed: boom");
    }

    #[tokio::test]
    async fn run_segment_task_uses_dynamic_prompt_budget_when_chunk_size_is_missing() {
        let long_text = "A".repeat(7_000);
        let segments = vec![
            LlmSegmentInput {
                id: "1".to_string(),
                text: long_text.clone(),
            },
            LlmSegmentInput {
                id: "2".to_string(),
                text: long_text.clone(),
            },
            LlmSegmentInput {
                id: "3".to_string(),
                text: long_text,
            },
        ];
        let mut chunk_events = Vec::new();
        let mut progress_events = Vec::new();

        let result = run_segment_task(
            SegmentTaskContext {
                task_id: "task-3",
                task_type: LlmTaskType::Translate,
                segments: &segments,
                chunk_size: None,
                prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            },
            BufferedSegmentTaskConfig {
                build_prompt: |chunk: &[LlmSegmentInput]| {
                    chunk
                        .iter()
                        .map(|segment| format!("{}:{}", segment.id, segment.text))
                        .collect::<Vec<_>>()
                        .join("\n")
                },
                parse_chunk: parse_translate_chunk,
                generate_text: {
                    let mut call_count = 0usize;
                    move |_prompt: String| {
                        call_count += 1;
                        let response = match call_count {
                            1 => {
                                r#"[{"id":"1","translation":"One"},{"id":"2","translation":"Two"}]"#
                            }
                            2 => r#"[{"id":"3","translation":"Three"}]"#,
                            _ => unreachable!(),
                        }
                        .to_string();

                        let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                            Box::pin(async move {
                                Ok(StandardLlmResponse {
                                    text: response,
                                    usage: None,
                                })
                            });
                        future
                    }
                },
                on_success: |_response: &StandardLlmResponse| {},
                emit_chunk: |payload: LlmTaskChunkPayload<TranslatedSegment>| {
                    chunk_events.push(payload);
                    Ok(())
                },
                emit_progress: |payload: LlmTaskProgressPayload| {
                    progress_events.push(payload);
                    Ok(())
                },
            },
        )
        .await
        .expect("dynamic task should succeed");

        assert_eq!(
            result,
            vec![
                TranslatedSegment {
                    id: "1".to_string(),
                    translation: "One".to_string(),
                },
                TranslatedSegment {
                    id: "2".to_string(),
                    translation: "Two".to_string(),
                },
                TranslatedSegment {
                    id: "3".to_string(),
                    translation: "Three".to_string(),
                },
            ]
        );
        assert_eq!(
            chunk_events
                .iter()
                .map(|payload| {
                    payload
                        .items
                        .iter()
                        .map(|item| item.id.as_str())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            vec![vec!["1", "2"], vec!["3"]]
        );
        assert_eq!(
            progress_events,
            vec![
                LlmTaskProgressPayload {
                    task_id: "task-3".to_string(),
                    task_type: LlmTaskType::Translate,
                    completed_chunks: 1,
                    total_chunks: 2,
                },
                LlmTaskProgressPayload {
                    task_id: "task-3".to_string(),
                    task_type: LlmTaskType::Translate,
                    completed_chunks: 2,
                    total_chunks: 2,
                },
            ]
        );
    }

    #[tokio::test]
    async fn run_segment_task_sends_single_over_budget_segment_alone() {
        let huge_text = "A".repeat(DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET + 100);
        let segments = vec![
            LlmSegmentInput {
                id: "1".to_string(),
                text: huge_text,
            },
            LlmSegmentInput {
                id: "2".to_string(),
                text: "small".to_string(),
            },
            LlmSegmentInput {
                id: "3".to_string(),
                text: "tiny".to_string(),
            },
        ];
        let mut chunk_events = Vec::new();
        let mut progress_events = Vec::new();

        let result = run_segment_task(
            SegmentTaskContext {
                task_id: "task-4",
                task_type: LlmTaskType::Translate,
                segments: &segments,
                chunk_size: None,
                prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            },
            BufferedSegmentTaskConfig {
                build_prompt: |chunk: &[LlmSegmentInput]| {
                    chunk
                        .iter()
                        .map(|segment| segment.text.as_str())
                        .collect::<Vec<_>>()
                        .join("")
                },
                parse_chunk: parse_translate_chunk,
                generate_text: {
                    let mut call_count = 0usize;
                    move |_prompt: String| {
                        call_count += 1;
                        let response = match call_count {
                            1 => r#"[{"id":"1","translation":"Huge"}]"#,
                            2 => {
                                r#"[{"id":"2","translation":"Small"},{"id":"3","translation":"Tiny"}]"#
                            }
                            _ => unreachable!(),
                        }
                        .to_string();

                        let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                            Box::pin(async move {
                                Ok(StandardLlmResponse {
                                    text: response,
                                    usage: None,
                                })
                            });
                        future
                    }
                },
                on_success: |_response: &StandardLlmResponse| {},
                emit_chunk: |payload: LlmTaskChunkPayload<TranslatedSegment>| {
                    chunk_events.push(payload);
                    Ok(())
                },
                emit_progress: |payload: LlmTaskProgressPayload| {
                    progress_events.push(payload);
                    Ok(())
                },
            },
        )
        .await
        .expect("single-over-budget task should succeed");

        assert_eq!(
            result,
            vec![
                TranslatedSegment {
                    id: "1".to_string(),
                    translation: "Huge".to_string(),
                },
                TranslatedSegment {
                    id: "2".to_string(),
                    translation: "Small".to_string(),
                },
                TranslatedSegment {
                    id: "3".to_string(),
                    translation: "Tiny".to_string(),
                },
            ]
        );
        assert_eq!(
            chunk_events
                .iter()
                .map(|payload| {
                    payload
                        .items
                        .iter()
                        .map(|item| item.id.as_str())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            vec![vec!["1"], vec!["2", "3"]]
        );
        assert_eq!(
            progress_events,
            vec![
                LlmTaskProgressPayload {
                    task_id: "task-4".to_string(),
                    task_type: LlmTaskType::Translate,
                    completed_chunks: 1,
                    total_chunks: 2,
                },
                LlmTaskProgressPayload {
                    task_id: "task-4".to_string(),
                    task_type: LlmTaskType::Translate,
                    completed_chunks: 2,
                    total_chunks: 2,
                },
            ]
        );
    }

    #[tokio::test]
    async fn run_google_translate_free_requests_in_order_limits_concurrency() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));

        let translations = run_google_translate_free_requests_in_order(
            vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
                "four".to_string(),
            ],
            2,
            {
                let active = active.clone();
                let max_seen = max_seen.clone();
                move |index, text| {
                    let active = active.clone();
                    let max_seen = max_seen.clone();
                    async move {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_seen.fetch_max(current, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok((index, text.to_uppercase()))
                    }
                }
            },
        )
        .await
        .expect("concurrency-limited run should succeed");

        assert_eq!(
            translations,
            vec![
                "ONE".to_string(),
                "TWO".to_string(),
                "THREE".to_string(),
                "FOUR".to_string(),
            ]
        );
        assert!(max_seen.load(Ordering::SeqCst) <= 2);
    }

    #[tokio::test]
    async fn execute_google_translate_free_request_retries_429_then_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let delays = Arc::new(Mutex::new(Vec::new()));

        let result = execute_google_translate_free_request(
            7,
            "hello".to_string(),
            "ja".to_string(),
            {
                let attempts = attempts.clone();
                move |_text, _target| {
                    let attempts = attempts.clone();
                    async move {
                        let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                        if attempt <= 2 {
                            Err(GoogleTranslateFreeAttemptError::HttpStatus {
                                status: StatusCode::TOO_MANY_REQUESTS,
                                retry_after: None,
                            })
                        } else {
                            Ok("\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}".to_string())
                        }
                    }
                }
            },
            {
                let delays = delays.clone();
                move |delay| {
                    let delays = delays.clone();
                    async move {
                        delays.lock().unwrap().push(delay);
                    }
                }
            },
        )
        .await
        .expect("request should succeed after retries");

        assert_eq!(
            result,
            (7, "\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}".to_string())
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(
            *delays.lock().unwrap(),
            vec![Duration::from_millis(500), Duration::from_millis(1000)]
        );
    }

    #[tokio::test]
    async fn execute_google_translate_free_request_prefers_retry_after_and_clamps() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let delays = Arc::new(Mutex::new(Vec::new()));

        let result = execute_google_translate_free_request(
            1,
            "hello".to_string(),
            "fr".to_string(),
            {
                let attempts = attempts.clone();
                move |_text, _target| {
                    let attempts = attempts.clone();
                    async move {
                        let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                        if attempt == 1 {
                            Err(GoogleTranslateFreeAttemptError::HttpStatus {
                                status: StatusCode::TOO_MANY_REQUESTS,
                                retry_after: Some(Duration::from_secs(9)),
                            })
                        } else {
                            Ok("bonjour".to_string())
                        }
                    }
                }
            },
            {
                let delays = delays.clone();
                move |delay| {
                    let delays = delays.clone();
                    async move {
                        delays.lock().unwrap().push(delay);
                    }
                }
            },
        )
        .await
        .expect("request should succeed after honoring retry-after");

        assert_eq!(result, (1, "bonjour".to_string()));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(*delays.lock().unwrap(), vec![Duration::from_secs(5)]);
    }

    #[tokio::test]
    async fn execute_google_translate_free_request_does_not_retry_non_429() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let slept = Arc::new(AtomicUsize::new(0));

        let err = execute_google_translate_free_request(
            2,
            "hello".to_string(),
            "de".to_string(),
            {
                let attempts = attempts.clone();
                move |_text, _target| {
                    let attempts = attempts.clone();
                    async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        Err(GoogleTranslateFreeAttemptError::HttpStatus {
                            status: StatusCode::INTERNAL_SERVER_ERROR,
                            retry_after: None,
                        })
                    }
                }
            },
            {
                let slept = slept.clone();
                move |_delay| {
                    let slept = slept.clone();
                    async move {
                        slept.fetch_add(1, Ordering::SeqCst);
                    }
                }
            },
        )
        .await
        .expect_err("non-429 should fail immediately");

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(slept.load(Ordering::SeqCst), 0);
        assert!(err.contains("500 Internal Server Error"));
        assert!(err.contains("after 1 attempt"));
    }

    #[tokio::test]
    async fn run_google_translate_free_requests_in_order_fails_chunk_when_retries_exhaust() {
        let err = run_google_translate_free_requests_in_order(
            vec!["first".to_string(), "second".to_string()],
            2,
            move |index, text| async move {
                if index == 0 {
                    execute_google_translate_free_request(
                        index,
                        text,
                        "es".to_string(),
                        |_text, _target| async {
                            Err(GoogleTranslateFreeAttemptError::HttpStatus {
                                status: StatusCode::TOO_MANY_REQUESTS,
                                retry_after: None,
                            })
                        },
                        |_delay| async {},
                    )
                    .await
                } else {
                    Ok((index, text))
                }
            },
        )
        .await
        .expect_err("chunk should fail when a request exhausts retries");

        assert!(err.contains("429 Too Many Requests"));
        assert!(err.contains("after 3 attempts"));
    }

    #[tokio::test]
    async fn run_summary_task_emits_chunk_and_final_progress() {
        let segments = vec![
            SummarySegmentInput {
                id: "1".to_string(),
                text: "A".repeat(700),
                start: 0.0,
                end: 12.0,
                is_final: true,
            },
            SummarySegmentInput {
                id: "2".to_string(),
                text: "B".repeat(700),
                start: 12.0,
                end: 24.0,
                is_final: true,
            },
            SummarySegmentInput {
                id: "3".to_string(),
                text: "C".repeat(700),
                start: 24.0,
                end: 36.0,
                is_final: true,
            },
        ];
        let mut progress_events = Vec::new();
        let template = sample_summary_template(
            "meeting",
            "Meeting",
            "1. Meeting overview.\n2. Decisions made.\n3. Action items with owners when the transcript names them.\n4. Open questions, blockers, or risks.",
        );
        let streamed_calls = Arc::new(AtomicUsize::new(0));

        let result = run_summary_task(
            "summary-task-1",
            &template,
            &segments,
            Some(1200),
            {
                let mut call_count = 0usize;
                move |_prompt| {
                    call_count += 1;
                    let response = match call_count {
                        1 => "Intermediate summary A".to_string(),
                        2 => "Intermediate summary B".to_string(),
                        3 => "Intermediate summary C".to_string(),
                        4 => "Final meeting summary".to_string(),
                        _ => unreachable!(),
                    };

                    Box::pin(async move { Ok(response) })
                }
            },
            {
                let streamed_calls = streamed_calls.clone();
                move |_prompt| {
                    streamed_calls.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async { Ok("Final meeting summary".to_string()) })
                }
            },
            |payload| {
                progress_events.push(payload);
                Ok(())
            },
        )
        .await
        .expect("summary task should succeed");

        assert_eq!(
            result,
            TranscriptSummaryResult {
                template_id: "meeting".to_string(),
                content: "Final meeting summary".to_string(),
            }
        );
        assert_eq!(
            progress_events,
            vec![
                LlmTaskProgressPayload {
                    task_id: "summary-task-1".to_string(),
                    task_type: LlmTaskType::Summary,
                    completed_chunks: 1,
                    total_chunks: 4,
                },
                LlmTaskProgressPayload {
                    task_id: "summary-task-1".to_string(),
                    task_type: LlmTaskType::Summary,
                    completed_chunks: 2,
                    total_chunks: 4,
                },
                LlmTaskProgressPayload {
                    task_id: "summary-task-1".to_string(),
                    task_type: LlmTaskType::Summary,
                    completed_chunks: 3,
                    total_chunks: 4,
                },
                LlmTaskProgressPayload {
                    task_id: "summary-task-1".to_string(),
                    task_type: LlmTaskType::Summary,
                    completed_chunks: 4,
                    total_chunks: 4,
                },
            ]
        );
        assert_eq!(streamed_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_summary_task_streams_direct_single_chunk_without_intermediate_stage() {
        let segments = sample_summary_segments();
        let mut progress_events = Vec::new();
        let buffered_calls = Arc::new(AtomicUsize::new(0));
        let streamed_calls = Arc::new(AtomicUsize::new(0));
        let template = sample_summary_template(
            "direct",
            "Direct",
            "1. Main points.\n2. Decisions.\n3. Action items.",
        );

        let result = run_summary_task(
            "summary-task-direct",
            &template,
            &segments,
            Some(DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET),
            {
                let buffered_calls = buffered_calls.clone();
                move |_prompt| {
                    buffered_calls.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async { Ok("unexpected intermediate summary".to_string()) })
                }
            },
            {
                let streamed_calls = streamed_calls.clone();
                move |_prompt| {
                    streamed_calls.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async { Ok("Direct summary".to_string()) })
                }
            },
            |payload| {
                progress_events.push(payload);
                Ok(())
            },
        )
        .await
        .expect("single chunk summary should succeed");

        assert_eq!(
            result,
            TranscriptSummaryResult {
                template_id: "direct".to_string(),
                content: "Direct summary".to_string(),
            }
        );
        assert_eq!(buffered_calls.load(Ordering::SeqCst), 0);
        assert_eq!(streamed_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            progress_events,
            vec![LlmTaskProgressPayload {
                task_id: "summary-task-direct".to_string(),
                task_type: LlmTaskType::Summary,
                completed_chunks: 1,
                total_chunks: 1,
            }]
        );
    }
}
