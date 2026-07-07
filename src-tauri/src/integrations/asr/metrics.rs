use log::{info, warn};
pub use sona_core::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{ProcessesToUpdate, System};

const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

pub(crate) type AsrMetricsStore = Arc<Mutex<AsrRuntimeMetricsSnapshot>>;

pub(crate) fn new_metrics_store() -> AsrMetricsStore {
    Arc::new(Mutex::new(AsrRuntimeMetricsSnapshot::default()))
}

pub(crate) fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn duration_to_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub(crate) fn samples_to_ms(samples: usize, sample_rate: f64) -> f64 {
    if sample_rate <= 0.0 {
        return 0.0;
    }

    samples as f64 / sample_rate * 1000.0
}

pub(crate) fn calculate_rtf(decode_ms: f64, audio_duration_ms: f64) -> Option<f64> {
    (audio_duration_ms > 0.0).then(|| decode_ms / audio_duration_ms)
}

pub(crate) fn calculate_rss_delta_mb(
    before_mb: Option<f64>,
    after_mb: Option<f64>,
    reused_from_pool: bool,
) -> Option<f64> {
    if reused_from_pool {
        return None;
    }

    match (before_mb, after_mb) {
        (Some(before), Some(after)) => Some(after - before),
        _ => None,
    }
}

pub(crate) fn capture_process_memory_mb() -> Option<f64> {
    let pid = sysinfo::get_current_pid().ok()?;
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    system
        .process(pid)
        .map(|process| process.memory() as f64 / BYTES_PER_MB)
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

pub(crate) fn log_model_load_metric(metric: &AsrModelLoadMetric) {
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
        metric.rtf.map(|value| format!("{value:.4}")).unwrap_or_else(|| "unknown".to_string()),
        format_optional_mb(metric.process_rss_mb),
        metric.segment_count.map(|value| value.to_string()).unwrap_or_else(|| "unknown".to_string()),
    );
}

fn format_optional_mb(value: Option<f64>) -> String {
    value
        .map(|inner| format!("{inner:.1}"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_optional_ms(value: Option<f64>) -> String {
    value
        .map(|inner| format!("{inner:.1}"))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_memory_capture_is_optional_but_never_negative() {
        let memory = capture_process_memory_mb();

        assert!(memory.is_none() || memory.unwrap() >= 0.0);
    }

    #[test]
    fn rss_delta_is_suppressed_for_pool_reuse() {
        assert_eq!(calculate_rss_delta_mb(Some(100.0), Some(140.0), true), None);
        assert_eq!(
            calculate_rss_delta_mb(Some(100.0), Some(140.0), false),
            Some(40.0)
        );
    }
}
