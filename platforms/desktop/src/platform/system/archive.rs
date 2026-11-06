use sona_core::ports::event::EventEmitterPort;
use std::sync::Arc;

use crate::platform::blocking::{map_err_string, spawn_blocking_map};

pub const EXTRACT_PROGRESS_EVENT: &str = "extract-progress";

pub async fn extract_tar_bz2(
    emitter: Option<Arc<dyn EventEmitterPort>>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    spawn_blocking_map(move || {
        sona_archive::extract_tar_bz2(&archive_path, &target_dir, |path_str| {
            if let Some(emitter) = &emitter {
                let _ = emitter.emit(EXTRACT_PROGRESS_EVENT, serde_json::json!(path_str));
            }
        })
        .map_err(map_err_string)
    })
    .await
}

pub async fn create_tar_bz2(source_dir: String, archive_path: String) -> Result<(), String> {
    spawn_blocking_map(move || {
        sona_archive::create_tar_bz2(&source_dir, &archive_path).map_err(map_err_string)
    })
    .await
}
