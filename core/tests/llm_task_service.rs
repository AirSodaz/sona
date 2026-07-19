use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use sona_core::llm::requests::{
    LlmConfig, PolishSegmentsRequest, SummarizeTranscriptRequest, TranslateSegmentsRequest,
};
use sona_core::llm::runtime::{
    LlmCompletionRequest, LlmPromptCachePolicy, LlmResponseFormat, LlmRuntimeError, LlmStreamDelta,
};
use sona_core::llm::tasks::{
    LlmProviderStrategy, LlmSegmentInput, LlmTaskBudget, LlmTaskError, LlmTaskEvent,
    LlmTaskObserver, LlmTaskObserverError, LlmTaskService, SummarySegmentInput,
    SummaryTemplateConfig, llm_task_retry_delay, parse_polish_chunk,
    plan_segment_chunks_with_budget, resolve_task_budget,
};
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelMetadataPort, LlmPortError, LlmPortErrorKind, LlmStreamingPort,
    LlmTaskDelayPort, LlmTranslationPort, LlmTranslationRequest,
};

#[derive(Clone)]
struct FakeRuntime {
    state: Arc<FakeState>,
}

struct FakeState {
    queued: Mutex<VecDeque<Result<StandardLlmResponse, LlmPortError>>>,
    requests: Mutex<Vec<LlmCompletionRequest>>,
    delays: Mutex<Vec<Duration>>,
    active: AtomicUsize,
    max_active: AtomicUsize,
    translation_requests: Mutex<Vec<LlmTranslationRequest>>,
    stream_attempts: AtomicUsize,
    stream_failures_after_delta: AtomicUsize,
    model: LlmModelSummary,
}

impl FakeRuntime {
    fn new(queued: Vec<Result<StandardLlmResponse, LlmPortError>>) -> Self {
        Self::with_model(
            queued,
            LlmModelSummary {
                model: "test-model".to_string(),
                context_window: Some(32_000),
                max_output_tokens: Some(4096),
                supports_structured_output: Some(true),
                ..LlmModelSummary::default()
            },
        )
    }

    fn with_model(
        queued: Vec<Result<StandardLlmResponse, LlmPortError>>,
        model: LlmModelSummary,
    ) -> Self {
        Self {
            state: Arc::new(FakeState {
                queued: Mutex::new(queued.into()),
                requests: Mutex::new(Vec::new()),
                delays: Mutex::new(Vec::new()),
                active: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                translation_requests: Mutex::new(Vec::new()),
                stream_attempts: AtomicUsize::new(0),
                stream_failures_after_delta: AtomicUsize::new(0),
                model,
            }),
        }
    }

    fn with_stream_failures(self, failures: usize) -> Self {
        self.state
            .stream_failures_after_delta
            .store(failures, Ordering::SeqCst);
        self
    }

