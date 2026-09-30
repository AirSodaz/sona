use crate::integrations::audio::{AudioDevice, AudioState};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn get_system_audio_devices() -> Result<Vec<AudioDevice>, String> {
    crate::platform::blocking::spawn_blocking_map(
        crate::integrations::audio::get_system_audio_devices,
    )
    .await
}

#[tauri::command]
pub async fn get_microphone_devices() -> Result<Vec<AudioDevice>, String> {
    crate::platform::blocking::spawn_blocking_map(
        crate::integrations::audio::get_microphone_devices,
    )
    .await
}

#[tauri::command]
pub async fn start_system_audio_capture(
    app: AppHandle,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    let capture_app = app.clone();
    crate::platform::blocking::spawn_blocking_map(move || {
        let state = capture_app.state::<AudioState>();
        crate::integrations::audio::start_system_audio_capture(
            capture_app.clone(),
            &state,
            device_name,
            instance_id,
            output_path,
        )
    })
    .await
}

#[tauri::command]
pub async fn start_microphone_capture(
    app: AppHandle,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    let capture_app = app.clone();
    crate::platform::blocking::spawn_blocking_map(move || {
        let state = capture_app.state::<AudioState>();
        crate::integrations::audio::start_microphone_capture(
            capture_app.clone(),
            &state,
            device_name,
            instance_id,
            output_path,
        )
    })
    .await
}
#[tauri::command]
pub async fn stop_system_audio_capture(
    state: State<'_, AudioState>,
    instance_id: String,
) -> Result<String, String> {
    crate::integrations::audio::stop_system_audio_capture(state, instance_id).await
}

#[tauri::command]
pub async fn stop_microphone_capture(
    state: State<'_, AudioState>,
    instance_id: String,
) -> Result<String, String> {
    crate::integrations::audio::stop_microphone_capture(state, instance_id).await
}

#[tauri::command]
pub fn set_system_audio_capture_paused(
    state: State<'_, AudioState>,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    crate::integrations::audio::set_system_audio_capture_paused(state, instance_id, paused)
}

#[tauri::command]
pub fn set_microphone_capture_paused(
    state: State<'_, AudioState>,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    crate::integrations::audio::set_microphone_capture_paused(state, instance_id, paused)
}

#[tauri::command]
pub async fn set_system_audio_mute(mute: bool) -> Result<(), String> {
    crate::platform::system_audio::set_system_audio_mute(mute).await
}
