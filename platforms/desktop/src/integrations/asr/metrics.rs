use log::{info, warn};
pub use sona_core::transcription::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot, duration_to_ms,
    format_optional_count, format_optional_mb, format_optional_ms, format_optional_rtf,
};
use std::sync::{Arc, Mutex};

pub(crate) type AsrMetricsStore = Arc<Mutex<AsrRuntimeMetricsSnapshot>>;

pub(crate) fn new_metrics_store() -> AsrMetricsStore {
    Arc::new(Mutex::new(AsrRuntimeMetricsSnapshot::default()))
}

pub(crate) fn current_time_millis() -> u64 {
    crate::platform::time::unix_timestamp_millis()
}

fn update_metrics_snapshot(
    metrics_store: &AsrMetricsStore,
    update: impl FnOnce(&mut AsrRuntimeMetricsSnapshot),
) {
    match metrics_store.lock() {
        Ok(mut snapshot) => update(&mut snapshot),
        Err(error) => warn!("[ASR Metrics] failed to lock metrics store: {error}"),
    }
}

pub(crate) fn set_model_load_metric(metrics_store: &AsrMetricsStore, metric: AsrModelLoadMetric) {
    update_metrics_snapshot(metrics_store, |snapshot| {
        snapshot.model_load = Some(metric);
    });
}

pub(crate) fn set_live_inference_metric(
    metrics_store: &AsrMetricsStore,
    metric: AsrInferenceMetric,
) {
    update_metrics_snapshot(metrics_store, |snapshot| {
        snapshot.live_inference = Some(metric);
    });
}

pub(crate) fn set_batch_inference_metric(
    metrics_store: &AsrMetricsStore,
    metric: AsrInferenceMetric,
) {
    update_metrics_snapshot(metrics_store, |snapshot| {
        snapshot.batch_inference = Some(metric);
    });
}

pub(crate) fn snapshot_metrics(metrics_store: &AsrMetricsStore) -> AsrRuntimeMetricsSnapshot {
    metrics_store
        .lock()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_default()
}

pub(crate) fn log_inference_metric(metric: &AsrInferenceMetric) {
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
