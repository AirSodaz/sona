use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::LlmTaskError;

pub fn clean_json_response(response_text: &str) -> String {
    let mut cleaned = response_text.trim().to_string();

    if cleaned.starts_with("```json") {
        cleaned = cleaned[7..].to_string();
    } else if cleaned.starts_with("```") {
        cleaned = cleaned[3..].to_string();
    }

    if cleaned.ends_with("```") {
        cleaned.truncate(cleaned.len() - 3);
    }

    cleaned.trim().to_string()
}

pub fn normalize_incremental_json_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed == "```" || trimmed == "```json" {
        return None;
    }

    let trimmed = trimmed.trim_end_matches(',').trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }

    None
}

pub fn parse_json_array_or_ndjson<T: DeserializeOwned>(
    response_text: &str,
    task_type: super::LlmTaskType,
    chunk_number: usize,
) -> Result<Vec<T>, LlmTaskError> {
    let cleaned = clean_json_response(response_text);
    if cleaned.starts_with('[') {
        return serde_json::from_str::<Vec<T>>(&cleaned).map_err(|error| {
            LlmTaskError::InvalidResponse {
                reason: super::chunk_error(
                    task_type,
                    chunk_number,
                    format!("invalid JSON response: {error}"),
                ),
            }
        });
    }

    let mut items = Vec::new();
    for line in cleaned.lines() {
        if let Some(normalized) = normalize_incremental_json_line(line) {
            let parsed = serde_json::from_str::<T>(&normalized).map_err(|error| {
                LlmTaskError::InvalidResponse {
                    reason: super::chunk_error(
                        task_type,
                        chunk_number,
                        format!("invalid JSON response: {error}"),
                    ),
                }
            })?;
            items.push(parsed);
        }
    }

    if items.is_empty() {
        return Err(LlmTaskError::InvalidResponse {
            reason: super::chunk_error(
                task_type,
                chunk_number,
                "invalid JSON response: expected NDJSON lines or a JSON array",
            ),
        });
    }

    Ok(items)
}

pub fn parse_polish_chunk(
    response_text: &str,
    expected: &[super::LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<super::PolishedSegment>, LlmTaskError> {
    let parsed = parse_json_array_or_ndjson::<super::PolishedSegment>(
        response_text,
        super::LlmTaskType::Polish,
        chunk_number,
    )?;
    super::validate_segment_ids(
        &parsed,
        expected,
        super::LlmTaskType::Polish,
        chunk_number,
        |item| &item.id,
    )?;
    Ok(parsed)
}

pub fn parse_translate_chunk(
    response_text: &str,
    expected: &[super::LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<super::TranslatedSegment>, LlmTaskError> {
    let parsed = parse_json_array_or_ndjson::<super::TranslatedSegment>(
        response_text,
        super::LlmTaskType::Translate,
        chunk_number,
    )?;
    super::validate_segment_ids(
        &parsed,
        expected,
        super::LlmTaskType::Translate,
        chunk_number,
        |item| &item.id,
    )?;
    Ok(parsed)
}

#[derive(Deserialize)]
struct StructuredItems<T> {
    items: Vec<T>,
}

fn items_schema(count: usize, text_field: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["items"],
        "properties": {
            "items": {
                "type": "array",
                "minItems": count,
                "maxItems": count,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", text_field],
                    "properties": {
                        "id": { "type": "string" },
                        (text_field): { "type": "string" }
                    }
                }
            }
        }
    })
}

pub fn polish_output_schema(count: usize) -> Value {
    items_schema(count, "text")
}

pub fn translate_output_schema(count: usize) -> Value {
    items_schema(count, "translation")
}

fn parse_items<T: DeserializeOwned>(value: &Value) -> Result<Vec<T>, LlmTaskError> {
    serde_json::from_value::<StructuredItems<T>>(value.clone())
        .map(|payload| payload.items)
        .map_err(|error| LlmTaskError::InvalidResponse {
            reason: format!("invalid structured response: {error}"),
        })
}

pub fn parse_polish_object(
    value: &Value,
    expected: &[super::LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<super::PolishedSegment>, LlmTaskError> {
    let items = parse_items::<super::PolishedSegment>(value)?;
    super::validate_segment_ids(
        &items,
        expected,
        super::LlmTaskType::Polish,
        chunk_number,
        |item| item.id.as_str(),
    )?;
    Ok(items)
}

pub fn parse_translate_object(
    value: &Value,
    expected: &[super::LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<super::TranslatedSegment>, LlmTaskError> {
    let items = parse_items::<super::TranslatedSegment>(value)?;
    super::validate_segment_ids(
        &items,
        expected,
        super::LlmTaskType::Translate,
        chunk_number,
        |item| item.id.as_str(),
    )?;
    Ok(items)
}

pub fn build_structured_repair_input(
    original_input: &str,
    validation_error: &str,
    previous_response: Option<&str>,
) -> String {
    let mut prompt = format!(
        "The previous response failed validation: {validation_error}\nRegenerate the complete response for the original task and return only a valid JSON object.\n\nOriginal task:\n{original_input}"
    );
    if let Some(response) = previous_response.filter(|value| !value.trim().is_empty()) {
        prompt.push_str("\n\nInvalid response:\n");
        prompt.push_str(response.trim());
    }
    prompt
}
