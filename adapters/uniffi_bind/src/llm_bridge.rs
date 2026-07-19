use crate::json_bridge::{map_core_validation_result, parse_core_json, serialize_core_json};
use crate::mapper::{
    self, FfiLlmConfig, FfiLlmPromptChunk, FfiLlmProvider, FfiLlmSegmentInput,
    FfiPolishSegmentsRequest, FfiPolishedSegment, FfiSummarizeTranscriptRequest,
    FfiSummarySegmentInput, FfiTranslateSegmentsRequest, FfiTranslatedSegment,
};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::llm::jobs::{
    compute_summary_source_fingerprint as core_compute_summary_source_fingerprint,
    merge_polished_items_into_segments as core_merge_polished_items_into_segments,
    merge_translated_items_into_segments as core_merge_translated_items_into_segments,
    segment_inputs_from_transcript as core_segment_inputs_from_transcript,
    summary_inputs_from_transcript as core_summary_inputs_from_transcript,
};
use sona_core::llm::providers::{
    find_llm_provider_by_id_or_alias as core_find_llm_provider_by_id_or_alias,
    llm_providers as core_llm_providers,
};
use sona_core::llm::requests::{
    LlmConfig, LlmGenerateRequest, PolishSegmentsRequest, SummarizeTranscriptRequest,
    TranslateSegmentsRequest, validate_llm_config as core_validate_llm_config,
    validate_llm_generate_request as core_validate_llm_generate_request,
    validate_polish_segments_request as core_validate_polish_segments_request,
    validate_summarize_transcript_request as core_validate_summarize_transcript_request,
    validate_translate_segments_request as core_validate_translate_segments_request,
};
use sona_core::llm::tasks::{
    DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET, DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET, LlmSegmentInput,
    LlmTaskType, MIN_SUMMARY_CHUNK_CHAR_BUDGET, PlannedSegmentChunk, PolishedSegment,
    SummarySegmentInput, SummaryTemplateConfig, TranslatedSegment,
    build_polish_prompt as core_build_polish_prompt,
    build_summary_chunk_prompt as core_build_summary_chunk_prompt,
    build_summary_finalize_prompt as core_build_summary_finalize_prompt,
    build_translate_prompt as core_build_translate_prompt,
    parse_polish_chunk as core_parse_polish_chunk,
    parse_translate_chunk as core_parse_translate_chunk,
    plan_segment_task_chunks as core_plan_segment_task_chunks,
    split_summary_segments as core_split_summary_segments,
};
use sona_core::transcription::transcript::TranscriptSegment;

pub(crate) fn llm_providers() -> Vec<FfiLlmProvider> {
    core_llm_providers()
        .iter()
        .map(mapper::llm_provider_to_ffi)
        .collect()
}

pub(crate) fn find_llm_provider_by_id_or_alias(id_or_alias: String) -> Option<FfiLlmProvider> {
    core_find_llm_provider_by_id_or_alias(&id_or_alias).map(mapper::llm_provider_to_ffi)
}

pub(crate) fn llm_config_from_json(config_json: String) -> SonaCoreBindingResult<FfiLlmConfig> {
    let config: LlmConfig = parse_core_json(&config_json, "LLM config")?;
    Ok(mapper::llm_config_to_ffi(config))
}

pub(crate) fn validate_llm_config_json(config_json: String) -> SonaCoreBindingResult<()> {
    let config: LlmConfig = parse_core_json(&config_json, "LLM config")?;
    map_core_validation_result(core_validate_llm_config(&config))
}

pub(crate) fn validate_llm_generate_request_json(
    request_json: String,
) -> SonaCoreBindingResult<()> {
    let request: LlmGenerateRequest = parse_core_json(&request_json, "LLM generate request")?;
    map_core_validation_result(core_validate_llm_generate_request(&request))
}

pub(crate) fn validate_polish_segments_request_json(
    request_json: String,
) -> SonaCoreBindingResult<()> {
    let request: PolishSegmentsRequest = parse_core_json(&request_json, "polish segments request")?;
    map_core_validation_result(core_validate_polish_segments_request(&request))
}

pub(crate) fn validate_translate_segments_request_json(
    request_json: String,
) -> SonaCoreBindingResult<()> {
    let request: TranslateSegmentsRequest =
        parse_core_json(&request_json, "translate segments request")?;
    map_core_validation_result(core_validate_translate_segments_request(&request))
}

pub(crate) fn validate_summarize_transcript_request_json(
    request_json: String,
) -> SonaCoreBindingResult<()> {
    let request: SummarizeTranscriptRequest =
        parse_core_json(&request_json, "summarize transcript request")?;
    map_core_validation_result(core_validate_summarize_transcript_request(&request))
}

pub(crate) fn llm_segment_inputs_from_transcript_json(
    segments_json: String,
) -> SonaCoreBindingResult<Vec<FfiLlmSegmentInput>> {
    let segments: Vec<TranscriptSegment> = parse_core_json(&segments_json, "transcript segments")?;
    Ok(core_segment_inputs_from_transcript(&segments)
        .into_iter()
        .map(mapper::llm_segment_input_to_ffi)
        .collect())
}

