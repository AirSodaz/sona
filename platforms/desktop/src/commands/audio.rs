use tauri::State;

use crate::integrations::audio::AudioDevice;
use crate::services::DesktopServices;

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
    app: tauri::AppHandle,
    services: State<'_, DesktopServices>,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    let audio_state = services.audio.clone();
    let asr_state = services.asr.clone();
    let emitter = services.emitter.clone();
    let app_data_dir = services
        .sqlite
        .current_context()?
        .app_data_dir()
        .to_path_buf();
    let res = crate::platform::blocking::spawn_blocking_map(move || {
        crate::integrations::audio::start_system_audio_capture(
            audio_state,
            asr_state,
            emitter,
            app_data_dir,
            device_name,
            instance_id,
            output_path,
        )
    })
    .await;
    if res.is_ok() {
        crate::app::tray::set_recording_active(&app, true);
    }
    res
}

#[tauri::command]
pub async fn start_microphone_capture(
    app: tauri::AppHandle,
    services: State<'_, DesktopServices>,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    let audio_state = services.audio.clone();
    let asr_state = services.asr.clone();
    let emitter = services.emitter.clone();
    let app_data_dir = services
        .sqlite
        .current_context()?
        .app_data_dir()
        .to_path_buf();
    let res = crate::platform::blocking::spawn_blocking_map(move || {
        crate::integrations::audio::start_microphone_capture(
            audio_state,
            asr_state,
            emitter,
            app_data_dir,
            device_name,
            instance_id,
            output_path,
        )
    })
    .await;
    if res.is_ok() {
        crate::app::tray::set_recording_active(&app, true);
    }
    res
}

#[tauri::command]
pub async fn stop_system_audio_capture(
    app: tauri::AppHandle,
    services: State<'_, DesktopServices>,
    instance_id: String,
) -> Result<String, String> {
    let res =
        crate::integrations::audio::stop_system_audio_capture(&services.audio, instance_id).await;
    if !services.audio.has_active_captures() {
        crate::app::tray::set_recording_active(&app, false);
    }
    res
}

#[tauri::command]
pub async fn stop_microphone_capture(
    app: tauri::AppHandle,
    services: State<'_, DesktopServices>,
    instance_id: String,
) -> Result<String, String> {
    let res =
        crate::integrations::audio::stop_microphone_capture(&services.audio, instance_id).await;
    if !services.audio.has_active_captures() {
        crate::app::tray::set_recording_active(&app, false);
    }
    res
}
#[tauri::command]
pub fn set_system_audio_capture_paused(
    services: State<'_, DesktopServices>,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    crate::integrations::audio::set_system_audio_capture_paused(
        &services.audio,
        instance_id,
        paused,
    )
}

#[tauri::command]
pub fn set_microphone_capture_paused(
    services: State<'_, DesktopServices>,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    crate::integrations::audio::set_microphone_capture_paused(&services.audio, instance_id, paused)
}

#[tauri::command]
pub async fn set_system_audio_mute(mute: bool) -> Result<(), String> {
    crate::platform::system_audio::set_system_audio_mute(mute).await
}
