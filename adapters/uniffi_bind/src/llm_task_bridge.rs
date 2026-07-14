use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use sona_core::llm::requests::{
    PolishSegmentsRequest, SummarizeTranscriptRequest, TranslateSegmentsRequest,
};
use sona_core::llm::runtime::LlmRuntimeError;
use sona_core::llm::tasks::{
    LlmTaskError, LlmTaskEvent, LlmTaskObserver, LlmTaskResult, LlmTaskService,
};
use sona_core::ports::llm::LlmTaskRuntimePort;
use sona_online_llm::OnlineLlmAdapter;

use crate::json_bridge::parse_core_json;
use crate::llm_runtime_bridge::port_error_code;
use crate::mapper::{
    FfiLlmTaskChunk, FfiLlmTaskFinal, FfiLlmTaskProgress, FfiLlmTaskText,
    llm_task_items_chunk_to_ffi, llm_task_progress_to_ffi, llm_task_result_to_ffi,
    llm_task_summary_chunk_to_ffi, llm_task_text_to_ffi,
};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};

#[uniffi::export(foreign)]
pub trait FfiLlmTaskObserver: Send + Sync {
    fn on_progress(&self, event: FfiLlmTaskProgress);
    fn on_chunk(&self, event: FfiLlmTaskChunk);
    fn on_text(&self, event: FfiLlmTaskText);
    fn on_final(&self, event: FfiLlmTaskFinal);
}

struct FfiLlmTaskObserverAdapter {
    task_id: String,
    observer: Arc<dyn FfiLlmTaskObserver>,
}

impl FfiLlmTaskObserverAdapter {
    fn new(task_id: String, observer: Arc<dyn FfiLlmTaskObserver>) -> Self {
        Self { task_id, observer }
    }

    fn notify(&self, callback: impl FnOnce(&dyn FfiLlmTaskObserver)) -> Result<(), String> {
        catch_unwind(AssertUnwindSafe(|| callback(self.observer.as_ref())))
            .map_err(|_| "LLM task observer callback panicked".to_string())
    }
}

impl LlmTaskObserver for FfiLlmTaskObserverAdapter {
    fn on_event(&self, event: LlmTaskEvent) -> Result<(), String> {
        match event {
            LlmTaskEvent::Progress(payload) => {
                let event = llm_task_progress_to_ffi(payload);
                self.notify(move |observer| observer.on_progress(event))
            }
            LlmTaskEvent::PolishChunk(payload) => {
                let event = llm_task_items_chunk_to_ffi(payload)
                    .map_err(|error| format!("Failed to serialize polish chunk: {error}"))?;
                self.notify(move |observer| observer.on_chunk(event))
            }
            LlmTaskEvent::TranslateChunk(payload) => {
                let event = llm_task_items_chunk_to_ffi(payload)
                    .map_err(|error| format!("Failed to serialize translation chunk: {error}"))?;
                self.notify(move |observer| observer.on_chunk(event))
            }
            LlmTaskEvent::SummaryChunk(payload) => {
                let event = llm_task_summary_chunk_to_ffi(payload);
                self.notify(move |observer| observer.on_chunk(event))
            }
            LlmTaskEvent::Text(payload) => {
                let event = llm_task_text_to_ffi(payload);
                self.notify(move |observer| observer.on_text(event))
            }
            LlmTaskEvent::Completed(result) => {
                let event = llm_task_result_to_ffi(self.task_id.clone(), result)
                    .map_err(|error| format!("Failed to serialize LLM task result: {error}"))?;
                self.notify(move |observer| observer.on_final(event))
            }
        }
    }
}

