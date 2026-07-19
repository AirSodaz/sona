use sona_core::ports::fs::FileSystemError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Query error: {0}")]
    QueryError(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    NotFoundError(String),

    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error("All database connections are busy")]
    PoolBusyError,

    #[error("Unsupported database schema version {found}; this build supports up to {current}")]
    UnsupportedSchemaVersion { found: i64, current: i64 },

    #[error("Database schema version {found} requires migration to {current}")]
    SchemaMigrationRequired { found: i64, current: i64 },

    #[error("{0}")]
    Internal(String),
}
