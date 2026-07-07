use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AsrRuntimeMetricsSnapshot {
    pub model_load: Option<AsrModelLoadMetric>,
    pub live_inference: Option<AsrInferenceMetric>,
    pub batch_inference: Option<AsrInferenceMetric>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AsrModelLoadMetric {
    pub occurred_at_ms: u64,
    pub instance_id: String,
    pub model_path: String,
    pub model_type: String,
    pub recognizer_kind: String,
    pub num_threads: i32,
    pub reused_from_pool: bool,
    pub load_ms: f64,
    pub rss_before_mb: Option<f64>,
    pub rss_after_mb: Option<f64>,
    pub rss_delta_mb: Option<f64>,
    pub process_rss_mb: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AsrInferenceMetric {
    pub occurred_at_ms: u64,
    pub source: String,
    pub instance_id: Option<String>,
    pub stage: String,
    pub is_final: bool,
    pub audio_duration_ms: f64,
    pub buffered_samples: usize,
    pub audio_extract_ms: Option<f64>,
    pub decode_ms: f64,
    pub emit_latency_ms: Option<f64>,
    pub total_ms: Option<f64>,
    pub rtf: Option<f64>,
    pub segment_count: Option<usize>,
    pub process_rss_mb: Option<f64>,
}

pub fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn duration_to_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

pub fn samples_to_ms(samples: usize, sample_rate: f64) -> f64 {
    if sample_rate <= 0.0 {
        return 0.0;
    }

    samples as f64 / sample_rate * 1000.0
}

pub fn calculate_rtf(decode_ms: f64, audio_duration_ms: f64) -> Option<f64> {
    (audio_duration_ms > 0.0).then(|| decode_ms / audio_duration_ms)
}

pub fn calculate_rss_delta_mb(
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

pub fn format_optional_mb(value: Option<f64>) -> String {
    value
        .map(|inner| format!("{inner:.1}"))
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn format_optional_ms(value: Option<f64>) -> String {
    value
        .map(|inner| format!("{inner:.1}"))
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn format_optional_rtf(value: Option<f64>) -> String {
    value
        .map(|inner| format!("{inner:.4}"))
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn format_optional_count(value: Option<usize>) -> String {
    value
        .map(|inner| inner.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_and_sample_helpers_use_milliseconds() {
        assert_eq!(duration_to_ms(Duration::from_millis(1250)), 1250.0);
        assert_eq!(samples_to_ms(16_000, 16_000.0), 1000.0);
        assert_eq!(samples_to_ms(16_000, 0.0), 0.0);
    }

    #[test]
    fn rtf_is_only_available_for_positive_audio_duration() {
        assert_eq!(calculate_rtf(250.0, 1000.0), Some(0.25));
        assert_eq!(calculate_rtf(250.0, 0.0), None);
    }

    #[test]
    fn rss_delta_is_suppressed_for_pool_reuse() {
        assert_eq!(calculate_rss_delta_mb(Some(100.0), Some(140.0), true), None);
        assert_eq!(
            calculate_rss_delta_mb(Some(100.0), Some(140.0), false),
            Some(40.0)
        );
        assert_eq!(calculate_rss_delta_mb(Some(100.0), None, false), None);
    }

    #[test]
    fn metric_optional_formatting_is_stable() {
        assert_eq!(format_optional_mb(Some(12.34)), "12.3");
        assert_eq!(format_optional_ms(Some(12.34)), "12.3");
        assert_eq!(format_optional_rtf(Some(0.12345)), "0.1235");
        assert_eq!(format_optional_count(Some(7)), "7");
        assert_eq!(format_optional_mb(None), "unknown");
        assert_eq!(format_optional_ms(None), "unknown");
        assert_eq!(format_optional_rtf(None), "unknown");
        assert_eq!(format_optional_count(None), "unknown");
    }
}
