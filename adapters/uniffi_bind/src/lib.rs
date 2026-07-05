use sona_core::export::ExportFormat;
use sona_core::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, find_preset_model,
};
use sona_core::runtime::resolve_runtime_path_status;

mod mapper;
pub use mapper::{FfiRuntimePathKind, FfiRuntimePathStatus};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SonaCoreBindingError {
    #[error("{message}")]
    InvalidInput { message: String },
}

pub type SonaCoreBindingResult<T> = Result<T, SonaCoreBindingError>;

/// Rust facade used by tests and by the top-level UniFFI exports.
pub struct SonaCoreFacade;

impl SonaCoreFacade {
    pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
        let format = ExportFormat::parse(&value)
            .map_err(|message| SonaCoreBindingError::InvalidInput { message })?;
        Ok(match format {
            ExportFormat::Json => "json",
            ExportFormat::Txt => "txt",
            ExportFormat::Srt => "srt",
            ExportFormat::Vtt => "vtt",
            ExportFormat::Md => "md",
        }
        .to_string())
    }

    pub fn default_vad_model_id() -> String {
        DEFAULT_SILERO_VAD_MODEL_ID.to_string()
    }

    pub fn default_punctuation_model_id() -> String {
        DEFAULT_PUNCTUATION_MODEL_ID.to_string()
    }

    pub fn preset_model_name(model_id: String) -> Option<String> {
        find_preset_model(&model_id).map(|model| model.name.clone())
    }

    pub fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
        mapper::runtime_path_status_to_ffi(resolve_runtime_path_status(&path))
    }
}

#[uniffi::export]
pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
    SonaCoreFacade::normalize_export_format(value)
}

#[uniffi::export]
pub fn default_vad_model_id() -> String {
    SonaCoreFacade::default_vad_model_id()
}

#[uniffi::export]
pub fn default_punctuation_model_id() -> String {
    SonaCoreFacade::default_punctuation_model_id()
}

#[uniffi::export]
pub fn preset_model_name(model_id: String) -> Option<String> {
    SonaCoreFacade::preset_model_name(model_id)
}

#[uniffi::export]
pub fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
    SonaCoreFacade::runtime_path_status(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_returns_owned_binding_safe_values_from_core() {
        assert_eq!(
            SonaCoreFacade::normalize_export_format("SRT".to_string()).unwrap(),
            "srt"
        );
        assert_eq!(SonaCoreFacade::default_vad_model_id(), "silero-vad");
        assert_eq!(
            SonaCoreFacade::preset_model_name("silero-vad".to_string()).as_deref(),
            find_preset_model("silero-vad").map(|model| model.name.as_str())
        );
    }

    #[test]
    fn facade_maps_core_errors_to_binding_errors() {
        let error = SonaCoreFacade::normalize_export_format("docx".to_string()).unwrap_err();
        assert_eq!(error.to_string(), "Unsupported export format: docx");
    }

    #[test]
    fn top_level_exports_delegate_to_core_facade() {
        assert_eq!(normalize_export_format("VTT".to_string()).unwrap(), "vtt");
        assert_eq!(default_punctuation_model_id(), DEFAULT_PUNCTUATION_MODEL_ID);
        assert_eq!(
            preset_model_name("missing-model".to_string()).as_deref(),
            None
        );
    }

    #[test]
    fn runtime_path_status_returns_binding_safe_record() {
        let missing_path = std::env::temp_dir()
            .join("sona-uniffi-bind-missing-runtime-path")
            .to_string_lossy()
            .into_owned();

        let status = SonaCoreFacade::runtime_path_status(missing_path.clone());

        assert_eq!(status.path, missing_path);
        assert_eq!(status.kind, FfiRuntimePathKind::Missing);
        assert_eq!(status.error, None);
    }
}
