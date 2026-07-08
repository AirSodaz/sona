use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStatus {
    pub ffmpeg_path: String,
    pub ffmpeg_exists: bool,
    pub log_dir_path: String,
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
