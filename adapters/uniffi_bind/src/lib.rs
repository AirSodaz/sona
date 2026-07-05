use sona_core::export::ExportFormat;
use sona_core::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, find_preset_model,
};

#[derive(Debug, thiserror::Error)]
pub enum SonaCoreBindingError {
    #[error("{0}")]
    InvalidInput(String),
}

pub type SonaCoreBindingResult<T> = Result<T, SonaCoreBindingError>;

/// FFI-friendly facade for mobile bindings. Keep methods owned-string based.
pub struct SonaCoreFacade;

impl SonaCoreFacade {
    pub fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
        let format = ExportFormat::parse(&value).map_err(SonaCoreBindingError::InvalidInput)?;
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
            Some("Silero VAD")
        );
    }

    #[test]
    fn facade_maps_core_errors_to_binding_errors() {
        let error = SonaCoreFacade::normalize_export_format("docx".to_string()).unwrap_err();
        assert_eq!(error.to_string(), "Unsupported export format: docx");
    }
}
