pub const POLISH_SYSTEM_PROMPT: &str = "You edit speech-to-text segments. Fix recognition, grammar, and clarity without changing meaning or language. Return only one JSON object matching the supplied schema. Preserve every segment id and order; never combine or split segments.";

pub fn build_polish_prompt(
    segments: &[super::LlmSegmentInput],
    context: Option<&str>,
    keywords: Option<&str>,
) -> String {
    let json_str = serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string());
    let mut prompt = String::new();

    if let Some(value) = context
        && !value.trim().is_empty()
    {
        prompt.push_str("[User Context]\n");
        prompt.push_str(value.trim());
        prompt.push_str("\n\n");
    }

    if let Some(value) = keywords
        && !value.trim().is_empty()
    {
        prompt.push_str("[User Keywords]\n");
        prompt.push_str(value.trim());
        prompt.push_str("\n\n");
    }

    prompt.push_str("You are a professional editor. The following text segments are from a speech-to-text transcription and may contain errors.\n");
    prompt.push_str("Your task is to:\n");
    prompt.push_str("1. Fix any speech recognition errors.\n");
    prompt.push_str("2. Improve grammar and clarity.\n");
    prompt.push_str("3. Keep the meaning unchanged.\n");
    prompt.push_str("4. Do NOT translate. Keep the original language.\n\n");
    prompt.push_str("CRITICAL INSTRUCTIONS:\n");
    prompt.push_str(
        "1. Output newline-delimited JSON (NDJSON) only. Do not wrap the result in a JSON array.\n",
    );
    prompt.push_str("2. Each output line must be one valid JSON object. Do not include markdown formatting like ```json.\n");
    prompt.push_str(
        "3. Return the EXACT SAME 'id' field, and the polished text in the 'text' field.\n",
    );
    prompt.push_str(&format!(
        "4. Do not combine or split segments. There must be exactly {} JSON lines in the output.\n\n",
        segments.len()
    ));
    prompt.push_str("Input:\n");
    prompt.push_str(&json_str);

    prompt
}

pub fn build_polish_task_input(
    segments: &[super::LlmSegmentInput],
    context: Option<&str>,
    keywords: Option<&str>,
) -> String {
    let mut sections = Vec::new();
    if let Some(context) = context.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("User context:\n{}", context.trim()));
    }
    if let Some(keywords) = keywords.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("Preferred terms:\n{}", keywords.trim()));
    }
    sections.push(format!(
        "Polish these segments and return them in an `items` array:\n{}",
        serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string())
    ));
    sections.join("\n\n")
}
