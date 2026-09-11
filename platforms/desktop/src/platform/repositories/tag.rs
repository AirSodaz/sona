use sona_core::project::{
    ProjectCreateInput, ProjectPipelineConfig, ProjectRecord, ProjectUpdateInput,
};
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

fn project_from_tag(tag: TagRecord, pipeline: Option<ProjectPipelineConfig>) -> ProjectRecord {
    ProjectRecord {
        id: tag.id,
        name: tag.name,
        description: tag.description,
        icon: (!tag.icon.is_empty()).then_some(tag.icon),
        color: (!tag.color.is_empty()).then_some(tag.color),
        sort_order: tag.sort_order,
        created_at: tag.created_at,
        updated_at: tag.updated_at,
        pipeline,
    }
}

pub async fn list_projects<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<ProjectRecord>, String> {
    run_tag_adapter(app, |adapter| {
        adapter
            .list_tags(TagListOptions::default())?
            .into_iter()
            .map(|tag| {
                let pipeline = adapter.get_project_pipeline(&tag.id)?;
                Ok(project_from_tag(tag, pipeline))
            })
            .collect()
    })
    .await
}

pub async fn create_project<R: Runtime>(
    app: &AppHandle<R>,
    input: ProjectCreateInput,
) -> Result<ProjectRecord, String> {
    run_tag_adapter(app, move |adapter| {
        let pipeline = input.pipeline;
        let tag = adapter.create_tag(TagCreateInput {
            name: input.name,
            description: input.description,
            icon: input.icon,
            color: input.color,
        })?;
        if let Some(value) = pipeline.as_ref() {
            adapter.set_project_pipeline(&tag.id, value)?;
        }
        Ok(project_from_tag(tag, pipeline))
    })
    .await
}

pub async fn update_project<R: Runtime>(
    app: &AppHandle<R>,
    project_id: String,
    updates: ProjectUpdateInput,
) -> Result<Option<ProjectRecord>, String> {
    run_tag_adapter(app, move |adapter| {
        let pipeline_update = updates.pipeline;
        let tag = adapter.update_tag(
            &project_id,
            TagUpdateInput {
                name: updates.name,
                description: updates.description,
                icon: updates.icon,
                color: updates.color,
            },
        )?;
        let Some(tag) = tag else {
            return Ok(None);
        };
        if let Some(value) = pipeline_update.as_ref() {
            adapter.set_project_pipeline(&project_id, value)?;
        }
        let pipeline = if pipeline_update.is_some() {
            pipeline_update
        } else {
            adapter.get_project_pipeline(&project_id)?
        };
        Ok(Some(project_from_tag(tag, pipeline)))
    })
    .await
}

pub async fn delete_project<R: Runtime>(
    app: &AppHandle<R>,
    project_id: String,
) -> Result<(), String> {
    run_tag_adapter(app, move |adapter| adapter.delete_tag(&project_id)).await
}

pub async fn delete_project_with_cascade<R: Runtime>(
    app: &AppHandle<R>,
    project_id: String,
    cascade_action: String,
) -> Result<(), String> {
    run_tag_adapter(app, move |adapter| {
        adapter.delete_project_with_cascade(&project_id, &cascade_action)
    })
    .await
}

pub async fn reorder_projects<R: Runtime>(
    app: &AppHandle<R>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    run_tag_adapter(app, move |adapter| {
        adapter
            .reorder_tags(project_ids)?
            .into_iter()
            .map(|tag| {
                let pipeline = adapter.get_project_pipeline(&tag.id)?;
                Ok(project_from_tag(tag, pipeline))
            })
            .collect()
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
