use crate::integrations::audio::{AudioDevice, AudioState};
use tauri::{AppHandle, State, Window};

#[tauri::command(async)]
pub fn get_system_audio_devices() -> Result<Vec<AudioDevice>, String> {
    crate::integrations::audio::get_system_audio_devices()
}

#[tauri::command(async)]
pub fn get_microphone_devices() -> Result<Vec<AudioDevice>, String> {
    crate::integrations::audio::get_microphone_devices()
}

#[tauri::command(async)]
pub fn start_system_audio_capture(
    app: AppHandle,
    window: Window,
    state: State<'_, AudioState>,
    sherpa_state: State<'_, crate::integrations::asr::AsrState>,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    crate::integrations::audio::start_system_audio_capture(
        app,
        window,
        state,
        sherpa_state,
        device_name,
        instance_id,
        output_path,
    )
}

#[tauri::command(async)]
pub fn start_microphone_capture(
    app: AppHandle,
    window: Window,
    state: State<'_, AudioState>,
    sherpa_state: State<'_, crate::integrations::asr::AsrState>,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    crate::integrations::audio::start_microphone_capture(
        app,
        window,
        state,
        sherpa_state,
        device_name,
        instance_id,
        output_path,
    )
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
pub fn set_microphone_boost(state: State<'_, AudioState>, boost: f32) -> Result<(), String> {
    crate::integrations::audio::set_microphone_boost(state, boost)
}

#[tauri::command]
pub async fn set_system_audio_mute(mute: bool) -> Result<(), String> {
    crate::platform::system_audio::set_system_audio_mute(mute).await
}
