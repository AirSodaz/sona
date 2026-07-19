use crate::llm::provider_protocol::LlmModelSummary;

pub const DEFAULT_SEGMENT_CHUNK_SIZE: usize = 30;
pub const DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET: usize = 32_000;
pub const DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET: usize = DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET / 2;
pub const DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET: usize = 6000;
pub const MIN_SUMMARY_CHUNK_CHAR_BUDGET: usize = 1200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSegmentChunk {
    pub start: usize,
    pub end: usize,
    pub prompt: String,
}

fn normalize_chunk_size(chunk_size: Option<usize>) -> usize {
    chunk_size
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SEGMENT_CHUNK_SIZE)
}

pub fn prompt_char_count(prompt: &str) -> usize {
    prompt.chars().count()
}

pub fn plan_segment_task_chunks<BuildPrompt>(
    _task_id: &str,
    _task_type: super::LlmTaskType,
    segments: &[super::LlmSegmentInput],
    chunk_size: Option<usize>,
    prompt_char_budget: usize,
    build_prompt: &mut BuildPrompt,
) -> Vec<PlannedSegmentChunk>
where
    BuildPrompt: FnMut(&[super::LlmSegmentInput]) -> String,
{
    if segments.is_empty() {
        return Vec::new();
    }

    if chunk_size.is_some() {
        let normalized_chunk_size = normalize_chunk_size(chunk_size);
        let mut planned = Vec::with_capacity(segments.len().div_ceil(normalized_chunk_size));

        for start in (0..segments.len()).step_by(normalized_chunk_size) {
            let end = (start + normalized_chunk_size).min(segments.len());
            planned.push(PlannedSegmentChunk {
                start,
                end,
                prompt: build_prompt(&segments[start..end]),
            });
        }

        return planned;
    }

    let prompt_char_budget = prompt_char_budget.max(1);
    let mut planned = Vec::new();
    let mut chunk_start = 0usize;
    let mut chunk_end = 0usize;
    let mut current_prompt: Option<String> = None;

    while chunk_start < segments.len() {
        let candidate_end = chunk_end + 1;
        let candidate_prompt = build_prompt(&segments[chunk_start..candidate_end]);
        let candidate_prompt_chars = prompt_char_count(&candidate_prompt);

        if candidate_prompt_chars <= prompt_char_budget {
            chunk_end = candidate_end;
            current_prompt = Some(candidate_prompt);

            if chunk_end == segments.len() {
                planned.push(PlannedSegmentChunk {
                    start: chunk_start,
                    end: chunk_end,
                    prompt: current_prompt
                        .take()
                        .expect("accepted chunk should always have a prompt"),
                });
                break;
            }

            continue;
        }

        if chunk_end == chunk_start {
            planned.push(PlannedSegmentChunk {
                start: chunk_start,
                end: candidate_end,
                prompt: candidate_prompt,
            });
            chunk_start = candidate_end;
            chunk_end = chunk_start;
            current_prompt = None;
            continue;
        }

        planned.push(PlannedSegmentChunk {
            start: chunk_start,
            end: chunk_end,
            prompt: current_prompt
                .take()
                .expect("previous accepted chunk should always have a prompt"),
        });
        let next_chunk_start = chunk_end;
        chunk_start = next_chunk_start;
        chunk_end = next_chunk_start;
    }

    planned
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LlmTaskBudget {
    pub prompt_char_budget: usize,
    pub prompt_token_budget: Option<u64>,
    pub max_output_tokens: Option<u64>,
}

pub fn resolve_task_budget(
    model: Option<&LlmModelSummary>,
    legacy_prompt_char_budget: usize,
    desired_output_tokens: u64,
) -> LlmTaskBudget {
    let Some(model) = model else {
        return LlmTaskBudget {
            prompt_char_budget: legacy_prompt_char_budget.max(1),
            prompt_token_budget: None,
            max_output_tokens: None,
        };
    };

    let safe_context = model
        .context_window
        .map(|context_window| context_window.saturating_mul(9) / 10);
    let context_output_cap = safe_context.map(|context| (context / 2).max(1));
    let max_output_tokens = match (model.max_output_tokens, context_output_cap) {
        (Some(model_limit), Some(context_limit)) => Some(
            model_limit
                .min(context_limit)
                .min(desired_output_tokens)
                .max(1),
        ),
        (Some(model_limit), None) => Some(model_limit.min(desired_output_tokens).max(1)),
        (None, Some(context_limit)) => Some(context_limit.min(desired_output_tokens).max(1)),
        (None, None) => None,
    };
    let reserved_output_tokens = max_output_tokens.unwrap_or(desired_output_tokens);
    let model_prompt_budget =
        safe_context.map(|context| context.saturating_sub(reserved_output_tokens).max(1));
    let prompt_char_budget = model_prompt_budget
        .and_then(|value| usize::try_from(value).ok())
        .map(|value| value.min(legacy_prompt_char_budget.max(1)))
        .unwrap_or_else(|| legacy_prompt_char_budget.max(1));

    LlmTaskBudget {
        prompt_char_budget,
        prompt_token_budget: model_prompt_budget,
        max_output_tokens,
    }
}

pub fn estimate_structured_output_tokens(segments: &[super::LlmSegmentInput]) -> u64 {
    segments
        .iter()
        .map(|segment| {
            conservative_token_estimate(&segment.text)
                .saturating_mul(2)
                .saturating_add(conservative_token_estimate(&segment.id))
                .saturating_add(32)
        })
        .sum::<u64>()
        .saturating_add(128)
}

pub fn conservative_token_estimate(text: &str) -> u64 {
    let mut ascii = 0u64;
    let mut non_ascii = 0u64;
    for character in text.chars() {
        if character.is_ascii() {
            ascii += 1;
        } else {
            non_ascii += 1;
        }
    }

    ascii
        .div_ceil(3)
        .saturating_add(non_ascii.saturating_mul(2))
}

pub fn plan_segment_chunks_with_budget<BuildPrompt>(
    segments: &[super::LlmSegmentInput],
    chunk_size: Option<usize>,
    budget: LlmTaskBudget,
    mut build_prompt: BuildPrompt,
) -> Result<Vec<super::PlannedSegmentChunk>, super::LlmTaskError>
where
    BuildPrompt: FnMut(&[super::LlmSegmentInput]) -> String,
{
    if segments.is_empty() {
        return Ok(Vec::new());
    }

    let max_items = chunk_size.filter(|value| *value > 0).unwrap_or(usize::MAX);
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < segments.len() {
        let mut accepted: Option<(usize, String)> = None;
        let max_end = start.saturating_add(max_items).min(segments.len());

        for end in (start + 1)..=max_end {
            let prompt = build_prompt(&segments[start..end]);
            let within_chars = prompt.chars().count() <= budget.prompt_char_budget;
            let within_tokens = budget
                .prompt_token_budget
                .map(|limit| conservative_token_estimate(&prompt) <= limit)
                .unwrap_or(true);
            let within_output = budget
                .max_output_tokens
                .map(|limit| estimate_structured_output_tokens(&segments[start..end]) <= limit)
                .unwrap_or(true);
            if !within_chars || !within_tokens || !within_output {
                break;
            }
            accepted = Some((end, prompt));
        }

        let Some((end, prompt)) = accepted else {
            return Err(super::LlmTaskError::InvalidRequest {
                reason: format!(
                    "Segment '{}' exceeds the model context or output budget",
                    segments[start].id
                ),
            });
        };
        chunks.push(super::PlannedSegmentChunk { start, end, prompt });
        start = end;
    }

    Ok(chunks)
}
