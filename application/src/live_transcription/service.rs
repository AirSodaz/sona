use sona_core::ports::asr::{
    AsrAudioFrame, AsrPortError, AsrRuntimeObserver, AsrStreamBoundaryEvent,
    AsrStreamingErrorEvent, AsrStreamingSession, AsrTranscriptUpdateEvent, AsrTranscriptionRequest,
    StreamingAsrFactoryPort, StreamingInferenceSpec, StreamingOutputPolicy,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Mutex as AsyncMutex, Notify};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LiveSourceEpoch {
    pub source_id: String,
    pub generation: u64,
}

impl LiveSourceEpoch {
    pub fn new(source_id: impl Into<String>, generation: u64) -> Self {
        Self {
            source_id: source_id.into(),
            generation,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LiveInputTransform {
    pub gain: f32,
}

impl Default for LiveInputTransform {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}

impl PartialEq for LiveInputTransform {
    fn eq(&self, other: &Self) -> bool {
        self.gain.to_bits() == other.gain.to_bits()
    }
}

impl Eq for LiveInputTransform {}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTranscriptionMetrics {
    pub active_sources: usize,
    pub active_pipelines: usize,
    pub active_consumers: usize,
    pub shared_pipelines: usize,
    pub avoided_feed_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTranscriptionSubscription {
    pub consumer_id: String,
    pub pipeline_id: String,
    pub shared: bool,
    pub transient: bool,
}

#[derive(Clone)]
struct PipelineKey {
    source: LiveSourceEpoch,
    transform: LiveInputTransform,
    spec: StreamingInferenceSpec,
}

impl PartialEq for PipelineKey {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.transform == other.transform && self.spec == other.spec
    }
}

struct Subscriber {
    output_policy: StreamingOutputPolicy,
    mailbox: Arc<SubscriberMailbox>,
    visible_partial_ids: Vec<String>,
}

struct Pipeline {
    id: String,
    key: PipelineKey,
    session: Arc<dyn AsrStreamingSession>,
    observer: Arc<PipelineObserver>,
    subscribers: HashMap<String, Subscriber>,
    at_boundary: bool,
    last_boundary_sample: u64,
    audio_since_boundary: Vec<AsrAudioFrame>,
    transient: bool,
    closing: bool,
}

#[derive(Default)]
struct SourceCursor {
    sequence: Option<u64>,
    end_sample: u64,
}

#[derive(Default)]
struct CoordinatorState {
    pipelines: HashMap<String, Pipeline>,
    consumer_to_pipeline: HashMap<String, String>,
    source_cursors: HashMap<LiveSourceEpoch, SourceCursor>,
    retired_sources: HashSet<LiveSourceEpoch>,
    avoided_feed_count: u64,
}

pub struct LiveTranscriptionCoordinator {
    inner: Arc<CoordinatorInner>,
}

struct CoordinatorInner {
    factory: Arc<dyn StreamingAsrFactoryPort>,
    telemetry: Arc<dyn AsrRuntimeObserver>,
    state: AsyncMutex<CoordinatorState>,
    lifecycle_guard: AsyncMutex<()>,
    feed_guard: AsyncMutex<()>,
    next_pipeline_id: AtomicU64,
}

enum Delivery {
    Transcript(AsrTranscriptUpdateEvent),
    Error(AsrStreamingErrorEvent),
}

#[derive(Default)]
struct MailboxState {
    latest_partial: Option<Delivery>,
    guaranteed: VecDeque<Delivery>,
}

struct SubscriberMailbox {
    state: Mutex<MailboxState>,
    notify: Notify,
    closed: AtomicBool,
    drained: AtomicBool,
    drained_notify: Notify,
}

impl SubscriberMailbox {
    fn new(
        observer: Arc<dyn AsrRuntimeObserver>,
        coordinator: Weak<CoordinatorInner>,
        consumer_id: String,
    ) -> Arc<Self> {
        let mailbox = Arc::new(Self {
            state: Mutex::new(MailboxState::default()),
            notify: Notify::new(),
            closed: AtomicBool::new(false),
            drained: AtomicBool::new(false),
            drained_notify: Notify::new(),
        });
        let worker_mailbox = Arc::clone(&mailbox);
        tokio::spawn(async move {
            while let Some(delivery) = worker_mailbox.next().await {
                let delivered = catch_unwind(AssertUnwindSafe(|| match delivery {
                    Delivery::Transcript(event) => observer.on_transcript_update(&event),
                    Delivery::Error(event) => observer.on_streaming_error(&event),
                }));
                if delivered.is_err() {
                    worker_mailbox.close();
                    if let Some(coordinator) = coordinator.upgrade() {
                        tokio::spawn(async move {
                            coordinator.evict_failed_consumer(&consumer_id).await;
                        });
                    }
                    break;
                }
            }
            worker_mailbox.drained.store(true, Ordering::Release);
            worker_mailbox.drained_notify.notify_waiters();
        });
        mailbox
    }

    fn push_transcript(&self, event: AsrTranscriptUpdateEvent) {
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        let is_guaranteed = event
            .update
            .upsert_segments
            .iter()
            .any(|segment| segment.is_final)
            || !event.update.remove_ids.is_empty();
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if is_guaranteed {
            if let Some(partial) = state.latest_partial.take() {
                state.guaranteed.push_back(partial);
            }
            state.guaranteed.push_back(Delivery::Transcript(event));
        } else {
            state.latest_partial = Some(Delivery::Transcript(event));
        }
        drop(state);
        self.notify.notify_one();
    }

    fn push_error(&self, event: AsrStreamingErrorEvent) {
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(partial) = state.latest_partial.take() {
            state.guaranteed.push_back(partial);
        }
        state.guaranteed.push_back(Delivery::Error(event));
        drop(state);
        self.notify.notify_one();
    }

    async fn next(&self) -> Option<Delivery> {
        loop {
            let notified = self.notify.notified();
            {
                let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
                if let Some(delivery) = state.guaranteed.pop_front() {
                    return Some(delivery);
                }
                if let Some(delivery) = state.latest_partial.take() {
                    return Some(delivery);
                }
            }
            if self.closed.load(Ordering::Acquire) {
                return None;
            }
            notified.await;
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    async fn close_and_drain(&self) {
        self.close();
        loop {
            let notified = self.drained_notify.notified();
            if self.drained.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }
}

enum PipelineEvent {
    Transcript(AsrTranscriptUpdateEvent),
    Error(AsrStreamingErrorEvent),
    Boundary(AsrStreamBoundaryEvent),
    Barrier(tokio::sync::oneshot::Sender<()>),
}

struct PipelineObserver {
    pipeline_id: String,
    session_start_sample: u64,
    telemetry: Arc<dyn AsrRuntimeObserver>,
    events: tokio::sync::mpsc::UnboundedSender<PipelineEvent>,
    boundary_version: Arc<AtomicU64>,
}

impl PipelineObserver {
    fn new(
        pipeline_id: String,
        session_start_sample: u64,
        coordinator: Weak<CoordinatorInner>,
        telemetry: Arc<dyn AsrRuntimeObserver>,
    ) -> Arc<Self> {
        let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let observer = Arc::new(Self {
            pipeline_id,
            session_start_sample,
            telemetry,
            events,
            boundary_version: Arc::new(AtomicU64::new(0)),
        });
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                let Some(coordinator) = coordinator.upgrade() else {
                    break;
                };
                match event {
                    PipelineEvent::Transcript(event) => {
                        coordinator.dispatch_transcript(event).await;
                    }
                    PipelineEvent::Error(event) => coordinator.dispatch_error(event).await,
                    PipelineEvent::Boundary(event) => coordinator.record_boundary(event).await,
                    PipelineEvent::Barrier(completed) => {
                        let _ = completed.send(());
                    }
                }
            }
        });
        observer
    }

    async fn drain(&self) {
        let (completed, received) = tokio::sync::oneshot::channel();
        if self.events.send(PipelineEvent::Barrier(completed)).is_ok() {
            let _ = received.await;
        }
    }
}

impl AsrRuntimeObserver for PipelineObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        let mut event = event.clone();
        event.instance_id.clone_from(&self.pipeline_id);
        let final_end = event
            .update
            .upsert_segments
            .iter()
            .filter(|segment| segment.is_final)
            .map(|segment| segment.end)
            .reduce(f64::max);
        let _ = self.events.send(PipelineEvent::Transcript(event));
        if let Some(final_end) = final_end {
            let end_sample = self
                .session_start_sample
                .saturating_add((final_end.max(0.0) * 16_000.0).round() as u64);
            let boundary_version = Arc::clone(&self.boundary_version);
            let expected_version = boundary_version.load(Ordering::Acquire);
            let events = self.events.clone();
            let instance_id = self.pipeline_id.clone();
            tokio::spawn(async move {
                tokio::task::yield_now().await;
                if boundary_version.load(Ordering::Acquire) == expected_version {
                    let _ = events.send(PipelineEvent::Boundary(AsrStreamBoundaryEvent {
                        instance_id,
                        sequence: 0,
                        end_sample,
                    }));
                }
            });
        }
    }

    fn on_model_load(&self, metric: &AsrModelLoadMetric) {
        self.telemetry.on_model_load(metric);
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        self.telemetry.on_live_inference(metric);
    }

    fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
        let mut event = event.clone();
        event.instance_id.clone_from(&self.pipeline_id);
        let _ = self.events.send(PipelineEvent::Error(event));
    }

