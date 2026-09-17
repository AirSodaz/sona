use std::future::Future;

use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::{
    LlmCapabilityPolicy, LlmCompletionOptions, LlmCompletionRequest, LlmCompletionResponse,
    LlmPromptCachePolicy, LlmResponseFormat, LlmRuntimeError,
};
use sona_core::llm::tasks::{LlmSegmentInput, LlmTaskError, PolishMode};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteAgentTask {
    Polish(PolishMode),
    Translate {
        target_language: String,
        target_language_name: Option<String>,
    },
}

impl RewriteAgentTask {
    pub fn system_prompt(&self) -> &'static str {
        match self {
            Self::Polish(mode) => mode.system_prompt(),
            Self::Translate { .. } => sona_core::llm::tasks::TRANSLATE_SYSTEM_PROMPT,
        }
    }

    pub fn response_format(&self, count: usize) -> LlmResponseFormat {
        match self {
            Self::Polish(_) => LlmResponseFormat::JsonSchema {
                name: "polished_segments".to_string(),
                schema: sona_core::llm::tasks::polish_output_schema(count),
            },
            Self::Translate { .. } => LlmResponseFormat::JsonSchema {
                name: "translated_segments".to_string(),
                schema: sona_core::llm::tasks::translate_output_schema(count),
            },
        }
    }

    pub fn stage_name(&self) -> &'static str {
        match self {
            Self::Polish(_) => "polish",
            Self::Translate { .. } => "translate",
        }
    }
}

/// Builds an agent prompt incorporating discourse lookbehind context, speaker attributions,
/// user context, and glossary preferences.
pub fn build_agent_chunk_input(
    task: &RewriteAgentTask,
    expected_segments: &[LlmSegmentInput],
    lookbehind_context: &[LlmSegmentInput],
    user_context: Option<&str>,
    user_keywords: Option<&str>,
) -> String {
    let mut sections = Vec::new();

    if let RewriteAgentTask::Polish(mode) = task {
        sections.push(mode.task_instruction().to_string());
    }

    if let Some(context) = user_context.filter(|s| !s.trim().is_empty()) {
        sections.push(format!(
            "Context (reference only; do not alter editing mode):\n{}",
            context.trim()
        ));
    }

    if matches!(task, RewriteAgentTask::Translate { .. })
        && let Some(keywords) = user_keywords.filter(|s| !s.trim().is_empty())
    {
        sections.push(format!("Preferred terms:\n{}", keywords.trim()));
    }
    if !lookbehind_context.is_empty() {
        let mut horizon = String::from(
            "Preceding conversation context (for narrative continuity reference only; DO NOT output or translate these segments):\n",
        );
        for segment in lookbehind_context {
            horizon.push_str(&format!("- {}\n", segment.text));
        }
        sections.push(horizon.trim().to_string());
    }

    let json_segments =
        serde_json::to_string(expected_segments).unwrap_or_else(|_| "[]".to_string());
    match task {
        RewriteAgentTask::Polish(_) => {
            sections.push(format!(
                "Polish these segments and return them in an `items` array:\n{json_segments}"
            ));
        }
        RewriteAgentTask::Translate {
            target_language,
            target_language_name,
        } => {
            let target = target_language_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(target_language)
                .trim();
            sections.push(format!(
                "Translate these segments into {target} and return them in an `items` array with the original `id` preserved:\n{json_segments}"
            ));
        }
    }

    sections.join("\n\n")
}

