use std::time::{SystemTime, UNIX_EPOCH};

pub fn ensure_json_array_value(
    value: serde_json::Value,
    label: &str,
) -> Result<serde_json::Value, String> {
    if value.is_array() {
        Ok(value)
    } else {
        Err(format!("{label} must be an array."))
    }
}

pub fn current_time_millis() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|error| error.to_string())
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
        assert_eq!(res, Err("Test list must be an array.".to_string()));

        let null_val = json!(null);
        let res = super::ensure_json_array_value(null_val, "Test list");
        assert_eq!(res, Err("Test list must be an array.".to_string()));
    }
}
