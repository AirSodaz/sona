#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiConfigMigrationResult {
    pub config_json: String,
    pub migrated: bool,
}
