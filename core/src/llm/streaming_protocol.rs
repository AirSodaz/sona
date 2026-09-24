use crate::llm::runtime::LlmStreamDelta;

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
