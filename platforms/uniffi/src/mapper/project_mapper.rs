use sona_core::project::ProjectRecord;
use sona_core::tag::{TagCreateInput, TagRecord, TagRepositorySnapshot, TagUpdateInput};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiProjectCreateInputV1 {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiProjectRecordV1 {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub sort_order: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiProjectUpdateInputV1 {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiProjectRepositorySnapshotV1 {
    pub projects: Vec<FfiProjectRecordV1>,
    pub active_project_id: Option<String>,
}

impl From<FfiProjectCreateInputV1> for TagCreateInput {
    fn from(value: FfiProjectCreateInputV1) -> Self {
        Self {
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
        }
    }
}

impl From<TagRecord> for FfiProjectRecordV1 {
    fn from(value: TagRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
            sort_order: value.sort_order as u64,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<ProjectRecord> for FfiProjectRecordV1 {
    fn from(value: ProjectRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            icon: value.icon.unwrap_or_default(),
            color: value.color.unwrap_or_default(),
            sort_order: value.sort_order as u64,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl TryFrom<FfiProjectRecordV1> for TagRecord {
    type Error = String;

    fn try_from(value: FfiProjectRecordV1) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
            sort_order: usize::try_from(value.sort_order)
                .map_err(|_| format!("project sort order {} is too large", value.sort_order))?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl From<FfiProjectUpdateInputV1> for TagUpdateInput {
    fn from(value: FfiProjectUpdateInputV1) -> Self {
        Self {
            name: value.name,
            icon: value.icon,
            color: value.color,
            description: value.description,
        }
    }
}

impl From<TagRepositorySnapshot> for FfiProjectRepositorySnapshotV1 {
    fn from(value: TagRepositorySnapshot) -> Self {
        Self {
            projects: value.tags.into_iter().map(Into::into).collect(),
            active_project_id: value.active_tag_id,
        }
    }
}
