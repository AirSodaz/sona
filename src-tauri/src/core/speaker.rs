use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerProfileSample {
    pub id: String,
    pub file_path: String,
    pub source_name: String,
    pub duration_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerProfile {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub samples: Vec<SpeakerProfileSample>,
}