pub(crate) async fn run_llm_polish_json(
    request_json: String,
    observer: Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal> {
    run_polish_with_runtime(&request_json, OnlineLlmAdapter, observer).await
}

pub(crate) async fn run_llm_translate_json(
    request_json: String,
    observer: Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal> {
    run_translate_with_runtime(&request_json, OnlineLlmAdapter, observer).await
}

pub(crate) async fn run_llm_summary_json(
    request_json: String,
    observer: Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal> {
    run_summary_with_runtime(&request_json, OnlineLlmAdapter, observer).await
}

async fn run_polish_with_runtime<Runtime>(
    request_json: &str,
    runtime: Runtime,
    observer: Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal>
where
    Runtime: LlmTaskRuntimePort,
{
    let request: PolishSegmentsRequest = parse_core_json(request_json, "polish segments request")?;
    let task_id = request.task_id.clone();
    let observer = FfiLlmTaskObserverAdapter::new(task_id.clone(), observer);
    let result = LlmTaskService::new(runtime)
        .polish(request, &observer)
        .await
        .map_err(map_task_error)?;
    map_final_result(task_id, LlmTaskResult::Polish(result))
}

async fn run_translate_with_runtime<Runtime>(
    request_json: &str,
    runtime: Runtime,
    observer: Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal>
where
    Runtime: LlmTaskRuntimePort,
{
    let request: TranslateSegmentsRequest =
        parse_core_json(request_json, "translate segments request")?;
    let task_id = request.task_id.clone();
    let observer = FfiLlmTaskObserverAdapter::new(task_id.clone(), observer);
    let result = LlmTaskService::new(runtime)
        .translate(request, &observer)
        .await
        .map_err(map_task_error)?;
    map_final_result(task_id, LlmTaskResult::Translate(result))
}

async fn run_summary_with_runtime<Runtime>(
    request_json: &str,
    runtime: Runtime,
    observer: Arc<dyn FfiLlmTaskObserver>,
) -> SonaCoreBindingResult<FfiLlmTaskFinal>
where
    Runtime: LlmTaskRuntimePort,
{
    let request: SummarizeTranscriptRequest =
        parse_core_json(request_json, "summarize transcript request")?;
    let task_id = request.task_id.clone();
    let observer = FfiLlmTaskObserverAdapter::new(task_id.clone(), observer);
    let result = LlmTaskService::new(runtime)
        .summarize(request, &observer)
        .await
        .map_err(map_task_error)?;
    map_final_result(task_id, LlmTaskResult::Summary(result))
}

fn map_final_result(
    task_id: String,
    result: LlmTaskResult,
) -> SonaCoreBindingResult<FfiLlmTaskFinal> {
    llm_task_result_to_ffi(task_id, result).map_err(|error| SonaCoreBindingError::LlmRuntime {
        code: "invalid_response".to_string(),
        reason: format!("Failed to serialize LLM task result: {error}"),
        retry_after_ms: None,
    })
}

fn map_task_error(error: LlmTaskError) -> SonaCoreBindingError {
    let reason = error.to_string();
    let (code, retry_after_ms) = match &error {
        LlmTaskError::InvalidRequest { .. } => ("invalid_request", None),
        LlmTaskError::InvalidResponse { .. } => ("invalid_response", None),
        LlmTaskError::Observer { .. } => ("observer", None),
        LlmTaskError::Runtime { source, .. } => runtime_error_metadata(source),
    };
    SonaCoreBindingError::LlmRuntime {
        code: code.to_string(),
        reason,
        retry_after_ms,
    }
}

fn runtime_error_metadata(error: &LlmRuntimeError) -> (&'static str, Option<u64>) {
    match error {
        LlmRuntimeError::InvalidRequest { .. } => ("invalid_request", None),
        LlmRuntimeError::UnsupportedCapability { .. } => ("unsupported_capability", None),
        LlmRuntimeError::InvalidResponse { .. } => ("invalid_response", None),
        LlmRuntimeError::Adapter {
            kind,
            retry_after_ms,
            ..
        } => (port_error_code(*kind), *retry_after_ms),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
    use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
    use sona_core::llm::requests::LlmConfig;
    use sona_core::llm::runtime::{LlmCompletionRequest, LlmStreamDelta};
    use sona_core::llm::tasks::{LlmProviderStrategy, LlmSegmentInput, PolishedSegment};
    use sona_core::ports::llm::{
        LlmCompletionPort, LlmModelMetadataPort, LlmPortError, LlmStreamingPort, LlmTaskDelayPort,
        LlmTranslationPort, LlmTranslationRequest,
    };

    use super::*;

    #[derive(Clone, Default)]
    struct FakeRuntime {
        request: Arc<Mutex<Option<LlmCompletionRequest>>>,
    }

    #[async_trait]
    impl LlmCompletionPort for FakeRuntime {
        async fn complete(
            &self,
            request: LlmCompletionRequest,
        ) -> Result<StandardLlmResponse, LlmPortError> {
            *self.request.lock().unwrap() = Some(request);
            Ok(StandardLlmResponse {
                text: r#"{"items":[{"id":"segment-1","text":"clean"}]}"#.to_string(),
                usage: None,
            })
        }
    }

    #[async_trait]
    impl LlmModelMetadataPort for FakeRuntime {
        async fn describe_model(
            &self,
            _config: &LlmConfig,
        ) -> Result<Option<LlmModelSummary>, LlmPortError> {
            Ok(Some(LlmModelSummary {
                supports_structured_output: Some(true),
                ..LlmModelSummary::default()
            }))
        }
    }

    #[async_trait]
    impl LlmStreamingPort for FakeRuntime {
        async fn stream_completion(
            &self,
            request: LlmCompletionRequest,
            _emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
        ) -> Result<StandardLlmResponse, LlmPortError> {
            self.complete(request).await
        }
    }

    #[async_trait]
    impl LlmTaskDelayPort for FakeRuntime {
        async fn delay(&self, _duration: Duration) {}
    }

    #[async_trait]
    impl LlmTranslationPort for FakeRuntime {
        async fn translate_batch(
            &self,
            request: LlmTranslationRequest,
        ) -> Result<Vec<String>, LlmPortError> {
            Ok(request.texts)
        }
    }

    #[derive(Default)]
    struct RecordingObserver {
        events: Mutex<Vec<&'static str>>,
    }

    impl FfiLlmTaskObserver for RecordingObserver {
        fn on_progress(&self, _event: FfiLlmTaskProgress) {
            self.events.lock().unwrap().push("progress");
        }

        fn on_chunk(&self, _event: FfiLlmTaskChunk) {
            self.events.lock().unwrap().push("chunk");
        }

        fn on_text(&self, _event: FfiLlmTaskText) {
            self.events.lock().unwrap().push("text");
        }

        fn on_final(&self, _event: FfiLlmTaskFinal) {
            self.events.lock().unwrap().push("final");
        }
    }

    fn config() -> LlmConfig {
        LlmConfig {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
            strategy: LlmProviderStrategy::OpenAi,
            base_url: "https://api.example.com".to_string(),
            api_key: "secret".to_string(),
            model: "test-model".to_string(),
            api_path: None,
            api_version: None,
            temperature: None,
            reasoning_enabled: None,
            reasoning_level: None,
            timeout_seconds: None,
        }
    }

    #[tokio::test]
    async fn polish_maps_request_and_observer_events() {
        let runtime = FakeRuntime::default();
        let observer = Arc::new(RecordingObserver::default());
        let request = PolishSegmentsRequest {
            task_id: "task-1".to_string(),
            config: config(),
            segments: vec![LlmSegmentInput {
                id: "segment-1".to_string(),
                text: "raw".to_string(),
            }],
            chunk_size: Some(1),
            context: Some("meeting".to_string()),
            keywords: None,
        };

        let result = run_polish_with_runtime(
            &serde_json::to_string(&request).unwrap(),
            runtime.clone(),
            observer.clone(),
        )
        .await
        .unwrap();

        let items: Vec<PolishedSegment> = serde_json::from_str(&result.result_json).unwrap();
        let mapped = runtime.request.lock().unwrap().clone().unwrap();
        assert_eq!(
            (items[0].text.as_str(), mapped.config.model.as_str()),
            ("clean", "test-model")
        );
        assert!(mapped.input.contains("meeting") && mapped.input.contains("raw"));
        assert_eq!(
            *observer.events.lock().unwrap(),
            ["chunk", "progress", "final"]
        );
    }
}
