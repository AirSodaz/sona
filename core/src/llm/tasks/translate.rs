pub const TRANSLATE_SYSTEM_PROMPT: &str = "You translate transcript segments. Return only one JSON object matching the supplied schema. Preserve every segment id and order; never combine or split segments, and put translated text in the translation field.";

pub fn build_translate_prompt(
    segments: &[super::LlmSegmentInput],
    target_language: &str,
    target_language_name: Option<&str>,
) -> String {
    let json_str = serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string());
    let resolved_name = target_language_name.unwrap_or(target_language);

    format!(
        "You are a professional translator. Translate the following array of text segments into {}.\n\
CRITICAL INSTRUCTIONS:\n\
1. Output newline-delimited JSON (NDJSON) only. Do not wrap the result in a JSON array.\n\
2. Each output line must be one valid JSON object. Do not include markdown formatting like ```json.\n\
3. Return objects with the EXACT SAME 'id' field, but replace 'text' with 'translation'.\n\
4. Do not combine or split segments. There must be exactly {} JSON lines in the output.\n\n\
Input:\n\
{}",
        resolved_name,
        segments.len(),
        json_str
    )
}

pub fn build_translate_task_input(
    segments: &[super::LlmSegmentInput],
    target_language: &str,
    target_language_name: Option<&str>,
) -> String {
    let target = target_language_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(target_language)
        .trim();
    format!(
        "Translate these segments into {target} and return them in an `items` array:\n{}",
        serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string())
    )
}
