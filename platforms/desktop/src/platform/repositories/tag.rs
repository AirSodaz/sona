use sona_core::project::{
    ProjectCreateInput, ProjectPipelineConfig, ProjectRecord, ProjectUpdateInput,
};
use sona_core::tag::TagError;
use sona_core::tag::{TagCreateInput, TagListOptions, TagRecord, TagUpdateInput};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::SqliteTagAdapter;
use std::sync::Arc;

use crate::platform::database::DesktopSqliteState;
use crate::services::db_runner::{map_err_string, run_sqlite_task};

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

/// Pure Rust Project and Tag domain service without any Tauri dependency.
#[derive(Clone)]
pub struct ProjectService {
    sqlite: DesktopSqliteState,
}

pub type TagService = ProjectService;

impl ProjectService {
    pub fn new(sqlite: DesktopSqliteState) -> Self {
        Self { sqlite }
    }

    pub fn sqlite(&self) -> &DesktopSqliteState {
        &self.sqlite
    }

    async fn run_adapter<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&SqliteTagAdapter) -> Result<T, TagError> + Send + 'static,
    {
        run_sqlite_task(&self.sqlite, move |context| {
            let adapter = context.tag_adapter(Arc::new(UuidGenerator), Arc::new(SystemClock));
            task(&adapter)
        })
        .await
    }

    pub async fn list(&self) -> Result<Vec<ProjectRecord>, String> {
        self.run_adapter(|adapter| {
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

    pub async fn create(&self, input: ProjectCreateInput) -> Result<ProjectRecord, String> {
        self.run_adapter(move |adapter| {
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

    pub async fn update(
        &self,
        project_id: String,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, String> {
        self.run_adapter(move |adapter| {
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

    pub async fn delete(&self, project_id: String) -> Result<(), String> {
        self.run_adapter(move |adapter| adapter.delete_tag(&project_id))
            .await
    }

    pub async fn delete_with_cascade(
        &self,
        project_id: String,
        cascade_action: String,
    ) -> Result<(), String> {
        self.run_adapter(move |adapter| {
            adapter.delete_project_with_cascade(&project_id, &cascade_action)
        })
        .await
    }

    pub async fn reorder(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
        self.run_adapter(move |adapter| {
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

    pub async fn list_tags(
        &self,
        fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
        fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
    ) -> Result<Vec<TagRecord>, String> {
        let tags = self
            .run_adapter(move |adapter| {
                adapter.list_tags(TagListOptions {
                    fallback_enabled_polish_keyword_set_ids:
                        fallback_enabled_polish_keyword_set_ids.unwrap_or_default(),
                    fallback_enabled_speaker_profile_ids: fallback_enabled_speaker_profile_ids
                        .unwrap_or_default(),
                })
            })
            .await?;
        sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(map_err_string)?;
        Ok(tags)
    }

    pub async fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), String> {
        sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(map_err_string)?;
        self.run_adapter(move |adapter| adapter.replace_tags(tags))
            .await
    }

    pub async fn create_tag(
        &self,
        name: String,
        description: Option<String>,
        icon: Option<String>,
        color: Option<String>,
    ) -> Result<TagRecord, String> {
        let tag = self
            .run_adapter(move |adapter| {
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

    pub async fn update_tag(
        &self,
        tag_id: String,
        updates: TagUpdateInput,
    ) -> Result<Option<TagRecord>, String> {
        let tag = self
            .run_adapter(move |adapter| adapter.update_tag(&tag_id, updates))
            .await?;
        if let Some(tag) = tag.as_ref() {
            sona_ts_bind::validate_tag_record_for_typescript(tag).map_err(map_err_string)?;
        }
        Ok(tag)
    }

    pub async fn delete_tag(&self, tag_id: String) -> Result<(), String> {
        self.run_adapter(move |adapter| adapter.delete_tag(&tag_id))
            .await
    }

    pub async fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, String> {
        let tags = self
            .run_adapter(move |adapter| adapter.reorder_tags(tag_ids))
            .await?;
        sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(map_err_string)?;
        Ok(tags)
    }

    pub async fn get_active_tag_id(&self) -> Result<Option<String>, String> {
        Ok(self
            .run_adapter(|adapter| adapter.get_active_tag_selection())
            .await?
            .tag_id)
    }

    pub async fn set_active_tag_id(&self, tag_id: Option<String>) -> Result<(), String> {
        self.run_adapter(move |adapter| adapter.set_active_tag_id(tag_id))
            .await
    }
}

// Standalone functions for convenience or transition
pub async fn list_projects(sqlite: &DesktopSqliteState) -> Result<Vec<ProjectRecord>, String> {
    ProjectService::new(sqlite.clone()).list().await
}

pub async fn create_project(
    sqlite: &DesktopSqliteState,
    input: ProjectCreateInput,
) -> Result<ProjectRecord, String> {
    ProjectService::new(sqlite.clone()).create(input).await
}

pub async fn update_project(
    sqlite: &DesktopSqliteState,
    project_id: String,
    updates: ProjectUpdateInput,
) -> Result<Option<ProjectRecord>, String> {
    ProjectService::new(sqlite.clone())
        .update(project_id, updates)
        .await
}

pub async fn delete_project(sqlite: &DesktopSqliteState, project_id: String) -> Result<(), String> {
    ProjectService::new(sqlite.clone()).delete(project_id).await
}

pub async fn delete_project_with_cascade(
    sqlite: &DesktopSqliteState,
    project_id: String,
    cascade_action: String,
) -> Result<(), String> {
    ProjectService::new(sqlite.clone())
        .delete_with_cascade(project_id, cascade_action)
        .await
}

pub async fn reorder_projects(
    sqlite: &DesktopSqliteState,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    ProjectService::new(sqlite.clone())
        .reorder(project_ids)
        .await
}

pub async fn list_tags(
    sqlite: &DesktopSqliteState,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<TagRecord>, String> {
    ProjectService::new(sqlite.clone())
        .list_tags(
            fallback_enabled_polish_keyword_set_ids,
            fallback_enabled_speaker_profile_ids,
        )
        .await
}

pub async fn replace_tags(sqlite: &DesktopSqliteState, tags: Vec<TagRecord>) -> Result<(), String> {
    ProjectService::new(sqlite.clone()).replace_tags(tags).await
}

pub async fn create_tag(
    sqlite: &DesktopSqliteState,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
) -> Result<TagRecord, String> {
    ProjectService::new(sqlite.clone())
        .create_tag(name, description, icon, color)
        .await
}

pub async fn update_tag(
    sqlite: &DesktopSqliteState,
    tag_id: String,
    updates: TagUpdateInput,
) -> Result<Option<TagRecord>, String> {
    ProjectService::new(sqlite.clone())
        .update_tag(tag_id, updates)
        .await
}

pub async fn delete_tag(sqlite: &DesktopSqliteState, tag_id: String) -> Result<(), String> {
    ProjectService::new(sqlite.clone()).delete_tag(tag_id).await
}

pub async fn reorder_tags(
    sqlite: &DesktopSqliteState,
    tag_ids: Vec<String>,
) -> Result<Vec<TagRecord>, String> {
    ProjectService::new(sqlite.clone())
        .reorder_tags(tag_ids)
        .await
}

pub async fn get_active_tag_id(sqlite: &DesktopSqliteState) -> Result<Option<String>, String> {
    ProjectService::new(sqlite.clone())
        .get_active_tag_id()
        .await
}

pub async fn set_active_tag_id(
    sqlite: &DesktopSqliteState,
    tag_id: Option<String>,
) -> Result<(), String> {
    ProjectService::new(sqlite.clone())
        .set_active_tag_id(tag_id)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_project_service_without_tauri() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let state = DesktopSqliteState::new(ctx);
        let project_service = ProjectService::new(state);
        let projects = project_service.list().await.unwrap();
        assert_eq!(projects.len(), 0);

        let created = project_service
            .create(ProjectCreateInput {
                name: "Test Project".to_string(),
                description: Some("Desc".to_string()),
                icon: None,
                color: None,
                pipeline: None,
            })
            .await
            .unwrap();
        assert_eq!(created.name, "Test Project");

        let listed = project_service.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
    }
}
