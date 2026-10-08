use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStatus {
    pub ffmpeg_path: String,
    pub ffmpeg_exists: bool,
    pub log_dir_path: String,
    #[serde(default = "default_true")]
    pub builtin_audio_decoder_ready: bool,
}

impl Default for RuntimeEnvironmentStatus {
    fn default() -> Self {
        Self {
            ffmpeg_path: String::new(),
            ffmpeg_exists: false,
            log_dir_path: String::new(),
            builtin_audio_decoder_ready: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum RuntimePathKind {
    File,
    Directory,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RuntimePathStatus {
    pub path: String,
    pub kind: RuntimePathKind,
    pub error: Option<String>,
}
