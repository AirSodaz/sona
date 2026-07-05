use crate::core::database::DatabaseError;

pub use sona_core::history_store::{HistoryStore, HistoryStoreError};

impl From<DatabaseError> for HistoryStoreError {
    fn from(error: DatabaseError) -> Self {
        HistoryStoreError::Database(error.to_string())
    }
}