    fn requests(&self) -> Vec<LlmCompletionRequest> {
        self.state.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmCompletionPort for FakeRuntime {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        self.state.requests.lock().unwrap().push(request.clone());
        let active = self.state.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.state.max_active.fetch_max(active, Ordering::SeqCst);
        tokio::task::yield_now().await;

        let queued = self.state.queued.lock().unwrap().pop_front();
        let result = queued.unwrap_or_else(|| Ok(dynamic_response(&request)));
        self.state.active.fetch_sub(1, Ordering::SeqCst);
        result
    }
}

#[async_trait]
impl LlmModelMetadataPort for FakeRuntime {
    async fn describe_model(
        &self,
        _config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError> {
        Ok(Some(self.state.model.clone()))
    }
}

#[async_trait]
impl LlmStreamingPort for FakeRuntime {
    async fn stream_completion(
        &self,
        _request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let attempt = self.state.stream_attempts.fetch_add(1, Ordering::SeqCst);
        if attempt
            < self
                .state
                .stream_failures_after_delta
                .load(Ordering::SeqCst)
        {
            emit_delta(LlmStreamDelta {
                text: "partial".to_string(),
                delta: "partial".to_string(),
            })?;
            return Err(LlmPortError::new(
                LlmPortErrorKind::Network,
                "stream interrupted",
            ));
        }
        emit_delta(LlmStreamDelta {
            text: "final summary".to_string(),
            delta: "final summary".to_string(),
        })?;
        Ok(StandardLlmResponse {
            text: "final summary".to_string(),
            usage: None,
        })
    }
}

#[async_trait]
impl LlmTranslationPort for FakeRuntime {
    async fn translate_batch(
        &self,
        request: LlmTranslationRequest,
    ) -> Result<Vec<String>, LlmPortError> {
        self.state
            .translation_requests
            .lock()
            .unwrap()
            .push(request.clone());
        Ok(request
            .texts
            .into_iter()
            .map(|text| format!("translated {text}"))
            .collect())
    }
}

#[async_trait]
impl LlmTaskDelayPort for FakeRuntime {
    async fn delay(&self, duration: Duration) {
        self.state.delays.lock().unwrap().push(duration);
    }
}

#[derive(Default)]
struct RecordingObserver(Mutex<Vec<LlmTaskEvent>>);

impl LlmTaskObserver for RecordingObserver {
    fn on_event(&self, event: LlmTaskEvent) -> Result<(), LlmTaskObserverError> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}

struct FailingObserver;

impl LlmTaskObserver for FailingObserver {
    fn on_event(&self, _event: LlmTaskEvent) -> Result<(), LlmTaskObserverError> {
        Err(LlmTaskObserverError {
            reason: "observer unavailable".to_string(),
        })
    }
}

fn config() -> LlmConfig {
    LlmConfig {
        provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        strategy: LlmProviderStrategy::OpenAi,
        base_url: "https://example.com".to_string(),
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

fn segments(count: usize) -> Vec<LlmSegmentInput> {
    (1..=count)
        .map(|index| LlmSegmentInput {
            id: format!("s{index}"),
            text: format!("text {index}"),
        })
        .collect()
}

fn dynamic_response(request: &LlmCompletionRequest) -> StandardLlmResponse {
    let text = match &request.options.response_format {
        LlmResponseFormat::JsonSchema { name, .. } if name == "polished_segments" => {
            let items = segments(8)
                .into_iter()
                .filter(|segment| request.input.contains(&format!(r#""id":"{}""#, segment.id)))
                .map(|segment| serde_json::json!({"id": segment.id, "text": segment.text}))
                .collect::<Vec<_>>();
            serde_json::json!({"items": items}).to_string()
        }
        LlmResponseFormat::JsonSchema { .. } => {
            let items = segments(8)
                .into_iter()
                .filter(|segment| request.input.contains(&format!(r#""id":"{}""#, segment.id)))
                .map(|segment| {
                    serde_json::json!({"id": segment.id, "translation": format!("translated {}", segment.text)})
                })
                .collect::<Vec<_>>();
            serde_json::json!({"items": items}).to_string()
        }
        LlmResponseFormat::Text => "partial summary".to_string(),
        LlmResponseFormat::JsonObject => unreachable!(),
    };
    StandardLlmResponse { text, usage: None }
}

#[test]
fn budgets_and_retry_policy_use_model_limits() {
    let model = LlmModelSummary {
        model: "small".to_string(),
        context_window: Some(1000),
        max_output_tokens: Some(200),
        ..LlmModelSummary::default()
    };
    let budget = resolve_task_budget(Some(&model), 5000, 300);
    assert_eq!(
        (budget.prompt_char_budget, budget.max_output_tokens),
        (700, Some(200))
    );

    let rate_limit = LlmRuntimeError::Adapter {
        kind: LlmPortErrorKind::RateLimited,
        reason: "slow down".to_string(),
        retry_after_ms: Some(9000),
    };
    assert_eq!(
        llm_task_retry_delay(&rate_limit, 1),
        Some(Duration::from_secs(5))
    );
    assert_eq!(llm_task_retry_delay(&rate_limit, 3), None);

    let oversized = vec![LlmSegmentInput {
        id: "large".to_string(),
        text: "x".repeat(1000),
    }];
    let error = plan_segment_chunks_with_budget(
        &oversized,
        None,
        LlmTaskBudget {
            prompt_char_budget: 10_000,
            prompt_token_budget: Some(10_000),
            max_output_tokens: Some(100),
        },
        |items| serde_json::to_string(items).unwrap(),
    )
    .unwrap_err();
    assert!(matches!(error, LlmTaskError::InvalidRequest { .. }));
    assert_eq!(
        error.to_string(),
        "Segment 'large' exceeds the model context or output budget"
    );

    let invalid_response = parse_polish_chunk(
        "not-json",
        &[LlmSegmentInput {
            id: "s1".to_string(),
            text: "hello".to_string(),
        }],
        2,
    )
    .unwrap_err();
    assert!(matches!(
        invalid_response,
        LlmTaskError::InvalidResponse { .. }
    ));
    assert!(
        invalid_response
            .to_string()
            .contains("polish chunk 2 failed")
    );
}

#[tokio::test]
async fn observer_failure_preserves_reason_and_maps_to_task_error() {
    let error = LlmTaskService::new(FakeRuntime::new(Vec::new()))
        .polish(
            PolishSegmentsRequest {
                task_id: "observer-failure".to_string(),
                config: config(),
                segments: Vec::new(),
                chunk_size: None,
                context: None,
                keywords: None,
            },
            &FailingObserver,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        LlmTaskError::Observer { ref reason } if reason == "observer unavailable"
    ));
    assert_eq!(
        error.to_string(),
        "LLM task observer failed: observer unavailable"
    );
}

#[tokio::test]
async fn polish_repairs_one_invalid_structured_response() {
    let fake = FakeRuntime::new(vec![
        Err(LlmPortError {
            kind: LlmPortErrorKind::RateLimited,
            message: "slow down".to_string(),
            retry_after_ms: Some(9000),
        }),
        Ok(StandardLlmResponse {
            text: serde_json::json!({"items": [{"id": "wrong", "text": "fixed"}]}).to_string(),
            usage: None,
        }),
        Ok(StandardLlmResponse {
            text: serde_json::json!({"items": [{"id": "s1", "text": "fixed"}]}).to_string(),
            usage: None,
        }),
    ]);
    let result = LlmTaskService::new(fake.clone())
        .polish(
            PolishSegmentsRequest {
                task_id: "polish".to_string(),
                config: config(),
                segments: segments(1),
                chunk_size: None,
                context: None,
                keywords: None,
            },
            &(),
        )
        .await
        .unwrap();

    let requests = fake.requests();
    assert_eq!((result[0].id.as_str(), requests.len()), ("s1", 3));
    assert_eq!(
        fake.state.delays.lock().unwrap().as_slice(),
        &[Duration::from_secs(5)]
    );
    assert!(matches!(
        requests[0].options.response_format,
        LlmResponseFormat::JsonSchema { .. }
    ));
    assert!(requests[2].input.contains("failed validation"));
}

#[tokio::test]
async fn structured_chunks_run_two_at_a_time_and_merge_in_input_order() {
    let fake = FakeRuntime::with_model(
        Vec::new(),
        LlmModelSummary {
            model: "small-output".to_string(),
            context_window: Some(32_000),
            max_output_tokens: Some(600),
            supports_structured_output: Some(true),
            ..LlmModelSummary::default()
        },
    );
    let mut input = segments(4);
    for segment in &mut input {
        segment.text = "x".repeat(500);
    }
    let result = LlmTaskService::new(fake.clone())
        .translate(
            TranslateSegmentsRequest {
                task_id: "translate".to_string(),
                config: config(),
                segments: input,
                chunk_size: Some(4),
                target_language: "ja".to_string(),
                target_language_name: Some("Japanese".to_string()),
            },
            &(),
        )
        .await
        .unwrap();

    assert_eq!(
        result
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["s1", "s2", "s3", "s4"]
    );
    assert_eq!(fake.requests().len(), 4);
    assert_eq!(fake.state.max_active.load(Ordering::SeqCst), 2);
    assert!(
        fake.requests()
            .iter()
            .all(|request| request.options.prompt_cache == LlmPromptCachePolicy::Automatic)
    );
}

#[tokio::test]
async fn google_translate_free_uses_core_concurrency_per_segment() {
    let fake = FakeRuntime::new(Vec::new());
    let mut direct_config = config();
    direct_config.strategy = LlmProviderStrategy::GoogleTranslateFree;
    let result = LlmTaskService::new(fake.clone())
        .translate(
            TranslateSegmentsRequest {
                task_id: "free-translate".to_string(),
                config: direct_config,
                segments: segments(4),
                chunk_size: Some(4),
                target_language: "ja".to_string(),
                target_language_name: None,
            },
            &(),
        )
        .await
        .unwrap();

    let requests = fake.state.translation_requests.lock().unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(requests.len(), 4);
    assert!(requests.iter().all(|request| request.texts.len() == 1));
}

#[tokio::test]
async fn summary_maps_concurrently_then_streams_one_final_result() {
    let long_partial = StandardLlmResponse {
        text: "summary ".repeat(60),
        usage: None,
    };
    let fake = FakeRuntime::with_model(
        vec![Ok(long_partial); 4],
        LlmModelSummary {
            model: "small-context".to_string(),
            context_window: Some(2000),
            max_output_tokens: Some(200),
            supports_structured_output: Some(true),
            ..LlmModelSummary::default()
        },
    );
    let observer = RecordingObserver::default();
    let result = LlmTaskService::new(fake.clone())
        .summarize(
            SummarizeTranscriptRequest {
                task_id: "summary".to_string(),
                config: config(),
                template: SummaryTemplateConfig {
                    id: "general".to_string(),
                    name: "General".to_string(),
                    instructions: "Key points".to_string(),
                },
                segments: (1..=4)
                    .map(|index| SummarySegmentInput {
                        id: format!("s{index}"),
                        text: format!("line {index}"),
                        start: index as f32,
                        end: index as f32 + 1.0,
                        is_final: true,
                    })
                    .collect(),
                chunk_char_budget: Some(32),
            },
            &observer,
        )
        .await
        .unwrap();

    let events = observer.0.lock().unwrap();
    assert_eq!(result.content, "final summary");
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, LlmTaskEvent::SummaryChunk(_)))
            .count(),
        4
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, LlmTaskEvent::Text(_)))
    );
    assert_eq!(fake.state.max_active.load(Ordering::SeqCst), 2);
    assert!(fake.requests().len() > 4);
}

#[tokio::test]
async fn summary_retries_a_transient_stream_after_resetting_partial_text() {
    let fake = FakeRuntime::new(Vec::new()).with_stream_failures(1);
    let observer = RecordingObserver::default();
    let result = LlmTaskService::new(fake.clone())
        .summarize(
            SummarizeTranscriptRequest {
                task_id: "stream-retry".to_string(),
                config: config(),
                template: SummaryTemplateConfig {
                    id: "general".to_string(),
                    name: "General".to_string(),
                    instructions: "Key points".to_string(),
                },
                segments: vec![SummarySegmentInput {
                    id: "s1".to_string(),
                    text: "one line".to_string(),
                    start: 0.0,
                    end: 1.0,
                    is_final: true,
                }],
                chunk_char_budget: None,
            },
            &observer,
        )
        .await
        .unwrap();

    let events = observer.0.lock().unwrap();
    let text_events = events
        .iter()
        .filter_map(|event| match event {
            LlmTaskEvent::Text(payload) => Some((payload.text.as_str(), payload.reset)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(result.content, "final summary");
    assert_eq!(
        text_events,
        vec![("partial", false), ("", true), ("final summary", false)]
    );
    assert_eq!(
        fake.state.delays.lock().unwrap().as_slice(),
        &[Duration::from_millis(250)]
    );
}
