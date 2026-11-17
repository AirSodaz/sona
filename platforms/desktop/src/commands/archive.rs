use crate::services::DesktopServices;
use tauri::State;

#[tauri::command]
pub async fn extract_tar_bz2(
    services: State<'_, DesktopServices>,
    archive_path: String,
    target_dir: String,
) -> Result<(), String> {
    log::info!(
        "[ArchiveCommand] Starting extract_tar_bz2: archive_path={archive_path}, target_dir={target_dir}"
    );
    match crate::platform::archive::extract_tar_bz2_with_emitter(
        services.emitter.clone(),
        archive_path,
        target_dir,
    )
    .await
    {
        Ok(()) => {
            log::info!("[ArchiveCommand] extract_tar_bz2 completed successfully");
            Ok(())
        }
        Err(err) => {
            log::error!("[ArchiveCommand] extract_tar_bz2 failed: {err}");
            Err(err)
        }
    }
}

#[tauri::command]
pub async fn create_tar_bz2(source_dir: String, archive_path: String) -> Result<(), String> {
    log::info!(
        "[ArchiveCommand] Starting create_tar_bz2: source_dir={source_dir}, archive_path={archive_path}"
    );
    match crate::platform::archive::create_tar_bz2(source_dir, archive_path).await {
        Ok(()) => {
            log::info!("[ArchiveCommand] create_tar_bz2 completed successfully");
            Ok(())
        }
        Err(err) => {
            log::error!("[ArchiveCommand] create_tar_bz2 failed: {err}");
            Err(err)
        }
    }
}
