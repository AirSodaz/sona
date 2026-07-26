use crate::FfiTranscriptSegment;
use sona_core::export::{
    ExportFormat, ExportMode, ExportTranscriptFileRequest, ExportTranscriptFileResult,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiExportFormatV1 {
    Json,
    Txt,
    Srt,
    Vtt,
    Md,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiExportModeV1 {
    Original,
    Translation,
    Bilingual,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiExportTranscriptFileRequestV1 {
    pub segments: Vec<FfiTranscriptSegment>,
    pub format: FfiExportFormatV1,
    pub mode: FfiExportModeV1,
    pub output_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiExportTranscriptFileResultV1 {
    pub output_path: String,
    pub bytes_written: u64,
}

impl From<FfiExportFormatV1> for ExportFormat {
    fn from(value: FfiExportFormatV1) -> Self {
        match value {
            FfiExportFormatV1::Json => Self::Json,
            FfiExportFormatV1::Txt => Self::Txt,
            FfiExportFormatV1::Srt => Self::Srt,
            FfiExportFormatV1::Vtt => Self::Vtt,
            FfiExportFormatV1::Md => Self::Md,
        }
    }
}

impl From<ExportFormat> for FfiExportFormatV1 {
    fn from(value: ExportFormat) -> Self {
        match value {
            ExportFormat::Json => Self::Json,
            ExportFormat::Txt => Self::Txt,
            ExportFormat::Srt => Self::Srt,
            ExportFormat::Vtt => Self::Vtt,
            ExportFormat::Md => Self::Md,
        }
    }
}

impl From<FfiExportModeV1> for ExportMode {
    fn from(value: FfiExportModeV1) -> Self {
        match value {
            FfiExportModeV1::Original => Self::Original,
            FfiExportModeV1::Translation => Self::Translation,
            FfiExportModeV1::Bilingual => Self::Bilingual,
        }
    }
}

impl From<ExportMode> for FfiExportModeV1 {
    fn from(value: ExportMode) -> Self {
        match value {
            ExportMode::Original => Self::Original,
            ExportMode::Translation => Self::Translation,
            ExportMode::Bilingual => Self::Bilingual,
        }
    }
}

impl From<ExportTranscriptFileResult> for FfiExportTranscriptFileResultV1 {
    fn from(value: ExportTranscriptFileResult) -> Self {
        Self {
            output_path: value.output_path,
            bytes_written: value.bytes_written,
        }
    }
}

/// Builds the core request. Segment conversion is fallible because
/// `FfiTranscriptSegment` carries timing/speaker leaves that Core validates.
pub(crate) fn export_request_from_ffi(
    value: FfiExportTranscriptFileRequestV1,
) -> Result<ExportTranscriptFileRequest, String> {
    Ok(ExportTranscriptFileRequest {
        segments: super::history_mapper::history_transcript_segments_from_ffi(value.segments)
            .map_err(|error| error.to_string())?,
        format: value.format.into(),
        mode: value.mode.into(),
        output_path: value.output_path,
    })
}
