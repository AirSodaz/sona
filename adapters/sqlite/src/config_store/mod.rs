mod app_config;
mod library;
mod settings;

use std::sync::Arc;

use sona_core::config::{AppConfigStartupProjection, AppConfigStore, AppConfigStoredState};

use crate::ports::Database as DatabasePort;

#[derive(Clone)]
pub struct SqliteConfigStore<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteConfigStore);

impl<D> AppConfigStore for SqliteConfigStore<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<Option<AppConfigStoredState>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(|conn| {
                let tx = conn.unchecked_transaction()?;
                let Some(base) = app_config::load(&tx)? else {
                    tx.commit()?;
                    return Ok(None);
                };
                let library = library::load(&tx)?;
                tx.commit()?;
                Ok(Some(AppConfigStoredState {
                    base_config_json: base.base_config_json,
                    library,
                    config_version: base.config_version,
                    updated_at: base.updated_at,
                    startup_projection: base.startup_projection,
                }))
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
            .with_rw_transaction(|tx| {
                app_config::replace(tx, &state)?;
                library::replace(tx, &state.library, state.updated_at)
            })
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
