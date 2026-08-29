pub mod defaults;
pub mod error;
pub mod migration;
pub mod repository;
pub mod types;

pub use defaults::*;
pub use error::ConfigError;
pub use repository::*;
pub use types::*;

use serde_json::Value;

pub fn migrate_app_config(
    saved_config: Option<Value>,
    default_rule_set_name: String,
) -> MigrationResult {
    migration::migrate_app_config_inner(saved_config, &default_rule_set_name)
}

pub fn resolve_effective_config(global_config: Value, project: Option<Value>) -> Value {
    migration::resolve_effective_config_inner(global_config, project.as_ref())
}

pub fn validate_app_config(config: &Value) -> Result<(), ConfigError> {
    serde_json::from_value::<AppConfig>(
        config
            .get("sona-config")
            .filter(|v| v.is_object())
            .or_else(|| config.get("sona_config").filter(|v| v.is_object()))
            .or_else(|| config.get("config").filter(|v| v.is_object()))
            .unwrap_or(config)
            .clone(),
    )
    .map_err(ConfigError::Json)?;
    Ok(())
}
