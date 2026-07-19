use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sona_core::automation::service::AutomationIdGenerator;
use sona_core::history::HistoryIdGenerator;
use sona_core::ports::time::UnixMillisClock;
#[allow(deprecated)]
use sona_core::project::ProjectIdGenerator;
use sona_core::tag::TagIdGenerator;

#[allow(deprecated)]
use crate::{
    Database, DatabaseError, SqliteAppConfigAdapter, SqliteAutomationAdapter,
    SqliteAutomationRepository, SqliteBackupStateRepository, SqliteDashboardService,
    SqliteHistoryStore, SqliteLedgerRepository, SqliteProjectAdapter, SqliteProjectRepository,
    SqliteSyncRepositoryFactory, SqliteTagAdapter, SqliteTagRepository, SqliteTaskLedgerAdapter,
    create_dashboard_service,
};

/// Shared SQLite composition root for one application-data directory.
///
/// The context owns one connection pool and creates every SQLite-backed
/// repository or service against that pool. Host-specific DTOs, error mapping,
/// clocks, and ID generators remain outside this type.
#[derive(Clone)]
pub struct SqliteApplicationContext {
    app_data_dir: PathBuf,
    database: Arc<Database>,
}

impl SqliteApplicationContext {
    pub fn normalize_writable_app_data_dir(
        app_data_dir: impl AsRef<Path>,
    ) -> Result<PathBuf, DatabaseError> {
        normalize_directory(app_data_dir.as_ref(), true)
    }

    pub fn normalize_existing_app_data_dir(
        app_data_dir: impl AsRef<Path>,
    ) -> Result<PathBuf, DatabaseError> {
        normalize_directory(app_data_dir.as_ref(), false)
    }

    pub fn open(app_data_dir: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let app_data_dir = Self::normalize_writable_app_data_dir(app_data_dir)?;
        let database = Arc::new(Database::open(&app_data_dir)?);
        Ok(Self {
            app_data_dir,
            database,
        })
    }

    pub fn open_read_only(app_data_dir: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let app_data_dir = Self::normalize_existing_app_data_dir(app_data_dir)?;
        let database = Arc::new(Database::open_read_only(&app_data_dir)?);
        Ok(Self {
            app_data_dir,
            database,
        })
    }

    pub fn from_database(
        app_data_dir: impl AsRef<Path>,
        database: Arc<Database>,
    ) -> Result<Self, DatabaseError> {
        let app_data_dir = Self::normalize_existing_app_data_dir(app_data_dir)?;
        if !database.is_for_app_local_data_dir(&app_data_dir) {
            return Err(DatabaseError::Internal(format!(
                "Injected database does not belong to application data directory: {}",
                app_data_dir.display()
            )));
        }
        Ok(Self {
            app_data_dir,
            database,
        })
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn database(&self) -> Arc<Database> {
        Arc::clone(&self.database)
    }

    pub fn app_config_adapter(&self, clock: Arc<dyn UnixMillisClock>) -> SqliteAppConfigAdapter {
        SqliteAppConfigAdapter::new(self.database(), clock)
    }

    pub fn automation_adapter(
        &self,
        ids: Arc<dyn AutomationIdGenerator>,
    ) -> SqliteAutomationAdapter {
        SqliteAutomationAdapter::new(self.database(), ids)
    }

    pub fn automation_repository(&self) -> SqliteAutomationRepository {
        SqliteAutomationRepository::new(self.database())
    }

    #[allow(deprecated)]
    pub fn project_adapter(
        &self,
        ids: Arc<dyn ProjectIdGenerator>,
        clock: Arc<dyn UnixMillisClock>,
    ) -> SqliteProjectAdapter {
        SqliteProjectAdapter::new(self.database(), ids, clock)
    }

    #[allow(deprecated)]
    pub fn project_repository(&self) -> SqliteProjectRepository {
        SqliteProjectRepository::new(self.database())
    }

    pub fn tag_adapter(
        &self,
        ids: Arc<dyn TagIdGenerator>,
        clock: Arc<dyn UnixMillisClock>,
    ) -> SqliteTagAdapter {
        SqliteTagAdapter::new(self.database(), ids, clock)
    }

    pub fn tag_repository(&self) -> SqliteTagRepository {
        SqliteTagRepository::new(self.database())
    }

    pub fn task_ledger_adapter(&self, clock: Arc<dyn UnixMillisClock>) -> SqliteTaskLedgerAdapter {
        SqliteTaskLedgerAdapter::new(self.database(), clock)
    }

    pub fn task_ledger_repository(&self) -> SqliteLedgerRepository {
        SqliteLedgerRepository::new(self.database())
    }

    pub fn history_store(
        &self,
        clock: Arc<dyn UnixMillisClock>,
        ids: Arc<dyn HistoryIdGenerator>,
    ) -> SqliteHistoryStore {
        SqliteHistoryStore::with_environment(self.app_data_dir.clone(), self.database(), clock, ids)
    }

    pub fn backup_state_repository(&self) -> SqliteBackupStateRepository {
        SqliteBackupStateRepository::new(self.app_data_dir.clone(), self.database())
    }

    pub fn dashboard_service(&self) -> SqliteDashboardService {
        create_dashboard_service(self.app_data_dir.clone(), self.database())
    }

    pub fn sync_repository_factory(
        &self,
        clock: Arc<dyn UnixMillisClock>,
    ) -> SqliteSyncRepositoryFactory {
        SqliteSyncRepositoryFactory::new(self.database(), clock)
    }
}

impl fmt::Debug for SqliteApplicationContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteApplicationContext")
            .field("app_data_dir", &self.app_data_dir)
            .finish_non_exhaustive()
    }
}

fn normalize_directory(path: &Path, create: bool) -> Result<PathBuf, DatabaseError> {
    let absolute = std::path::absolute(path)
        .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
    if create {
        std::fs::create_dir_all(&absolute)
            .map_err(|error| DatabaseError::ConnectionError(error.to_string()))?;
    } else if !absolute.is_dir() {
        return Err(DatabaseError::ConnectionError(format!(
            "Application data directory does not exist or is not a directory: {}",
            absolute.display()
        )));
    }
    absolute
        .canonicalize()
        .map_err(|error| DatabaseError::ConnectionError(error.to_string()))
}
