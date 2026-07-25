pub mod automation;
pub mod backup;
pub mod config;
pub mod dashboard;
/// Shared product-identity enums (LLM providers, polish presets, summary templates).
/// Full domain modules live alongside this one (`history`, `tag`, `transcription`, …).
pub mod domain;
pub mod export;
pub mod history;
/// Compatibility module path for [`history::store`]. Prefer `sona_core::history::store`
/// or `sona_core::history::{HistoryStore, HistoryStoreError}` in new code.
pub mod history_store {
    pub use crate::history::store::*;
}
pub mod llm;
pub mod models;
pub mod ports;
pub mod recovery;
pub mod runtime;
pub mod storage_usage;
pub mod sync;
pub mod tag;
pub mod task_ledger;
pub mod transcription;
