use tauri::State;

use crate::services::DesktopServices;
use sona_core::tag::{TagRecord, TagUpdateInput};

#[tauri::command]
pub async fn tag_list(
    services: State<'_, DesktopServices>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<TagRecord>, String> {
    services
        .projects
        .list_tags(
            fallback_enabled_polish_keyword_set_ids,
            fallback_enabled_speaker_profile_ids,
        )
        .await
}

#[tauri::command]
pub async fn tag_save_all(
    services: State<'_, DesktopServices>,
    tags: Vec<TagRecord>,
) -> Result<(), String> {
    services.projects.replace_tags(tags).await
}

#[tauri::command]
pub async fn tag_create(
    services: State<'_, DesktopServices>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
) -> Result<TagRecord, String> {
    services
        .projects
        .create_tag(name, description, icon, color)
        .await
}

#[tauri::command]
pub async fn tag_update(
    services: State<'_, DesktopServices>,
    tag_id: String,
    updates: TagUpdateInput,
) -> Result<Option<TagRecord>, String> {
    services.projects.update_tag(tag_id, updates).await
}

#[tauri::command]
pub async fn tag_delete(
    services: State<'_, DesktopServices>,
    tag_id: String,
) -> Result<(), String> {
    services.projects.delete_tag(tag_id).await
}

#[tauri::command]
pub async fn tag_reorder(
    services: State<'_, DesktopServices>,
    tag_ids: Vec<String>,
) -> Result<Vec<TagRecord>, String> {
    services.projects.reorder_tags(tag_ids).await
}

#[tauri::command]
pub async fn tag_get_active_id(
    services: State<'_, DesktopServices>,
) -> Result<Option<String>, String> {
    services.projects.get_active_tag_id().await
}

#[tauri::command]
pub async fn tag_set_active_id(
    services: State<'_, DesktopServices>,
    tag_id: Option<String>,
) -> Result<(), String> {
    services.projects.set_active_tag_id(tag_id).await
}
