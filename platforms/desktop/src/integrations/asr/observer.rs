use super::metrics::{AsrMetricsStore, set_live_inference_metric, set_model_load_metric};
use super::recognizer_output_event;
use crate::platform::event::EventEmitterPort;
use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent};
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::TranscriptSegment;
use std::sync::Arc;

pub(crate) struct DesktopAsrRuntimeObserver {
    emitter: Arc<dyn EventEmitterPort>,
    metrics: AsrMetricsStore,
}

impl DesktopAsrRuntimeObserver {
    pub(crate) fn new(emitter: Arc<dyn EventEmitterPort>, metrics: AsrMetricsStore) -> Self {
        Self { emitter, metrics }
    }
}

impl AsrRuntimeObserver for DesktopAsrRuntimeObserver {
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

pub(crate) struct CollectingAsrRuntimeObserver {
    inner: DesktopAsrRuntimeObserver,
    segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
}

impl CollectingAsrRuntimeObserver {
    pub(crate) fn new(
        emitter: Arc<dyn EventEmitterPort>,
        metrics: AsrMetricsStore,
        segments: Arc<std::sync::Mutex<Vec<TranscriptSegment>>>,
    ) -> Self {
        Self {
            inner: DesktopAsrRuntimeObserver::new(emitter, metrics),
            segments,
        }
    }
}

impl AsrRuntimeObserver for CollectingAsrRuntimeObserver {
    fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
        self.inner.on_transcript_update(event);
        if let Ok(mut segments) = self.segments.lock() {
            if !event.update.remove_ids.is_empty() {
                segments.retain(|segment| !event.update.remove_ids.contains(&segment.id));
            }
            for upsert in &event.update.upsert_segments {
                if let Some(existing) = segments.iter_mut().find(|item| item.id == upsert.id) {
                    *existing = upsert.clone();
                } else {
                    segments.push(upsert.clone());
                }
            }
        }
    }

    fn on_model_load(&self, metric: &AsrModelLoadMetric) {
        self.inner.on_model_load(metric);
    }

    fn on_live_inference(&self, metric: &AsrInferenceMetric) {
        self.inner.on_live_inference(metric);
    }
}

#[cfg(test)]
mod tests {
    use super::{CollectingAsrRuntimeObserver, DesktopAsrRuntimeObserver};
    use crate::integrations::asr::metrics::{new_metrics_store, snapshot_metrics};
    use crate::platform::event::MockEventEmitter;
    use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent};
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
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
        let observer = DesktopAsrRuntimeObserver::new(emitter.clone(), new_metrics_store());
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
            DesktopAsrRuntimeObserver::new(Arc::new(MockEventEmitter::new()), metrics.clone());
        let model_load = model_load_metric();
        let live = live_metric();

        observer.on_model_load(&model_load);
        observer.on_live_inference(&live);

        let snapshot = snapshot_metrics(&metrics);
        assert_eq!(snapshot.model_load, Some(model_load));
        assert_eq!(snapshot.live_inference, Some(live));
    }

    fn segment(id: &str, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: text.to_string(),
            start: 0.0,
            end: 1.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            draft_translation: None,
            speaker: None,
            speaker_attribution: None,
        }
    }

    #[test]
    fn collecting_observer_applies_incremental_upserts_and_removals() {
        let segments = Arc::new(std::sync::Mutex::new(vec![
            segment("replace", "old"),
            segment("remove", "gone"),
        ]));
        let observer = CollectingAsrRuntimeObserver::new(
            Arc::new(MockEventEmitter::new()),
            new_metrics_store(),
            segments.clone(),
        );
        let event = AsrTranscriptUpdateEvent {
            instance_id: "live-1".to_string(),
            stage: "final".to_string(),
            update: TranscriptUpdate {
                remove_ids: vec!["remove".to_string()],
                upsert_segments: vec![segment("replace", "updated"), segment("new", "added")],
            },
        };

        observer.on_transcript_update(&event);

        let collected = segments.lock().unwrap();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].id, "replace");
        assert_eq!(collected[0].text, "updated");
        assert_eq!(collected[1].id, "new");
        assert_eq!(collected[1].text, "added");
    }
}
