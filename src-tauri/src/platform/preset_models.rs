use crate::platform::paths::{PathKind, PathProvider};
pub use sona_core::preset_models::*;
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

async fn build_model_catalog_snapshot_for_models_dir(
    models_dir: PathBuf,
) -> Result<ModelCatalogSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&models_dir).map_err(|error| {
            format!(
                "Failed to create models directory {}: {error}",
                models_dir.display()
            )
        })?;

        Ok(build_model_catalog_snapshot(&models_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}
