use serde::de::DeserializeOwned;

pub fn parse_params<T: DeserializeOwned>(
    params: Option<serde_json::Value>,
) -> Result<T, (i32, String)> {
    let value = params.unwrap_or_else(|| serde_json::json!({}));
    serde_json::from_value(value).map_err(|e| (-32602, format!("Invalid params: {e}")))
}

pub fn extract_history_id(params: &Option<serde_json::Value>) -> Result<String, (i32, String)> {
    match params {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(map)) => {
            if let Some(serde_json::Value::String(s)) =
                map.get("history_id").or_else(|| map.get("historyId"))
            {
                Ok(s.clone())
            } else {
                Err((-32602, "Missing 'history_id' parameter".to_string()))
            }
        }
        _ => Err((
            -32602,
            "Invalid or missing parameters for history_id".to_string(),
        )),
    }
}

pub fn parse_delete_history(
    params: &Option<serde_json::Value>,
) -> Result<(String, bool), (i32, String)> {
    match params {
        Some(serde_json::Value::String(s)) => Ok((s.clone(), false)),
        Some(serde_json::Value::Object(map)) => {
            let id = if let Some(serde_json::Value::String(s)) =
                map.get("history_id").or_else(|| map.get("historyId"))
            {
                s.clone()
            } else {
                return Err((-32602, "Missing 'history_id' parameter".to_string()));
            };
            let permanent = map
                .get("permanent")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok((id, permanent))
        }
        _ => Err((-32602, "Invalid parameters for delete_history".to_string())),
    }
}

pub fn parse_optional_project_id(params: &Option<serde_json::Value>) -> Option<String> {
    match params {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Object(map)) => map
            .get("project_id")
            .or_else(|| map.get("projectId"))
            .or_else(|| map.get("id"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        _ => None,
    }
}

pub fn extract_project_id(params: &Option<serde_json::Value>) -> Result<String, (i32, String)> {
    match params.as_ref() {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(map)) => {
            if let Some(serde_json::Value::String(s)) = map
                .get("project_id")
                .or_else(|| map.get("projectId"))
                .or_else(|| map.get("id"))
            {
                Ok(s.clone())
            } else {
                Err((-32602, "Missing 'project_id' or 'id' parameter".to_string()))
            }
        }
        _ => Err((
            -32602,
            "Invalid or missing parameters for project id".to_string(),
        )),
    }
}

pub fn parse_optional_key(params: &Option<serde_json::Value>) -> Option<String> {
    match params {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Object(map)) => map
            .get("key")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        _ => None,
    }
}

pub fn parse_update_setting(
    params: &Option<serde_json::Value>,
) -> Result<(String, serde_json::Value), (i32, String)> {
    match params {
        Some(serde_json::Value::Object(map)) => {
            let key = if let Some(serde_json::Value::String(s)) = map.get("key") {
                s.clone()
            } else {
                return Err((-32602, "Missing 'key' parameter".to_string()));
            };
            let value = map.get("value").cloned().unwrap_or(serde_json::Value::Null);
            Ok((key, value))
        }
        _ => Err((-32602, "Invalid parameters for update_setting".to_string())),
    }
}

pub fn find_string_param<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    snake_key: &str,
) -> Option<&'a str> {
    if let Some(val) = map.get(snake_key).and_then(|v| v.as_str()) {
        return Some(val);
    }

    if snake_key == "instance_id" {
        let alias = map
            .get("task_id")
            .or_else(|| map.get("taskId"))
            .and_then(|v| v.as_str());
        if alias.is_some() {
            return alias;
        }
    }

    let static_camel = match snake_key {
        "history_id" => Some("historyId"),
        "project_id" => Some("projectId"),
        "snapshot_id" => Some("snapshotId"),
        "conflict_id" => Some("conflictId"),
        "instance_id" => Some("instanceId"),
        "download_id" => Some("downloadId"),
        "model_id" => Some("modelId"),
        "cascade_action" => Some("cascadeAction"),
        _ => None,
    };

    if let Some(camel) = static_camel {
        if let Some(val) = map.get(camel).and_then(|v| v.as_str()) {
            return Some(val);
        }
    } else {
        let mut camel = String::with_capacity(snake_key.len());
        let mut capitalize_next = false;
        for ch in snake_key.chars() {
            if ch == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                for upper in ch.to_uppercase() {
                    camel.push(upper);
                }
                capitalize_next = false;
            } else {
                camel.push(ch);
            }
        }
        if let Some(val) = map.get(&camel).and_then(|v| v.as_str()) {
            return Some(val);
        }
    }

    None
}

pub fn extract_string_param(
    params: &Option<serde_json::Value>,
    key: &str,
) -> Result<String, (i32, String)> {
    match params.as_ref() {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(map)) => {
            if let Some(s) = find_string_param(map, key) {
                Ok(s.to_string())
            } else {
                Err((-32602, format!("Missing '{key}' parameter")))
            }
        }
        _ => Err((-32602, format!("Invalid or missing parameters for '{key}'"))),
    }
}

pub fn parse_optional_string(params: &Option<serde_json::Value>, key: &str) -> Option<String> {
    match params.as_ref() {
        Some(serde_json::Value::Object(map)) => {
            find_string_param(map, key).map(ToString::to_string)
        }
        _ => None,
    }
}
