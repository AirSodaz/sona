use std::path::Path;

use rusqlite::{Connection, Transaction};

use super::DatabaseError;

pub trait Database: Send + Sync + 'static {
    fn with_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>;

    fn with_read_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>;

    fn with_write_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>;

    fn with_transaction<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction) -> Result<T, DatabaseError>;

    fn with_rw_transaction<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction) -> Result<T, DatabaseError>;

    fn run_optimize(&self) -> Result<(), DatabaseError>;

    fn vacuum(&self) -> Result<(), DatabaseError>;

    fn is_for_app_local_data_dir(&self, app_local_data_dir: &Path) -> bool;
}

impl Database for super::Database {
    fn with_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        super::Database::with_connection(self, f)
    }

    fn with_read_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        super::Database::with_read_connection(self, f)
    }

    fn with_write_connection<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Connection) -> Result<T, DatabaseError>,
    {
        super::Database::with_write_connection(self, f)
    }

    fn with_transaction<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction) -> Result<T, DatabaseError>,
    {
        super::Database::with_transaction(self, f)
    }

    fn with_rw_transaction<F, T>(&self, f: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&Transaction) -> Result<T, DatabaseError>,
    {
        super::Database::with_rw_transaction(self, f)
    }

    fn run_optimize(&self) -> Result<(), DatabaseError> {
        super::Database::run_optimize(self)
    }

    fn vacuum(&self) -> Result<(), DatabaseError> {
        super::Database::vacuum(self)
    }

    fn is_for_app_local_data_dir(&self, app_local_data_dir: &Path) -> bool {
        super::Database::is_for_app_local_data_dir(self, app_local_data_dir)
    }
}