pub(crate) fn summary_segment_inputs_from_transcript_json(
    segments_json: String,
) -> SonaCoreBindingResult<Vec<FfiSummarySegmentInput>> {
    let segments: Vec<TranscriptSegment> = parse_core_json(&segments_json, "transcript segments")?;
    Ok(core_summary_inputs_from_transcript(&segments)
        .into_iter()
        .map(mapper::summary_segment_input_to_ffi)
        .collect())
}

pub(crate) fn merge_translated_items_into_transcript_json(
    segments_json: String,
    items_json: String,
) -> SonaCoreBindingResult<String> {
    let segments: Vec<TranscriptSegment> = parse_core_json(&segments_json, "transcript segments")?;
    let items: Vec<TranslatedSegment> = parse_core_json(&items_json, "translated segment items")?;
    let merged = core_merge_translated_items_into_segments(segments, &items);
    serialize_core_json(&merged, "merged transcript segments")
}

pub(crate) fn merge_polished_items_into_transcript_json(
    segments_json: String,
    items_json: String,
) -> SonaCoreBindingResult<String> {
    let segments: Vec<TranscriptSegment> = parse_core_json(&segments_json, "transcript segments")?;
    let items: Vec<PolishedSegment> = parse_core_json(&items_json, "polished segment items")?;
    let merged = core_merge_polished_items_into_segments(segments, &items);
    serialize_core_json(&merged, "merged transcript segments")
}

pub(crate) fn summary_source_fingerprint_from_transcript_json(
    segments_json: String,
) -> SonaCoreBindingResult<String> {
    let segments: Vec<TranscriptSegment> = parse_core_json(&segments_json, "transcript segments")?;
    Ok(core_compute_summary_source_fingerprint(&segments))
}

pub(crate) fn build_polish_prompt_json(
    segments_json: String,
    context: Option<String>,
    keywords: Option<String>,
) -> SonaCoreBindingResult<String> {
    let segments: Vec<LlmSegmentInput> = parse_core_json(&segments_json, "LLM segment inputs")?;
    Ok(core_build_polish_prompt(
        &segments,
        context.as_deref(),
        keywords.as_deref(),
    ))
}

pub(crate) fn build_translate_prompt_json(
    segments_json: String,
    target_language: String,
    target_language_name: Option<String>,
) -> SonaCoreBindingResult<String> {
    let segments: Vec<LlmSegmentInput> = parse_core_json(&segments_json, "LLM segment inputs")?;
    Ok(core_build_translate_prompt(
        &segments,
        &target_language,
        target_language_name.as_deref(),
    ))
}

pub(crate) fn build_summary_chunk_prompt_json(
    template_json: String,
    segments_json: String,
    chunk_number: u64,
    total_chunks: u64,
) -> SonaCoreBindingResult<String> {
    let template: SummaryTemplateConfig = parse_core_json(&template_json, "summary template")?;
    let segments: Vec<SummarySegmentInput> =
        parse_core_json(&segments_json, "summary segment inputs")?;

    Ok(core_build_summary_chunk_prompt(
        &template,
        &segments,
        u64_to_usize(chunk_number, "chunk number")?,
        u64_to_usize(total_chunks, "total chunks")?,
    ))
}

pub(crate) fn build_summary_finalize_prompt_json(
    template_json: String,
    partial_summaries: Vec<String>,
) -> SonaCoreBindingResult<String> {
    let template: SummaryTemplateConfig = parse_core_json(&template_json, "summary template")?;

    Ok(core_build_summary_finalize_prompt(
        &template,
        &partial_summaries,
    ))
}

pub(crate) fn plan_polish_prompt_chunks_json(
    segments_json: String,
    context: Option<String>,
    keywords: Option<String>,
    chunk_size: Option<u64>,
    prompt_char_budget: Option<u64>,
) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
    let segments: Vec<LlmSegmentInput> = parse_core_json(&segments_json, "LLM segment inputs")?;
    let chunk_size = optional_u64_to_usize(chunk_size, "chunk size")?;
    let prompt_char_budget = optional_u64_to_usize(prompt_char_budget, "prompt char budget")?
        .unwrap_or(DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET);
    let mut build_prompt = |chunk: &[LlmSegmentInput]| {
        core_build_polish_prompt(chunk, context.as_deref(), keywords.as_deref())
    };
    let chunks = core_plan_segment_task_chunks(
        "mobile-polish",
        LlmTaskType::Polish,
        &segments,
        chunk_size,
        prompt_char_budget,
        &mut build_prompt,
    );

    Ok(map_planned_prompt_chunks(chunks))
}

