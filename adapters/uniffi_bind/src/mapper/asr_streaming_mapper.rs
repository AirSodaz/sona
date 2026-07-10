use sona_core::ports::asr::AsrTranscriptUpdateEvent;
use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment, TranscriptTiming,
    TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit, TranscriptUpdate,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTranscriptTimingLevel {
    Token,
    Segment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTranscriptTimingSource {
    Model,
    Derived,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTranscriptTimingUnit {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTranscriptTiming {
    pub level: FfiTranscriptTimingLevel,
    pub source: FfiTranscriptTimingSource,
    pub units: Vec<FfiTranscriptTimingUnit>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSpeakerTag {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub score: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSpeakerCandidate {
    pub profile_id: String,
    pub profile_name: String,
    pub score: f32,
    pub rank: u64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSpeakerAttribution {
    pub group_id: String,
    pub anonymous_label: String,
    pub state: String,
    pub source: String,
    pub confidence: String,
    pub candidates: Vec<FfiSpeakerCandidate>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTranscriptSegment {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub is_final: bool,
    pub timing: Option<FfiTranscriptTiming>,
    pub tokens: Option<Vec<String>>,
    pub timestamps: Option<Vec<f32>>,
    pub durations: Option<Vec<f32>>,
    pub translation: Option<String>,
    pub speaker: Option<FfiSpeakerTag>,
    pub speaker_attribution: Option<FfiSpeakerAttribution>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTranscriptUpdate {
    pub remove_ids: Vec<String>,
    pub upsert_segments: Vec<FfiTranscriptSegment>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiAsrTranscriptUpdateEvent {
    pub instance_id: String,
    pub stage: String,
    pub update: FfiTranscriptUpdate,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiAsrModelLoadMetric {
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

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiAsrInferenceMetric {
    pub occurred_at_ms: u64,
    pub source: String,
    pub instance_id: Option<String>,
    pub stage: String,
    pub is_final: bool,
    pub audio_duration_ms: f64,
    pub buffered_samples: u64,
    pub audio_extract_ms: Option<f64>,
    pub decode_ms: f64,
    pub emit_latency_ms: Option<f64>,
    pub total_ms: Option<f64>,
    pub rtf: Option<f64>,
    pub segment_count: Option<u64>,
    pub process_rss_mb: Option<f64>,
}

fn transcript_timing_level_to_ffi(level: TranscriptTimingLevel) -> FfiTranscriptTimingLevel {
    match level {
        TranscriptTimingLevel::Token => FfiTranscriptTimingLevel::Token,
        TranscriptTimingLevel::Segment => FfiTranscriptTimingLevel::Segment,
    }
}

fn transcript_timing_source_to_ffi(source: TranscriptTimingSource) -> FfiTranscriptTimingSource {
    match source {
        TranscriptTimingSource::Model => FfiTranscriptTimingSource::Model,
        TranscriptTimingSource::Derived => FfiTranscriptTimingSource::Derived,
    }
}

fn transcript_timing_unit_to_ffi(unit: &TranscriptTimingUnit) -> FfiTranscriptTimingUnit {
    FfiTranscriptTimingUnit {
        text: unit.text.clone(),
        start: unit.start,
        end: unit.end,
    }
}

fn transcript_timing_to_ffi(timing: &TranscriptTiming) -> FfiTranscriptTiming {
    FfiTranscriptTiming {
        level: transcript_timing_level_to_ffi(timing.level),
        source: transcript_timing_source_to_ffi(timing.source),
        units: timing
            .units
            .iter()
            .map(transcript_timing_unit_to_ffi)
            .collect(),
    }
}

fn speaker_tag_to_ffi(speaker: &SpeakerTag) -> FfiSpeakerTag {
    FfiSpeakerTag {
        id: speaker.id.clone(),
        label: speaker.label.clone(),
        kind: speaker.kind.clone(),
        score: speaker.score,
    }
}

fn speaker_candidate_to_ffi(candidate: &SpeakerCandidate) -> FfiSpeakerCandidate {
    FfiSpeakerCandidate {
        profile_id: candidate.profile_id.clone(),
        profile_name: candidate.profile_name.clone(),
        score: candidate.score,
        rank: u64::try_from(candidate.rank).expect("usize fits u64 on supported UniFFI targets"),
    }
}

fn speaker_attribution_to_ffi(attribution: &SpeakerAttribution) -> FfiSpeakerAttribution {
    FfiSpeakerAttribution {
        group_id: attribution.group_id.clone(),
        anonymous_label: attribution.anonymous_label.clone(),
        state: attribution.state.clone(),
        source: attribution.source.clone(),
        confidence: attribution.confidence.clone(),
        candidates: attribution
            .candidates
            .iter()
            .map(speaker_candidate_to_ffi)
            .collect(),
    }
}

fn transcript_segment_to_ffi(segment: &TranscriptSegment) -> FfiTranscriptSegment {
    FfiTranscriptSegment {
        id: segment.id.clone(),
        text: segment.text.clone(),
        start: segment.start,
        end: segment.end,
        is_final: segment.is_final,
        timing: segment.timing.as_ref().map(transcript_timing_to_ffi),
        tokens: segment.tokens.clone(),
        timestamps: segment.timestamps.clone(),
        durations: segment.durations.clone(),
        translation: segment.translation.clone(),
        speaker: segment.speaker.as_ref().map(speaker_tag_to_ffi),
        speaker_attribution: segment
            .speaker_attribution
            .as_ref()
            .map(speaker_attribution_to_ffi),
    }
}

fn transcript_update_to_ffi(update: &TranscriptUpdate) -> FfiTranscriptUpdate {
    FfiTranscriptUpdate {
        remove_ids: update.remove_ids.clone(),
        upsert_segments: update
            .upsert_segments
            .iter()
            .map(transcript_segment_to_ffi)
            .collect(),
    }
}

pub fn asr_transcript_update_event_to_ffi(
    event: &AsrTranscriptUpdateEvent,
) -> FfiAsrTranscriptUpdateEvent {
    FfiAsrTranscriptUpdateEvent {
        instance_id: event.instance_id.clone(),
        stage: event.stage.clone(),
        update: transcript_update_to_ffi(&event.update),
    }
}

pub fn asr_model_load_metric_to_ffi(metric: &AsrModelLoadMetric) -> FfiAsrModelLoadMetric {
    FfiAsrModelLoadMetric {
        occurred_at_ms: metric.occurred_at_ms,
        instance_id: metric.instance_id.clone(),
        model_path: metric.model_path.clone(),
        model_type: metric.model_type.clone(),
        recognizer_kind: metric.recognizer_kind.clone(),
        num_threads: metric.num_threads,
        reused_from_pool: metric.reused_from_pool,
        load_ms: metric.load_ms,
        rss_before_mb: metric.rss_before_mb,
        rss_after_mb: metric.rss_after_mb,
        rss_delta_mb: metric.rss_delta_mb,
        process_rss_mb: metric.process_rss_mb,
    }
}

pub fn asr_inference_metric_to_ffi(metric: &AsrInferenceMetric) -> FfiAsrInferenceMetric {
    FfiAsrInferenceMetric {
        occurred_at_ms: metric.occurred_at_ms,
        source: metric.source.clone(),
        instance_id: metric.instance_id.clone(),
        stage: metric.stage.clone(),
        is_final: metric.is_final,
        audio_duration_ms: metric.audio_duration_ms,
        buffered_samples: u64::try_from(metric.buffered_samples)
            .expect("usize fits u64 on supported UniFFI targets"),
        audio_extract_ms: metric.audio_extract_ms,
        decode_ms: metric.decode_ms,
        emit_latency_ms: metric.emit_latency_ms,
        total_ms: metric.total_ms,
        rtf: metric.rtf,
        segment_count: metric
            .segment_count
            .map(|count| u64::try_from(count).expect("usize fits u64 on supported UniFFI targets")),
        process_rss_mb: metric.process_rss_mb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::AsrTranscriptUpdateEvent;
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::transcript::{
        SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment, TranscriptTiming,
        TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit, TranscriptUpdate,
    };

    fn complete_segment() -> TranscriptSegment {
        TranscriptSegment {
            id: "segment-1".into(),
            text: "hello world".into(),
            start: 1.25,
            end: 2.5,
            is_final: true,
            timing: Some(TranscriptTiming {
                level: TranscriptTimingLevel::Token,
                source: TranscriptTimingSource::Model,
                units: vec![TranscriptTimingUnit {
                    text: "hello".into(),
                    start: 1.25,
                    end: 1.75,
                }],
            }),
            tokens: Some(vec!["hello".into(), "world".into()]),
            timestamps: Some(vec![1.25, 1.75]),
            durations: Some(vec![0.5, 0.75]),
            translation: Some("你好，世界".into()),
            speaker: Some(SpeakerTag {
                id: "speaker-1".into(),
                label: "Speaker 1".into(),
                kind: "profile".into(),
                score: Some(0.95),
            }),
            speaker_attribution: Some(SpeakerAttribution {
                group_id: "group-1".into(),
                anonymous_label: "Speaker 1".into(),
                state: "matched".into(),
                source: "embedding".into(),
                confidence: "high".into(),
                candidates: vec![SpeakerCandidate {
                    profile_id: "profile-1".into(),
                    profile_name: "Alice".into(),
                    score: 0.9,
                    rank: 1,
                }],
            }),
        }
    }

    fn complete_model_load_metric() -> AsrModelLoadMetric {
        AsrModelLoadMetric {
            occurred_at_ms: 1_725_000_000_000,
            instance_id: "live-1".into(),
            model_path: "models/asr.onnx".into(),
            model_type: "zipformer".into(),
            recognizer_kind: "streaming".into(),
            num_threads: 4,
            reused_from_pool: false,
            load_ms: 123.5,
            rss_before_mb: Some(100.0),
            rss_after_mb: Some(140.0),
            rss_delta_mb: Some(40.0),
            process_rss_mb: Some(160.0),
        }
    }

    fn complete_inference_metric() -> AsrInferenceMetric {
        AsrInferenceMetric {
            occurred_at_ms: 1_725_000_000_100,
            source: "microphone".into(),
            instance_id: Some("live-1".into()),
            stage: "volcengine_streaming".into(),
            is_final: true,
            audio_duration_ms: 1_000.0,
            buffered_samples: 16_000,
            audio_extract_ms: Some(2.5),
            decode_ms: 250.0,
            emit_latency_ms: Some(12.0),
            total_ms: Some(264.5),
            rtf: Some(0.25),
            segment_count: Some(2),
            process_rss_mb: Some(180.0),
        }
    }

    #[test]
    fn transcript_update_mapping_preserves_nested_timing_speaker_and_legacy_fields() {
        let event = AsrTranscriptUpdateEvent {
            instance_id: "live-1".into(),
            stage: "volcengine_streaming".into(),
            update: TranscriptUpdate {
                remove_ids: vec!["old".into()],
                upsert_segments: vec![complete_segment()],
            },
        };

        let mapped = asr_transcript_update_event_to_ffi(&event);
        let segment = &mapped.update.upsert_segments[0];
        let timing = segment.timing.as_ref().expect("timing");
        let speaker = segment.speaker.as_ref().expect("speaker");
        let attribution = segment
            .speaker_attribution
            .as_ref()
            .expect("speaker attribution");
        let candidate = &attribution.candidates[0];

        assert_eq!(mapped.instance_id, "live-1");
        assert_eq!(mapped.stage, "volcengine_streaming");
        assert_eq!(mapped.update.remove_ids, vec!["old"]);
        assert_eq!(segment.id, "segment-1");
        assert_eq!(segment.text, "hello world");
        assert_eq!(segment.start, 1.25);
        assert_eq!(segment.end, 2.5);
        assert!(segment.is_final);
        assert_eq!(timing.level, FfiTranscriptTimingLevel::Token);
        assert_eq!(timing.source, FfiTranscriptTimingSource::Model);
        assert_eq!(timing.units.len(), 1);
        assert_eq!(timing.units[0].text, "hello");
        assert_eq!(timing.units[0].start, 1.25);
        assert_eq!(timing.units[0].end, 1.75);
        assert_eq!(
            segment.tokens.as_deref(),
            Some(&["hello".into(), "world".into()][..])
        );
        assert_eq!(segment.timestamps.as_deref(), Some(&[1.25, 1.75][..]));
        assert_eq!(segment.durations.as_deref(), Some(&[0.5, 0.75][..]));
        assert_eq!(segment.translation.as_deref(), Some("你好，世界"));
        assert_eq!(speaker.id, "speaker-1");
        assert_eq!(speaker.label, "Speaker 1");
        assert_eq!(speaker.kind, "profile");
        assert_eq!(speaker.score, Some(0.95));
        assert_eq!(attribution.group_id, "group-1");
        assert_eq!(attribution.anonymous_label, "Speaker 1");
        assert_eq!(attribution.state, "matched");
        assert_eq!(attribution.source, "embedding");
        assert_eq!(attribution.confidence, "high");
        assert_eq!(candidate.profile_id, "profile-1");
        assert_eq!(candidate.profile_name, "Alice");
        assert_eq!(candidate.score, 0.9);
        let rank: u64 = candidate.rank;
        assert_eq!(rank, 1);
    }

    #[test]
    fn metric_mapping_preserves_optional_values_and_counts() {
        let model_load = asr_model_load_metric_to_ffi(&complete_model_load_metric());
        assert_eq!(model_load.occurred_at_ms, 1_725_000_000_000);
        assert_eq!(model_load.instance_id, "live-1");
        assert_eq!(model_load.model_path, "models/asr.onnx");
        assert_eq!(model_load.model_type, "zipformer");
        assert_eq!(model_load.recognizer_kind, "streaming");
        assert_eq!(model_load.num_threads, 4);
        assert!(!model_load.reused_from_pool);
        assert_eq!(model_load.load_ms, 123.5);
        assert_eq!(model_load.rss_before_mb, Some(100.0));
        assert_eq!(model_load.rss_after_mb, Some(140.0));
        assert_eq!(model_load.rss_delta_mb, Some(40.0));
        assert_eq!(model_load.process_rss_mb, Some(160.0));

        let inference = asr_inference_metric_to_ffi(&complete_inference_metric());
        assert_eq!(inference.occurred_at_ms, 1_725_000_000_100);
        assert_eq!(inference.source, "microphone");
        assert_eq!(inference.instance_id.as_deref(), Some("live-1"));
        assert_eq!(inference.stage, "volcengine_streaming");
        assert!(inference.is_final);
        assert_eq!(inference.audio_duration_ms, 1_000.0);
        let buffered_samples: u64 = inference.buffered_samples;
        assert_eq!(buffered_samples, 16_000);
        assert_eq!(inference.audio_extract_ms, Some(2.5));
        assert_eq!(inference.decode_ms, 250.0);
        assert_eq!(inference.emit_latency_ms, Some(12.0));
        assert_eq!(inference.total_ms, Some(264.5));
        assert_eq!(inference.rtf, Some(0.25));
        let segment_count: Option<u64> = inference.segment_count;
        assert_eq!(segment_count, Some(2));
        assert_eq!(inference.process_rss_mb, Some(180.0));
    }

    #[test]
    fn mapping_preserves_absent_optional_fields() {
        let event = AsrTranscriptUpdateEvent {
            instance_id: "live-2".into(),
            stage: "local_streaming".into(),
            update: TranscriptUpdate {
                remove_ids: Vec::new(),
                upsert_segments: vec![TranscriptSegment {
                    id: "segment-2".into(),
                    text: "pending".into(),
                    start: 0.0,
                    end: 0.5,
                    is_final: false,
                    timing: None,
                    tokens: None,
                    timestamps: None,
                    durations: None,
                    translation: None,
                    speaker: None,
                    speaker_attribution: None,
                }],
            },
        };
        let segment = &asr_transcript_update_event_to_ffi(&event)
            .update
            .upsert_segments[0];
        assert_eq!(segment.timing, None);
        assert_eq!(segment.tokens, None);
        assert_eq!(segment.timestamps, None);
        assert_eq!(segment.durations, None);
        assert_eq!(segment.translation, None);
        assert_eq!(segment.speaker, None);
        assert_eq!(segment.speaker_attribution, None);

        let mut segment_with_speaker = complete_segment();
        segment_with_speaker.speaker = Some(SpeakerTag {
            id: "speaker-2".into(),
            label: "Speaker 2".into(),
            kind: "anonymous".into(),
            score: None,
        });
        let event = AsrTranscriptUpdateEvent {
            instance_id: "live-3".into(),
            stage: "local_streaming".into(),
            update: TranscriptUpdate {
                remove_ids: Vec::new(),
                upsert_segments: vec![segment_with_speaker],
            },
        };
        let mapped = asr_transcript_update_event_to_ffi(&event);
        let speaker = mapped.update.upsert_segments[0]
            .speaker
            .as_ref()
            .expect("speaker");
        assert_eq!(speaker.score, None);

        let model_load = AsrModelLoadMetric {
            rss_before_mb: None,
            rss_after_mb: None,
            rss_delta_mb: None,
            process_rss_mb: None,
            ..complete_model_load_metric()
        };
        let model_load = asr_model_load_metric_to_ffi(&model_load);
        assert_eq!(model_load.rss_before_mb, None);
        assert_eq!(model_load.rss_after_mb, None);
        assert_eq!(model_load.rss_delta_mb, None);
        assert_eq!(model_load.process_rss_mb, None);

        let inference = AsrInferenceMetric {
            instance_id: None,
            audio_extract_ms: None,
            emit_latency_ms: None,
            total_ms: None,
            rtf: None,
            segment_count: None,
            process_rss_mb: None,
            ..complete_inference_metric()
        };
        let inference = asr_inference_metric_to_ffi(&inference);
        assert_eq!(inference.instance_id, None);
        assert_eq!(inference.audio_extract_ms, None);
        assert_eq!(inference.emit_latency_ms, None);
        assert_eq!(inference.total_ms, None);
        assert_eq!(inference.rtf, None);
        assert_eq!(inference.segment_count, None);
        assert_eq!(inference.process_rss_mb, None);
    }
}
