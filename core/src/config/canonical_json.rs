use serde_json::Value;
use std::collections::BTreeMap;

/// Recursively sorts all JSON object keys in lexicographical (alphabetical) order.
///
/// Even when `serde_json` has the `preserve_order` feature enabled (which backs
/// `serde_json::Map` with `IndexMap` instead of `BTreeMap`), inserting keys in
/// sorted order guarantees that serialization will output keys in deterministic
/// canonical order.
pub fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut btree = BTreeMap::new();
            for (k, v) in map {
                btree.insert(k.clone(), canonicalize_json_value(v));
            }
            let mut sorted_map = serde_json::Map::new();
            for (k, v) in btree {
                sorted_map.insert(k, v);
            }
            Value::Object(sorted_map)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(canonicalize_json_value).collect()),
        _ => value.clone(),
    }
}

/// Serializes a `serde_json::Value` to a canonical JSON string with sorted object keys.
pub fn to_canonical_json_string(value: &Value) -> Result<String, serde_json::Error> {
    let canonical = canonicalize_json_value(value);
    serde_json::to_string(&canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonicalizes_keys_in_alphabetical_order() {
        let input = json!({
            "z": 1,
            "m": {
                "b": 2,
                "a": 1
            },
            "a": [
                {"d": 4, "c": 3}
            ]
        });
        let canonical_str = to_canonical_json_string(&input).unwrap();
        assert_eq!(
            canonical_str,
            r#"{"a":[{"c":3,"d":4}],"m":{"a":1,"b":2},"z":1}"#
        );
    }
}
