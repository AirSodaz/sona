use crate::platform::paths::{PathKind, PathPort, TauriPathProvider};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiServerRuntimeDirs {
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
}

pub fn resolve_api_server_runtime_dirs(
    provider: &dyn PathPort,
) -> Result<ApiServerRuntimeDirs, String> {
    let app_local_data_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;

    let active_data_dir =
        crate::platform::storage_location::resolve_active_data_dir(&app_local_data_dir);
    let models_dir = crate::platform::storage_location::resolve_active_models_dir(
        &app_local_data_dir,
        &active_data_dir,
    );

    Ok(ApiServerRuntimeDirs {
        temp_dir: app_local_data_dir.join("api_temp"),
        models_dir,
    })
}

pub fn resolve_api_server_runtime_dirs_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<ApiServerRuntimeDirs, String> {
    let provider = TauriPathProvider::from_app(app);
    let mut dirs = resolve_api_server_runtime_dirs(&provider)?;
    if let Ok(active_models_dir) =
        crate::platform::storage_location::resolve_active_models_dir_for_app(app)
    {
        dirs.models_dir = active_models_dir;
    }
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::paths::{MockPathProvider, PathPortError};
    use std::collections::HashMap;

    #[test]
    fn resolves_api_server_runtime_dirs_from_app_local_data() {
        let app_local_data = PathBuf::from("C:/sona/app-local-data");
        let mut entries = HashMap::new();
        entries.insert(PathKind::AppLocalData, Ok(app_local_data.clone()));
        let provider = MockPathProvider::from_map(entries);

        let dirs = resolve_api_server_runtime_dirs(&provider).unwrap();

        assert_eq!(dirs.temp_dir, app_local_data.join("api_temp"));
        assert_eq!(dirs.models_dir, app_local_data.join("models"));
    }

    #[test]
    fn resolves_custom_models_dir_when_configured() {
        let temp = tempfile::tempdir().unwrap();
        let app_local_data = temp.path().to_path_buf();
        let custom_models_dir = temp.path().join("my_custom_models");
        std::fs::create_dir_all(&custom_models_dir).unwrap();

        let bootstrap = crate::platform::storage_location::StorageBootstrapConfig {
            custom_data_dir: None,
            custom_models_dir: Some(custom_models_dir.clone()),
            pending_cleanup_dirs: vec![],
        };
        crate::platform::storage_location::save_bootstrap_config(&app_local_data, &bootstrap)
            .unwrap();

        let mut entries = HashMap::new();
        entries.insert(PathKind::AppLocalData, Ok(app_local_data));
        let provider = MockPathProvider::from_map(entries);

        let dirs = resolve_api_server_runtime_dirs(&provider).unwrap();
        assert_eq!(dirs.models_dir, custom_models_dir);
    }

    #[test]
    fn propagates_app_local_data_resolution_errors() {
        let mut entries = HashMap::new();
        entries.insert(
            PathKind::AppLocalData,
            Err(PathPortError::new(
                PathKind::AppLocalData,
                "app local data unavailable",
            )),
        );
        let provider = MockPathProvider::from_map(entries);

        let error = resolve_api_server_runtime_dirs(&provider).unwrap_err();

        assert_eq!(
            error,
            "Failed to resolve AppLocalData path: app local data unavailable"
        );
    }
}
