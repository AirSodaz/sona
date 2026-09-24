use crate::llm::runtime::LlmStreamDelta;
use serde_json::{Value, json};

use crate::llm::provider_protocol::join_url;
use crate::llm::tasks::LlmProviderStrategy;

/// Keeps progressively built streaming text and emits the complete accumulated
/// text together with the latest delta for replacement-style consumers.
pub struct StreamTextAccumulator<'a, EmitFn, EmitError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), EmitError> + Send + ?Sized,
{
    text: String,
    emitted_any: bool,
    emit_delta: &'a mut EmitFn,
}

impl<'a, EmitFn, EmitError> StreamTextAccumulator<'a, EmitFn, EmitError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), EmitError> + Send + ?Sized,
{
    pub fn new(emit_delta: &'a mut EmitFn) -> Self {
        Self {
            text: String::new(),
            emitted_any: false,
            emit_delta,
        }
    }

    pub fn push(&mut self, delta: &str) -> Result<(), EmitError> {
        if delta.is_empty() {
            return Ok(());
        }

        self.text.push_str(delta);
        self.emitted_any = true;
        (self.emit_delta)(&self.text, delta)
    }

    pub fn text(&self) -> String {
        self.text.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn emitted_any(&self) -> bool {
        self.emitted_any
    }
}

/// Manages both thought reasoning text and final content text streams separately,
/// emitting structured `LlmStreamDelta` events for real-time progress.
pub struct DualStreamAccumulator<'a, EmitFn, EmitError>
where
    EmitFn: FnMut(LlmStreamDelta) -> Result<(), EmitError> + Send + ?Sized,
{
    content_text: String,
    thought_text: String,
    emitted_any: bool,
    emit_delta: &'a mut EmitFn,
}

impl<'a, EmitFn, EmitError> DualStreamAccumulator<'a, EmitFn, EmitError>
where
    EmitFn: FnMut(LlmStreamDelta) -> Result<(), EmitError> + Send + ?Sized,
{
    pub fn new(emit_delta: &'a mut EmitFn) -> Self {
        Self {
            content_text: String::new(),
            thought_text: String::new(),
            emitted_any: false,
            emit_delta,
        }
    }

    pub fn push_content(&mut self, delta: &str) -> Result<(), EmitError> {
        if delta.is_empty() {
            return Ok(());
        }
        self.content_text.push_str(delta);
        self.emitted_any = true;
        (self.emit_delta)(LlmStreamDelta::content(&self.content_text, delta))
    }

    pub fn push_thought(&mut self, delta: &str) -> Result<(), EmitError> {
        if delta.is_empty() {
            return Ok(());
        }
        self.thought_text.push_str(delta);
        self.emitted_any = true;
        (self.emit_delta)(LlmStreamDelta::thought(&self.thought_text, delta))
    }

    pub fn content_text(&self) -> &str {
        &self.content_text
    }

    pub fn thought_text(&self) -> &str {
        &self.thought_text
    }

    pub fn is_empty(&self) -> bool {
        self.content_text.is_empty() && self.thought_text.is_empty()
    }

    pub fn emitted_any(&self) -> bool {
        self.emitted_any
    }
}

pub fn is_temperature_prohibited_for_model(model: &str) -> bool {
    let lower = model.to_lowercase();
    let trimmed = lower.trim();
    let core_model = trimmed.rsplit('/').next().unwrap_or(trimmed);
    core_model.starts_with("o1")
        || core_model.starts_with("o3")
        || core_model.starts_with("o4")
        || core_model.contains("deepseek-reasoner")
        || core_model.contains("deepseek-r1")
}
pub fn requires_max_completion_tokens(model: &str) -> bool {
    let lower = model.to_lowercase();
    let trimmed = lower.trim();
    let core_model = trimmed.rsplit('/').next().unwrap_or(trimmed);
    core_model.starts_with("o1")
        || core_model.starts_with("o3")
        || core_model.starts_with("o4")
        || core_model.starts_with("gpt-5")
}

/// Reassembles transport chunks into complete lines before higher-level
/// streaming parsers inspect them.
#[derive(Default)]
pub struct StreamingLineBuffer {
    buffer: String,
}

impl StreamingLineBuffer {
    pub fn process(&mut self, chunk: &str) -> Vec<String> {
        if chunk.find('\n').is_none() {
            self.buffer.push_str(chunk);
            return Vec::new();
        }

        self.buffer.push_str(chunk);
        let mut lines = self
            .buffer
            .split('\n')
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        self.buffer = lines.pop().unwrap_or_default();
        lines
    }