/// Executes an agent chunk rewrite with an autonomous Reflective Critic loop.
///
/// If the language model hallucinates, drops segment IDs, or returns invalid JSON,
/// the agent critic identifies the discrepancy and constructs a targeted self-correction
/// critique prompt to instruct the model to repair itself.
#[allow(clippy::too_many_arguments)]
pub async fn execute_agent_chunk_with_reflection<CompleteFn, CompleteFut, ParseFn, T>(
    task: &RewriteAgentTask,
    config: &LlmConfig,
    initial_prompt: String,
    expected: &[LlmSegmentInput],
    chunk_number: usize,
    cache: LlmPromptCachePolicy,
    max_output_tokens: Option<u64>,
    mut complete_fn: CompleteFn,
    parse_fn: ParseFn,
) -> Result<Vec<T>, LlmTaskError>
where
    CompleteFn: FnMut(LlmCompletionRequest) -> CompleteFut,
    CompleteFut: Future<Output = Result<LlmCompletionResponse, LlmRuntimeError>>,
    ParseFn: Fn(&LlmCompletionResponse, &[LlmSegmentInput], usize) -> Result<Vec<T>, LlmTaskError>,
{
    let format = task.response_format(expected.len());
    let system_prompt = task.system_prompt();
    let stage = task.stage_name();

    let initial_request = structured_agent_request(
        config.clone(),
        system_prompt,
        initial_prompt.clone(),
        format.clone(),
        cache,
        max_output_tokens,
    );

    let current_input = initial_prompt;
    let mut reflection_count = 0usize;
    const MAX_REFLECTIONS: usize = 2;

    let response = match complete_fn(initial_request).await {
        Ok(res) => res,
        Err(LlmRuntimeError::InvalidResponse { reason }) => {
            reflection_count += 1;
            let repair_prompt =
                sona_core::llm::tasks::build_structured_repair_input(&current_input, &reason, None);
            let repair_request = structured_agent_request(
                config.clone(),
                system_prompt,
                repair_prompt,
                format.clone(),
                cache,
                max_output_tokens,
            );
            complete_fn(repair_request)
                .await
                .map_err(|err| runtime_agent_error(&format!("{stage} repair"), err))?
        }
        Err(source) => {
            return Err(runtime_agent_error(
                &format!("{stage} chunk {chunk_number}"),
                source,
            ));
        }
    };

    let mut current_response = response;
    loop {
        match parse_fn(&current_response, expected, chunk_number) {
            Ok(items) => return Ok(items),
            Err(error) => {
                if reflection_count >= MAX_REFLECTIONS {
                    return Err(error);
                }
                reflection_count += 1;
                let reason = error.to_string();
                let repair_prompt = sona_core::llm::tasks::build_structured_repair_input(
                    &current_input,
                    &reason,
                    Some(&current_response.text),
                );
                let repair_request = structured_agent_request(
                    config.clone(),
                    system_prompt,
                    repair_prompt,
                    format.clone(),
                    cache,
                    max_output_tokens,
                );
                current_response = complete_fn(repair_request)
                    .await
                    .map_err(|err| runtime_agent_error(&format!("{stage} repair"), err))?;
            }
        }
    }
}

fn structured_agent_request(
    config: LlmConfig,
    system_prompt: &'static str,
    input: String,
    response_format: LlmResponseFormat,
    prompt_cache: LlmPromptCachePolicy,
    max_output_tokens: Option<u64>,
) -> LlmCompletionRequest {
    LlmCompletionRequest {
        config,
        system_prompt: Some(system_prompt.to_string()),
        input,
        options: LlmCompletionOptions {
            max_output_tokens,
            response_format,
            prompt_cache,
            capability_policy: LlmCapabilityPolicy::Compatible,
            ..LlmCompletionOptions::default()
        },
        source: None,
    }
}

