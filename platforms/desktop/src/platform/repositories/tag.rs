use sona_core::tag::TagError;
use sona_core::tag::{TagCreateInput, TagListOptions, TagRecord, TagUpdateInput};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::SqliteTagAdapter;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};

use crate::platform::blocking::{map_err_string, with_sqlite_context};

async fn run_tag_adapter<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&SqliteTagAdapter) -> Result<T, TagError> + Send + 'static,
{
    with_sqlite_context(app, move |context| {
        let adapter = context.tag_adapter(Arc::new(UuidGenerator), Arc::new(SystemClock));
        task(&adapter)
    })
    .await
}

pub async fn list_tags<R: Runtime>(
    app: &AppHandle<R>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<TagRecord>, String> {
    let tags = run_tag_adapter(app, move |adapter| {
        adapter.list_tags(TagListOptions {
            fallback_enabled_polish_keyword_set_ids: fallback_enabled_polish_keyword_set_ids
                .unwrap_or_default(),
            fallback_enabled_speaker_profile_ids: fallback_enabled_speaker_profile_ids
                .unwrap_or_default(),
        })
    })
    .await?;
    sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(map_err_string)?;
    Ok(tags)
}

pub async fn replace_tags<R: Runtime>(
    app: &AppHandle<R>,
    tags: Vec<TagRecord>,
) -> Result<(), String> {
    sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(map_err_string)?;
    run_tag_adapter(app, move |adapter| adapter.replace_tags(tags)).await
}

pub async fn create_tag<R: Runtime>(
    app: &AppHandle<R>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
) -> Result<TagRecord, String> {
    let tag = run_tag_adapter(app, move |adapter| {
        adapter.create_tag(TagCreateInput {
            name,
            description,
            icon,
            color,
        })
    })
    .await?;
    sona_ts_bind::validate_tag_record_for_typescript(&tag).map_err(map_err_string)?;
    Ok(tag)
}

pub async fn update_tag<R: Runtime>(
    app: &AppHandle<R>,
    tag_id: String,
    updates: TagUpdateInput,
) -> Result<Option<TagRecord>, String> {
    let tag = run_tag_adapter(app, move |adapter| adapter.update_tag(&tag_id, updates)).await?;
    if let Some(tag) = tag.as_ref() {
        sona_ts_bind::validate_tag_record_for_typescript(tag).map_err(map_err_string)?;
    }
    Ok(tag)
}

pub async fn delete_tag<R: Runtime>(app: &AppHandle<R>, tag_id: String) -> Result<(), String> {
    run_tag_adapter(app, move |adapter| adapter.delete_tag(&tag_id)).await
}

pub async fn reorder_tags<R: Runtime>(
    app: &AppHandle<R>,
    tag_ids: Vec<String>,
) -> Result<Vec<TagRecord>, String> {
    let tags = run_tag_adapter(app, move |adapter| adapter.reorder_tags(tag_ids)).await?;
    sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(map_err_string)?;
    Ok(tags)
}

pub async fn get_active_tag_id<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
    Ok(
        run_tag_adapter(app, |adapter| adapter.get_active_tag_selection())
            .await?
            .tag_id,
    )
}

pub async fn set_active_tag_id<R: Runtime>(
    app: &AppHandle<R>,
    tag_id: Option<String>,
) -> Result<(), String> {
    run_tag_adapter(app, move |adapter| adapter.set_active_tag_id(tag_id)).await
}
