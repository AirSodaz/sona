//! TypeScript-facing metadata for Sona core bindings.
//!
//! Desktop currently generates concrete Tauri command bindings from
//! `src-tauri` with tauri-specta. This crate keeps core-owned TS binding
//! metadata in the workspace so future non-Tauri consumers can depend on the
//! same pure Rust types without reaching into the desktop crate.

pub use sona_core::domain::{LlmProvider, PolishPresetId, SummaryTemplateId};
pub use sona_core::ports::asr::{AsrEngine, AsrMode, BatchSegmentationMode};
pub use sona_core::runtime::{RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus};

pub const DESKTOP_BINDINGS_OUTPUT: &str = "src/bindings.ts";

pub fn exported_core_type_names() -> &'static [&'static str] {
    &[
        "LlmProvider",
        "PolishPresetId",
        "SummaryTemplateId",
        "RuntimeEnvironmentStatus",
        "RuntimePathKind",
        "RuntimePathStatus",
        "AsrEngine",
        "AsrMode",
        "BatchSegmentationMode",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_core_types_owned_by_ts_bindings() {
        assert_eq!(
            exported_core_type_names(),
            &[
                "LlmProvider",
                "PolishPresetId",
                "SummaryTemplateId",
                "RuntimeEnvironmentStatus",
                "RuntimePathKind",
                "RuntimePathStatus",
                "AsrEngine",
                "AsrMode",
                "BatchSegmentationMode",
            ]
        );
    }

    #[test]
    fn keeps_desktop_binding_output_explicit() {
        assert_eq!(DESKTOP_BINDINGS_OUTPUT, "src/bindings.ts");
    }

    #[test]
    fn runtime_types_are_specta_exportable_through_ts_bindings() {
        fn assert_specta_type<T: specta::Type>() {}

        assert_specta_type::<sona_core::runtime::RuntimeEnvironmentStatus>();
        assert_specta_type::<sona_core::runtime::RuntimePathKind>();
        assert_specta_type::<sona_core::runtime::RuntimePathStatus>();
        assert_specta_type::<sona_core::ports::asr::AsrEngine>();
        assert_specta_type::<sona_core::ports::asr::AsrMode>();
        assert_specta_type::<sona_core::ports::asr::BatchSegmentationMode>();
    }
}
