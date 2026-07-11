#[tauri::command]
pub async fn extract_tar_bz2<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    crate::platform::archive::extract_tar_bz2(app, archive_path, target_dir).await
}

#[tauri::command]
pub async fn create_tar_bz2(source_dir: String, archive_path: String) -> Result<(), String> {
    crate::platform::archive::create_tar_bz2(source_dir, archive_path).await
}
