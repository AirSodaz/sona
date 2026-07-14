use super::*;
use async_trait::async_trait;
use serde::Serialize;
use sona_core::llm::runtime::LlmRuntimeService;
use sona_core::llm::runtime::LlmStreamDelta;
use sona_core::llm::tasks::{LlmTaskEvent, LlmTaskObserver, LlmTaskService};
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelMetadataPort, LlmPortError, LlmStreamingPort, LlmTaskDelayPort,
    LlmTranslationPort, LlmTranslationRequest,
};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
struct CommandEventEmitter {
    app: AppHandle,
}

impl CommandEventEmitter {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn emit<T>(&self, event: &str, payload: T) -> Result<(), String>
    where
        T: Serialize + Clone,
    {
        self.app
            .emit(event, payload)
            .map_err(|error| error.to_string())
    }

    fn emit_progress(&self, payload: LlmTaskProgressPayload) -> Result<(), String> {
        self.emit(LLM_TASK_PROGRESS_EVENT, payload)
    }

    fn emit_chunk<T>(&self, payload: LlmTaskChunkPayload<T>) -> Result<(), String>
    where
        T: Serialize + Clone,
    {
        self.emit(LLM_TASK_CHUNK_EVENT, payload)
    }

    fn emit_text(&self, payload: LlmTaskTextPayload) -> Result<(), String> {
        self.emit(LLM_TASK_TEXT_EVENT, payload)
    }
}

#[derive(Clone)]
struct UsageRecorder {
    app: AppHandle,
    config: LlmConfig,
    category: LlmUsageCategory,
}

impl UsageRecorder {
    fn new(app: AppHandle, config: LlmConfig, category: LlmUsageCategory) -> Self {
        Self {
            app,
            config,
            category,
        }
    }

    fn record(&self, response: &StandardLlmResponse) {
        self.record_usage(response.usage.clone());
    }

    fn record_usage(&self, usage: Option<TokenUsage>) {
        let occurred_at = crate::platform::time::utc_now_rfc3339();
        if let Err(error) = crate::platform::llm_usage::record_usage(
            &self.app,
            &UsageRecord {
                occurred_at: occurred_at.clone(),
                provider: self.config.provider.as_str(),
                category: self.category,
                usage: usage.clone(),
            },
        ) {
            log::warn!(
                "[LLM] failed to persist usage: provider={:?} category={:?} error={}",
                self.config.provider,
                self.category,
                error
            );
        }

        emit_llm_usage_event(&self.app, &self.config, self.category, occurred_at, usage);
    }
}

#[derive(Clone)]
struct DesktopTaskRuntime {
    inner: DesktopLlmAdapter,
    usage: UsageRecorder,
}

impl DesktopTaskRuntime {
    fn new(app: AppHandle, config: LlmConfig, category: LlmUsageCategory) -> Self {
        Self {
            inner: DesktopLlmAdapter::default(),
            usage: UsageRecorder::new(app, config, category),
        }
    }
}

#[async_trait]
impl LlmCompletionPort for DesktopTaskRuntime {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let response = self.inner.complete(request).await?;
        self.usage.record(&response);
        Ok(response)
    }
}

#[async_trait]
impl LlmModelMetadataPort for DesktopTaskRuntime {
    async fn describe_model(
        &self,
        config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError> {
        self.inner.describe_model(config).await
    }
}

#[async_trait]
impl LlmStreamingPort for DesktopTaskRuntime {
    async fn stream_completion(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let response = self.inner.stream_completion(request, emit_delta).await?;
        self.usage.record(&response);
        Ok(response)
    }
}

#[async_trait]
impl LlmTaskDelayPort for DesktopTaskRuntime {
    async fn delay(&self, duration: Duration) {
        self.inner.delay(duration).await;
    }
}

#[async_trait]
impl LlmTranslationPort for DesktopTaskRuntime {
    async fn translate_batch(
        &self,
        request: LlmTranslationRequest,
    ) -> Result<Vec<String>, LlmPortError> {
        let translations = self.inner.translate_batch(request).await?;
        self.usage.record_usage(None);
        Ok(translations)
    }
}

type PolishChunkCallback =
    Box<dyn FnMut(&[PolishedSegment]) -> Result<(), String> + Send + 'static>;
type TranslateChunkCallback =
    Box<dyn FnMut(&[TranslatedSegment]) -> Result<(), String> + Send + 'static>;

struct CommandTaskObserver {
    events: CommandEventEmitter,
    polish_chunk: Option<Mutex<PolishChunkCallback>>,
    translate_chunk: Option<Mutex<TranslateChunkCallback>>,
}

impl CommandTaskObserver {
    fn polish<F>(app: AppHandle, on_chunk: F) -> Self
    where
        F: FnMut(&[PolishedSegment]) -> Result<(), String> + Send + 'static,
    {
        Self {
            events: CommandEventEmitter::new(app),
            polish_chunk: Some(Mutex::new(Box::new(on_chunk))),
            translate_chunk: None,
        }
    }

