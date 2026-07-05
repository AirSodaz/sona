use serde::{Deserialize, Serialize};

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
