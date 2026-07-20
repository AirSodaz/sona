use std::future::Future;

use serde_json::Value;
use sona_core::tag::TagError;
use sona_core::tag::{
    ACTIVE_TAG_SETTINGS_KEY, ActiveTagSelection, LEGACY_ACTIVE_PROJECT_SETTINGS_KEY,
    TagCreateInput, TagListOptions, TagRecord, TagUpdateInput, active_tag_id_from_value,
};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::SqliteTagAdapter;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub(crate) const SETTINGS_FILE_NAME: &str = "settings.json";

async fn run_tag_adapter<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&SqliteTagAdapter) -> Result<T, TagError> + Send + 'static,
{
    let context = crate::platform::database::sqlite_application_context(app);
    tauri::async_runtime::spawn_blocking(move || {
        let adapter = context.tag_adapter(Arc::new(UuidGenerator), Arc::new(SystemClock));
        task(&adapter).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
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
    sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(|error| error.to_string())?;
    Ok(tags)
}

pub async fn replace_tags<R: Runtime>(
    app: &AppHandle<R>,
    tags: Vec<TagRecord>,
) -> Result<(), String> {
    sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(|error| error.to_string())?;
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
    sona_ts_bind::validate_tag_record_for_typescript(&tag).map_err(|error| error.to_string())?;
    Ok(tag)
}

pub async fn update_tag<R: Runtime>(
    app: &AppHandle<R>,
    tag_id: String,
    updates: TagUpdateInput,
) -> Result<Option<TagRecord>, String> {
    let tag = run_tag_adapter(app, move |adapter| adapter.update_tag(&tag_id, updates)).await?;
    if let Some(tag) = tag.as_ref() {
        sona_ts_bind::validate_tag_record_for_typescript(tag).map_err(|error| error.to_string())?;
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
    sona_ts_bind::validate_tag_records_for_typescript(&tags).map_err(|error| error.to_string())?;
    Ok(tags)
}

pub async fn get_active_tag_id<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
    let selection = run_tag_adapter(app, |adapter| adapter.get_active_tag_selection()).await?;
    if selection.setting_exists {
        return Ok(selection.tag_id);
    }

    let legacy_store = app
        .store(SETTINGS_FILE_NAME)
        .map_err(|error| error.to_string())?;
    resolve_active_tag_id(
        selection,
        || {
            legacy_store
                .get(ACTIVE_TAG_SETTINGS_KEY)
                .or_else(|| legacy_store.get(LEGACY_ACTIVE_PROJECT_SETTINGS_KEY))
        },
        |tag_id| async move {
            run_tag_adapter(app, move |adapter| adapter.set_active_tag_id(Some(tag_id))).await
        },
    )
    .await
}

async fn resolve_active_tag_id<L, P, F>(
    selection: ActiveTagSelection,
    load_legacy: L,
    persist: P,
) -> Result<Option<String>, String>
where
    L: FnOnce() -> Option<Value>,
    P: FnOnce(String) -> F,
    F: Future<Output = Result<(), String>>,
{
    if selection.setting_exists {
        return Ok(selection.tag_id);
    }

    let tag_id = load_legacy().and_then(|value| active_tag_id_from_value(&value));
    if let Some(tag_id) = &tag_id {
        persist(tag_id.clone()).await?;
    }
    Ok(tag_id)
}

pub async fn set_active_tag_id<R: Runtime>(
    app: &AppHandle<R>,
    tag_id: Option<String>,
) -> Result<(), String> {
    run_tag_adapter(app, move |adapter| adapter.set_active_tag_id(tag_id)).await
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::future::ready;

    use serde_json::json;
    use sona_core::tag::ActiveTagSelection;

    use super::resolve_active_tag_id;

    #[test]
    fn missing_active_setting_reads_legacy_and_migrates_trimmed_id() {
        let legacy_reads = Cell::new(0);
        let persisted = RefCell::new(Vec::new());

        let result = tauri::async_runtime::block_on(resolve_active_tag_id(
            ActiveTagSelection {
                setting_exists: false,
                tag_id: None,
            },
            || {
                legacy_reads.set(legacy_reads.get() + 1);
                Some(json!("  legacy-tag  "))
            },
            |tag_id| {
                persisted.borrow_mut().push(tag_id);
                ready(Ok(()))
            },
        ))
        .unwrap();

        assert_eq!(result.as_deref(), Some("legacy-tag"));
        assert_eq!(legacy_reads.get(), 1);
        assert_eq!(*persisted.borrow(), vec!["legacy-tag"]);
    }

    #[test]
    fn present_empty_active_settings_never_read_legacy() {
        let legacy_reads = Cell::new(0);
        let result = tauri::async_runtime::block_on(resolve_active_tag_id(
            ActiveTagSelection {
                setting_exists: true,
                tag_id: None,
            },
            || {
                legacy_reads.set(legacy_reads.get() + 1);
                Some(json!("legacy-tag"))
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
        let result = tauri::async_runtime::block_on(resolve_active_tag_id(
            ActiveTagSelection {
                setting_exists: true,
                tag_id: None,
            },
            || {
                legacy_reads.set(legacy_reads.get() + 1);
                Some(json!("legacy-tag"))
            },
            |_| ready(Err("unexpected persistence".to_string())),
        ))
        .unwrap();

        assert_eq!(result, None);
        assert_eq!(legacy_reads.get(), 0);
    }

    #[test]
    fn legacy_active_migration_propagates_persistence_errors() {
        let error = tauri::async_runtime::block_on(resolve_active_tag_id(
            ActiveTagSelection {
                setting_exists: false,
                tag_id: None,
            },
            || Some(json!("legacy-tag")),
            |_| ready(Err("persist failed".to_string())),
        ))
        .unwrap_err();

        assert_eq!(error, "persist failed");
    }
}