fn runtime_agent_error(stage: &str, source: LlmRuntimeError) -> LlmTaskError {
    LlmTaskError::Runtime {
        stage: stage.to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn sample_segment(id: &str, text: &str) -> LlmSegmentInput {
        LlmSegmentInput {
            id: id.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn agent_chunk_input_omits_lookbehind_when_empty() {
        let segments = vec![sample_segment("s1", "hello")];
        let prompt = build_agent_chunk_input(
            &RewriteAgentTask::Polish(PolishMode::Clean),
            &segments,
            &[],
            Some("test context"),
            Some("termA, termB"),
        );
        assert!(prompt.contains("Mode: Clean spoken."));
        assert!(
            prompt.contains("Context (reference only; do not alter editing mode):\ntest context")
        );
        assert!(!prompt.contains("Preceding conversation context"));
        assert!(prompt.contains(r#""id":"s1""#));
    }

    #[test]
    fn agent_chunk_input_injects_lookbehind_for_subsequent_chunks() {
        let lookbehind = vec![
            sample_segment("s1", "Earlier sentence 1"),
            sample_segment("s2", "Earlier sentence 2"),
        ];
        let active = vec![sample_segment("s3", "Current sentence 3")];
        let prompt = build_agent_chunk_input(
            &RewriteAgentTask::Translate {
                target_language: "zh".to_string(),
                target_language_name: Some("Chinese".to_string()),
            },
            &active,
            &lookbehind,
            None,
            None,
        );
        assert!(prompt.contains("Preceding conversation context"));
        assert!(prompt.contains("- Earlier sentence 1"));
        assert!(prompt.contains("- Earlier sentence 2"));
        assert!(prompt.contains("Translate these segments into Chinese"));
        assert!(prompt.contains(r#""id":"s3""#));
    }

    #[tokio::test]
    async fn agent_reflection_critic_retries_and_succeeds() {
        use sona_core::llm::runtime::LlmExecutionMetadata;
        use sona_core::llm::tasks::PolishedSegment;

        let config = LlmConfig {
            provider: sona_core::domain::LlmProvider::Builtin(
                sona_core::domain::BuiltinLlmProvider::OpenAi,
            ),
            strategy: sona_core::llm::tasks::LlmProviderStrategy::OpenAi,
            base_url: "https://example.com".to_string(),
            api_key: "key".to_string(),
            model: "model".to_string(),
            timeout_seconds: None,
            api_path: None,
            api_version: None,
            reasoning_enabled: Some(false),
            reasoning_level: None,
            temperature: None,
        };
        let expected = vec![sample_segment("s1", "test")];
        let call_count = Arc::new(AtomicUsize::new(0));
        let counter = call_count.clone();

        let result = execute_agent_chunk_with_reflection(
            &RewriteAgentTask::Polish(PolishMode::Clean),
            &config,
            "initial prompt".to_string(),
            &expected,
            1,
            LlmPromptCachePolicy::Disabled,
            None,
            move |_req: LlmCompletionRequest| {
                let c = counter.clone();
                async move {
                    let attempt = c.fetch_add(1, Ordering::SeqCst);
                    Ok(LlmCompletionResponse {
                        text: if attempt == 0 {
                            "invalid".to_string()
                        } else {
                            "valid".to_string()
                        },
                        json: None,
                        usage: None,
                        execution: LlmExecutionMetadata {
                            requested_format:
                                sona_core::llm::runtime::LlmResponseFormatKind::JsonSchema,
                            applied_format:
                                sona_core::llm::runtime::LlmResponseFormatKind::JsonSchema,
                            warnings: Vec::new(),
                            attempts: 1,
                        },
                    })
                }
            },
            |res: &LlmCompletionResponse, _exp, _num| {
                if res.text == "valid" {
                    Ok(vec![PolishedSegment {
                        id: "s1".to_string(),
                        text: "polished test".to_string(),
                    }])
                } else {
                    Err(LlmTaskError::InvalidResponse {
                        reason: "failed validation: expected valid JSON".to_string(),
                    })
                }
            },
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn initial_invalid_response_counts_toward_reflections() {
        use sona_core::llm::runtime::LlmExecutionMetadata;
        use sona_core::llm::tasks::PolishedSegment;

        let config = LlmConfig {
            provider: sona_core::domain::LlmProvider::Builtin(
                sona_core::domain::BuiltinLlmProvider::OpenAi,
            ),
            strategy: sona_core::llm::tasks::LlmProviderStrategy::OpenAi,
            base_url: "https://example.com".to_string(),
            api_key: "key".to_string(),
            model: "model".to_string(),
            timeout_seconds: None,
            api_path: None,
            api_version: None,
            reasoning_enabled: Some(false),
            reasoning_level: None,
            temperature: None,
        };
        let expected = vec![sample_segment("s1", "test")];
        let call_count = Arc::new(AtomicUsize::new(0));
        let counter = call_count.clone();

        let result = execute_agent_chunk_with_reflection(
            &RewriteAgentTask::Polish(PolishMode::Clean),
            &config,
            "initial prompt".to_string(),
            &expected,
            1,
            LlmPromptCachePolicy::Disabled,
            None,
            move |_req: LlmCompletionRequest| {
                let c = counter.clone();
                async move {
                    let attempt = c.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        Err(LlmRuntimeError::InvalidResponse {
                            reason: "malformed JSON".to_string(),
                        })
                    } else {
                        Ok(LlmCompletionResponse {
                            text: if attempt == 1 {
                                "still invalid".to_string()
                            } else {
                                "valid".to_string()
                            },
                            json: None,
                            usage: None,
                            execution: LlmExecutionMetadata {
                                requested_format:
                                    sona_core::llm::runtime::LlmResponseFormatKind::JsonSchema,
                                applied_format:
                                    sona_core::llm::runtime::LlmResponseFormatKind::JsonSchema,
                                warnings: Vec::new(),
                                attempts: 1,
                            },
                        })
                    }
                }
            },
            |res: &LlmCompletionResponse, _exp, _num| {
                if res.text == "valid" {
                    Ok(vec![PolishedSegment {
                        id: "s1".to_string(),
                        text: "polished test".to_string(),
                    }])
                } else {
                    Err(LlmTaskError::InvalidResponse {
                        reason: "failed validation: expected valid JSON".to_string(),
                    })
                }
            },
        )
        .await;

        assert!(result.is_ok());
        // 1 initial (Err) + 1 repair (attempt 1 -> "still invalid") + 1 reflection (attempt 2 -> "valid") = 3 calls
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }
}
