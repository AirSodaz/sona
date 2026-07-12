use rusqlite::{Connection, Transaction};
use sona_core::config::{AppConfigStartupProjection, AppConfigStoredState};

use crate::DatabaseError;

pub(super) struct BaseConfigRow {
    pub base_config_json: String,
    pub config_version: i64,
    pub updated_at: i64,
    pub startup_projection: AppConfigStartupProjection,
}

pub(super) fn load(tx: &Transaction<'_>) -> Result<Option<BaseConfigRow>, DatabaseError> {
    let mut statement = tx.prepare_cached(
        "SELECT config, config_version, updated_at, http_server_enabled,
                http_server_host, http_server_port, http_server_api_key,
                http_server_max_concurrent, http_server_max_queue_size,
                http_server_max_upload_size_mb, http_server_job_ttl_minutes,
                http_server_max_streaming, http_server_ip_whitelist, gpu_acceleration
         FROM app_config WHERE id = 1",
    )?;
    let mut rows = statement.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    Ok(Some(BaseConfigRow {
        base_config_json: row.get(0)?,
        config_version: row.get(1)?,
        updated_at: row.get(2)?,
        startup_projection: AppConfigStartupProjection {
            http_server_enabled: row.get::<_, i64>(3)? != 0,
            host: row.get(4)?,
            port: row.get(5)?,
            api_key: row.get(6)?,
            max_concurrent: row.get(7)?,
            max_queue_size: row.get(8)?,
            max_upload_size_mb: row.get(9)?,
            job_ttl_minutes: row.get(10)?,
            max_streaming: row.get(11)?,
            ip_whitelist: row.get(12)?,
            gpu_acceleration: row.get(13)?,
        },
    }))
}

pub(super) fn load_base_config_json(
    connection: &Connection,
) -> Result<Option<String>, DatabaseError> {
    let mut statement = connection.prepare_cached("SELECT config FROM app_config WHERE id = 1")?;
    let mut rows = statement.query([])?;
    rows.next()?
        .map(|row| row.get(0))
        .transpose()
        .map_err(Into::into)
}

pub(super) fn load_startup_projection(
    connection: &Connection,
) -> Result<Option<AppConfigStartupProjection>, DatabaseError> {
    let mut statement = connection.prepare_cached(
        "SELECT http_server_enabled, http_server_host, http_server_port,
                http_server_api_key, http_server_max_concurrent,
                http_server_max_queue_size, http_server_max_upload_size_mb,
                http_server_job_ttl_minutes, http_server_max_streaming,
                http_server_ip_whitelist, gpu_acceleration
         FROM app_config WHERE id = 1",
    )?;
    let mut rows = statement.query([])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    Ok(Some(AppConfigStartupProjection {
        http_server_enabled: row.get::<_, i64>(0)? != 0,
        host: row.get(1)?,
        port: row.get(2)?,
        api_key: row.get(3)?,
        max_concurrent: row.get(4)?,
        max_queue_size: row.get(5)?,
        max_upload_size_mb: row.get(6)?,
        job_ttl_minutes: row.get(7)?,
        max_streaming: row.get(8)?,
        ip_whitelist: row.get(9)?,
        gpu_acceleration: row.get(10)?,
    }))
}

pub(super) fn replace(
    tx: &Transaction<'_>,
    state: &AppConfigStoredState,
) -> Result<(), DatabaseError> {
    let projection = &state.startup_projection;
    tx.execute(
        "INSERT INTO app_config (
            id, config, config_version, updated_at, http_server_enabled,
            http_server_host, http_server_port, http_server_api_key,
            http_server_max_concurrent, http_server_max_queue_size,
            http_server_max_upload_size_mb, http_server_job_ttl_minutes,
            http_server_max_streaming, http_server_ip_whitelist, gpu_acceleration
         )
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
            config = excluded.config,
            config_version = excluded.config_version,
            updated_at = excluded.updated_at,
            http_server_enabled = excluded.http_server_enabled,
            http_server_host = excluded.http_server_host,
            http_server_port = excluded.http_server_port,
            http_server_api_key = excluded.http_server_api_key,
            http_server_max_concurrent = excluded.http_server_max_concurrent,
            http_server_max_queue_size = excluded.http_server_max_queue_size,
            http_server_max_upload_size_mb = excluded.http_server_max_upload_size_mb,
            http_server_job_ttl_minutes = excluded.http_server_job_ttl_minutes,
            http_server_max_streaming = excluded.http_server_max_streaming,
            http_server_ip_whitelist = excluded.http_server_ip_whitelist,
            gpu_acceleration = excluded.gpu_acceleration",
        rusqlite::params![
            state.base_config_json,
            state.config_version,
            state.updated_at,
            projection.http_server_enabled as i64,
            projection.host,
            projection.port,
            projection.api_key,
            projection.max_concurrent,
            projection.max_queue_size,
            projection.max_upload_size_mb,
            projection.job_ttl_minutes,
            projection.max_streaming,
            projection.ip_whitelist,
            projection.gpu_acceleration,
        ],
    )?;
    Ok(())
}
