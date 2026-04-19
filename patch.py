import re

with open("src-tauri/src/llm.rs", "r") as f:
    content = f.read()

# 1. Update run_segment_task signature
content = content.replace(
    "segments: &[LlmSegmentInput],",
    "mut segments: Vec<LlmSegmentInput>,"
)

# 2. Update BuildPrompt and ParseChunk signatures in where clause
content = content.replace(
    "BuildPrompt: FnMut(&[LlmSegmentInput]) -> String,",
    "BuildPrompt: FnMut(&[LlmSegmentInput]) -> String," # Keep as &[LlmSegmentInput] to avoid clone, build_prompt can just borrow
)
content = content.replace(
    "ParseChunk: FnMut(&str, &[LlmSegmentInput], usize) -> Result<Vec<Output>, String>,",
    "ParseChunk: FnMut(&str, Vec<LlmSegmentInput>, usize) -> Result<Vec<Output>, String>,"
)

# 3. Update run_segment_task implementation
#     for (chunk_index, chunk) in segments.chunks(normalized_chunk_size).enumerate() {
#         let chunk_number = chunk_index + 1;
#         let prompt = build_prompt(chunk);
#         let response_text = generate_text(prompt)
#             .await
#             .map_err(|error| chunk_error(task_type, chunk_number, error))?;
#         let parsed = parse_chunk(&response_text, chunk, chunk_number)?;

new_loop = """
    let mut chunk_index = 0;
    while !segments.is_empty() {
        let chunk_number = chunk_index + 1;
        let chunk_len = std::cmp::min(normalized_chunk_size, segments.len());
        let chunk: Vec<_> = segments.drain(..chunk_len).collect();

        let prompt = build_prompt(&chunk);
        let response_text = generate_text(prompt)
            .await
            .map_err(|error| chunk_error(task_type, chunk_number, error))?;
        let parsed = parse_chunk(&response_text, chunk, chunk_number)?;
"""

content = content.replace(
"""    for (chunk_index, chunk) in segments.chunks(normalized_chunk_size).enumerate() {
        let chunk_number = chunk_index + 1;
        let prompt = build_prompt(chunk);
        let response_text = generate_text(prompt)
            .await
            .map_err(|error| chunk_error(task_type, chunk_number, error))?;
        let parsed = parse_chunk(&response_text, chunk, chunk_number)?;""",
new_loop
)

# Add chunk_index += 1; at the end of the while loop
content = content.replace(
"""        results.extend(parsed);
    }""",
"""        results.extend(parsed);
        chunk_index += 1;
    }"""
)


# 4. Update callers in translate_transcript_segments
content = content.replace(
"""        return run_segment_task(
            &request.task_id,
            LlmTaskType::Translate,
            &request.segments,""",
"""        return run_segment_task(
            &request.task_id,
            LlmTaskType::Translate,
            request.segments,"""
)
content = content.replace(
"""    run_segment_task(
        &request.task_id,
        LlmTaskType::Translate,
        &request.segments,""",
"""    run_segment_task(
        &request.task_id,
        LlmTaskType::Translate,
        request.segments,"""
)

# 5. Update callers in polish_transcript_segments
content = content.replace(
"""    run_segment_task(
        &request.task_id,
        LlmTaskType::Polish,
        &request.segments,""",
"""    run_segment_task(
        &request.task_id,
        LlmTaskType::Polish,
        request.segments,"""
)


# 6. Update the GoogleTranslate code in translate_transcript_segments
google_translate_old = """                    let mut translated_segments = Vec::with_capacity(chunk.len());
                    for (index, translation) in parsed.data.translations.into_iter().enumerate() {
                        translated_segments.push(TranslatedSegment {
                            id: chunk[index].id.clone(),
                            translation: translation.translated_text,
                        });
                    }

                    Ok(translated_segments)"""
google_translate_new = """                    let translated_segments: Vec<_> = chunk.into_iter().zip(parsed.data.translations).map(|(s, t)| TranslatedSegment {
                        id: s.id,
                        translation: t.translated_text,
                    }).collect();

                    Ok(translated_segments)"""
content = content.replace(google_translate_old, google_translate_new)

google_translate_free_old = """                    Ok(chunk.iter().zip(translations).map(|(s, t)| TranslatedSegment {
                        id: s.id.clone(),
                        translation: t,
                    }).collect())"""
google_translate_free_new = """                    Ok(chunk.into_iter().zip(translations).map(|(s, t)| TranslatedSegment {
                        id: s.id,
                        translation: t,
                    }).collect())"""
content = content.replace(google_translate_free_old, google_translate_free_new)


# 7. Update parse_polish_chunk
content = content.replace(
"""fn parse_polish_chunk(
    response_text: &str,
    expected: &[LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<PolishedSegment>, String> {""",
"""fn parse_polish_chunk(
    response_text: &str,
    expected: Vec<LlmSegmentInput>,
    chunk_number: usize,
) -> Result<Vec<PolishedSegment>, String> {"""
)
content = content.replace(
"""    validate_segment_ids(&parsed, expected, LlmTaskType::Polish, chunk_number, |item| &item.id)?;""",
"""    validate_segment_ids(&parsed, &expected, LlmTaskType::Polish, chunk_number, |item| &item.id)?;"""
)

# 8. Update parse_translate_chunk
content = content.replace(
"""fn parse_translate_chunk(
    response_text: &str,
    expected: &[LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<TranslatedSegment>, String> {""",
"""fn parse_translate_chunk(
    response_text: &str,
    expected: Vec<LlmSegmentInput>,
    chunk_number: usize,
) -> Result<Vec<TranslatedSegment>, String> {"""
)
content = content.replace(
"""    validate_segment_ids(&parsed, expected, LlmTaskType::Translate, chunk_number, |item| &item.id)?;""",
"""    validate_segment_ids(&parsed, &expected, LlmTaskType::Translate, chunk_number, |item| &item.id)?;"""
)


with open("src-tauri/src/llm.rs", "w") as f:
    f.write(content)