    fn on_stream_boundary(&self, event: &AsrStreamBoundaryEvent) {
        self.boundary_version.fetch_add(1, Ordering::AcqRel);
        let mut event = event.clone();
        event.instance_id.clone_from(&self.pipeline_id);
        let _ = self.events.send(PipelineEvent::Boundary(event));
    }
}

impl LiveTranscriptionCoordinator {
    pub fn new(
        factory: Arc<dyn StreamingAsrFactoryPort>,
        telemetry: Arc<dyn AsrRuntimeObserver>,
    ) -> Self {
        Self {
            inner: Arc::new(CoordinatorInner {
                factory,
                telemetry,
                state: AsyncMutex::new(CoordinatorState::default()),
                lifecycle_guard: AsyncMutex::new(()),
                feed_guard: AsyncMutex::new(()),
                next_pipeline_id: AtomicU64::new(1),
            }),
        }
    }

    pub async fn prepare(&self, request: &AsrTranscriptionRequest) -> Result<(), AsrPortError> {
        let spec = StreamingInferenceSpec::from_request(request)?;
        self.inner.factory.prepare(&spec).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn acquire(
        &self,
        consumer_id: impl Into<String>,
        source: LiveSourceEpoch,
        source_cursor: u64,
        transform: LiveInputTransform,
        request: AsrTranscriptionRequest,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<LiveTranscriptionSubscription, AsrPortError> {
        let _guard = self.inner.lifecycle_guard.lock().await;
        let _feed_guard = self.inner.feed_guard.lock().await;
        let consumer_id = consumer_id.into();
        if !transform.gain.is_finite() || transform.gain < 0.0 {
            return Err(AsrPortError::invalid_request(
                "Live transcription gain must be finite and non-negative",
            ));
        }
        let spec = StreamingInferenceSpec::from_request(&request)?;
        let output_policy = StreamingOutputPolicy::from_request(&request)?;
        let key = PipelineKey {
            source,
            transform,
            spec: spec.clone(),
        };

        {
            let mut state = self.inner.state.lock().await;
            if state.retired_sources.contains(&key.source) {
                return Err(AsrPortError::invalid_request(format!(
                    "Live transcription source epoch {}:{} is retired",
                    key.source.source_id, key.source.generation
                )));
            }
            if state.consumer_to_pipeline.contains_key(&consumer_id) {
                return Err(AsrPortError::invalid_request(format!(
                    "Live transcription consumer {consumer_id} is already active"
                )));
            }
            let matching = state
                .pipelines
                .values_mut()
                .find(|pipeline| pipeline.key == key && pipeline.at_boundary && !pipeline.closing);
            if let Some(pipeline) = matching {
                let mailbox = SubscriberMailbox::new(
                    observer,
                    Arc::downgrade(&self.inner),
                    consumer_id.clone(),
                );
                pipeline.subscribers.insert(
                    consumer_id.clone(),
                    Subscriber {
                        output_policy,
                        mailbox,
                        visible_partial_ids: Vec::new(),
                    },
                );
                let pipeline_id = pipeline.id.clone();
                let shared = pipeline.subscribers.len() > 1;
                state
                    .consumer_to_pipeline
                    .insert(consumer_id.clone(), pipeline_id.clone());
                return Ok(LiveTranscriptionSubscription {
                    consumer_id,
                    pipeline_id,
                    shared,
                    transient: false,
                });
            }
        }

        let pipeline_id = format!(
            "live-pipeline-{}",
            self.inner.next_pipeline_id.fetch_add(1, Ordering::Relaxed)
        );
        let pipeline_observer = PipelineObserver::new(
            pipeline_id.clone(),
            source_cursor,
            Arc::downgrade(&self.inner),
            Arc::clone(&self.inner.telemetry),
        );
        let session = self
            .inner
            .factory
            .create(&pipeline_id, &spec, pipeline_observer.clone())
            .await?;
        let mailbox =
            SubscriberMailbox::new(observer, Arc::downgrade(&self.inner), consumer_id.clone());
        let transient = {
            let state = self.inner.state.lock().await;
            state.pipelines.values().any(|pipeline| pipeline.key == key)
        };
        {
            let mut state = self.inner.state.lock().await;
            let mut subscribers = HashMap::new();
            subscribers.insert(
                consumer_id.clone(),
                Subscriber {
                    output_policy,
                    mailbox: Arc::clone(&mailbox),
                    visible_partial_ids: Vec::new(),
                },
            );
            state.pipelines.insert(
                pipeline_id.clone(),
                Pipeline {
                    id: pipeline_id.clone(),
                    key,
                    session: Arc::clone(&session),
                    observer: pipeline_observer,
                    subscribers,
                    at_boundary: true,
                    last_boundary_sample: source_cursor,
                    audio_since_boundary: Vec::new(),
                    transient,
                    closing: false,
                },
            );
            state
                .consumer_to_pipeline
                .insert(consumer_id.clone(), pipeline_id.clone());
        }
        if let Err(error) = session.start().await {
            let _ = session.stop().await;
            let mut state = self.inner.state.lock().await;
            state.pipelines.remove(&pipeline_id);
            state.consumer_to_pipeline.remove(&consumer_id);
            drop(state);
            mailbox.close_and_drain().await;
            return Err(error);
        }

        Ok(LiveTranscriptionSubscription {
            consumer_id,
            pipeline_id,
            shared: false,
            transient,
        })
    }

    pub async fn feed_source(
        &self,
        source: &LiveSourceEpoch,
        frame: AsrAudioFrame,
    ) -> Result<(), AsrPortError> {
        let _guard = self.inner.feed_guard.lock().await;
        let sessions = {
            let mut state = self.inner.state.lock().await;
            if state.retired_sources.contains(source) {
                return Err(AsrPortError::invalid_request(format!(
                    "Live transcription source epoch {}:{} is retired",
                    source.source_id, source.generation
                )));
            }
            let cursor = state.source_cursors.entry(source.clone()).or_default();
            if cursor
                .sequence
                .is_some_and(|sequence| frame.sequence <= sequence)
                || frame.start_sample < cursor.end_sample
            {
                return Err(AsrPortError::invalid_request(format!(
                    "Stale or out-of-order audio frame for source epoch {}:{}",
                    source.source_id, source.generation
                )));
            }
            cursor.sequence = Some(frame.sequence);
            cursor.end_sample = frame.end_sample();

            let mut sessions = Vec::new();
            let mut consumer_count = 0_u64;
            for pipeline in state
                .pipelines
                .values_mut()
                .filter(|pipeline| &pipeline.key.source == source)
            {
                let transformed = transform_frame(&frame, pipeline.key.transform);
                pipeline.at_boundary = false;
                pipeline.audio_since_boundary.push(transformed.clone());
                consumer_count = consumer_count.saturating_add(pipeline.subscribers.len() as u64);
                sessions.push((Arc::clone(&pipeline.session), transformed));
            }
            state.avoided_feed_count = state
                .avoided_feed_count
                .saturating_add(consumer_count.saturating_sub(sessions.len() as u64));
            sessions
        };

        let mut first_error = None;
        for (session, frame) in sessions {
            if let Err(error) = session.feed_audio_frame(frame).await {
                first_error.get_or_insert(error);
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }

    pub async fn release(&self, consumer_id: &str) -> Result<(), AsrPortError> {
        let _guard = self.inner.lifecycle_guard.lock().await;
        let feed_guard = self.inner.feed_guard.lock().await;
        let release = {
            let mut state = self.inner.state.lock().await;
            let pipeline_id = state
                .consumer_to_pipeline
                .remove(consumer_id)
                .ok_or_else(|| {
                    AsrPortError::runtime(format!(
                        "Live transcription consumer {consumer_id} is not active"
                    ))
                })?;
            let pipeline = state
                .pipelines
                .get_mut(&pipeline_id)
                .expect("consumer pipeline must exist");
            if pipeline.subscribers.len() == 1 {
                pipeline.closing = true;
                let subscriber = pipeline
                    .subscribers
                    .get(consumer_id)
                    .expect("consumer subscription must exist");
                ReleaseAction::StopLast {
                    pipeline_id,
                    session: Arc::clone(&pipeline.session),
                    observer: Arc::clone(&pipeline.observer),
                    mailbox: Arc::clone(&subscriber.mailbox),
                }
            } else {
                let subscriber = pipeline
                    .subscribers
                    .remove(consumer_id)
                    .expect("consumer subscription must exist");
                if pipeline.at_boundary || pipeline.audio_since_boundary.is_empty() {
                    ReleaseAction::DetachOnly(subscriber.mailbox)
                } else {
                    ReleaseAction::Replay {
                        key: Box::new(pipeline.key.clone()),
                        frames: pipeline.audio_since_boundary.clone(),
                        output_policy: subscriber.output_policy,
                        observer: subscriber.mailbox,
                        visible_partial_ids: subscriber.visible_partial_ids,
                    }
                }
            }
        };

        match release {
            ReleaseAction::DetachOnly(mailbox) => {
                drop(feed_guard);
                mailbox.close_and_drain().await;
                Ok(())
            }
            ReleaseAction::StopLast {
                pipeline_id,
                session,
                observer,
                mailbox,
            } => {
                let flush_result = session.flush().await;
                observer.drain().await;
                let stop_result = session.stop().await;
                observer.drain().await;
                let mut state = self.inner.state.lock().await;
                state.pipelines.remove(&pipeline_id);
                drop(state);
                mailbox.close_and_drain().await;
                flush_result.and(stop_result)
            }
            ReleaseAction::Replay {
                key,
                frames,
                output_policy,
                observer,
                visible_partial_ids,
            } => {
                drop(feed_guard);
                self.replay_final(
                    consumer_id,
                    *key,
                    frames,
                    output_policy,
                    observer,
                    visible_partial_ids,
                )
                .await
            }
        }
    }

    pub async fn retire_source(&self, source: &LiveSourceEpoch) -> Result<(), AsrPortError> {
        let _guard = self.inner.lifecycle_guard.lock().await;
        let _feed_guard = self.inner.feed_guard.lock().await;
        let pipelines = {
            let mut state = self.inner.state.lock().await;
            for pipeline in state
                .pipelines
                .values_mut()
                .filter(|pipeline| &pipeline.key.source == source)
            {
                pipeline.closing = true;
            }
            state.retired_sources.insert(source.clone());
            state.source_cursors.remove(source);
            state
                .pipelines
                .values()
                .filter(|pipeline| &pipeline.key.source == source)
                .map(|pipeline| {
                    (
                        pipeline.id.clone(),
                        Arc::clone(&pipeline.session),
                        Arc::clone(&pipeline.observer),
                    )
                })
                .collect::<Vec<_>>()
        };

        let mut first_error = None;
        for (pipeline_id, session, observer) in &pipelines {
            if let Err(error) = session.flush().await {
                first_error.get_or_insert(error);
            }
            observer.drain().await;
            if let Err(error) = session.stop().await {
                first_error.get_or_insert(error);
            }
            observer.drain().await;

            let mailboxes = {
                let mut state = self.inner.state.lock().await;
                let Some(pipeline) = state.pipelines.remove(pipeline_id) else {
                    continue;
                };
                for consumer_id in pipeline.subscribers.keys() {
                    state.consumer_to_pipeline.remove(consumer_id);
                }
                pipeline
                    .subscribers
                    .into_values()
                    .map(|subscriber| subscriber.mailbox)
                    .collect::<Vec<_>>()
            };
            for mailbox in mailboxes {
                mailbox.close_and_drain().await;
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub async fn metrics(&self) -> LiveTranscriptionMetrics {
        let state = self.inner.state.lock().await;
        let active_consumers = state.consumer_to_pipeline.len();
        LiveTranscriptionMetrics {
            active_sources: state
                .pipelines
                .values()
                .map(|pipeline| &pipeline.key.source)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            active_pipelines: state.pipelines.len(),
            active_consumers,
            shared_pipelines: state
                .pipelines
                .values()
                .filter(|pipeline| pipeline.subscribers.len() > 1)
                .count(),
            avoided_feed_count: state.avoided_feed_count,
        }
    }

    pub async fn has_consumer(&self, consumer_id: &str) -> bool {
        self.inner
            .state
            .lock()
            .await
            .consumer_to_pipeline
            .contains_key(consumer_id)
    }

    #[allow(clippy::too_many_arguments)]
    async fn replay_final(
        &self,
        consumer_id: &str,
        key: PipelineKey,
        frames: Vec<AsrAudioFrame>,
        output_policy: StreamingOutputPolicy,
        mailbox: Arc<SubscriberMailbox>,
        visible_partial_ids: Vec<String>,
    ) -> Result<(), AsrPortError> {
        let replay_id = format!(
            "live-replay-{}",
            self.inner.next_pipeline_id.fetch_add(1, Ordering::Relaxed)
        );
        let observer = Arc::new(ReplayObserver {
            consumer_id: consumer_id.to_string(),
            output_policy,
            mailbox: Arc::clone(&mailbox),
            visible_partial_ids,
        });
        let session = match self
            .inner
            .factory
            .create(&replay_id, &key.spec, observer)
            .await
        {
            Ok(session) => session,
            Err(error) => {
                mailbox.close_and_drain().await;
                return Err(error);
            }
        };
        if let Err(error) = session.start().await {
            let _ = session.stop().await;
            mailbox.close_and_drain().await;
            return Err(error);
        }
        let mut feed_error = None;
        for frame in frames {
            if let Err(error) = session.feed_audio_frame(frame).await {
                feed_error = Some(error);
                break;
            }
        }
        let flush_result = if feed_error.is_none() {
            session.flush().await
        } else {
            Ok(())
        };
        let stop_result = session.stop().await;
        mailbox.close_and_drain().await;
        feed_error.map_or_else(|| flush_result.and(stop_result), Err)
    }
}

enum ReleaseAction {
    DetachOnly(Arc<SubscriberMailbox>),
    StopLast {
        pipeline_id: String,
        session: Arc<dyn AsrStreamingSession>,
        observer: Arc<PipelineObserver>,
        mailbox: Arc<SubscriberMailbox>,
    },
    Replay {
        key: Box<PipelineKey>,
        frames: Vec<AsrAudioFrame>,
        output_policy: StreamingOutputPolicy,
        observer: Arc<SubscriberMailbox>,
        visible_partial_ids: Vec<String>,
    },
}

struct ReplayObserver {
    consumer_id: String,
    output_policy: StreamingOutputPolicy,
    mailbox: Arc<SubscriberMailbox>,
    visible_partial_ids: Vec<String>,
}

impl AsrRuntimeObserver for ReplayObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        if !event
            .update
            .upsert_segments
            .iter()
            .any(|segment| segment.is_final)
        {
            return;
        }
        let mut update = self.output_policy.process_update(event.update.clone());
        if let Some(visible_id) = self.visible_partial_ids.first() {
            if let Some(segment) = update.upsert_segments.first_mut() {
                segment.id.clone_from(visible_id);
            }
            update.remove_ids.retain(|id| id != visible_id);
        }
        self.mailbox.push_transcript(AsrTranscriptUpdateEvent {
            instance_id: self.consumer_id.clone(),
            stage: "replay_final".to_string(),
            update,
        });
    }

    fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

    fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}

    fn on_streaming_error(&self, event: &AsrStreamingErrorEvent) {
        self.mailbox.push_error(AsrStreamingErrorEvent {
            instance_id: self.consumer_id.clone(),
            code: event.code.clone(),
            message: event.message.clone(),
        });
    }
}

impl CoordinatorInner {
    async fn evict_failed_consumer(&self, consumer_id: &str) {
        let _guard = self.lifecycle_guard.lock().await;
        let _feed_guard = self.feed_guard.lock().await;
        let stopped = {
            let mut state = self.state.lock().await;
            let Some(pipeline_id) = state.consumer_to_pipeline.remove(consumer_id) else {
                return;
            };
            let Some(pipeline) = state.pipelines.get_mut(&pipeline_id) else {
                return;
            };
            pipeline.subscribers.remove(consumer_id);
            if pipeline.subscribers.is_empty() {
                state
                    .pipelines
                    .remove(&pipeline_id)
                    .map(|pipeline| (pipeline.session, pipeline.observer))
            } else {
                None
            }
        };
        if let Some((session, observer)) = stopped {
            let _ = session.stop().await;
            observer.drain().await;
        }
    }

    async fn dispatch_transcript(&self, event: AsrTranscriptUpdateEvent) {
        let mut state = self.state.lock().await;
        let Some(pipeline) = state.pipelines.get_mut(&event.instance_id) else {
            return;
        };
        for (consumer_id, subscriber) in &mut pipeline.subscribers {
            let update = subscriber
                .output_policy
                .process_update(event.update.clone());
            subscriber.visible_partial_ids = update
                .upsert_segments
                .iter()
                .filter(|segment| !segment.is_final)
                .map(|segment| segment.id.clone())
                .collect();
            subscriber
                .mailbox
                .push_transcript(AsrTranscriptUpdateEvent {
                    instance_id: consumer_id.clone(),
                    stage: event.stage.clone(),
                    update,
                });
        }
    }

    async fn dispatch_error(&self, event: AsrStreamingErrorEvent) {
        let state = self.state.lock().await;
        let Some(pipeline) = state.pipelines.get(&event.instance_id) else {
            return;
        };
        for (consumer_id, subscriber) in &pipeline.subscribers {
            subscriber.mailbox.push_error(AsrStreamingErrorEvent {
                instance_id: consumer_id.clone(),
                code: event.code.clone(),
                message: event.message.clone(),
            });
        }
    }

    async fn record_boundary(&self, event: AsrStreamBoundaryEvent) {
        let redundant_session = {
            let mut state = self.state.lock().await;
            let Some(pipeline) = state.pipelines.get_mut(&event.instance_id) else {
                return;
            };
            pipeline.last_boundary_sample = event.end_sample;
            pipeline
                .audio_since_boundary
                .retain(|frame| frame.end_sample() > event.end_sample);
            pipeline.at_boundary = pipeline.audio_since_boundary.is_empty();

            if pipeline.closing || !pipeline.at_boundary {
                return;
            }

            let key = pipeline.key.clone();
            let boundary = event.end_sample;
            let target_id = state
                .pipelines
                .values()
                .filter(|candidate| {
                    candidate.id != event.instance_id
                        && candidate.key == key
                        && candidate.at_boundary
                        && !candidate.closing
                        && candidate.last_boundary_sample == boundary
                })
                .map(|candidate| candidate.id.clone())
                .min();
            let Some(target_id) = target_id else {
                return;
            };

            let source_id = event.instance_id.clone();
            let (canonical_id, redundant_id) = if target_id < source_id {
                (target_id, source_id)
            } else {
                (source_id, target_id)
            };
            let Some(mut redundant) = state.pipelines.remove(&redundant_id) else {
                return;
            };
            let moved_subscribers = redundant.subscribers.drain().collect::<Vec<_>>();
            let Some(canonical) = state.pipelines.get_mut(&canonical_id) else {
                state.pipelines.insert(redundant_id, redundant);
                return;
            };
            for (consumer_id, subscriber) in &moved_subscribers {
                canonical.subscribers.insert(
                    consumer_id.clone(),
                    Subscriber {
                        output_policy: subscriber.output_policy.clone(),
                        mailbox: Arc::clone(&subscriber.mailbox),
                        visible_partial_ids: subscriber.visible_partial_ids.clone(),
                    },
                );
            }
            canonical.transient = false;
            let _ = canonical;
            for (consumer_id, _) in moved_subscribers {
                state
                    .consumer_to_pipeline
                    .insert(consumer_id.clone(), canonical_id.clone());
            }
            Some(redundant.session)
        };

        if let Some(session) = redundant_session {
            let _ = session.stop().await;
        }
    }
}

fn transform_frame(frame: &AsrAudioFrame, transform: LiveInputTransform) -> AsrAudioFrame {
    if (transform.gain - 1.0).abs() <= f32::EPSILON {
        return frame.clone();
    }
    let samples = frame
        .samples
        .iter()
        .map(|sample| (sample * transform.gain).clamp(-1.0, 1.0))
        .collect::<Vec<_>>();
    AsrAudioFrame::new(frame.sequence, frame.start_sample, samples)
}
