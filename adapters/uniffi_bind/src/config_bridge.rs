use crate::SonaCoreBindingResult;
use crate::json_bridge::{parse_core_json, parse_optional_core_json};
use crate::mapper::FfiConfigMigrationResult;
use sona_core::config::{
    default_config, migrate_app_config as core_migrate_app_config,
    resolve_effective_config as core_resolve_effective_config,
};

pub(crate) fn default_config_json() -> String {
    default_config().to_string()
}

pub(crate) fn migrate_app_config_json(
    saved_config_json: Option<String>,
    legacy_config_json: Option<String>,
    default_rule_set_name: String,
) -> SonaCoreBindingResult<FfiConfigMigrationResult> {
    let saved_config = parse_optional_core_json(saved_config_json.as_deref(), "saved config")?;
    let legacy_config = parse_optional_core_json(legacy_config_json.as_deref(), "legacy config")?;
    let result = core_migrate_app_config(saved_config, legacy_config, default_rule_set_name);

    Ok(FfiConfigMigrationResult {
        config_json: result.config.to_string(),
        migrated: result.migrated,
    })
}

pub(crate) fn resolve_effective_config_json(
    global_config_json: String,
    project_json: Option<String>,
) -> SonaCoreBindingResult<String> {
    let global_config = parse_core_json(&global_config_json, "global config")?;
    let project = parse_optional_core_json(project_json.as_deref(), "project")?;

    Ok(core_resolve_effective_config(global_config, project).to_string())
}
