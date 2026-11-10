use crate::services::DesktopServices;
use tauri::State;

#[tauri::command]
pub async fn extract_tar_bz2(
    services: State<'_, DesktopServices>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    crate::platform::archive::extract_tar_bz2_with_emitter(
        services.emitter.clone(),
        archive_path,
        target_dir,
    )
    .await
}

#[tauri::command]
pub async fn create_tar_bz2(source_dir: String, archive_path: String) -> Result<(), String> {
    crate::platform::archive::create_tar_bz2(source_dir, archive_path).await
}
