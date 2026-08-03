use crate::mapper::{self, FfiRuntimePathStatus};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::export::ExportFormat;
use sona_runtime_fs::resolve_runtime_path_status;

pub(crate) fn normalize_export_format(value: String) -> SonaCoreBindingResult<String> {
    let format =
        ExportFormat::parse(&value).map_err(|error| SonaCoreBindingError::InvalidInput {
            reason: error.to_string(),
        })?;
    Ok(match format {
        ExportFormat::Json => "json",
        ExportFormat::Txt => "txt",
        ExportFormat::Srt => "srt",
        ExportFormat::Vtt => "vtt",
        ExportFormat::Md => "md",
    }
    .to_string())
}

pub(crate) fn runtime_path_status(path: String) -> FfiRuntimePathStatus {
    mapper::runtime_path_status_to_ffi(resolve_runtime_path_status(&path))
}