    pub fn flush(&mut self) -> Vec<String> {
        if self.buffer.trim().is_empty() {
            self.buffer.clear();
            return Vec::new();
        }

        let line = self.buffer.clone();
        self.buffer.clear();
        vec![line]
    }
}

/// Collects SSE `data:` lines and emits one logical event per blank-line
/// separator. Other SSE fields are ignored because Sona currently consumes the
/// payload carried in `data:`.
#[derive(Default)]
pub struct SseEventBuffer {
    line_buffer: StreamingLineBuffer,
    data_lines: Vec<String>,
}

impl SseEventBuffer {
    pub fn process(&mut self, chunk: &str) -> Vec<String> {
        let mut events = Vec::new();
        for line in self.line_buffer.process(chunk) {
            self.process_line(&line, &mut events);
        }
        events
    }

    pub fn flush(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        for line in self.line_buffer.flush() {
            self.process_line(&line, &mut events);
        }

        if !self.data_lines.is_empty() {
            events.push(self.data_lines.join("\n"));
            self.data_lines.clear();
        }

        events
    }

    fn process_line(&mut self, raw_line: &str, events: &mut Vec<String>) {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            if !self.data_lines.is_empty() {
                events.push(self.data_lines.join("\n"));
                self.data_lines.clear();
            }
            return;
        }

        if let Some(rest) = line.strip_prefix("data:") {
            self.data_lines.push(rest.trim_start().to_string());
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OpenAiStreamUrlConfig<'a> {
    pub strategy: LlmProviderStrategy,
    pub base_url: &'a str,
    pub model: &'a str,
    pub api_path: Option<&'a str>,
    pub api_version: Option<&'a str>,
}

pub fn build_openai_stream_url(config: OpenAiStreamUrlConfig<'_>) -> String {
    match config.strategy {
        LlmProviderStrategy::AzureOpenAi => {
            let version = config.api_version.unwrap_or("2024-10-21");
            format!(
                "{}/openai/deployments/{}/chat/completions?api-version={}",
                config.base_url.trim_end_matches('/'),
                config.model.trim(),
                version
            )
        }
        LlmProviderStrategy::Perplexity => join_url(
            config.base_url,
            config.api_path.unwrap_or("/chat/completions"),
        ),
        _ => join_url(
            config.base_url,
            config.api_path.unwrap_or("/v1/chat/completions"),
        ),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OpenAiChatPayloadConfig<'a> {
    pub strategy: LlmProviderStrategy,
    pub model: &'a str,
    pub temperature: Option<f32>,
    pub reasoning_enabled: bool,
    pub reasoning_level: Option<&'a str>,
}

pub fn build_openai_chat_payload(
    config: OpenAiChatPayloadConfig<'_>,
    input: &str,
    stream: bool,
) -> Value {
    let mut payload = if config.strategy == LlmProviderStrategy::AzureOpenAi {
        json!({
            "messages": [
                {
                    "role": "user",
                    "content": input,
                }
            ],
        })
    } else {
        json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": input,
                }
            ],
        })
    };

    if stream {
        payload["stream"] = json!(true);
        if config.strategy != LlmProviderStrategy::AzureOpenAi {
            payload["stream_options"] = json!({"include_usage": true});
        }
    }

    if !is_temperature_prohibited_for_model(config.model) {
        payload["temperature"] = json!(config.temperature.unwrap_or(0.7));
    }

    if config.reasoning_enabled
        && let Some(level) = config.reasoning_level
    {
        let thinking =
            crate::llm::runtime::ThinkingLevel::from_legacy_options(Some(true), Some(level));
        let lower = config.model.to_lowercase();
        let core_model = lower.trim().rsplit('/').next().unwrap_or(&lower);
        let is_openai_o_series = core_model.starts_with("o1")
            || core_model.starts_with("o3")
            || core_model.starts_with("o4");
        let clamped = if is_openai_o_series {
            match thinking {
                crate::llm::runtime::ThinkingLevel::None
                | crate::llm::runtime::ThinkingLevel::Auto => thinking,
                _ => thinking
                    .clamp_to_supported(&[
                        crate::llm::runtime::ThinkingLevel::Low,
                        crate::llm::runtime::ThinkingLevel::Medium,
                        crate::llm::runtime::ThinkingLevel::High,
                    ])
                    .unwrap_or(crate::llm::runtime::ThinkingLevel::Medium),
            }
        } else {
            thinking
        };

        if let Some(effort) = clamped.as_effort_str() {
            payload["reasoning_effort"] = json!(effort);
        }
    }

    payload
}
