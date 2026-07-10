use crate::json_bridge::parse_core_json;
use crate::mapper::{
    asr_inference_metric_to_ffi, asr_model_load_metric_to_ffi, asr_transcript_update_event_to_ffi,
};
use crate::{
    FfiAsrInferenceMetric, FfiAsrModelLoadMetric, FfiAsrTranscriptUpdateEvent,
    SonaCoreBindingError, SonaCoreBindingResult,
};
use sona_core::ports::asr::{
    AsrEngineConfig, AsrRuntimeObserver, AsrStreamingSession, AsrTranscriptUpdateEvent,
    AsrTranscriptionRequest, LOCAL_SHERPA_PROVIDER_ID, SherpaError, VOLCENGINE_DOUBAO_PROVIDER_ID,
    find_online_asr_provider,
};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

#[uniffi::export(foreign)]
pub trait FfiAsrStreamingObserver: Send + Sync {
    fn on_transcript_update(&self, event: FfiAsrTranscriptUpdateEvent);
    fn on_model_load(&self, metric: FfiAsrModelLoadMetric);
    fn on_live_inference(&self, metric: FfiAsrInferenceMetric);
}

struct FfiAsrRuntimeObserver {
    observer: Arc<dyn FfiAsrStreamingObserver>,
}

impl FfiAsrRuntimeObserver {
    fn new(observer: Arc<dyn FfiAsrStreamingObserver>) -> Self {
        Self { observer }
    }
}

impl AsrRuntimeObserver for FfiAsrRuntimeObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        let event = asr_transcript_update_event_to_ffi(event);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            self.observer.on_transcript_update(event);
        }));
    }

    fn on_model_load(&self, metric: &AsrModelLoadMetric) {
        let metric = asr_model_load_metric_to_ffi(metric);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            self.observer.on_model_load(metric);
        }));
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        let metric = asr_inference_metric_to_ffi(metric);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            self.observer.on_live_inference(metric);
        }));
    }
}

#[derive(uniffi::Object)]
pub struct FfiAsrStreamingSession {
    inner: Arc<dyn AsrStreamingSession>,
}

#[uniffi::export(async_runtime = "tokio")]
impl FfiAsrStreamingSession {
    pub async fn start(&self) -> SonaCoreBindingResult<()> {
        self.inner.start().await.map_err(Into::into)
    }

    pub async fn stop(&self) -> SonaCoreBindingResult<()> {
        self.inner.stop().await.map_err(Into::into)
    }

    pub async fn flush(&self) -> SonaCoreBindingResult<()> {
        self.inner.flush().await.map_err(Into::into)
    }

    pub async fn feed_audio_chunk(&self, samples: Vec<u8>) -> SonaCoreBindingResult<()> {
        self.inner
            .feed_audio_chunk(samples)
            .await
            .map_err(Into::into)
    }

    pub async fn feed_audio_samples(&self, samples: Vec<f32>) -> SonaCoreBindingResult<()> {
        self.inner
            .feed_audio_samples(&samples)
            .await
            .map_err(Into::into)
    }
}

