use sona_core::runtime::{RuntimePathKind, RuntimePathStatus};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRuntimePathKind {
    File,
    Directory,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiRuntimePathStatus {
    pub path: String,
    pub kind: FfiRuntimePathKind,
    pub error: Option<String>,
}

pub fn runtime_path_status_to_ffi(status: RuntimePathStatus) -> FfiRuntimePathStatus {
    FfiRuntimePathStatus {
        path: status.path,
        kind: runtime_path_kind_to_ffi(status.kind),
        error: status.error,
    }
}

fn runtime_path_kind_to_ffi(kind: RuntimePathKind) -> FfiRuntimePathKind {
    match kind {
        RuntimePathKind::File => FfiRuntimePathKind::File,
        RuntimePathKind::Directory => FfiRuntimePathKind::Directory,
        RuntimePathKind::Missing => FfiRuntimePathKind::Missing,
        RuntimePathKind::Unknown => FfiRuntimePathKind::Unknown,
    }
}
