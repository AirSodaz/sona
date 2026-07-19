use crate::platform::paths::{PathKind, PathProvider};
pub use sona_core::models::preset_models::*;
pub use sona_runtime_fs::{build_model_catalog_snapshot, is_preset_model_installed_at};
use std::path::PathBuf;

/// Returns a settings-page-ready catalog snapshot for the app-local models dir.
pub async fn get_model_catalog_snapshot(
    provider: &dyn PathProvider,
) -> Result<ModelCatalogSnapshot, String> {
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?
        .join("models");

    build_model_catalog_snapshot_for_models_dir(models_dir).await
}

pub async fn get_model_catalog_snapshot_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<ModelCatalogSnapshot, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    get_model_catalog_snapshot(&provider).await
}

pub async fn resolve_model_catalog_selected_ids_command(
    provider: &dyn PathProvider,
    paths: ModelSelectionPaths,
) -> Result<ModelCatalogSelectedIds, String> {
    let models_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?
        .join("models");

    let snapshot = build_model_catalog_snapshot_for_models_dir(models_dir).await?;
    Ok(resolve_model_catalog_selected_ids(&snapshot, &paths))
}

pub async fn resolve_model_catalog_selected_ids_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    paths: ModelSelectionPaths,
) -> Result<ModelCatalogSelectedIds, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    resolve_model_catalog_selected_ids_command(&provider, paths).await
}

async fn build_model_catalog_snapshot_for_models_dir(
    models_dir: PathBuf,
) -> Result<ModelCatalogSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        sona_runtime_fs::ensure_directory_exists(&models_dir).map_err(|error| error.to_string())?;

        Ok(build_model_catalog_snapshot(&models_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}
