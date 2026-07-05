use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::io::ErrorKind;

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

pub fn resolve_runtime_path_status(path: &str) -> RuntimePathStatus {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::File,
            error: None,
        },
        Ok(metadata) if metadata.is_dir() => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Directory,
            error: None,
        },
        Ok(_) => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Unknown,
            error: Some("Path exists but is neither a regular file nor directory.".to_string()),
        },
        Err(error) if error.kind() == ErrorKind::NotFound => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Missing,
            error: None,
        },
        Err(error) => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Unknown,
            error: Some(error.to_string()),
        },
    }
}
