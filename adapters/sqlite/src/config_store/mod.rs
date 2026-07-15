mod app_config;
mod library;
mod settings;

use std::sync::Arc;

use rusqlite::Transaction;
use serde_json::Value;
use sona_core::config::{
    AppConfigRepositoryService, AppConfigRepositorySnapshot, AppConfigStartupProjection,
    AppConfigStore, AppConfigStoredState,
};
use sona_core::ports::time::UnixMillisClock;
use sona_core::runtime::serve::ServeStartupSettings;

use crate::{DatabaseError, ports::Database as DatabasePort};

pub(crate) fn load_state_in_transaction(
    tx: &Transaction<'_>,
) -> Result<Option<AppConfigStoredState>, DatabaseError> {
    let Some(base) = app_config::load(tx)? else {
        return Ok(None);
    };
    let library = library::load(&tx)?;
    Ok(Some(AppConfigStoredState {
        base_config_json: base.base_config_json,
        library,
        config_version: base.config_version,
        updated_at: base.updated_at,
        startup_projection: base.startup_projection,
    }))
}

pub(crate) fn replace_state_in_transaction(
    tx: &Transaction<'_>,
    state: &AppConfigStoredState,
) -> Result<(), DatabaseError> {
    library::replace(tx, &state.library, state.updated_at)?;
    app_config::replace(tx, state)
}

pub(crate) fn clear_setting_in_transaction(
    tx: &Transaction<'_>,
    key: &str,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM app_settings WHERE key = ?1", [key])?;
    Ok(())
}

#[derive(Clone)]
pub struct SqliteConfigStore<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteConfigStore);

pub struct SqliteAppConfigAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    store: SqliteConfigStore<D>,
    clock: Arc<dyn UnixMillisClock>,
}

impl<D> SqliteAppConfigAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(db: Arc<D>, clock: Arc<dyn UnixMillisClock>) -> Self {
        Self {
            store: SqliteConfigStore::new(db),
            clock,
        }
    }

    pub fn load_config(&self) -> Result<Option<Value>, String> {
        self.service().load_config()
    }

    pub fn inspect_state(&self) -> Result<Option<AppConfigRepositorySnapshot>, String> {
        self.service().inspect_state()
    }

    pub fn save_config(&self, config: &Value) -> Result<(), String> {
        self.service().save_config(config)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<Value>, String> {
        self.service().get_setting(key)
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> Result<(), String> {
        self.service().set_setting(key, value)
    }

    pub fn load_app_config_payload(&self) -> Result<Option<Value>, String> {
        self.service().load_app_config_payload()
    }

    pub fn load_serve_startup_settings(&self) -> Result<Option<ServeStartupSettings>, String> {
        self.service().load_serve_startup_settings()
    }

    fn service(&self) -> AppConfigRepositoryService<'_> {
        AppConfigRepositoryService::new(&self.store, self.clock.as_ref())
    }
}

impl<D> AppConfigStore for SqliteConfigStore<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<Option<AppConfigStoredState>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(|conn| {
                let tx = conn.unchecked_transaction()?;
                let state = load_state_in_transaction(&tx)?;
                tx.commit()?;
                Ok(state)
            })
            .map_err(|error| error.to_string())
    }

    fn load_base_config_json(&self) -> Result<Option<String>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(app_config::load_base_config_json)
            .map_err(|error| error.to_string())
    }

    fn load_startup_projection(&self) -> Result<Option<AppConfigStartupProjection>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(app_config::load_startup_projection)
            .map_err(|error| error.to_string())
    }

    fn replace_state(&self, state: AppConfigStoredState) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_rw_transaction(|tx| replace_state_in_transaction(tx, &state))
            .map_err(|error| error.to_string())
    }

    fn load_setting_json(&self, key: &str) -> Result<Option<String>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(|conn| settings::load(conn, key))
            .map_err(|error| error.to_string())
    }

    fn set_setting_json(&self, key: &str, value_json: String) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_rw_transaction(|tx| settings::set(tx, key, &value_json))
            .map_err(|error| error.to_string())
    }
}
