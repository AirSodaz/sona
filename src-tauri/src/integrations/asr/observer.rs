use super::metrics::{AsrMetricsStore, set_live_inference_metric, set_model_load_metric};
use super::recognizer_output_event;
use crate::platform::event::EventEmitter;
use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use std::sync::Arc;

pub(crate) struct TauriAsrRuntimeObserver {
    emitter: Arc<dyn EventEmitter>,
    metrics: AsrMetricsStore,
}

impl TauriAsrRuntimeObserver {
    pub(crate) fn new(emitter: Arc<dyn EventEmitter>, metrics: AsrMetricsStore) -> Self {
        Self { emitter, metrics }
    }
}

impl AsrRuntimeObserver for TauriAsrRuntimeObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        let payload = match serde_json::to_value(&event.update) {
            Ok(payload) => payload,
            Err(error) => {
                log::warn!("[ASR] failed to serialize transcript update: {error}");
                return;
            }
        };
        let _ = self
            .emitter
            .emit(&recognizer_output_event(&event.instance_id), payload);
    }

    fn on_model_load(&self, metric: &AsrModelLoadMetric) {
        set_model_load_metric(&self.metrics, metric.clone());
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        set_live_inference_metric(&self.metrics, metric.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::TauriAsrRuntimeObserver;
    use crate::integrations::asr::metrics::{new_metrics_store, snapshot_metrics};
    use crate::platform::event::MockEventEmitter;
    use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent};
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::transcript::TranscriptUpdate;
    use std::sync::Arc;

    fn model_load_metric() -> AsrModelLoadMetric {
        AsrModelLoadMetric {
            occurred_at_ms: 1,
            instance_id: "live-1".to_string(),
            model_path: "model".to_string(),
            model_type: "sensevoice".to_string(),
            recognizer_kind: "offline".to_string(),
            num_threads: 4,
            reused_from_pool: false,
            load_ms: 12.0,
            rss_before_mb: None,
            rss_after_mb: None,
            rss_delta_mb: None,
            process_rss_mb: None,
        }
    }

    fn live_metric() -> AsrInferenceMetric {
        AsrInferenceMetric {
            occurred_at_ms: 2,
            source: "live".to_string(),
            instance_id: Some("live-1".to_string()),
            stage: "partial".to_string(),
            is_final: false,
            audio_duration_ms: 100.0,
            buffered_samples: 1_600,
            audio_extract_ms: None,
            decode_ms: 10.0,
            emit_latency_ms: None,
            total_ms: None,
            rtf: Some(0.1),
            segment_count: Some(1),
            process_rss_mb: None,
        }
    }

    #[test]
    fn emits_the_existing_instance_event_payload() {
        let emitter = Arc::new(MockEventEmitter::new());
        let observer = TauriAsrRuntimeObserver::new(emitter.clone(), new_metrics_store());
        let event = AsrTranscriptUpdateEvent {
            instance_id: "live-1".to_string(),
            stage: "partial".to_string(),
            update: TranscriptUpdate {
                remove_ids: vec!["old".to_string()],
                upsert_segments: Vec::new(),
            },
        };

        observer.on_transcript_update(&event);

        let emitted = emitter.emitted.lock().unwrap();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].0, "recognizer-output-live-1");
        assert_eq!(emitted[0].1, serde_json::to_value(event.update).unwrap());
    }

    #[test]
    fn records_model_and_live_metrics_in_existing_slots() {
        let metrics = new_metrics_store();
        let observer =
            TauriAsrRuntimeObserver::new(Arc::new(MockEventEmitter::new()), metrics.clone());
        let model_load = model_load_metric();
        let live = live_metric();

        observer.on_model_load(&model_load);
        observer.on_live_inference(&live);

        let snapshot = snapshot_metrics(&metrics);
        assert_eq!(snapshot.model_load, Some(model_load));
        assert_eq!(snapshot.live_inference, Some(live));
    }
}
