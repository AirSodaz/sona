use tauri::Emitter;

const EXTRACT_PROGRESS_EVENT: &str = "extract-progress";

pub async fn extract_tar_bz2<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        sona_archive::extract_tar_bz2(&archive_path, &target_dir, |path_str| {
            let _ = app.emit(EXTRACT_PROGRESS_EVENT, path_str);
        })
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}

pub async fn create_tar_bz2(source_dir: String, archive_path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        sona_archive::create_tar_bz2(&source_dir, &archive_path)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}