pub(crate) fn create_online_asr_streaming_session(
    instance_id: String,
    request_json: String,
    observer: Arc<dyn FfiAsrStreamingObserver>,
) -> SonaCoreBindingResult<Arc<FfiAsrStreamingSession>> {
    let request: AsrTranscriptionRequest =
        parse_core_json(&request_json, "ASR transcription request")?;
    let provider_id = match &request.engine_config {
        AsrEngineConfig::LocalSherpa { .. } => {
            return Err(SherpaError::StreamingNotSupported {
                provider_id: LOCAL_SHERPA_PROVIDER_ID.to_string(),
            }
            .into());
        }
        AsrEngineConfig::Online { provider } => provider.provider_id.clone(),
    };

    let provider = find_online_asr_provider(&provider_id).ok_or_else(|| {
        SonaCoreBindingError::from(SherpaError::UnsupportedOnlineProvider {
            provider_id: provider_id.clone(),
        })
    })?;
    if provider.streaming.supported == Some(false) {
        return Err(SherpaError::StreamingNotSupported { provider_id }.into());
    }

    let inner = match provider_id.as_str() {
        VOLCENGINE_DOUBAO_PROVIDER_ID => sona_online_asr::create_volcengine_streaming_session(
            instance_id,
            request,
            Arc::new(FfiAsrRuntimeObserver::new(observer)),
        )?,
        _ => {
            return Err(SherpaError::UnsupportedOnlineProvider { provider_id }.into());
        }
    };

    Ok(Arc::new(FfiAsrStreamingSession { inner }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, AsrRuntimeObserver, AsrStreamingSession,
        AsrTranscriptUpdateEvent, AsrTranscriptionRequest, OnlineAsrProviderRequest, SherpaError,
        VOLCENGINE_DOUBAO_PROVIDER_ID,
    };
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::postprocess::{
        TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    };
    use sona_core::transcription::transcript::TranscriptUpdate;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingFfiObserver {
        transcript_events: Mutex<Vec<FfiAsrTranscriptUpdateEvent>>,
        model_metrics: Mutex<Vec<FfiAsrModelLoadMetric>>,
        inference_metrics: Mutex<Vec<FfiAsrInferenceMetric>>,
    }

    impl FfiAsrStreamingObserver for RecordingFfiObserver {
        fn on_transcript_update(&self, event: FfiAsrTranscriptUpdateEvent) {
            self.transcript_events.lock().unwrap().push(event);
        }

        fn on_model_load(&self, metric: FfiAsrModelLoadMetric) {
            self.model_metrics.lock().unwrap().push(metric);
        }

        fn on_live_inference(&self, metric: FfiAsrInferenceMetric) {
            self.inference_metrics.lock().unwrap().push(metric);
        }
    }

    struct PanickingFfiObserver;

    impl FfiAsrStreamingObserver for PanickingFfiObserver {
        fn on_transcript_update(&self, _event: FfiAsrTranscriptUpdateEvent) {
            panic!("foreign transcript callback panicked");
        }

        fn on_model_load(&self, _metric: FfiAsrModelLoadMetric) {
            panic!("foreign model callback panicked");
        }

        fn on_live_inference(&self, _metric: FfiAsrInferenceMetric) {
            panic!("foreign inference callback panicked");
        }
    }

    struct RecordingCoreSession {
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl AsrStreamingSession for RecordingCoreSession {
        fn start<'life0, 'async_trait>(
            &'life0 self,
        ) -> Pin<Box<dyn Future<Output = Result<(), SherpaError>> + Send + 'async_trait>>
        where
            'life0: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                self.calls.lock().unwrap().push("start");
                Ok(())
            })
        }

        fn stop<'life0, 'async_trait>(
            &'life0 self,
        ) -> Pin<Box<dyn Future<Output = Result<(), SherpaError>> + Send + 'async_trait>>
        where
            'life0: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                self.calls.lock().unwrap().push("stop");
                Ok(())
            })
        }

        fn flush<'life0, 'async_trait>(
            &'life0 self,
        ) -> Pin<Box<dyn Future<Output = Result<(), SherpaError>> + Send + 'async_trait>>
        where
            'life0: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                self.calls.lock().unwrap().push("flush");
                Ok(())
            })
        }

        fn feed_audio_chunk<'life0, 'async_trait>(
            &'life0 self,
            _samples: Vec<u8>,
        ) -> Pin<Box<dyn Future<Output = Result<(), SherpaError>> + Send + 'async_trait>>
        where
            'life0: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                self.calls.lock().unwrap().push("bytes");
                Ok(())
            })
        }

        fn feed_audio_samples<'life0, 'life1, 'async_trait>(
            &'life0 self,
            _samples: &'life1 [f32],
        ) -> Pin<Box<dyn Future<Output = Result<(), SherpaError>> + Send + 'async_trait>>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                self.calls.lock().unwrap().push("samples");
                Ok(())
            })
        }
    }

    fn recording_observer() -> Arc<dyn FfiAsrStreamingObserver> {
        Arc::new(RecordingFfiObserver::default())
    }

    fn request_json(provider_id: &str, mode: &str) -> String {
        let mode = match mode {
            "streaming" => AsrMode::Streaming,
            "batch" => AsrMode::Batch,
            value => panic!("unsupported test mode: {value}"),
        };
        serde_json::to_string(&AsrTranscriptionRequest {
            mode,
            language: "auto".into(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.into(),
                    profile_id: format!("{provider_id}-default"),
                    config: serde_json::json!({
                        "apiKey": "test-key",
                        "streamingEndpoint": "ws://127.0.0.1:1",
                        "streamingResourceId": "test-resource"
                    }),
                },
            },
        })
        .unwrap()
    }

    fn local_request_json() -> String {
        serde_json::to_string(&AsrTranscriptionRequest::local_sherpa(
            AsrMode::Streaming,
            "model".into(),
            1,
            false,
            "auto".into(),
            None,
            None,
            5.0,
            "zipformer".into(),
            None,
            None,
            TranscriptNormalizationOptions::default(),
            TranscriptPostprocessOptions::default(),
            None,
            None,
        ))
        .unwrap()
    }

    fn sample_transcript_event() -> AsrTranscriptUpdateEvent {
        AsrTranscriptUpdateEvent {
            instance_id: "live-1".into(),
            stage: "streaming".into(),
            update: TranscriptUpdate {
                remove_ids: Vec::new(),
                upsert_segments: Vec::new(),
            },
        }
    }

    fn sample_model_load_metric() -> AsrModelLoadMetric {
        AsrModelLoadMetric {
            occurred_at_ms: 1,
            instance_id: "live-1".into(),
            model_path: "model".into(),
            model_type: "online".into(),
            recognizer_kind: "streaming".into(),
            num_threads: 1,
            reused_from_pool: false,
            load_ms: 1.0,
            rss_before_mb: None,
            rss_after_mb: None,
            rss_delta_mb: None,
            process_rss_mb: None,
        }
    }

    fn sample_inference_metric() -> AsrInferenceMetric {
        AsrInferenceMetric {
            occurred_at_ms: 1,
            source: "microphone".into(),
            instance_id: Some("live-1".into()),
            stage: "streaming".into(),
            is_final: false,
            audio_duration_ms: 10.0,
            buffered_samples: 160,
            audio_extract_ms: None,
            decode_ms: 2.0,
            emit_latency_ms: None,
            total_ms: None,
            rtf: None,
            segment_count: None,
            process_rss_mb: None,
        }
    }

    #[test]
    fn observer_adapter_forwards_all_three_typed_callbacks() {
        let observer = Arc::new(RecordingFfiObserver::default());
        let adapter = FfiAsrRuntimeObserver::new(observer.clone());
        adapter.on_transcript_update(&sample_transcript_event());
        adapter.on_model_load(&sample_model_load_metric());
        adapter.on_live_inference(&sample_inference_metric());
        assert_eq!(
            observer.transcript_events.lock().unwrap()[0].instance_id,
            "live-1"
        );
        assert_eq!(
            observer.model_metrics.lock().unwrap()[0].model_path,
            "model"
        );
        assert_eq!(
            observer.inference_metrics.lock().unwrap()[0].stage,
            "streaming"
        );
    }

    #[test]
    fn observer_adapter_contains_foreign_callback_panics() {
        let adapter = FfiAsrRuntimeObserver::new(Arc::new(PanickingFfiObserver));

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                adapter.on_transcript_update(&sample_transcript_event());
                adapter.on_model_load(&sample_model_load_metric());
                adapter.on_live_inference(&sample_inference_metric());
            }))
            .is_ok()
        );
    }

    #[tokio::test]
    async fn session_object_delegates_all_five_core_methods() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let session = FfiAsrStreamingSession {
            inner: Arc::new(RecordingCoreSession {
                calls: calls.clone(),
            }),
        };
        session.start().await.unwrap();
        session.feed_audio_chunk(vec![1, 2]).await.unwrap();
        session.feed_audio_samples(vec![0.25]).await.unwrap();
        session.flush().await.unwrap();
        session.stop().await.unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["start", "bytes", "samples", "flush", "stop"]
        );
    }

    #[test]
    fn malformed_request_json_stays_an_invalid_input_error() {
        let error =
            create_online_asr_streaming_session("live-1".into(), "{".into(), recording_observer())
                .err()
                .expect("invalid JSON should fail");
        assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
    }

    #[test]
    fn non_online_request_uses_the_core_streaming_error_code() {
        let error = create_online_asr_streaming_session(
            "live-1".into(),
            local_request_json(),
            recording_observer(),
        )
        .err()
        .expect("local streaming should fail in the online factory");
        assert!(matches!(
            error,
            SonaCoreBindingError::AsrRuntime { code, .. }
                if code == "STREAMING_NOT_SUPPORTED"
        ));
    }

    #[test]
    fn unknown_online_provider_uses_the_core_provider_error_code() {
        let error = create_online_asr_streaming_session(
            "live-1".into(),
            request_json("future-provider", "streaming"),
            recording_observer(),
        )
        .err()
        .expect("unknown provider should fail");
        assert!(matches!(
            error,
            SonaCoreBindingError::AsrRuntime { code, .. }
                if code == "UNSUPPORTED_ONLINE_PROVIDER"
        ));
    }

    #[test]
    fn unsupported_online_provider_uses_the_core_streaming_error_code() {
        let error = create_online_asr_streaming_session(
            "live-1".into(),
            request_json("groq-whisper", "streaming"),
            recording_observer(),
        )
        .err()
        .expect("unsupported streaming provider should fail");
        assert!(matches!(
            error,
            SonaCoreBindingError::AsrRuntime { code, .. }
                if code == "STREAMING_NOT_SUPPORTED"
        ));
    }

    #[tokio::test]
    async fn byte_feed_before_start_preserves_the_volcengine_error_code() {
        let session = create_online_asr_streaming_session(
            "live-1".into(),
            request_json(VOLCENGINE_DOUBAO_PROVIDER_ID, "streaming"),
            recording_observer(),
        )
        .unwrap();
        let error = session.feed_audio_chunk(vec![1, 2]).await.unwrap_err();
        assert!(matches!(
            error,
            SonaCoreBindingError::AsrRuntime { code, .. }
                if code == "VOLCENGINE_WEB_SOCKET_NOT_CONNECTED"
        ));
    }
}
