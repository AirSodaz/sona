use std::future::Future;

use serde_json::Value;
use sona_core::project::{
    ACTIVE_PROJECT_SETTINGS_KEY, ActiveProjectSelection, ProjectCreateInput, ProjectDefaultsInput,
    ProjectListOptions, ProjectRecord, ProjectRepositoryService, active_project_id_from_value,
};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::project::SqliteProjectRepository;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub(crate) const SETTINGS_FILE_NAME: &str = "settings.json";

async fn run_project_service<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: for<'a> FnOnce(ProjectRepositoryService<'a>) -> Result<T, String> + Send + 'static,
{
    let db = crate::platform::database::sqlite_database(app);
    tauri::async_runtime::spawn_blocking(move || {
        let repository = SqliteProjectRepository::new(db);
        let ids = UuidGenerator;
        let clock = SystemClock;
        task(ProjectRepositoryService::new(&repository, &ids, &clock))
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn list_projects<R: Runtime>(
    app: &AppHandle<R>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_service(app, move |service| {
        service.list_projects(ProjectListOptions {
            fallback_enabled_polish_keyword_set_ids: fallback_enabled_polish_keyword_set_ids
                .unwrap_or_default(),
            fallback_enabled_speaker_profile_ids: fallback_enabled_speaker_profile_ids
                .unwrap_or_default(),
        })
    })
    .await
}

pub async fn replace_projects<R: Runtime>(
    app: &AppHandle<R>,
    projects: Vec<Value>,
) -> Result<(), String> {
    run_project_service(app, move |service| service.replace_projects_json(projects)).await
}

pub async fn create_project<R: Runtime>(
    app: &AppHandle<R>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    defaults: ProjectDefaultsInput,
) -> Result<ProjectRecord, String> {
    run_project_service(app, move |service| {
        service.create_project(ProjectCreateInput {
            name,
            description,
            icon,
            defaults,
        })
    })
    .await
}

pub async fn update_project<R: Runtime>(
    app: &AppHandle<R>,
    project_id: String,
    updates: Value,
) -> Result<Option<ProjectRecord>, String> {
    run_project_service(app, move |service| {
        service.update_project_json(&project_id, updates)
    })
    .await
}

pub async fn delete_project<R: Runtime>(
    app: &AppHandle<R>,
    project_id: String,
) -> Result<(), String> {
    run_project_service(app, move |service| service.delete_project(&project_id)).await
}

pub async fn reorder_projects<R: Runtime>(
    app: &AppHandle<R>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_service(app, move |service| service.reorder_projects(project_ids)).await
}

pub async fn get_active_project_id<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, String> {
    let selection =
        run_project_service(app, |service| service.get_active_project_selection()).await?;
    if selection.setting_exists {
        return Ok(selection.project_id);
    }

    let legacy_store = app
        .store(SETTINGS_FILE_NAME)
        .map_err(|error| error.to_string())?;
    resolve_active_project_id(
        selection,
        || legacy_store.get(ACTIVE_PROJECT_SETTINGS_KEY),
        |project_id| async move {
            run_project_service(app, move |service| {
                service.set_active_project_id(Some(project_id))
            })
            .await
        },
    )
    .await
}

async fn resolve_active_project_id<L, P, F>(
    selection: ActiveProjectSelection,
    load_legacy: L,
    persist: P,
) -> Result<Option<String>, String>
where
    L: FnOnce() -> Option<Value>,
    P: FnOnce(String) -> F,
    F: Future<Output = Result<(), String>>,
{
    if selection.setting_exists {
        return Ok(selection.project_id);
    }

    let project_id = load_legacy().and_then(|value| active_project_id_from_value(&value));
    if let Some(project_id) = &project_id {
        persist(project_id.clone()).await?;
    }
    Ok(project_id)
}

pub async fn set_active_project_id<R: Runtime>(
    app: &AppHandle<R>,
    project_id: Option<String>,
) -> Result<(), String> {
    run_project_service(app, move |service| {
        service.set_active_project_id(project_id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::future::ready;

    use serde_json::json;
    use sona_core::project::ActiveProjectSelection;

    use super::resolve_active_project_id;

    #[test]
    fn missing_active_setting_reads_legacy_and_migrates_trimmed_id() {
        let legacy_reads = Cell::new(0);
        let persisted = RefCell::new(Vec::new());

        let result = tauri::async_runtime::block_on(resolve_active_project_id(
            ActiveProjectSelection {
                setting_exists: false,
                project_id: None,
            },
            || {
                legacy_reads.set(legacy_reads.get() + 1);
                Some(json!("  legacy-project  "))
            },
            |project_id| {
                persisted.borrow_mut().push(project_id);
                ready(Ok(()))
            },
        ))
        .unwrap();

        assert_eq!(result.as_deref(), Some("legacy-project"));
        assert_eq!(legacy_reads.get(), 1);
        assert_eq!(*persisted.borrow(), vec!["legacy-project"]);
    }

    #[test]
    fn present_empty_active_settings_never_read_legacy() {
        let legacy_reads = Cell::new(0);
        let result = tauri::async_runtime::block_on(resolve_active_project_id(
            ActiveProjectSelection {
                setting_exists: true,
                project_id: None,
            },
            || {
                legacy_reads.set(legacy_reads.get() + 1);
                Some(json!("legacy-project"))
            },
            |_| ready(Err("unexpected persistence".to_string())),
        ))
        .unwrap();

        assert_eq!(result, None);
        assert_eq!(legacy_reads.get(), 0);
    }

    #[test]
    fn present_non_string_active_setting_never_reads_legacy() {
        let legacy_reads = Cell::new(0);
        let result = tauri::async_runtime::block_on(resolve_active_project_id(
            ActiveProjectSelection {
                setting_exists: true,
                project_id: None,
            },
            || {
                legacy_reads.set(legacy_reads.get() + 1);
                Some(json!("legacy-project"))
            },
            |_| ready(Err("unexpected persistence".to_string())),
        ))
        .unwrap();

        assert_eq!(result, None);
        assert_eq!(legacy_reads.get(), 0);
    }

    #[test]
    fn legacy_active_migration_propagates_persistence_errors() {
        let error = tauri::async_runtime::block_on(resolve_active_project_id(
            ActiveProjectSelection {
                setting_exists: false,
                project_id: None,
            },
            || Some(json!("legacy-project")),
            |_| ready(Err("persist failed".to_string())),
        ))
        .unwrap_err();

        assert_eq!(error, "persist failed");
    }
}
