pub const SUMMARY_MAP_SYSTEM_PROMPT: &str = "Summarize one transcript chunk concisely and factually. Use the transcript language, preserve the requested structure, and do not use Markdown code fences.";

pub const SUMMARY_FINAL_SYSTEM_PROMPT: &str = "Produce the final transcript summary. Use the transcript language, follow the requested structure, merge overlap, and include only supported information. Do not use Markdown code fences.";

pub const SUMMARY_REDUCE_SYSTEM_PROMPT: &str = "Compress intermediate transcript summaries into a shorter factual summary. Preserve the requested structure and language, merge overlap, and do not use Markdown code fences.";

fn format_summary_timestamp(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0).floor() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

fn format_summary_segments_for_prompt(segments: &[super::SummarySegmentInput]) -> String {
    segments
        .iter()
        .filter_map(|segment| {
            let text = segment.text.trim();
            if text.is_empty() {
                return None;
            }

            Some(format!(
                "[{} - {}] {}",
                format_summary_timestamp(segment.start),
                format_summary_timestamp(segment.end),
                text
            ))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn build_summary_chunk_prompt(
    template: &super::SummaryTemplateConfig,
    segments: &[super::SummarySegmentInput],
    chunk_number: usize,
    total_chunks: usize,
) -> String {
    format!(
        "You are preparing an intermediate transcript summary using the \"{template_name}\" template.\n\
Use the same language as the transcript. Do not translate or switch languages.\n\
Follow this structure:\n\
{structure}\n\n\
This is chunk {chunk_number} of {total_chunks}. Capture only information supported by this chunk.\n\
Keep it concise, factual, and easy to merge later.\n\
Do not use markdown code fences.\n\n\
Transcript chunk:\n\
{chunk_text}",
        template_name = template.name.trim(),
        structure = template.instructions.trim(),
        chunk_number = chunk_number,
        total_chunks = total_chunks,
        chunk_text = format_summary_segments_for_prompt(segments),
    )
}

pub fn build_summary_finalize_prompt(
    template: &super::SummaryTemplateConfig,
    partial_summaries: &[String],
) -> String {
    format!(
        "You are combining intermediate transcript summaries into one final summary using the \"{template_name}\" template.\n\
Use the same language as the transcript. Do not translate or switch languages.\n\
Follow this structure:\n\
{structure}\n\n\
Merge overlapping points, keep the wording concise, and preserve only information supported by the intermediate summaries.\n\
Do not use markdown code fences.\n\n\
Intermediate summaries:\n\
{partials}",
        template_name = template.name.trim(),
        structure = template.instructions.trim(),
        partials = partial_summaries
            .iter()
            .enumerate()
            .map(|(index, item)| format!("[Chunk {}]\n{}", index + 1, item.trim()))
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

pub fn build_summary_reduce_prompt(
    template: &super::SummaryTemplateConfig,
    partial_summaries: &[String],
) -> String {
    format!(
        "Compress these intermediate summaries using the \"{template_name}\" template.\n\
Use the same language as the transcript and follow this structure:\n\
{structure}\n\n\
Merge overlap and keep only supported information. Return one concise intermediate summary.\n\n\
Intermediate summaries:\n\
{partials}",
        template_name = template.name.trim(),
        structure = template.instructions.trim(),
        partials = partial_summaries
            .iter()
            .enumerate()
            .map(|(index, item)| format!("[Chunk {}]\n{}", index + 1, item.trim()))
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

pub fn summary_prompt_fits_budget(
    system_prompt: &str,
    prompt: &str,
    budget: super::LlmTaskBudget,
) -> bool {
    let prompt_chars = system_prompt
        .chars()
        .count()
        .saturating_add(prompt.chars().count());
    let prompt_tokens = super::conservative_token_estimate(system_prompt)
        .saturating_add(super::conservative_token_estimate(prompt));
    prompt_chars <= budget.prompt_char_budget
        && budget
            .prompt_token_budget
            .map(|limit| prompt_tokens <= limit)
            .unwrap_or(true)
}

pub fn split_summary_partials_for_reduce(
    template: &super::SummaryTemplateConfig,
    partials: &[String],
    budget: super::LlmTaskBudget,
) -> Vec<Vec<String>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();

    for partial in partials {
        let mut candidate = current.clone();
        candidate.push(partial.clone());
        let prompt = build_summary_reduce_prompt(template, &candidate);
        if !current.is_empty()
            && !summary_prompt_fits_budget(SUMMARY_REDUCE_SYSTEM_PROMPT, &prompt, budget)
        {
            groups.push(current);
            current = vec![partial.clone()];
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

pub fn resolve_summary_chunk_char_budget(
    template: &super::SummaryTemplateConfig,
    explicit_budget: usize,
    budget: super::LlmTaskBudget,
    maximum_chunk_count: usize,
) -> usize {
    let overhead = build_summary_chunk_prompt(
        template,
        &[],
        maximum_chunk_count.max(1),
        maximum_chunk_count.max(1),
    );
    let overhead_chars = SUMMARY_MAP_SYSTEM_PROMPT
        .chars()
        .count()
        .saturating_add(overhead.chars().count());
    let overhead_tokens = super::conservative_token_estimate(SUMMARY_MAP_SYSTEM_PROMPT)
        .saturating_add(super::conservative_token_estimate(&overhead));
    let available_chars = budget
        .prompt_char_budget
        .saturating_sub(overhead_chars)
        .max(1);
    let token_limited_chars = budget
        .prompt_token_budget
        .map(|tokens| tokens.saturating_sub(overhead_tokens) / 2)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(available_chars)
        .max(1);
    explicit_budget
        .max(1)
        .min(available_chars)
        .min(token_limited_chars)
}

pub(crate) fn build_summary_direct_prompt(
    template: &super::SummaryTemplateConfig,
    segments: &[super::SummarySegmentInput],
) -> String {
    format!(
        "You are creating a final transcript summary using the \"{template_name}\" template.\n\
Use the same language as the transcript. Do not translate or switch languages.\n\
Follow this structure:\n\
{structure}\n\n\
Keep the summary concise, factual, and limited to information supported by the transcript.\n\
Do not use markdown code fences.\n\n\
Transcript:\n\
{chunk_text}",
        template_name = template.name.trim(),
        structure = template.instructions.trim(),
        chunk_text = format_summary_segments_for_prompt(segments),
    )
}

fn estimate_summary_segment_chars(segment: &super::SummarySegmentInput) -> usize {
    segment.text.chars().count() + 24
}

pub fn split_summary_segments(
    segments: &[super::SummarySegmentInput],
    chunk_char_budget: usize,
) -> Vec<Vec<super::SummarySegmentInput>> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current_chunk = Vec::new();
    let mut current_chars = 0usize;

    for segment in segments {
        let segment_chars = estimate_summary_segment_chars(segment);

        if !current_chunk.is_empty() && current_chars + segment_chars > chunk_char_budget {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_chars = 0;
        }

        current_chars += segment_chars;
        current_chunk.push(segment.clone());
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

pub fn validate_summary_strategy(
    strategy: super::LlmProviderStrategy,
) -> Result<(), super::LlmTaskError> {
    if matches!(
        strategy,
        super::LlmProviderStrategy::GoogleTranslate
            | super::LlmProviderStrategy::GoogleTranslateFree
    ) {
        return Err(super::LlmTaskError::InvalidRequest {
            reason: "Google Translate does not support transcript summaries".to_string(),
        });
    }

    Ok(())
}
