use crate::{SonaCoreBindingError, SonaCoreBindingResult};

pub(crate) fn parse_core_json<T>(json: &str, label: &str) -> SonaCoreBindingResult<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(json).map_err(|error| SonaCoreBindingError::InvalidInput {
        reason: format!("Invalid {label} JSON: {error}"),
    })
}

pub(crate) fn parse_optional_core_json<T>(
    json: Option<&str>,
    label: &str,
) -> SonaCoreBindingResult<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    json.map(|value| parse_core_json(value, label)).transpose()
}

pub(crate) fn serialize_core_json<T>(value: &T, label: &str) -> SonaCoreBindingResult<String>
where
    T: serde::Serialize,
{
    serde_json::to_string(value).map_err(|error| SonaCoreBindingError::InvalidInput {
        reason: format!("Invalid {label} JSON: {error}"),
    })
}

pub(crate) fn map_core_validation_result(result: Result<(), String>) -> SonaCoreBindingResult<()> {
    result.map_err(|message| SonaCoreBindingError::InvalidInput { reason: message })
}
