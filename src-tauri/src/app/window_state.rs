use std::collections::HashMap;

pub struct AuxWindowStateStore {
    states: std::sync::Mutex<HashMap<String, serde_json::Value>>,
}

impl Default for AuxWindowStateStore {
    fn default() -> Self {
        Self {
            states: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

pub(crate) fn set_aux_window_state(
    state: tauri::State<'_, AuxWindowStateStore>,
    label: String,
    payload: serde_json::Value,
) -> Result<(), String> {
    let mut states = state.states.lock().map_err(|e| e.to_string())?;
    states.insert(label, payload);
    Ok(())
}

pub(crate) fn get_aux_window_state(
    state: tauri::State<'_, AuxWindowStateStore>,
    label: String,
) -> Result<Option<serde_json::Value>, String> {
    let states = state.states.lock().map_err(|e| e.to_string())?;
    Ok(states.get(&label).cloned())
}

pub(crate) fn clear_aux_window_state(
    state: tauri::State<'_, AuxWindowStateStore>,
    label: String,
) -> Result<(), String> {
    let mut states = state.states.lock().map_err(|e| e.to_string())?;
    states.remove(&label);
    Ok(())
}
