use thiserror::Error;

#[derive(Error, Debug)]
pub enum DashboardServiceError {
    #[error("History store error: {0}")]
    HistoryStore(String),

    #[error("Project repository error: {0}")]
    ProjectRepository(String),

    #[error("Analytics repository error: {0}")]
    AnalyticsRepository(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
