use tauri::Manager;

pub use sona_core::runtime::{
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus, resolve_runtime_path_status,
};

pub(crate) async fn open_log_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e: tauri::Error| e.to_string())?;

    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).map_err(|e: std::io::Error| e.to_string())?;
    }

    app.opener()
        .open_path(log_dir.to_string_lossy(), None::<&str>)
        .map_err(|e: tauri_plugin_opener::Error| e.to_string())?;
    Ok(())
}

pub(crate) fn resolve_runtime_environment_status(
    provider: &dyn crate::core::paths::PathProvider,
) -> Result<RuntimeEnvironmentStatus, String> {
    let ffmpeg_path = crate::core::pipeline::resolve_ffmpeg_sidecar_path()?;
    let log_dir = provider
        .resolve_path(crate::core::paths::PathKind::AppLogData)
        .map_err(|e: String| -> String { e })?;

    Ok(RuntimeEnvironmentStatus {
        ffmpeg_path: ffmpeg_path.to_string_lossy().into_owned(),
        ffmpeg_exists: ffmpeg_path.exists(),
        log_dir_path: log_dir.to_string_lossy().into_owned(),
    })
}

pub(crate) async fn get_runtime_environment_status(
    app: tauri::AppHandle,
) -> Result<RuntimeEnvironmentStatus, String> {
    let provider = crate::core::paths::TauriPathProvider::from_app(&app);
    resolve_runtime_environment_status(&provider)
}

pub(crate) async fn get_path_statuses(
    paths: Vec<String>,
) -> Result<Vec<RuntimePathStatus>, String> {
    Ok(paths
        .into_iter()
        .map(|path| resolve_runtime_path_status(&path))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{RuntimePathKind, resolve_runtime_path_status};
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn resolve_runtime_path_status_detects_existing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("sample.txt");
        File::create(&file_path).unwrap();

        let status = resolve_runtime_path_status(file_path.to_string_lossy().as_ref());

        assert_eq!(status.kind, RuntimePathKind::File);
        assert_eq!(status.error, None);
    }

    #[test]
    fn resolve_runtime_path_status_detects_existing_directory() {
        let dir = tempdir().unwrap();

        let status = resolve_runtime_path_status(dir.path().to_string_lossy().as_ref());

        assert_eq!(status.kind, RuntimePathKind::Directory);
        assert_eq!(status.error, None);
    }

    #[test]
    fn resolve_runtime_path_status_detects_missing_path() {
        let dir = tempdir().unwrap();
        let missing_path = dir.path().join("missing.txt");

        let status = resolve_runtime_path_status(missing_path.to_string_lossy().as_ref());

        assert_eq!(status.kind, RuntimePathKind::Missing);
        assert_eq!(status.error, None);
    }

    #[test]
    fn resolve_runtime_path_status_returns_unknown_for_invalid_path() {
        let invalid_path = "C:\\0\0invalid";

        let status = resolve_runtime_path_status(invalid_path);

        assert_eq!(status.kind, RuntimePathKind::Unknown);
        assert!(status.error.is_some());
    }
}
