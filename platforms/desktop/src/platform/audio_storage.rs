use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};

pub fn create_history_recording_path(provider: &dyn PathProvider) -> Result<String, String> {
    let app_data_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;
    let history_dir = app_data_dir.join("history");
    sona_runtime_fs::ensure_directory_exists(&history_dir).map_err(|error| error.to_string())?;

    let wav_filename = format!("{}.wav", uuid::Uuid::new_v4());
    let wav_filepath = history_dir.join(&wav_filename);
    Ok(wav_filepath.to_string_lossy().into_owned())
}

pub fn create_history_recording_path_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<String, String> {
    let provider = TauriPathProvider::from_app(app);
    create_history_recording_path(&provider)
}
