use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingLevel {
    Token,
    Segment,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingSource {
    Model,
    Derived,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTimingUnit {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTiming {
    pub level: TranscriptTimingLevel,
    pub source: TranscriptTimingSource,
    pub units: Vec<TranscriptTimingUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerTag {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerCandidate {
    pub profile_id: String,
    pub profile_name: String,
    pub score: f32,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerAttribution {
    pub group_id: String,
    pub anonymous_label: String,
    pub state: String,
    pub source: String,
    pub confidence: String,
    pub candidates: Vec<SpeakerCandidate>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TranscriptTiming>,
    // Legacy raw fields are still written for compatibility with older
    // persisted transcript records and upgrade paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durations: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<SpeakerTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_attribution: Option<SpeakerAttribution>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptUpdate {
    pub remove_ids: Vec<String>,
    pub upsert_segments: Vec<TranscriptSegment>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_segment_keeps_camel_case_transport_shape() {
        let segment = TranscriptSegment {
            id: "seg-1".to_string(),
            text: "hello".to_string(),
            start: 0.0,
            end: 1.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };

        let value = serde_json::to_value(segment).unwrap();

        assert_eq!(value["isFinal"], true);
        assert!(value.get("is_final").is_none());
    }
}
