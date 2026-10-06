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

/// Recursively sanitizes credentials and sensitive API keys in an application configuration JSON value.
/// Replaces sensitive string values with an empty string (`""`).
pub fn strip_config_credentials(config: &mut Value) {
    match config {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                let lower_key = key.to_ascii_lowercase();
                if lower_key == "httpserverapikey"
                    || lower_key == "http_server_api_key"
                    || lower_key == "apikey"
                    || lower_key == "api_key"
                    || lower_key == "apisecret"
                    || lower_key == "api_secret"
                    || lower_key == "accesstoken"
                    || lower_key == "access_token"
                    || lower_key == "secretkey"
                    || lower_key == "secret_key"
                {
                    if let Value::String(s) = value
                        && !s.is_empty()
                    {
                        *s = String::new();
                    }
                } else {
                    strip_config_credentials(value);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                strip_config_credentials(item);
            }
        }
        _ => {}
    }
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

    #[test]
    fn strips_sensitive_credentials_from_config() {
        let mut config = json!({
            "language": "en",
            "httpServerApiKey": "my-secret-token",
            "llmSettings": {
                "providers": {
                    "openai": {
                        "apiKey": "sk-123456",
                        "model": "gpt-4o"
                    }
                },
                "models": {
                    "custom": {
                        "apiKey": "sk-custom"
                    }
                }
            },
            "asr": {
                "providers": {
                    "online": {
                        "doubao": {
                            "api_key": "doubao-secret",
                            "app_id": "app-123"
                        }
                    }
                }
            }
        });

        strip_config_credentials(&mut config);

        assert_eq!(config["language"], "en");
        assert_eq!(config["httpServerApiKey"], "");
        assert_eq!(config["llmSettings"]["providers"]["openai"]["apiKey"], "");
        assert_eq!(
            config["llmSettings"]["providers"]["openai"]["model"],
            "gpt-4o"
        );
        assert_eq!(config["llmSettings"]["models"]["custom"]["apiKey"], "");
        assert_eq!(
            config["asr"]["providers"]["online"]["doubao"]["api_key"],
            ""
        );
        assert_eq!(
            config["asr"]["providers"]["online"]["doubao"]["app_id"],
            "app-123"
        );
    }
}
