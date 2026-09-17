use serde::{Deserialize, Serialize};

#[cfg(feature = "specta")]
use specta::Type;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum PolishMode {
    Verbatim,
    #[default]
    Clean,
    Formal,
}

impl PolishMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verbatim => "verbatim",
            Self::Clean => "clean",
            Self::Formal => "formal",
        }
    }

    pub fn system_prompt(&self) -> &'static str {
        match self {
            Self::Verbatim => POLISH_VERBATIM_SYSTEM_PROMPT,
            Self::Clean => POLISH_CLEAN_SYSTEM_PROMPT,
            Self::Formal => POLISH_FORMAL_SYSTEM_PROMPT,
        }
    }

    pub fn task_instruction(&self) -> &'static str {
        match self {
            Self::Verbatim => {
                "Mode: Verbatim. Correct ASR recognition errors and punctuation only. Preserve every filler word, stutter, and repetition exactly as spoken. Do not rephrase or omit words."
            }
            Self::Clean => {
                "Mode: Clean spoken. Remove conversational filler words, stutters, and verbal clutter. Keep original meaning, phrasing, and factual content intact."
            }
            Self::Formal => {
                "Mode: Formal written. Restructure fragmented oral sentences into concise, grammatical written text. Fix syntax; do not invent information."
            }
        }
    }
}

pub const POLISH_VERBATIM_SYSTEM_PROMPT: &str = "You proofread speech-to-text segments. Fix ASR errors and punctuation while preserving every verbal detail, filler word, and stutter. Return only one JSON object matching the schema.";
pub const POLISH_CLEAN_SYSTEM_PROMPT: &str = "You edit speech-to-text segments. Remove conversational filler words, stutters, and verbal clutter while preserving original meaning, phrasing, and facts. Return only one JSON object matching the schema.";
pub const POLISH_FORMAL_SYSTEM_PROMPT: &str = "You rewrite speech-to-text segments into grammatically sound written text. Restructure fragmented oral sentences without fabricating facts. Return only one JSON object matching the schema.";
pub const POLISH_SYSTEM_PROMPT: &str = POLISH_CLEAN_SYSTEM_PROMPT;

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
    mode: PolishMode,
) -> String {
    let mut sections = Vec::new();
    sections.push(mode.task_instruction().to_string());

    if let Some(context) = context.filter(|value| !value.trim().is_empty()) {
        sections.push(format!(
            "Context (reference only; do not alter editing mode):\n{}",
            context.trim()
        ));
    }

    sections.push(format!(
        "Polish these segments and return them in an `items` array:\n{}",
        serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string())
    ));
    sections.join("\n\n")
}
