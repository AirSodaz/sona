use log::info;
use sona_core::ports::asr::AsrRuntimeObserver;
use sona_core::transcription::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, calculate_rtf, format_optional_count,
    format_optional_mb, format_optional_ms, format_optional_rtf, samples_to_ms,
};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{ProcessesToUpdate, System};

const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

pub(super) fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(super) fn capture_process_memory_mb() -> Option<f64> {
    let pid = sysinfo::get_current_pid().ok()?;
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    system
        .process(pid)
        .map(|process| process.memory() as f64 / BYTES_PER_MB)
}

pub(super) fn build_live_metric(
    instance_id: &str,
    stage: &str,
    is_final: bool,
    buffered_samples: usize,
    decode_ms: f64,
    emit_latency_ms: Option<f64>,
    total_ms: Option<f64>,
) -> AsrInferenceMetric {
    let audio_duration_ms = samples_to_ms(buffered_samples, 16_000.0);
    AsrInferenceMetric {
        occurred_at_ms: current_time_millis(),
        source: "live".to_string(),
        instance_id: Some(instance_id.to_string()),
        stage: stage.to_string(),
        is_final,
        audio_duration_ms,
        buffered_samples,
        audio_extract_ms: None,
        decode_ms,
        emit_latency_ms,
        total_ms,
        rtf: calculate_rtf(decode_ms, audio_duration_ms),
        segment_count: None,
        process_rss_mb: capture_process_memory_mb(),
    }
}

pub(super) fn record_live_metric(observer: &dyn AsrRuntimeObserver, metric: AsrInferenceMetric) {
    observer.on_live_inference(&metric);
    log_inference_metric(&metric);
}

pub(super) fn log_model_load_metric(metric: &AsrModelLoadMetric) {
    info!(
        target: "asr_metrics",
        "event=asr_model_load instance_id={} model_path={:?} model_type={} recognizer_kind={} num_threads={} reused_from_pool={} load_ms={:.1} rss_before_mb={} rss_after_mb={} rss_delta_mb={} process_rss_mb={}",
        metric.instance_id,
        metric.model_path,
        metric.model_type,
        metric.recognizer_kind,
        metric.num_threads,
        metric.reused_from_pool,
        metric.load_ms,
        format_optional_mb(metric.rss_before_mb),
        format_optional_mb(metric.rss_after_mb),
        format_optional_mb(metric.rss_delta_mb),
        format_optional_mb(metric.process_rss_mb),
    );
}

pub(super) fn log_inference_metric(metric: &AsrInferenceMetric) {
    info!(
        target: "asr_metrics",
        "event=asr_inference source={} instance_id={} stage={} final={} audio_duration_ms={:.1} buffered_samples={} audio_extract_ms={} decode_ms={:.1} emit_latency_ms={} total_ms={} rtf={} process_rss_mb={} segment_count={}",
        metric.source,
        metric.instance_id.as_deref().unwrap_or("none"),
        metric.stage,
        metric.is_final,
        metric.audio_duration_ms,
        metric.buffered_samples,
        format_optional_ms(metric.audio_extract_ms),
        metric.decode_ms,
        format_optional_ms(metric.emit_latency_ms),
        format_optional_ms(metric.total_ms),
        format_optional_rtf(metric.rtf),
        format_optional_mb(metric.process_rss_mb),
        format_optional_count(metric.segment_count),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent};
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingObserver {
        live: Mutex<Vec<AsrInferenceMetric>>,
    }

    impl AsrRuntimeObserver for RecordingObserver {
        fn on_transcript_update(&self, _event: &AsrTranscriptUpdateEvent) {}
        fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}
        fn on_live_inference(&self, metric: &AsrInferenceMetric) {
            self.live.lock().unwrap().push(metric.clone());
        }
    }

    #[test]
    fn timestamp_and_process_memory_are_valid_runtime_values() {
        assert!(current_time_millis() > 0);
        let memory = capture_process_memory_mb();
        assert!(memory.is_none() || memory.unwrap() >= 0.0);
    }

    #[test]
    fn live_metric_is_typed_and_delivered_through_core_observer() {
        let observer = RecordingObserver::default();
        let metric = build_live_metric("live-1", "partial", false, 1_600, 10.0, None, None);

        record_live_metric(&observer, metric.clone());

        assert_eq!(*observer.live.lock().unwrap(), vec![metric]);
    }
}
