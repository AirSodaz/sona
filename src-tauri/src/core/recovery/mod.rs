pub(crate) mod repository;
pub(crate) mod normalization {
    use std::fs;

    pub(crate) use sona_core::recovery::normalization::{
        SourcePathStatus, SourcePathStatusProvider, empty_snapshot, now_ms,
        recovered_item_from_queue_value_with_source_paths,
        recovered_item_from_saved_value_with_source_paths, snapshot_from_items,
        snapshot_from_value_with_source_paths,
    };

    #[derive(Clone, Copy, Debug, Default)]
    pub(crate) struct FsSourcePathStatusProvider;

    impl SourcePathStatusProvider for FsSourcePathStatusProvider {
        fn status_for_path(&self, path: &str) -> SourcePathStatus {
            match fs::metadata(path) {
                Ok(metadata) if metadata.is_file() => SourcePathStatus::File,
                Ok(metadata) if metadata.is_dir() => SourcePathStatus::Directory,
                Ok(_) => SourcePathStatus::Unknown,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    SourcePathStatus::Missing
                }
                Err(_) => SourcePathStatus::Unknown,
            }
        }
    }
}
pub(crate) mod types {
    pub use sona_core::recovery::types::*;
}
