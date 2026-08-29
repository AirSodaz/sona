use tauri::Emitter;

use crate::platform::blocking::{map_err_string, spawn_blocking_map};

const EXTRACT_PROGRESS_EVENT: &str = "extract-progress";

pub async fn extract_tar_bz2<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    spawn_blocking_map(move || {
        sona_archive::extract_tar_bz2(&archive_path, &target_dir, |path_str| {
            let _ = app.emit(EXTRACT_PROGRESS_EVENT, path_str);
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
