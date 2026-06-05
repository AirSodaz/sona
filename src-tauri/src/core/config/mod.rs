pub mod error;
pub mod types;
pub mod defaults;
pub mod migration;

pub use error::ConfigError;
pub use types::*;

use serde_json::Value;

pub fn migrate_app_config(
    saved_config: Option<Value>,
    legacy_config: Option<Value>,
    default_rule_set_name: String,
) -> MigrationResult {
    migration::migrate_app_config_inner(saved_config, legacy_config, &default_rule_set_name)
}

pub fn resolve_effective_config(global_config: Value, project: Option<Value>) -> Value {
    migration::resolve_effective_config_inner(global_config, project.as_ref())
}