pub(crate) fn plan_translate_prompt_chunks_json(
    segments_json: String,
    target_language: String,
    target_language_name: Option<String>,
    chunk_size: Option<u64>,
    prompt_char_budget: Option<u64>,
) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
    let segments: Vec<LlmSegmentInput> = parse_core_json(&segments_json, "LLM segment inputs")?;
    let chunk_size = optional_u64_to_usize(chunk_size, "chunk size")?;
    let prompt_char_budget = optional_u64_to_usize(prompt_char_budget, "prompt char budget")?
        .unwrap_or(DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET);
    let mut build_prompt = |chunk: &[LlmSegmentInput]| {
        core_build_translate_prompt(chunk, &target_language, target_language_name.as_deref())
    };
    let chunks = core_plan_segment_task_chunks(
        "mobile-translate",
        LlmTaskType::Translate,
        &segments,
        chunk_size,
        prompt_char_budget,
        &mut build_prompt,
    );

    Ok(map_planned_prompt_chunks(chunks))
}

pub(crate) fn plan_summary_prompt_chunks_json(
    template_json: String,
    segments_json: String,
    chunk_char_budget: Option<u64>,
) -> SonaCoreBindingResult<Vec<FfiLlmPromptChunk>> {
    let template: SummaryTemplateConfig = parse_core_json(&template_json, "summary template")?;
    let segments: Vec<SummarySegmentInput> =
        parse_core_json(&segments_json, "summary segment inputs")?;
    let chunk_char_budget = optional_u64_to_usize(chunk_char_budget, "chunk char budget")?
        .unwrap_or(DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET)
        .max(MIN_SUMMARY_CHUNK_CHAR_BUDGET);
    let chunks = core_split_summary_segments(&segments, chunk_char_budget);
    let total_chunks = chunks.len();
    let mut start = 0usize;

    Ok(chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            let end = start + chunk.len();
            let prompt =
                core_build_summary_chunk_prompt(&template, &chunk, index + 1, total_chunks);
            let ffi_chunk =
                mapper::llm_prompt_chunk_to_ffi(start, end, index + 1, total_chunks, prompt);
            start = end;
            ffi_chunk
        })
        .collect())
}

pub(crate) fn parse_polish_chunk_json(
    response_text: String,
    expected_segments_json: String,
    chunk_number: u64,
) -> SonaCoreBindingResult<Vec<FfiPolishedSegment>> {
    let expected: Vec<LlmSegmentInput> =
        parse_core_json(&expected_segments_json, "LLM segment inputs")?;
    Ok(core_parse_polish_chunk(
        &response_text,
        &expected,
        u64_to_usize(chunk_number, "chunk number")?,
    )
    .map_err(|error| SonaCoreBindingError::InvalidInput {
        reason: error.to_string(),
    })?
    .into_iter()
    .map(mapper::polished_segment_to_ffi)
    .collect())
}

pub(crate) fn parse_translate_chunk_json(
    response_text: String,
    expected_segments_json: String,
    chunk_number: u64,
) -> SonaCoreBindingResult<Vec<FfiTranslatedSegment>> {
    let expected: Vec<LlmSegmentInput> =
        parse_core_json(&expected_segments_json, "LLM segment inputs")?;
    Ok(core_parse_translate_chunk(
        &response_text,
        &expected,
        u64_to_usize(chunk_number, "chunk number")?,
    )
    .map_err(|error| SonaCoreBindingError::InvalidInput {
        reason: error.to_string(),
    })?
    .into_iter()
    .map(mapper::translated_segment_to_ffi)
    .collect())
}

pub(crate) fn polish_segments_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiPolishSegmentsRequest> {
    let request: PolishSegmentsRequest = parse_core_json(&request_json, "polish segments request")?;
    Ok(mapper::polish_segments_request_to_ffi(request))
}

pub(crate) fn translate_segments_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiTranslateSegmentsRequest> {
    let request: TranslateSegmentsRequest =
        parse_core_json(&request_json, "translate segments request")?;
    Ok(mapper::translate_segments_request_to_ffi(request))
}

pub(crate) fn summarize_transcript_request_from_json(
    request_json: String,
) -> SonaCoreBindingResult<FfiSummarizeTranscriptRequest> {
    let request: SummarizeTranscriptRequest =
        parse_core_json(&request_json, "summarize transcript request")?;
    Ok(mapper::summarize_transcript_request_to_ffi(request))
}

fn u64_to_usize(value: u64, label: &str) -> SonaCoreBindingResult<usize> {
    usize::try_from(value).map_err(|_| SonaCoreBindingError::InvalidInput {
        reason: format!("{label} is too large"),
    })
}

fn optional_u64_to_usize(value: Option<u64>, label: &str) -> SonaCoreBindingResult<Option<usize>> {
    value.map(|item| u64_to_usize(item, label)).transpose()
}

fn map_planned_prompt_chunks(chunks: Vec<PlannedSegmentChunk>) -> Vec<FfiLlmPromptChunk> {
    let total_chunks = chunks.len();

    chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            mapper::llm_prompt_chunk_to_ffi(
                chunk.start,
                chunk.end,
                index + 1,
                total_chunks,
                chunk.prompt,
            )
        })
        .collect()
}
