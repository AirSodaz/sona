use tauri::{AppHandle, Runtime};

use crate::platform::tag_repository::{
    create_tag, delete_tag, get_active_tag_id, list_tags, reorder_tags, replace_tags,
    set_active_tag_id, update_tag,
};
use sona_core::tag::{TagDefaultsInput, TagRecord, TagUpdateInput};

#[tauri::command]
pub async fn tag_list<R: Runtime>(
    app: AppHandle<R>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<TagRecord>, String> {
    list_tags(
        &app,
        fallback_enabled_polish_keyword_set_ids,
        fallback_enabled_speaker_profile_ids,
    )
    .await
}

#[tauri::command]
pub async fn tag_save_all<R: Runtime>(
    app: AppHandle<R>,
    tags: Vec<TagRecord>,
) -> Result<(), String> {
    replace_tags(&app, tags).await
}

#[tauri::command]
pub async fn tag_create<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    defaults: TagDefaultsInput,
) -> Result<TagRecord, String> {
    create_tag(&app, name, description, icon, color, defaults).await
}

#[tauri::command]
pub async fn tag_update<R: Runtime>(
    app: AppHandle<R>,
    tag_id: String,
    updates: TagUpdateInput,
) -> Result<Option<TagRecord>, String> {
    update_tag(&app, tag_id, updates).await
}

#[tauri::command]
pub async fn tag_delete<R: Runtime>(app: AppHandle<R>, tag_id: String) -> Result<(), String> {
    delete_tag(&app, tag_id).await
}

#[tauri::command]
pub async fn tag_reorder<R: Runtime>(
    app: AppHandle<R>,
    tag_ids: Vec<String>,
) -> Result<Vec<TagRecord>, String> {
    reorder_tags(&app, tag_ids).await
}

#[tauri::command]
pub async fn tag_get_active_id<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    get_active_tag_id(&app).await
}

#[tauri::command]
pub async fn tag_set_active_id<R: Runtime>(
    app: AppHandle<R>,
    tag_id: Option<String>,
) -> Result<(), String> {
    set_active_tag_id(&app, tag_id).await
}
