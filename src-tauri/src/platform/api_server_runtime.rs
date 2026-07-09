use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiServerRuntimeDirs {
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
}

pub fn resolve_api_server_runtime_dirs(
    provider: &dyn PathProvider,
) -> Result<ApiServerRuntimeDirs, String> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    Ok(ApiServerRuntimeDirs {
        temp_dir: app_local_data_dir.join("api_temp"),
        models_dir: app_local_data_dir.join("models"),
    })
}

pub fn resolve_api_server_runtime_dirs_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<ApiServerRuntimeDirs, String> {
    let provider = TauriPathProvider::from_app(app);
    resolve_api_server_runtime_dirs(&provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::paths::MockPathProvider;
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
    fn propagates_app_local_data_resolution_errors() {
        let mut entries = HashMap::new();
        entries.insert(
            PathKind::AppLocalData,
            Err("app local data unavailable".to_string()),
        );
        let provider = MockPathProvider::from_map(entries);

        let error = resolve_api_server_runtime_dirs(&provider).unwrap_err();

        assert_eq!(error, "app local data unavailable");
    }
}
