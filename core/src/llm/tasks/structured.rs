use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::LlmTaskError;

pub fn clean_json_response(response_text: &str) -> String {
    let trimmed = response_text.trim();
    if let Some(start) = trimmed.find("```json") {
        let content_start = start + 7;
        if let Some(end) = trimmed[content_start..].find("```") {
            return trimmed[content_start..content_start + end]
                .trim()
                .to_string();
        }
        return trimmed[content_start..].trim().to_string();
    } else if let Some(start) = trimmed.find("```") {
        let content_start = start + 3;
        if let Some(end) = trimmed[content_start..].find("```") {
            return trimmed[content_start..content_start + end]
                .trim()
                .to_string();
        }
        return trimmed[content_start..].trim().to_string();
    }

    let mut cleaned = trimmed.to_string();
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
    if trimmed.starts_with('{') {
        if trimmed.ends_with('}') {
            return Some(trimmed.to_string());
        }
        let repaired = super::fix_json(trimmed);
        if repaired.ends_with('}') {
            return Some(repaired);
        }
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
        if let Ok(parsed) = serde_json::from_str::<Vec<T>>(&cleaned) {
            return Ok(parsed);
        }
        let repaired = super::fix_json(&cleaned);
        if let Ok(parsed) = serde_json::from_str::<Vec<T>>(&repaired) {
            return Ok(parsed);
        }
        return Err(LlmTaskError::InvalidResponse {
            reason: super::chunk_error(
                task_type,
                chunk_number,
                "invalid JSON response: failed to parse JSON array",
            ),
        });
    }

    let mut items = Vec::new();
    let mut any_line_parsed = false;
    for line in cleaned.lines() {
        if let Some(normalized) = normalize_incremental_json_line(line) {
            if let Ok(parsed) = serde_json::from_str::<T>(&normalized) {
                items.push(parsed);
                any_line_parsed = true;
            } else {
                let repaired = super::fix_json(&normalized);
                if let Ok(parsed) = serde_json::from_str::<T>(&repaired) {
                    items.push(parsed);
                    any_line_parsed = true;
                }
            }
        }
    }

    if any_line_parsed && !items.is_empty() {
        return Ok(items);
    }

    // Fallback: try repairing the entire cleaned string as an array or single item
    let repaired = super::fix_json(&cleaned);
    if let Ok(parsed) = serde_json::from_str::<Vec<T>>(&repaired) {
        return Ok(parsed);
    }
    if let Ok(single) = serde_json::from_str::<T>(&repaired) {
        return Ok(vec![single]);
    }

    Err(LlmTaskError::InvalidResponse {
        reason: super::chunk_error(
            task_type,
            chunk_number,
            "invalid JSON response: expected NDJSON lines or a JSON array",
        ),
    })
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

#[derive(Deserialize)]
#[serde(untagged)]
enum FlexibleSegmentsPayload<T> {
    Items { items: Vec<T> },
    Segments { segments: Vec<T> },
    Data { data: Vec<T> },
    Array(Vec<T>),
}

fn items_schema(_count: usize, text_field: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["items"],
        "properties": {
            "items": {
                "type": "array",
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
    serde_json::to_value(schemars::schema_for!(super::PolishedSegmentsBatch))
        .unwrap_or_else(|_| items_schema(count, "text"))
}

pub fn translate_output_schema(count: usize) -> Value {
    serde_json::to_value(schemars::schema_for!(super::TranslatedSegmentsBatch))
        .unwrap_or_else(|_| items_schema(count, "translation"))
}

fn parse_items<T: DeserializeOwned>(value: &Value) -> Result<Vec<T>, LlmTaskError> {
    if let Value::String(s) = value {
        let cleaned = clean_json_response(s);
        if let Ok(val) = serde_json::from_str::<Value>(&cleaned) {
            return parse_items(&val);
        }
    }

    if let Ok(payload) = serde_json::from_value::<FlexibleSegmentsPayload<T>>(value.clone()) {
        let items = match payload {
            FlexibleSegmentsPayload::Items { items } => items,
            FlexibleSegmentsPayload::Segments { segments } => segments,
            FlexibleSegmentsPayload::Data { data } => data,
            FlexibleSegmentsPayload::Array(array) => array,
        };
        return Ok(items);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::tasks::PolishedSegment;

    #[test]
    fn clean_json_response_handles_conversational_wrapping() {
        let text = "Here is your JSON:\n```json\n[{\"id\":\"1\",\"text\":\"hello\"}]\n```\nHope this helps!";
        assert_eq!(
            clean_json_response(text),
            "[{\"id\":\"1\",\"text\":\"hello\"}]"
        );
    }

    #[test]
    fn parse_json_array_repairs_unclosed_bracket_or_trailing_comma() {
        // Truncated array missing closing bracket:
        let truncated = "[{\"id\":\"1\",\"text\":\"hello\"},{\"id\":\"2\",\"text\":\"world\"}";
        let items: Vec<PolishedSegment> =
            parse_json_array_or_ndjson(truncated, crate::llm::tasks::LlmTaskType::Polish, 1)
                .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "1");
        assert_eq!(items[1].text, "world");
    }

    #[test]
    fn parse_json_array_repairs_truncated_ndjson_line() {
        let ndjson = "{\"id\":\"1\",\"text\":\"hello\"}\n{\"id\":\"2\",\"text\":\"world\"";
        let items: Vec<PolishedSegment> =
            parse_json_array_or_ndjson(ndjson, crate::llm::tasks::LlmTaskType::Polish, 1).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].id, "2");
    }
}
