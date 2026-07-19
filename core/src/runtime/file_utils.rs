use super::error::RuntimeValidationError;

pub fn ensure_json_array_value(
    value: serde_json::Value,
    label: &str,
) -> Result<serde_json::Value, RuntimeValidationError> {
    if value.is_array() {
        Ok(value)
    } else {
        Err(RuntimeValidationError::new(
            label,
            format!("{label} must be an array."),
        ))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_ensure_json_array_value() {
        let arr = json!([1, 2, 3]);
        let res = super::ensure_json_array_value(arr.clone(), "Test list");
        assert_eq!(res, Ok(arr));

        let obj = json!({"key": "value"});
        let res = super::ensure_json_array_value(obj, "Test list");
        assert_eq!(
            res,
            Err(super::RuntimeValidationError::new(
                "Test list",
                "Test list must be an array.",
            ))
        );

        let null_val = json!(null);
        let res = super::ensure_json_array_value(null_val, "Test list");
        assert_eq!(
            res,
            Err(super::RuntimeValidationError::new(
                "Test list",
                "Test list must be an array.",
            ))
        );
    }
}
