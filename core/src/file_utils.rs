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
