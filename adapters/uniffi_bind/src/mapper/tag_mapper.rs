use sona_core::tag::{TagCreateInput, TagRecord, TagRepositorySnapshot, TagUpdateInput};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagCreateInputV1 {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagRecordV1 {
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
pub struct FfiTagUpdateInputV1 {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagRepositorySnapshotV1 {
    pub tags: Vec<FfiTagRecordV1>,
    pub active_tag_id: Option<String>,
}

impl From<FfiTagCreateInputV1> for TagCreateInput {
    fn from(value: FfiTagCreateInputV1) -> Self {
        Self {
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
        }
    }
}

impl From<TagRecord> for FfiTagRecordV1 {
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

impl TryFrom<FfiTagRecordV1> for TagRecord {
    type Error = String;

    fn try_from(value: FfiTagRecordV1) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
            sort_order: usize::try_from(value.sort_order)
                .map_err(|_| format!("tag sort order {} is too large", value.sort_order))?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl From<FfiTagUpdateInputV1> for TagUpdateInput {
    fn from(value: FfiTagUpdateInputV1) -> Self {
        Self {
            name: value.name,
            icon: value.icon,
            color: value.color,
            description: value.description,
        }
    }
}

impl From<TagRepositorySnapshot> for FfiTagRepositorySnapshotV1 {
    fn from(value: TagRepositorySnapshot) -> Self {
        Self {
            tags: value.tags.into_iter().map(Into::into).collect(),
            active_tag_id: value.active_tag_id,
        }
    }
}
