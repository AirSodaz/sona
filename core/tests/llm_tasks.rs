use sona_core::llm::tasks::{
    LlmSegmentInput, LlmTaskType, SummarySegmentInput, SummaryTemplateConfig, build_polish_prompt,
    build_summary_chunk_prompt, build_summary_finalize_prompt, build_translate_prompt,
    plan_segment_task_chunks, prompt_char_count, split_summary_segments,
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

#[test]
fn dynamic_segment_planning_uses_prompt_budget() {
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
            .map(|chunk| (chunk.start, chunk.end, chunk.prompt.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, 2, "AAAAAA|BBBBBB"), (2, 3, "CCCCCC")]
    );
}

#[test]
fn polish_prompt_context_and_keywords_reduce_chunk_capacity() {
    let segments = sample_segments();
    let context = "Project roadmap context. ".repeat(3);
    let keywords = "Sona, roadmap, launch. ".repeat(3);
    let without_context_two = prompt_char_count(&build_polish_prompt(&segments[..2], None, None));
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
fn prompts_and_summary_planning_are_core_owned() {
    let template = SummaryTemplateConfig {
        id: "meeting".to_string(),
        name: "Meeting".to_string(),
        instructions: "1. Meeting overview.\n2. Decisions made.".to_string(),
    };
    let summary_segments = sample_summary_segments();

    let translate_prompt =
        build_translate_prompt(&sample_segments()[..2], "zh", Some("Chinese (Simplified)"));
    let chunk_prompt = build_summary_chunk_prompt(&template, &summary_segments[..2], 1, 2);
    let finalize_prompt =
        build_summary_finalize_prompt(&template, &["Chunk 1 summary".to_string()]);
    let chunks = split_summary_segments(&summary_segments, 70);

    assert!(translate_prompt.contains("Chinese (Simplified)"));
    assert!(translate_prompt.contains("replace 'text' with 'translation'"));
    assert!(chunk_prompt.contains("Use the same language as the transcript."));
    assert!(chunk_prompt.contains("Meeting overview."));
    assert!(finalize_prompt.contains("[Chunk 1]"));
    assert_eq!(chunks.len(), 3);
}