    fn translate<F>(app: AppHandle, on_chunk: F) -> Self
    where
        F: FnMut(&[TranslatedSegment]) -> Result<(), String> + Send + 'static,
    {
        Self {
            events: CommandEventEmitter::new(app),
            polish_chunk: None,
            translate_chunk: Some(Mutex::new(Box::new(on_chunk))),
        }
    }

    fn summary(app: AppHandle) -> Self {
        Self {
            events: CommandEventEmitter::new(app),
            polish_chunk: None,
            translate_chunk: None,
        }
    }
}

impl LlmTaskObserver for CommandTaskObserver {
    fn on_event(&self, event: LlmTaskEvent) -> Result<(), String> {
        match event {
            LlmTaskEvent::Progress(payload) => self.events.emit_progress(payload),
            LlmTaskEvent::PolishChunk(payload) => {
                if let Some(callback) = &self.polish_chunk {
                    let mut callback = callback.lock().map_err(|error| error.to_string())?;
                    callback(&payload.items)?;
                }
                self.events.emit_chunk(payload)
            }
            LlmTaskEvent::TranslateChunk(payload) => {
                if let Some(callback) = &self.translate_chunk {
                    let mut callback = callback.lock().map_err(|error| error.to_string())?;
                    callback(&payload.items)?;
                }
                self.events.emit_chunk(payload)
            }
            LlmTaskEvent::Text(payload) => self.events.emit_text(payload),
            LlmTaskEvent::SummaryChunk(_) | LlmTaskEvent::Completed(_) => Ok(()),
        }
    }
}

pub(super) async fn complete_llm_with_port<P>(
    request: LlmCompletionRequest,
    port: P,
) -> Result<LlmCompletionResponse, String>
where
    P: Clone + LlmCompletionPort + LlmModelMetadataPort,
{
    LlmRuntimeService::new(&port, port.clone())
        .complete(request)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn complete_llm_command(
    app: AppHandle,
    request: LlmCompletionRequest,
) -> Result<LlmCompletionResponse, String> {
    let category = request.source.unwrap_or(LlmGenerateSource::Generic).into();
    let usage = UsageRecorder::new(app, request.config.clone(), category);
    let response = complete_llm_with_port(request, DesktopLlmAdapter::default()).await?;
    usage.record_usage(response.usage.clone());
    Ok(response)
}

pub(crate) async fn generate_llm_text_command(
    app: AppHandle,
    request: LlmGenerateRequest,
) -> Result<String, String> {
    Ok(complete_llm_command(app, request.into()).await?.text)
}

pub(crate) async fn polish_transcript_segments_command(
    app: AppHandle,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    polish_transcript_segments_with_observer(app, request, |_| Ok(())).await
}

pub(super) async fn polish_transcript_segments_with_observer<F>(
    app: AppHandle,
    request: PolishSegmentsRequest,
    on_chunk_items: F,
) -> Result<Vec<PolishedSegment>, String>
where
    F: FnMut(&[PolishedSegment]) -> Result<(), String> + Send + 'static,
{
    let runtime = DesktopTaskRuntime::new(
        app.clone(),
        request.config.clone(),
        LlmUsageCategory::Polish,
    );
    let observer = CommandTaskObserver::polish(app, on_chunk_items);
    LlmTaskService::new(runtime)
        .polish(request, &observer)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn translate_transcript_segments_command(
    app: AppHandle,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    translate_transcript_segments_with_observer(app, request, |_| Ok(())).await
}

pub(super) async fn translate_transcript_segments_with_observer<F>(
    app: AppHandle,
    request: TranslateSegmentsRequest,
    on_chunk_items: F,
) -> Result<Vec<TranslatedSegment>, String>
where
    F: FnMut(&[TranslatedSegment]) -> Result<(), String> + Send + 'static,
{
    let runtime = DesktopTaskRuntime::new(
        app.clone(),
        request.config.clone(),
        LlmUsageCategory::Translation,
    );
    let observer = CommandTaskObserver::translate(app, on_chunk_items);
    LlmTaskService::new(runtime)
        .translate(request, &observer)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn summarize_transcript_command(
    app: AppHandle,
    request: SummarizeTranscriptRequest,
) -> Result<TranscriptSummaryResult, String> {
    let runtime = DesktopTaskRuntime::new(
        app.clone(),
        request.config.clone(),
        LlmUsageCategory::Summary,
    );
    let observer = CommandTaskObserver::summary(app);
    LlmTaskService::new(runtime)
        .summarize(request, &observer)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn list_llm_models_command(
    request: LlmModelsRequest,
) -> Result<Vec<LlmModelSummary>, String> {
    let adapter = DesktopLlmAdapter::default();
    LlmRuntimeService::new(&adapter, adapter)
        .list_models(request)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn describe_llm_model_command(
    config: LlmConfig,
) -> Result<Option<LlmModelSummary>, String> {
    let adapter = DesktopLlmAdapter::default();
    LlmRuntimeService::new(&adapter, adapter)
        .describe_model(&config)
        .await
        .map_err(|error| error.to_string())
}
