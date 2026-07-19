use std::collections::HashSet;
use std::path::Path;

use crate::mapper::{
    self, FfiModelCatalogSelectedIds, FfiModelCatalogSnapshot, FfiModelSelectionPaths,
    FfiPresetModel, FfiResolvedModelDownload,
};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::models::downloads::{
    required_companion_models, resolve_model_download as core_resolve_model_download,
};
use sona_core::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID,
    build_model_catalog_snapshot_with_installed_ids, find_preset_model,
    preset_models as core_preset_models, resolve_model_catalog_selected_ids,
};
use sona_core::runtime::gpu::resolve_gpu_acceleration as core_resolve_gpu_acceleration;

pub(crate) fn default_vad_model_id() -> String {
    DEFAULT_SILERO_VAD_MODEL_ID.to_string()
}

pub(crate) fn default_punctuation_model_id() -> String {
    DEFAULT_PUNCTUATION_MODEL_ID.to_string()
}

pub(crate) fn preset_model_name(model_id: String) -> Option<String> {
    find_preset_model(&model_id).map(|model| model.name.clone())
}

pub(crate) fn preset_models() -> Vec<FfiPresetModel> {
    core_preset_models()
        .iter()
        .map(mapper::preset_model_to_ffi)
        .collect()
}

pub(crate) fn model_catalog_snapshot(
    models_dir: String,
    installed_model_ids: Vec<String>,
) -> FfiModelCatalogSnapshot {
    let installed_model_ids = installed_model_ids.into_iter().collect::<HashSet<_>>();
    mapper::model_catalog_snapshot_to_ffi(build_model_catalog_snapshot_with_installed_ids(
        Path::new(&models_dir),
        &installed_model_ids,
    ))
}

pub(crate) fn model_catalog_selected_ids(
    models_dir: String,
    installed_model_ids: Vec<String>,
    paths: FfiModelSelectionPaths,
) -> FfiModelCatalogSelectedIds {
    let installed_model_ids = installed_model_ids.into_iter().collect::<HashSet<_>>();
    let snapshot = build_model_catalog_snapshot_with_installed_ids(
        Path::new(&models_dir),
        &installed_model_ids,
    );

    mapper::model_catalog_selected_ids_to_ffi(resolve_model_catalog_selected_ids(
        &snapshot,
        &mapper::model_selection_paths_from_ffi(paths),
    ))
}

pub(crate) fn resolve_model_download(
    model_id: String,
    models_dir: String,
) -> SonaCoreBindingResult<FfiResolvedModelDownload> {
    let resolved =
        core_resolve_model_download(&model_id, Path::new(&models_dir)).map_err(|error| {
            SonaCoreBindingError::InvalidInput {
                reason: error.to_string(),
            }
        })?;
    let required_companions = required_companion_models(&resolved.model);

    Ok(mapper::resolved_model_download_to_ffi(
        resolved,
        required_companions,
    ))
}

pub(crate) fn resolve_gpu_acceleration(
    value: Option<String>,
) -> SonaCoreBindingResult<Option<String>> {
    core_resolve_gpu_acceleration(value).map_err(|error| SonaCoreBindingError::InvalidInput {
        reason: error.to_string(),
    })
}
