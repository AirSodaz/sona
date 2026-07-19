use crate::{
    FfiSpeakerAttribution, FfiSpeakerCandidate, FfiSpeakerTag, FfiTranscriptTimingLevel,
    FfiTranscriptTimingSource,
};
use serde_json::Value;
use sona_core::recovery::types::{
    RecoveredQueueItem, RecoveredTranscriptSegment, RecoveredTranscriptTiming,
    RecoveredTranscriptTimingUnit, RecoveryFileStat, RecoveryItemInput, RecoveryItemStage,
    RecoveryResolution, RecoverySnapshot, RecoverySource,
};
use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptTimingLevel, TranscriptTimingSource,
};
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRecoverySourceV1 {
    BatchImport,
    Automation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRecoveryResolutionV1 {
    Pending,
    Resumed,
    Discarded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRecoveryItemStageV1 {
    Queued,
    Transcribing,
    Polishing,
    Translating,
    Exporting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRecoveryQueueStatusV1 {
    Pending,
    Processing,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiRecoveryFileStatV1 {
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiRecoveredTranscriptTimingUnitV1 {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiRecoveredTranscriptTimingV1 {
    pub level: FfiTranscriptTimingLevel,
    pub source: FfiTranscriptTimingSource,
    pub units: Vec<FfiRecoveredTranscriptTimingUnitV1>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiRecoveredTranscriptSegmentV1 {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub is_final: bool,
    pub timing: Option<FfiRecoveredTranscriptTimingV1>,
    pub tokens: Option<Vec<String>>,
    pub timestamps: Option<Vec<f64>>,
    pub durations: Option<Vec<f64>>,
    pub translation: Option<String>,
    pub speaker: Option<FfiSpeakerTag>,
    pub speaker_attribution: Option<FfiSpeakerAttribution>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiRecoveryItemInputV1 {
    pub id: Option<String>,
    pub recovery_id: Option<String>,
    pub filename: Option<String>,
    pub file_path: Option<String>,
    pub source: Option<FfiRecoverySourceV1>,
    pub origin: Option<FfiRecoverySourceV1>,
    pub resolution: Option<FfiRecoveryResolutionV1>,
    pub status: Option<FfiRecoveryQueueStatusV1>,
    pub progress: Option<f64>,
    pub segments: Vec<FfiRecoveredTranscriptSegmentV1>,
    pub tag_ids: Vec<String>,
    pub project_id: Option<String>,
    pub history_id: Option<String>,
    pub history_title: Option<String>,
    pub last_known_stage: Option<FfiRecoveryItemStageV1>,
    pub updated_at: Option<u64>,
    pub has_source_file: Option<bool>,
    pub can_resume: Option<bool>,
    pub automation_rule_id: Option<String>,
    pub automation_rule_name: Option<String>,
    pub resolved_config_snapshot_json: Option<String>,
    pub export_config_json: Option<String>,
    pub stage_config_json: Option<String>,
    pub source_fingerprint: Option<String>,
    pub file_stat: Option<FfiRecoveryFileStatV1>,
    pub export_file_name_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiRecoverySnapshotV1 {
    pub version: u32,
    pub updated_at: Option<u64>,
    pub items: Vec<FfiRecoveredQueueItemV1>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiRecoveredQueueItemV1 {
    pub id: String,
    pub filename: String,
    pub file_path: String,
    pub source: FfiRecoverySourceV1,
    pub resolution: FfiRecoveryResolutionV1,
    pub progress: f64,
    pub segments: Vec<FfiRecoveredTranscriptSegmentV1>,
    pub tag_ids: Vec<String>,
    pub history_id: Option<String>,
    pub history_title: Option<String>,
    pub last_known_stage: FfiRecoveryItemStageV1,
    pub updated_at: u64,
    pub has_source_file: bool,
    pub can_resume: bool,
    pub automation_rule_id: Option<String>,
    pub automation_rule_name: Option<String>,
    pub resolved_config_snapshot_json: Option<String>,
    pub export_config_json: String,
    pub stage_config_json: String,
    pub source_fingerprint: Option<String>,
    pub file_stat: Option<FfiRecoveryFileStatV1>,
    pub export_file_name_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryMapperError {
    InvalidJson { field: &'static str, reason: String },
    InvalidValue { field: &'static str, reason: String },
}

impl Display for RecoveryMapperError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson { field, reason } => {
                write!(formatter, "Invalid {field}: {reason}")
            }
            Self::InvalidValue { field, reason } => write!(formatter, "Invalid {field}: {reason}"),
        }
    }
}

impl From<FfiRecoverySourceV1> for RecoverySource {
    fn from(value: FfiRecoverySourceV1) -> Self {
        match value {
            FfiRecoverySourceV1::BatchImport => Self::BatchImport,
            FfiRecoverySourceV1::Automation => Self::Automation,
        }
    }
}

impl From<RecoverySource> for FfiRecoverySourceV1 {
    fn from(value: RecoverySource) -> Self {
        match value {
            RecoverySource::BatchImport => Self::BatchImport,
            RecoverySource::Automation => Self::Automation,
        }
    }
}

impl From<FfiRecoveryResolutionV1> for RecoveryResolution {
    fn from(value: FfiRecoveryResolutionV1) -> Self {
        match value {
            FfiRecoveryResolutionV1::Pending => Self::Pending,
            FfiRecoveryResolutionV1::Resumed => Self::Resumed,
            FfiRecoveryResolutionV1::Discarded => Self::Discarded,
        }
    }
}

impl From<RecoveryResolution> for FfiRecoveryResolutionV1 {
    fn from(value: RecoveryResolution) -> Self {
        match value {
            RecoveryResolution::Pending => Self::Pending,
            RecoveryResolution::Resumed => Self::Resumed,
            RecoveryResolution::Discarded => Self::Discarded,
        }
    }
}

impl From<FfiRecoveryItemStageV1> for RecoveryItemStage {
    fn from(value: FfiRecoveryItemStageV1) -> Self {
        match value {
            FfiRecoveryItemStageV1::Queued => Self::Queued,
            FfiRecoveryItemStageV1::Transcribing => Self::Transcribing,
            FfiRecoveryItemStageV1::Polishing => Self::Polishing,
            FfiRecoveryItemStageV1::Translating => Self::Translating,
            FfiRecoveryItemStageV1::Exporting => Self::Exporting,
        }
    }
}

impl From<RecoveryItemStage> for FfiRecoveryItemStageV1 {
    fn from(value: RecoveryItemStage) -> Self {
        match value {
            RecoveryItemStage::Queued => Self::Queued,
            RecoveryItemStage::Transcribing => Self::Transcribing,
            RecoveryItemStage::Polishing => Self::Polishing,
            RecoveryItemStage::Translating => Self::Translating,
            RecoveryItemStage::Exporting => Self::Exporting,
        }
    }
}

impl From<FfiTranscriptTimingLevel> for TranscriptTimingLevel {
    fn from(value: FfiTranscriptTimingLevel) -> Self {
        match value {
            FfiTranscriptTimingLevel::Token => Self::Token,
            FfiTranscriptTimingLevel::Segment => Self::Segment,
        }
    }
}

impl From<TranscriptTimingLevel> for FfiTranscriptTimingLevel {
    fn from(value: TranscriptTimingLevel) -> Self {
        match value {
            TranscriptTimingLevel::Token => Self::Token,
            TranscriptTimingLevel::Segment => Self::Segment,
        }
    }
}

impl From<FfiTranscriptTimingSource> for TranscriptTimingSource {
    fn from(value: FfiTranscriptTimingSource) -> Self {
        match value {
            FfiTranscriptTimingSource::Model => Self::Model,
            FfiTranscriptTimingSource::Derived => Self::Derived,
        }
    }
}

impl From<TranscriptTimingSource> for FfiTranscriptTimingSource {
    fn from(value: TranscriptTimingSource) -> Self {
        match value {
            TranscriptTimingSource::Model => Self::Model,
            TranscriptTimingSource::Derived => Self::Derived,
        }
    }
}

impl TryFrom<FfiRecoveryItemInputV1> for RecoveryItemInput {
    type Error = RecoveryMapperError;

    fn try_from(value: FfiRecoveryItemInputV1) -> Result<Self, Self::Error> {
        let FfiRecoveryItemInputV1 {
            id,
            recovery_id,
            filename,
            file_path,
            source,
            origin,
            resolution,
            status,
            progress,
            segments,
            tag_ids,
            project_id,
            history_id,
            history_title,
            last_known_stage,
            updated_at,
            has_source_file,
            can_resume,
            automation_rule_id,
            automation_rule_name,
            resolved_config_snapshot_json,
            export_config_json,
            stage_config_json,
            source_fingerprint,
            file_stat,
            export_file_name_prefix,
        } = value;

        Ok(Self {
            id,
            recovery_id,
            filename,
            file_path,
            source: source
                .map(|value| value.into())
                .map(|value: RecoverySource| value.to_string()),
            origin: origin
                .map(|value| value.into())
                .map(|value: RecoverySource| value.to_string()),
            resolution: resolution
                .map(|value| value.into())
                .map(|value: RecoveryResolution| value.to_string()),
            status: status.map(|value| match value {
                FfiRecoveryQueueStatusV1::Pending => "pending".to_string(),
                FfiRecoveryQueueStatusV1::Processing => "processing".to_string(),
            }),
            progress,
            segments: segments
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            tag_ids,
            project_id,
            history_id,
            history_title,
            last_known_stage: last_known_stage
                .map(|value| value.into())
                .map(|value: RecoveryItemStage| value.to_string()),
            updated_at,
            has_source_file,
            can_resume,
            automation_rule_id,
            automation_rule_name,
            resolved_config_snapshot: parse_json_leaf(
                "resolved_config_snapshot_json",
                resolved_config_snapshot_json,
            )?,
            export_config: parse_json_leaf("export_config_json", export_config_json)?,
            stage_config: parse_json_leaf("stage_config_json", stage_config_json)?,
            source_fingerprint,
            file_stat: file_stat.map(Into::into),
            export_file_name_prefix,
        })
    }
}

impl TryFrom<FfiRecoveredTranscriptSegmentV1> for RecoveredTranscriptSegment {
    type Error = RecoveryMapperError;

    fn try_from(value: FfiRecoveredTranscriptSegmentV1) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            text: value.text,
            start: value.start,
            end: value.end,
            is_final: value.is_final,
            timing: value.timing.map(Into::into),
            tokens: value.tokens,
            timestamps: value.timestamps,
            durations: value.durations,
            translation: value.translation,
            speaker: value.speaker.map(Into::into),
            speaker_attribution: value
                .speaker_attribution
                .map(TryInto::try_into)
                .transpose()?,
        })
    }
}

impl From<FfiRecoveredTranscriptTimingV1> for RecoveredTranscriptTiming {
    fn from(value: FfiRecoveredTranscriptTimingV1) -> Self {
        Self {
            level: value.level.into(),
            source: value.source.into(),
            units: value.units.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<FfiRecoveredTranscriptTimingUnitV1> for RecoveredTranscriptTimingUnit {
    fn from(value: FfiRecoveredTranscriptTimingUnitV1) -> Self {
        Self {
            text: value.text,
            start: value.start,
            end: value.end,
        }
    }
}

impl From<FfiSpeakerTag> for SpeakerTag {
    fn from(value: FfiSpeakerTag) -> Self {
        Self {
            id: value.id,
            label: value.label,
            kind: value.kind,
            score: value.score,
        }
    }
}

impl TryFrom<FfiSpeakerAttribution> for SpeakerAttribution {
    type Error = RecoveryMapperError;

    fn try_from(value: FfiSpeakerAttribution) -> Result<Self, Self::Error> {
        Ok(Self {
            group_id: value.group_id,
            anonymous_label: value.anonymous_label,
            state: value.state,
            source: value.source,
            confidence: value.confidence,
            candidates: value
                .candidates
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<FfiSpeakerCandidate> for SpeakerCandidate {
    type Error = RecoveryMapperError;

    fn try_from(value: FfiSpeakerCandidate) -> Result<Self, Self::Error> {
        Ok(Self {
            profile_id: value.profile_id,
            profile_name: value.profile_name,
            score: value.score,
            rank: usize::try_from(value.rank).map_err(|_| RecoveryMapperError::InvalidValue {
                field: "speaker_attribution.candidates.rank",
                reason: "value exceeds the host usize range".to_string(),
            })?,
        })
    }
}

impl From<FfiRecoveryFileStatV1> for RecoveryFileStat {
    fn from(value: FfiRecoveryFileStatV1) -> Self {
        Self {
            size: value.size,
            mtime_ms: value.mtime_ms,
        }
    }
}

impl From<RecoveryFileStat> for FfiRecoveryFileStatV1 {
    fn from(value: RecoveryFileStat) -> Self {
        Self {
            size: value.size,
            mtime_ms: value.mtime_ms,
        }
    }
}

impl TryFrom<RecoveredTranscriptSegment> for FfiRecoveredTranscriptSegmentV1 {
    type Error = RecoveryMapperError;

    fn try_from(value: RecoveredTranscriptSegment) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            text: value.text,
            start: value.start,
            end: value.end,
            is_final: value.is_final,
            timing: value.timing.map(Into::into),
            tokens: value.tokens,
            timestamps: value.timestamps,
            durations: value.durations,
            translation: value.translation,
            speaker: value.speaker.map(Into::into),
            speaker_attribution: value
                .speaker_attribution
                .map(TryInto::try_into)
                .transpose()?,
        })
    }
}

impl From<RecoveredTranscriptTiming> for FfiRecoveredTranscriptTimingV1 {
    fn from(value: RecoveredTranscriptTiming) -> Self {
        Self {
            level: value.level.into(),
            source: value.source.into(),
            units: value.units.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RecoveredTranscriptTimingUnit> for FfiRecoveredTranscriptTimingUnitV1 {
    fn from(value: RecoveredTranscriptTimingUnit) -> Self {
        Self {
            text: value.text,
            start: value.start,
            end: value.end,
        }
    }
}

impl From<SpeakerTag> for FfiSpeakerTag {
    fn from(value: SpeakerTag) -> Self {
        Self {
            id: value.id,
            label: value.label,
            kind: value.kind,
            score: value.score,
        }
    }
}

impl TryFrom<SpeakerAttribution> for FfiSpeakerAttribution {
    type Error = RecoveryMapperError;

    fn try_from(value: SpeakerAttribution) -> Result<Self, Self::Error> {
        Ok(Self {
            group_id: value.group_id,
            anonymous_label: value.anonymous_label,
            state: value.state,
            source: value.source,
            confidence: value.confidence,
            candidates: value
                .candidates
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<SpeakerCandidate> for FfiSpeakerCandidate {
    type Error = RecoveryMapperError;

    fn try_from(value: SpeakerCandidate) -> Result<Self, Self::Error> {
        Ok(Self {
            profile_id: value.profile_id,
            profile_name: value.profile_name,
            score: value.score,
            rank: u64::try_from(value.rank).map_err(|_| RecoveryMapperError::InvalidValue {
                field: "speaker_attribution.candidates.rank",
                reason: "value exceeds the UniFFI u64 range".to_string(),
            })?,
        })
    }
}

impl TryFrom<RecoverySnapshot> for FfiRecoverySnapshotV1 {
    type Error = RecoveryMapperError;

    fn try_from(value: RecoverySnapshot) -> Result<Self, Self::Error> {
        Ok(Self {
            version: value.version,
            updated_at: value.updated_at,
            items: value
                .items
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<RecoveredQueueItem> for FfiRecoveredQueueItemV1 {
    type Error = RecoveryMapperError;

    fn try_from(value: RecoveredQueueItem) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            filename: value.filename,
            file_path: value.file_path,
            source: value.source.into(),
            resolution: value.resolution.into(),
            progress: value.progress,
            segments: value
                .segments
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            tag_ids: value.tag_ids,
            history_id: value.history_id,
            history_title: value.history_title,
            last_known_stage: value.last_known_stage.into(),
            updated_at: value.updated_at,
            has_source_file: value.has_source_file,
            can_resume: value.can_resume,
            automation_rule_id: value.automation_rule_id,
            automation_rule_name: value.automation_rule_name,
            resolved_config_snapshot_json: value
                .resolved_config_snapshot
                .as_ref()
                .map(|value| serialize_json_leaf("resolved_config_snapshot_json", value))
                .transpose()?,
            export_config_json: serialize_json_leaf("export_config_json", &value.export_config)?,
            stage_config_json: serialize_json_leaf("stage_config_json", &value.stage_config)?,
            source_fingerprint: value.source_fingerprint,
            file_stat: value.file_stat.map(Into::into),
            export_file_name_prefix: value.export_file_name_prefix,
        })
    }
}

fn parse_json_leaf(
    field: &'static str,
    value: Option<String>,
) -> Result<Option<Value>, RecoveryMapperError> {
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| RecoveryMapperError::InvalidJson {
                field,
                reason: error.to_string(),
            })
        })
        .transpose()
}

fn serialize_json_leaf(field: &'static str, value: &Value) -> Result<String, RecoveryMapperError> {
    serde_json::to_string(value).map_err(|error| RecoveryMapperError::InvalidJson {
        field,
        reason: error.to_string(),
    })
}
