use crate::core::database::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DashboardServiceError {
    #[error("History store error: {0}")]
    HistoryStore(DatabaseError),

    #[error("Project repository error: {0}")]
    ProjectRepository(DatabaseError),

    #[error("Analytics repository error: {0}")]
    AnalyticsRepository(DatabaseError),

    #[error("Internal error: {0}")]
    Internal(String),
}
