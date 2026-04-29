use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime, State};
use uuid::Uuid;
use walkdir::WalkDir;

const HISTORY_DIR_NAME: &str = "history";
const HISTORY_INDEX_FILE_NAME: &str = "index.json";
const SUMMARY_FILE_SUFFIX: &str = ".summary.json";

const CONFIG_DIR_NAME: &str = "config";
const CONFIG_FILE_NAME: &str = "sona-config.json";
const PROJECTS_DIR_NAME: &str = "projects";
const PROJECTS_INDEX_FILE_NAME: &str = "index.json";
const AUTOMATION_DIR_NAME: &str = "automation";
const AUTOMATION_RULES_FILE_NAME: &str = "rules.json";
const AUTOMATION_PROCESSED_FILE_NAME: &str = "processed.json";
const ANALYTICS_DIR_NAME: &str = "analytics";
const ANALYTICS_USAGE_FILE_NAME: &str = "llm-usage.json";

const BACKUP_SCHEMA_VERSION: u64 = 1;
const BACKUP_HISTORY_MODE: &str = "light";

#[derive(Clone, Default)]
pub struct HistoryRepositoryState {
    lock: Arc<Mutex<()>>,
}

#[derive(Clone, Default)]
pub struct PreparedBackupImportState {
    inner: Arc<Mutex<HashMap<String, PreparedBackupImportSnapshot>>>,
}

#[derive(Clone, Debug)]
struct PreparedBackupImportSnapshot {
    archive_path: String,
    extraction_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryItemKind {
    Recording,
    Batch,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryItemStatus {
    Draft,
    Complete,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryDraftSource {
    LiveRecord,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemRecord {
    pub id: String,
    pub timestamp: u64,
    pub duration: f64,
    pub audio_path: String,
    pub transcript_path: String,
    pub title: String,
    pub preview_text: String,
    pub icon: Option<String>,
    #[serde(rename = "type")]
    pub kind: HistoryItemKind,
    pub search_content: String,
    pub project_id: Option<String>,
    pub status: HistoryItemStatus,
    pub draft_source: Option<HistoryDraftSource>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveRecordingDraftResult {
    pub item: HistoryItemRecord,
    pub audio_absolute_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBackupArchiveRequest {
    pub archive_path: String,
    pub app_version: String,
    pub config: Value,
    pub projects: Vec<Value>,
    pub automation_rules: Vec<Value>,
    pub automation_processed_entries: Vec<Value>,
    pub analytics_content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub schema_version: u64,
    pub created_at: String,
    pub app_version: String,
    pub history_mode: String,
    pub scopes: BackupManifestScopes,
    pub counts: BackupManifestCounts,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifestScopes {
    pub config: bool,
    pub workspace: bool,
    pub history: bool,
    pub automation: bool,
    pub analytics: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifestCounts {
    pub projects: u64,
    pub history_items: u64,
    pub transcript_files: u64,
    pub summary_files: u64,
    pub automation_rules: u64,
    pub automation_processed_entries: u64,
    pub analytics_files: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedBackupImport {
    pub import_id: String,
    pub archive_path: String,
    pub manifest: BackupManifest,
    pub config: Value,
    pub projects: Vec<Value>,
    pub automation_rules: Vec<Value>,
    pub automation_processed_entries: Vec<Value>,
    pub analytics_content: String,
}

#[derive(Clone, Debug)]
struct HistoryBackupSnapshot {
    items: Vec<HistoryItemRecord>,
    transcript_files: Vec<(String, Value)>,
    summary_files: Vec<(String, Value)>,
}

#[derive(Clone)]
struct HistoryRepository {
    app_local_data_dir: PathBuf,
}

impl HistoryRepository {
    fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn history_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(HISTORY_DIR_NAME)
    }

    fn history_index_path(&self) -> PathBuf {
        self.history_dir().join(HISTORY_INDEX_FILE_NAME)
    }

    fn ensure_ready(&self) -> Result<(), String> {
        let history_dir = self.history_dir();
        fs::create_dir_all(&history_dir).map_err(|error| error.to_string())?;

        let index_path = self.history_index_path();
        if !index_path.exists() {
            write_json_pretty_atomic(&index_path, &Value::Array(Vec::new()))?;
        }

        Ok(())
    }

    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, String> {
        self.ensure_ready()?;
        let raw = read_json_value(&self.history_index_path())?;
        let items = raw
            .as_array()
            .ok_or_else(|| "History index must be an array.".to_string())?
            .iter()
            .map(normalize_history_item_value)
            .collect();
        Ok(items)
    }

    fn insert_item_at_front(&self, item: HistoryItemRecord) -> Result<HistoryItemRecord, String> {
        let mut items = self.list_items()?;
        items.retain(|existing| existing.id != item.id);
        items.insert(0, item.clone());
        self.write_index(&items)?;
        Ok(item)
    }

    fn write_index(&self, items: &[HistoryItemRecord]) -> Result<(), String> {
        write_json_pretty_atomic(&self.history_index_path(), items)
    }

    fn transcript_path(&self, file_name: &str) -> Result<PathBuf, String> {
        Ok(self.history_dir().join(ensure_safe_file_name(file_name, "History transcript path")?))
    }

    fn audio_path(&self, file_name: &str) -> Result<PathBuf, String> {
        Ok(self.history_dir().join(ensure_safe_file_name(file_name, "History audio path")?))
    }

    fn summary_path(&self, history_id: &str) -> Result<PathBuf, String> {
        let safe_history_id = ensure_safe_file_name(history_id, "History summary id")?;
        Ok(self
            .history_dir()
            .join(format!("{safe_history_id}{SUMMARY_FILE_SUFFIX}")))
    }

    fn create_live_draft(&self, item_value: Value) -> Result<LiveRecordingDraftResult, String> {
        self.ensure_ready()?;
        let mut item = normalize_history_item_value(&item_value);
        item.status = HistoryItemStatus::Draft;
        item.draft_source = Some(HistoryDraftSource::LiveRecord);
        write_json_pretty_atomic(
            &self.transcript_path(&item.transcript_path)?,
            &Value::Array(Vec::new()),
        )?;
        let item = self.insert_item_at_front(item)?;
        let audio_absolute_path = self
            .audio_path(&item.audio_path)?
            .to_string_lossy()
            .into_owned();
        Ok(LiveRecordingDraftResult {
            item,
            audio_absolute_path,
        })
    }

    fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Value,
        preview_text: String,
        search_content: String,
        duration: f64,
    ) -> Result<HistoryItemRecord, String> {
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut items = self.list_items()?;
        let item = items
            .iter_mut()
            .find(|entry| entry.id == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        item.preview_text = preview_text;
        item.search_content = search_content;
        item.duration = duration.max(0.0);
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;
        let updated = item.clone();
        self.write_index(&items)?;
        Ok(updated)
    }

    fn save_recording(
        &self,
        item_value: Value,
        segments: Value,
        audio_bytes: Option<Vec<u8>>,
        native_audio_path: Option<String>,
    ) -> Result<HistoryItemRecord, String> {
        self.ensure_ready()?;
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut item = normalize_history_item_value(&item_value);
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;

        match (audio_bytes, native_audio_path) {
            (Some(bytes), _) => {
                let target_path = self.audio_path(&item.audio_path)?;
                write_binary_atomic(&target_path, &bytes)?;
            }
            (None, Some(_)) => {
                // Native capture already wrote the file. We only persist transcript/index here.
            }
            (None, None) => {
                return Err("History recording save requires audio bytes or a native audio path.".to_string());
            }
        }

        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        self.insert_item_at_front(item)
    }

    fn save_imported_file(
        &self,
        item_value: Value,
        segments: Value,
        source_path: String,
    ) -> Result<HistoryItemRecord, String> {
        self.ensure_ready()?;
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut item = normalize_history_item_value(&item_value);
        item.status = HistoryItemStatus::Complete;
        item.draft_source = None;

        let source = PathBuf::from(source_path);
        if !source.is_file() {
            return Err(format!(
                "Imported source file does not exist: {}",
                source.to_string_lossy()
            ));
        }

        let target_path = self.audio_path(&item.audio_path)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(&source, &target_path).map_err(|error| error.to_string())?;
        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        self.insert_item_at_front(item)
    }

    fn delete_items(&self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let id_set = ids.iter().cloned().collect::<std::collections::HashSet<_>>();
        let items = self.list_items()?;
        let items_to_delete = items
            .iter()
            .filter(|item| id_set.contains(&item.id))
            .cloned()
            .collect::<Vec<_>>();

        for item in &items_to_delete {
            if let Some(path) = optional_history_child_path(&self.history_dir(), &item.audio_path) {
                remove_path_if_exists(&path)?;
            }
            if let Some(path) = optional_history_child_path(&self.history_dir(), &item.transcript_path) {
                remove_path_if_exists(&path)?;
            }
            if let Ok(path) = self.summary_path(&item.id) {
                remove_path_if_exists(&path)?;
            }
        }

        let next_items = items
            .into_iter()
            .filter(|item| !id_set.contains(&item.id))
            .collect::<Vec<_>>();
        self.write_index(&next_items)?;
        Ok(())
    }

    fn load_transcript(&self, file_name: &str) -> Result<Option<Value>, String> {
        let transcript_path = self.transcript_path(file_name)?;
        if !transcript_path.exists() {
            return Ok(None);
        }

        let value = read_json_value(&transcript_path)?;
        Ok(Some(ensure_json_array_value(
            value,
            "History transcript file",
        )?))
    }

    fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
        preview_text: String,
        search_content: String,
    ) -> Result<(), String> {
        let segments = ensure_json_array_value(segments, "History transcript segments")?;
        let mut items = self.list_items()?;
        let item = items
            .iter_mut()
            .find(|entry| entry.id == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        write_json_pretty_atomic(&self.transcript_path(&item.transcript_path)?, &segments)?;
        item.preview_text = preview_text;
        item.search_content = search_content;
        self.write_index(&items)?;
        Ok(())
    }

    fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), String> {
        let updates = updates
            .as_object()
            .ok_or_else(|| "History item updates must be an object.".to_string())?;
        let mut items = self.list_items()?;
        let Some(item) = items.iter_mut().find(|entry| entry.id == history_id) else {
            return Ok(());
        };

        apply_history_item_updates(item, updates);
        self.write_index(&items)?;
        Ok(())
    }

    fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let id_set = ids.iter().cloned().collect::<std::collections::HashSet<_>>();
        let mut items = self.list_items()?;
        for item in &mut items {
            if id_set.contains(&item.id) {
                item.project_id = project_id.clone();
            }
        }
        self.write_index(&items)?;
        Ok(())
    }

    fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), String> {
        let mut items = self.list_items()?;
        for item in &mut items {
            if item.project_id.as_deref() == Some(current_project_id.as_str()) {
                item.project_id = next_project_id.clone();
            }
        }
        self.write_index(&items)?;
        Ok(())
    }

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, String> {
        let summary_path = self.summary_path(history_id)?;
        if !summary_path.exists() {
            return Ok(None);
        }

        let value = read_json_value(&summary_path)?;
        Ok(Some(ensure_json_object_value(
            value,
            "History summary file",
        )?))
    }

    fn save_summary(&self, history_id: &str, summary_payload: Value) -> Result<(), String> {
        self.ensure_ready()?;
        let summary_payload = ensure_json_object_value(summary_payload, "History summary payload")?;
        write_json_pretty_atomic(&self.summary_path(history_id)?, &summary_payload)
    }

    fn delete_summary(&self, history_id: &str) -> Result<(), String> {
        let summary_path = self.summary_path(history_id)?;
        remove_path_if_exists(&summary_path)
    }

    fn resolve_audio_path(&self, file_name: &str) -> Result<Option<String>, String> {
        let Some(audio_path) = optional_history_child_path(&self.history_dir(), file_name) else {
            return Ok(None);
        };

        match fs::metadata(&audio_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                Ok(Some(audio_path.to_string_lossy().into_owned()))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, String> {
        let items = self
            .list_items()?
            .into_iter()
            .filter(|item| item.status != HistoryItemStatus::Draft)
            .map(|mut item| {
                item.status = HistoryItemStatus::Complete;
                item.draft_source = None;
                item
            })
            .collect::<Vec<_>>();

        let mut transcript_files = Vec::with_capacity(items.len());
        let mut summary_files = Vec::new();

        for item in &items {
            let transcript_path = self.transcript_path(&item.transcript_path)?;
            if !transcript_path.exists() {
                return Err(format!(
                    "History item \"{}\" is missing its transcript file.",
                    item.title
                ));
            }
            let transcript = ensure_json_array_value(
                read_json_value(&transcript_path)?,
                &format!("Transcript for history item {}", item.id),
            )?;
            transcript_files.push((item.transcript_path.clone(), transcript));

            let summary_path = self.summary_path(&item.id)?;
            if summary_path.exists() {
                let summary = ensure_json_object_value(
                    read_json_value(&summary_path)?,
                    &format!("Summary for history item {}", item.id),
                )?;
                summary_files.push((item.id.clone(), summary));
            }
        }

        Ok(HistoryBackupSnapshot {
            items,
            transcript_files,
            summary_files,
        })
    }
}

impl PreparedBackupImportState {
    fn insert(&self, import_id: String, snapshot: PreparedBackupImportSnapshot) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|error| error.to_string())?;
        guard.insert(import_id, snapshot);
        Ok(())
    }

    fn get(&self, import_id: &str) -> Result<Option<PreparedBackupImportSnapshot>, String> {
        let guard = self.inner.lock().map_err(|error| error.to_string())?;
        Ok(guard.get(import_id).cloned())
    }

    fn remove(&self, import_id: &str) -> Result<Option<PreparedBackupImportSnapshot>, String> {
        let mut guard = self.inner.lock().map_err(|error| error.to_string())?;
        Ok(guard.remove(import_id))
    }
}

fn normalize_history_item_value(value: &Value) -> HistoryItemRecord {
    let object = value.as_object();

    HistoryItemRecord {
        id: object
            .and_then(|map| map.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        timestamp: object
            .and_then(|map| map.get("timestamp"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        duration: object
            .and_then(|map| map.get("duration"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0),
        audio_path: object
            .and_then(|map| map.get("audioPath"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        transcript_path: object
            .and_then(|map| map.get("transcriptPath"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        title: object
            .and_then(|map| map.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        preview_text: object
            .and_then(|map| map.get("previewText"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        icon: object
            .and_then(|map| map.get("icon"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        kind: match object
            .and_then(|map| map.get("type"))
            .and_then(Value::as_str)
        {
            Some("batch") => HistoryItemKind::Batch,
            _ => HistoryItemKind::Recording,
        },
        search_content: object
            .and_then(|map| map.get("searchContent"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        project_id: object
            .and_then(|map| map.get("projectId"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        status: match object
            .and_then(|map| map.get("status"))
            .and_then(Value::as_str)
        {
            Some("draft") => HistoryItemStatus::Draft,
            _ => HistoryItemStatus::Complete,
        },
        draft_source: match object
            .and_then(|map| map.get("draftSource"))
            .and_then(Value::as_str)
        {
            Some("live_record") => Some(HistoryDraftSource::LiveRecord),
            _ => None,
        },
    }
}

fn apply_history_item_updates(item: &mut HistoryItemRecord, updates: &Map<String, Value>) {
    if let Some(id) = updates.get("id").and_then(Value::as_str) {
        item.id = id.to_string();
    }
    if let Some(timestamp) = updates.get("timestamp").and_then(Value::as_u64) {
        item.timestamp = timestamp;
    }
    if let Some(duration) = updates.get("duration").and_then(Value::as_f64) {
        item.duration = duration.max(0.0);
    }
    if let Some(audio_path) = updates.get("audioPath").and_then(Value::as_str) {
        item.audio_path = audio_path.to_string();
    }
    if let Some(transcript_path) = updates.get("transcriptPath").and_then(Value::as_str) {
        item.transcript_path = transcript_path.to_string();
    }
    if let Some(title) = updates.get("title").and_then(Value::as_str) {
        item.title = title.to_string();
    }
    if let Some(preview_text) = updates.get("previewText").and_then(Value::as_str) {
        item.preview_text = preview_text.to_string();
    }
    if let Some(icon) = updates.get("icon") {
        item.icon = icon.as_str().map(ToString::to_string);
    }
    if let Some(kind) = updates.get("type").and_then(Value::as_str) {
        item.kind = if kind == "batch" {
            HistoryItemKind::Batch
        } else {
            HistoryItemKind::Recording
        };
    }
    if let Some(search_content) = updates.get("searchContent").and_then(Value::as_str) {
        item.search_content = search_content.to_string();
    }
    if let Some(project_id) = updates.get("projectId") {
        item.project_id = project_id.as_str().map(ToString::to_string);
    }
    if let Some(status) = updates.get("status").and_then(Value::as_str) {
        item.status = if status == "draft" {
            HistoryItemStatus::Draft
        } else {
            HistoryItemStatus::Complete
        };
    }
    if let Some(draft_source) = updates.get("draftSource") {
        item.draft_source = match draft_source.as_str() {
            Some("live_record") => Some(HistoryDraftSource::LiveRecord),
            _ => None,
        };
    }
}

fn ensure_safe_file_name(value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
        return Err(format!("{label} contains an invalid file name."));
    }
    Ok(trimmed.to_string())
}

fn optional_history_child_path(root: &Path, file_name: &str) -> Option<PathBuf> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() || trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
        return None;
    }
    Some(root.join(trimmed))
}

fn ensure_json_array_value(value: Value, label: &str) -> Result<Value, String> {
    if value.is_array() {
        Ok(value)
    } else {
        Err(format!("{label} must be an array."))
    }
}

fn ensure_json_object_value(value: Value, label: &str) -> Result<Value, String> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(format!("{label} must be an object."))
    }
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn write_json_pretty_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_binary_atomic(path, &serialized)
}

fn write_binary_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        Uuid::new_v4()
    ));
    {
        let mut writer = BufWriter::new(File::create(&temp_path).map_err(|error| error.to_string())?);
        writer.write_all(contents).map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;
    }

    replace_path_atomically(&temp_path, path)
}

fn replace_path_atomically(temp_path: &Path, final_path: &Path) -> Result<(), String> {
    let backup_path = final_path.with_extension(format!(
        "{}.bak-{}",
        final_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp"),
        Uuid::new_v4()
    ));
    let had_existing = final_path.exists();

    if had_existing {
        fs::rename(final_path, &backup_path).map_err(|error| error.to_string())?;
    }

    match fs::rename(temp_path, final_path) {
        Ok(()) => {
            if had_existing {
                remove_path_if_exists(&backup_path)?;
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && !final_path.exists() {
                let _ = fs::rename(&backup_path, final_path);
            }
            let _ = remove_path_if_exists(temp_path);
            Err(error.to_string())
        }
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).map_err(|error| error.to_string()),
        Ok(_) => fs::remove_file(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn copy_directory_recursive(source: &Path, target: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!(
            "Source directory does not exist: {}",
            source.to_string_lossy()
        ));
    }

    fs::create_dir_all(target).map_err(|error| error.to_string())?;
    for entry in WalkDir::new(source) {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let relative = path.strip_prefix(source).map_err(|error| error.to_string())?;
        if relative.as_os_str().is_empty() {
            continue;
        }

        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(path, &destination).map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn create_temp_directory(prefix: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("sona-{prefix}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

fn create_tar_bz2_archive(source_dir: &Path, archive_path: &Path) -> Result<(), String> {
    fn append_directory_contents(
        builder: &mut tar::Builder<BzEncoder<BufWriter<File>>>,
        root: &Path,
        current: &Path,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;

            if entry.file_type().map_err(|error| error.to_string())?.is_dir() {
                builder
                    .append_dir(relative, &path)
                    .map_err(|error| error.to_string())?;
                append_directory_contents(builder, root, &path)?;
                continue;
            }

            builder
                .append_path_with_name(&path, relative)
                .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    if !source_dir.is_dir() {
        return Err(format!(
            "Source directory does not exist: {}",
            source_dir.to_string_lossy()
        ));
    }
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let file = File::create(archive_path).map_err(|error| error.to_string())?;
    let writer = BufWriter::new(file);
    let encoder = BzEncoder::new(writer, bzip2::Compression::best());
    let mut builder = tar::Builder::new(encoder);
    append_directory_contents(&mut builder, source_dir, source_dir)?;
    let encoder = builder.into_inner().map_err(|error| error.to_string())?;
    encoder.finish().map_err(|error| error.to_string())?;
    Ok(())
}

fn extract_tar_bz2_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|error| error.to_string())?;
    let buffered = BufReader::new(file);
    let tar = BzDecoder::new(buffered);
    let mut archive = tar::Archive::new(tar);
    fs::create_dir_all(target_dir).map_err(|error| error.to_string())?;
    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        entry.unpack_in(target_dir).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn build_backup_manifest(
    app_version: String,
    project_count: usize,
    history_item_count: usize,
    transcript_file_count: usize,
    summary_file_count: usize,
    automation_rule_count: usize,
    automation_processed_entry_count: usize,
) -> BackupManifest {
    BackupManifest {
        schema_version: BACKUP_SCHEMA_VERSION,
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        app_version,
        history_mode: BACKUP_HISTORY_MODE.to_string(),
        scopes: BackupManifestScopes {
            config: true,
            workspace: true,
            history: true,
            automation: true,
            analytics: true,
        },
        counts: BackupManifestCounts {
            projects: project_count as u64,
            history_items: history_item_count as u64,
            transcript_files: transcript_file_count as u64,
            summary_files: summary_file_count as u64,
            automation_rules: automation_rule_count as u64,
            automation_processed_entries: automation_processed_entry_count as u64,
            analytics_files: 1,
        },
    }
}

fn validate_backup_manifest(value: &Value) -> Result<BackupManifest, String> {
    let manifest: BackupManifest = serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    if manifest.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported backup schema version: {}",
            manifest.schema_version
        ));
    }
    if manifest.history_mode != BACKUP_HISTORY_MODE {
        return Err(format!(
            "Unsupported backup history mode: {}",
            manifest.history_mode
        ));
    }
    if !manifest.scopes.config
        || !manifest.scopes.workspace
        || !manifest.scopes.history
        || !manifest.scopes.automation
        || !manifest.scopes.analytics
    {
        return Err("Backup manifest is missing one or more required scopes.".to_string());
    }
    Ok(manifest)
}

fn export_backup_archive_inner(
    app_local_data_dir: &Path,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    let config = ensure_json_object_value(request.config, "Backup config")?;
    let analytics_json: Value =
        serde_json::from_str(&request.analytics_content).map_err(|error| error.to_string())?;
    ensure_json_object_value(analytics_json, "Backup analytics")?;

    let repository = HistoryRepository::new(app_local_data_dir.to_path_buf());
    let history = repository.history_snapshot_for_backup()?;
    let manifest = build_backup_manifest(
        request.app_version,
        request.projects.len(),
        history.items.len(),
        history.transcript_files.len(),
        history.summary_files.len(),
        request.automation_rules.len(),
        request.automation_processed_entries.len(),
    );

    let staging_dir = create_temp_directory("backup-export")?;
    let result = (|| -> Result<(), String> {
        let config_dir = staging_dir.join(CONFIG_DIR_NAME);
        let projects_dir = staging_dir.join(PROJECTS_DIR_NAME);
        let history_dir = staging_dir.join(HISTORY_DIR_NAME);
        let automation_dir = staging_dir.join(AUTOMATION_DIR_NAME);
        let analytics_dir = staging_dir.join(ANALYTICS_DIR_NAME);

        for dir in [&config_dir, &projects_dir, &history_dir, &automation_dir, &analytics_dir] {
            fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }

        write_json_pretty_atomic(&staging_dir.join("manifest.json"), &manifest)?;
        write_json_pretty_atomic(&config_dir.join(CONFIG_FILE_NAME), &config)?;
        write_json_pretty_atomic(
            &projects_dir.join(PROJECTS_INDEX_FILE_NAME),
            &request.projects,
        )?;
        write_json_pretty_atomic(
            &history_dir.join(PROJECTS_INDEX_FILE_NAME),
            &history.items,
        )?;
        write_json_pretty_atomic(
            &automation_dir.join(AUTOMATION_RULES_FILE_NAME),
            &request.automation_rules,
        )?;
        write_json_pretty_atomic(
            &automation_dir.join(AUTOMATION_PROCESSED_FILE_NAME),
            &request.automation_processed_entries,
        )?;
        fs::write(
            analytics_dir.join(ANALYTICS_USAGE_FILE_NAME),
            request.analytics_content,
        )
        .map_err(|error| error.to_string())?;

        for (file_name, transcript) in history.transcript_files {
            write_json_pretty_atomic(&history_dir.join(file_name), &transcript)?;
        }
        for (history_id, summary) in history.summary_files {
            write_json_pretty_atomic(
                &history_dir.join(format!("{history_id}{SUMMARY_FILE_SUFFIX}")),
                &summary,
            )?;
        }

        create_tar_bz2_archive(&staging_dir, Path::new(&request.archive_path))?;
        Ok(())
    })();

    let cleanup_error = remove_path_if_exists(&staging_dir).err();
    match (result, cleanup_error) {
        (Ok(()), None) => Ok(manifest),
        (Ok(()), Some(error)) => Err(error),
        (Err(error), _) => Err(error),
    }
}

fn prepare_backup_import_inner(
    archive_path: &Path,
) -> Result<(PreparedBackupImport, PreparedBackupImportSnapshot), String> {
    let extraction_dir = create_temp_directory("backup-import")?;
    let result = (|| -> Result<(PreparedBackupImport, PreparedBackupImportSnapshot), String> {
        extract_tar_bz2_archive(archive_path, &extraction_dir)?;

        let manifest = validate_backup_manifest(&read_json_value(&extraction_dir.join("manifest.json"))?)?;
        let config = ensure_json_object_value(
            read_json_value(&extraction_dir.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME))?,
            "Backup config",
        )?;
        let projects = read_json_value(
            &extraction_dir
                .join(PROJECTS_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
        )?
        .as_array()
        .cloned()
        .ok_or_else(|| "Backup projects must be an array.".to_string())?;

        let history_items_value = read_json_value(
            &extraction_dir.join(HISTORY_DIR_NAME).join(PROJECTS_INDEX_FILE_NAME),
        )?;
        let history_items = history_items_value
            .as_array()
            .ok_or_else(|| "Backup history index must be an array.".to_string())?
            .iter()
            .map(normalize_history_item_value)
            .collect::<Vec<_>>();

        let mut transcript_count = 0usize;
        let mut summary_count = 0usize;
        for item in &history_items {
            if item.status == HistoryItemStatus::Draft {
                return Err(format!(
                    "Backup history item \"{}\" is a draft and cannot be imported.",
                    item.id
                ));
            }

            let transcript_file_name =
                ensure_safe_file_name(&item.transcript_path, &format!("History transcript path for {}", item.id))?;
            let transcript_path = extraction_dir.join(HISTORY_DIR_NAME).join(transcript_file_name);
            ensure_json_array_value(
                read_json_value(&transcript_path)?,
                &format!("Transcript for history item {}", item.id),
            )?;
            transcript_count += 1;

            let summary_path = extraction_dir
                .join(HISTORY_DIR_NAME)
                .join(format!("{}{}", item.id, SUMMARY_FILE_SUFFIX));
            if summary_path.exists() {
                ensure_json_object_value(
                    read_json_value(&summary_path)?,
                    &format!("Summary for history item {}", item.id),
                )?;
                summary_count += 1;
            }
        }

        let automation_rules = read_json_value(
            &extraction_dir
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_RULES_FILE_NAME),
        )?
        .as_array()
        .cloned()
        .ok_or_else(|| "Backup automation rules must be an array.".to_string())?;
        let automation_processed_entries = read_json_value(
            &extraction_dir
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_PROCESSED_FILE_NAME),
        )?
        .as_array()
        .cloned()
        .ok_or_else(|| "Backup automation processed entries must be an array.".to_string())?;

        let analytics_content = fs::read_to_string(
            extraction_dir
                .join(ANALYTICS_DIR_NAME)
                .join(ANALYTICS_USAGE_FILE_NAME),
        )
        .map_err(|error| error.to_string())?;
        let analytics_json: Value =
            serde_json::from_str(&analytics_content).map_err(|error| error.to_string())?;
        ensure_json_object_value(analytics_json, "Backup analytics")?;

        if manifest.counts.projects != projects.len() as u64 {
            return Err("Backup project count does not match the manifest.".to_string());
        }
        if manifest.counts.history_items != history_items.len() as u64 {
            return Err("Backup history count does not match the manifest.".to_string());
        }
        if manifest.counts.transcript_files != transcript_count as u64 {
            return Err("Backup transcript count does not match the manifest.".to_string());
        }
        if manifest.counts.summary_files != summary_count as u64 {
            return Err("Backup summary count does not match the manifest.".to_string());
        }
        if manifest.counts.automation_rules != automation_rules.len() as u64 {
            return Err("Backup automation-rule count does not match the manifest.".to_string());
        }
        if manifest.counts.automation_processed_entries
            != automation_processed_entries.len() as u64
        {
            return Err("Backup processed-entry count does not match the manifest.".to_string());
        }
        if manifest.counts.analytics_files != 1 {
            return Err("Backup analytics count does not match the manifest.".to_string());
        }

        let import_id = Uuid::new_v4().to_string();
        let response = PreparedBackupImport {
            import_id: import_id.clone(),
            archive_path: archive_path.to_string_lossy().into_owned(),
            manifest,
            config,
            projects,
            automation_rules,
            automation_processed_entries,
            analytics_content,
        };
        let snapshot = PreparedBackupImportSnapshot {
            archive_path: response.archive_path.clone(),
            extraction_dir: extraction_dir.clone(),
        };

        Ok((response, snapshot))
    })();

    if result.is_err() {
        let _ = remove_path_if_exists(&extraction_dir);
    }

    result
}

fn apply_prepared_history_import_inner(
    app_local_data_dir: &Path,
    import_id: &str,
    extraction_dir: &Path,
) -> Result<(), String> {
    let extracted_history_dir = extraction_dir.join(HISTORY_DIR_NAME);
    if !extracted_history_dir.is_dir() {
        return Err("Prepared backup import is missing the history directory.".to_string());
    }

    let repository = HistoryRepository::new(app_local_data_dir.to_path_buf());
    repository.ensure_ready()?;

    let target_history_dir = repository.history_dir();
    let staged_history_dir = app_local_data_dir.join(format!("history.importing-{import_id}"));
    let previous_history_dir = app_local_data_dir.join(format!("history.previous-{import_id}"));

    remove_path_if_exists(&staged_history_dir)?;
    remove_path_if_exists(&previous_history_dir)?;

    copy_directory_recursive(&extracted_history_dir, &staged_history_dir)?;

    let had_existing = target_history_dir.exists();
    if had_existing {
        fs::rename(&target_history_dir, &previous_history_dir).map_err(|error| error.to_string())?;
    }

    match fs::rename(&staged_history_dir, &target_history_dir) {
        Ok(()) => {
            if had_existing {
                remove_path_if_exists(&previous_history_dir)?;
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && !target_history_dir.exists() {
                let _ = fs::rename(&previous_history_dir, &target_history_dir);
            }
            let _ = remove_path_if_exists(&staged_history_dir);
            Err(error.to_string())
        }
    }
}

fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path().app_local_data_dir().map_err(|error| error.to_string())
}

async fn run_history_task<R, T, F>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(HistoryRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    let lock = state.lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        task(HistoryRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn history_list_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
) -> Result<Vec<HistoryItemRecord>, String> {
    run_history_task(app, state, |repository| repository.list_items()).await
}

#[tauri::command]
pub async fn history_create_live_draft<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    item: Value,
) -> Result<LiveRecordingDraftResult, String> {
    run_history_task(app, state, move |repository| repository.create_live_draft(item)).await
}

#[tauri::command]
pub async fn history_complete_live_draft<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    segments: Value,
    preview_text: String,
    search_content: String,
    duration: f64,
) -> Result<HistoryItemRecord, String> {
    run_history_task(app, state, move |repository| {
        repository.complete_live_draft(
            &history_id,
            segments,
            preview_text,
            search_content,
            duration,
        )
    })
    .await
}

#[tauri::command]
pub async fn history_save_recording<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    item: Value,
    segments: Value,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
) -> Result<HistoryItemRecord, String> {
    run_history_task(app, state, move |repository| {
        repository.save_recording(item, segments, audio_bytes, native_audio_path)
    })
    .await
}

#[tauri::command]
pub async fn history_save_imported_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    item: Value,
    segments: Value,
    source_path: String,
) -> Result<HistoryItemRecord, String> {
    run_history_task(app, state, move |repository| {
        repository.save_imported_file(item, segments, source_path)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| repository.delete_items(&ids)).await
}

#[tauri::command]
pub async fn history_load_transcript<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    filename: String,
) -> Result<Option<Value>, String> {
    run_history_task(app, state, move |repository| repository.load_transcript(&filename)).await
}

#[tauri::command]
pub async fn history_update_transcript<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    segments: Value,
    preview_text: String,
    search_content: String,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.update_transcript(&history_id, segments, preview_text, search_content)
    })
    .await
}

#[tauri::command]
pub async fn history_update_item_meta<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    updates: Value,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.update_item_meta(&history_id, updates)
    })
    .await
}

#[tauri::command]
pub async fn history_update_project_assignments<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
    project_id: Option<String>,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.update_project_assignments(&ids, project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_reassign_project<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    current_project_id: String,
    next_project_id: Option<String>,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.reassign_project(current_project_id, next_project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_summary<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
) -> Result<Option<Value>, String> {
    run_history_task(app, state, move |repository| repository.load_summary(&history_id)).await
}

#[tauri::command]
pub async fn history_save_summary<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
    summary_payload: Value,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| {
        repository.save_summary(&history_id, summary_payload)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_summary<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
) -> Result<(), String> {
    run_history_task(app, state, move |repository| repository.delete_summary(&history_id)).await
}

#[tauri::command]
pub async fn history_resolve_audio_path<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    filename: String,
) -> Result<Option<String>, String> {
    run_history_task(app, state, move |repository| repository.resolve_audio_path(&filename)).await
}

#[tauri::command]
pub async fn history_open_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
) -> Result<(), String> {
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    {
        let _guard = state.lock.lock().map_err(|error| error.to_string())?;
        HistoryRepository::new(app_local_data_dir.clone()).ensure_ready()?;
    }

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(app_local_data_dir.join(HISTORY_DIR_NAME).to_string_lossy(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn export_backup_archive<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    let lock = history_state.lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        export_backup_archive_inner(&app_local_data_dir, request)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn prepare_backup_import(
    state: State<'_, PreparedBackupImportState>,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    let archive_path_buf = PathBuf::from(&archive_path);
    let (prepared, snapshot) = tauri::async_runtime::spawn_blocking(move || {
        prepare_backup_import_inner(&archive_path_buf)
    })
    .await
    .map_err(|error| error.to_string())??;

    state.insert(prepared.import_id.clone(), snapshot)?;
    Ok(prepared)
}

#[tauri::command]
pub async fn apply_prepared_history_import<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    prepared_state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = prepared_state.get(&import_id)? else {
        return Err(format!("Prepared backup import not found: {import_id}"));
    };

    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    let lock = history_state.lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        apply_prepared_history_import_inner(
            &app_local_data_dir,
            &import_id,
            &snapshot.extraction_dir,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn dispose_prepared_backup_import(
    state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = state.remove(&import_id)? else {
        return Ok(());
    };

    tauri::async_runtime::spawn_blocking(move || {
        let _archive_path = snapshot.archive_path;
        remove_path_if_exists(&snapshot.extraction_dir)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn sample_history_item(id: &str, status: HistoryItemStatus) -> HistoryItemRecord {
        HistoryItemRecord {
            id: id.to_string(),
            timestamp: 1,
            duration: 2.0,
            audio_path: format!("{id}.wav"),
            transcript_path: format!("{id}.json"),
            title: format!("Item {id}"),
            preview_text: String::new(),
            icon: None,
            kind: HistoryItemKind::Recording,
            search_content: String::new(),
            project_id: None,
            status,
            draft_source: if status == HistoryItemStatus::Draft {
                Some(HistoryDraftSource::LiveRecord)
            } else {
                None
            },
        }
    }

    fn create_valid_backup_archive(archive_path: &Path) -> PathBuf {
        let staging_dir = tempdir().unwrap();
        let history_dir = staging_dir.path().join(HISTORY_DIR_NAME);
        let config_dir = staging_dir.path().join(CONFIG_DIR_NAME);
        let projects_dir = staging_dir.path().join(PROJECTS_DIR_NAME);
        let automation_dir = staging_dir.path().join(AUTOMATION_DIR_NAME);
        let analytics_dir = staging_dir.path().join(ANALYTICS_DIR_NAME);

        fs::create_dir_all(&history_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&projects_dir).unwrap();
        fs::create_dir_all(&automation_dir).unwrap();
        fs::create_dir_all(&analytics_dir).unwrap();

        let item = sample_history_item("history-1", HistoryItemStatus::Complete);
        write_json_pretty_atomic(&history_dir.join(PROJECTS_INDEX_FILE_NAME), &vec![item]).unwrap();
        write_json_pretty_atomic(
            &history_dir.join("history-1.json"),
            &json!([{ "id": "seg-1", "text": "hello", "start": 0, "end": 1, "isFinal": true }]),
        )
        .unwrap();
        write_json_pretty_atomic(
            &history_dir.join("history-1.summary.json"),
            &json!({ "activeTemplateId": "general" }),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir.path().join("manifest.json"),
            &build_backup_manifest("0.6.4".to_string(), 1, 1, 1, 1, 1, 1),
        )
        .unwrap();
        write_json_pretty_atomic(
            &config_dir.join(CONFIG_FILE_NAME),
            &json!({ "theme": "auto" }),
        )
        .unwrap();
        write_json_pretty_atomic(
            &projects_dir.join(PROJECTS_INDEX_FILE_NAME),
            &vec![json!({ "id": "project-1", "name": "Workspace", "defaults": {} })],
        )
        .unwrap();
        write_json_pretty_atomic(
            &automation_dir.join(AUTOMATION_RULES_FILE_NAME),
            &vec![json!({ "id": "rule-1" })],
        )
        .unwrap();
        write_json_pretty_atomic(
            &automation_dir.join(AUTOMATION_PROCESSED_FILE_NAME),
            &vec![json!({ "ruleId": "rule-1", "filePath": "C:\\watch\\file.wav" })],
        )
        .unwrap();
        fs::write(
            analytics_dir.join(ANALYTICS_USAGE_FILE_NAME),
            r#"{"schemaVersion":1}"#,
        )
        .unwrap();

        create_tar_bz2_archive(staging_dir.path(), archive_path).unwrap();
        archive_path.to_path_buf()
    }

    #[test]
    fn history_delete_items_removes_files_and_updates_index() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let keep_item = sample_history_item("keep", HistoryItemStatus::Complete);
        let delete_item = sample_history_item("delete", HistoryItemStatus::Complete);
        repository
            .write_index(&vec![keep_item.clone(), delete_item.clone()])
            .unwrap();
        fs::write(repository.audio_path(&delete_item.audio_path).unwrap(), b"audio").unwrap();
        write_json_pretty_atomic(
            &repository.transcript_path(&delete_item.transcript_path).unwrap(),
            &json!([]),
        )
        .unwrap();
        write_json_pretty_atomic(&repository.summary_path(&delete_item.id).unwrap(), &json!({}))
            .unwrap();

        repository.delete_items(&vec![delete_item.id.clone()]).unwrap();

        let items = repository.list_items().unwrap();
        assert_eq!(items, vec![keep_item]);
        assert!(!repository
            .audio_path(&delete_item.audio_path)
            .unwrap()
            .exists());
        assert!(!repository
            .transcript_path(&delete_item.transcript_path)
            .unwrap()
            .exists());
        assert!(!repository.summary_path(&delete_item.id).unwrap().exists());
    }

    #[test]
    fn live_draft_round_trip_updates_index_and_transcript() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());

        let draft = repository
            .create_live_draft(json!({
                "id": "draft-1",
                "timestamp": 1,
                "duration": 0,
                "audioPath": "draft-1.webm",
                "transcriptPath": "draft-1.json",
                "title": "Draft",
                "previewText": "",
                "type": "recording",
                "searchContent": "",
                "projectId": null,
                "status": "draft",
                "draftSource": "live_record"
            }))
            .unwrap();
        assert_eq!(draft.item.status, HistoryItemStatus::Draft);
        assert!(PathBuf::from(&draft.audio_absolute_path).ends_with("draft-1.webm"));

        let completed = repository
            .complete_live_draft(
                "draft-1",
                json!([{ "id": "seg-1", "text": "hello" }]),
                "hello".to_string(),
                "hello".to_string(),
                3.0,
            )
            .unwrap();

        assert_eq!(completed.status, HistoryItemStatus::Complete);
        assert_eq!(completed.preview_text, "hello");
        let transcript = repository.load_transcript("draft-1.json").unwrap().unwrap();
        assert_eq!(transcript.as_array().unwrap().len(), 1);
    }

    #[test]
    fn summary_round_trip_saves_loads_and_deletes() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        repository
            .save_summary("history-1", json!({ "activeTemplateId": "general" }))
            .unwrap();
        assert_eq!(
            repository.load_summary("history-1").unwrap(),
            Some(json!({ "activeTemplateId": "general" }))
        );

        repository.delete_summary("history-1").unwrap();
        assert_eq!(repository.load_summary("history-1").unwrap(), None);
    }

    #[test]
    fn export_backup_archive_skips_draft_items_and_preserves_manifest() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let keep_item = sample_history_item("keep", HistoryItemStatus::Complete);
        let draft_item = sample_history_item("draft", HistoryItemStatus::Draft);
        repository
            .write_index(&vec![keep_item.clone(), draft_item])
            .unwrap();
        write_json_pretty_atomic(
            &repository.transcript_path(&keep_item.transcript_path).unwrap(),
            &json!([{ "id": "seg-1" }]),
        )
        .unwrap();
        write_json_pretty_atomic(
            &repository.summary_path(&keep_item.id).unwrap(),
            &json!({ "activeTemplateId": "general" }),
        )
        .unwrap();

        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("backup.tar.bz2");
        let manifest = export_backup_archive_inner(
            root.path(),
            ExportBackupArchiveRequest {
                archive_path: archive_path.to_string_lossy().into_owned(),
                app_version: "0.6.4".to_string(),
                config: json!({ "theme": "auto" }),
                projects: vec![json!({ "id": "project-1" })],
                automation_rules: vec![json!({ "id": "rule-1" })],
                automation_processed_entries: vec![json!({ "ruleId": "rule-1" })],
                analytics_content: r#"{"schemaVersion":1}"#.to_string(),
            },
        )
        .unwrap();

        assert_eq!(manifest.history_mode, "light");
        assert_eq!(manifest.counts.history_items, 1);
        assert_eq!(manifest.counts.transcript_files, 1);
        assert_eq!(manifest.counts.summary_files, 1);

        let extract_dir = tempdir().unwrap();
        extract_tar_bz2_archive(&archive_path, extract_dir.path()).unwrap();
        let exported_items = read_json_value(
            &extract_dir.path().join(HISTORY_DIR_NAME).join(PROJECTS_INDEX_FILE_NAME),
        )
        .unwrap();
        assert_eq!(exported_items.as_array().unwrap().len(), 1);
        assert_eq!(
            exported_items.as_array().unwrap()[0]["id"].as_str().unwrap(),
            "keep"
        );
    }

    #[test]
    fn prepare_backup_import_rejects_missing_transcript_before_mutation() {
        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("invalid-backup.tar.bz2");
        let staging_dir = tempdir().unwrap();
        fs::create_dir_all(staging_dir.path().join(HISTORY_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(CONFIG_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(PROJECTS_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(AUTOMATION_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(ANALYTICS_DIR_NAME)).unwrap();
        write_json_pretty_atomic(
            &staging_dir.path().join("manifest.json"),
            &build_backup_manifest("0.6.4".to_string(), 0, 1, 1, 0, 0, 0),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir.path().join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME),
            &json!({}),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(PROJECTS_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
            &Vec::<Value>::new(),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(HISTORY_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
            &vec![json!({
                "id": "history-1",
                "audioPath": "history-1.webm",
                "transcriptPath": "history-1.json",
                "title": "Broken",
                "projectId": null,
                "status": "complete"
            })],
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_RULES_FILE_NAME),
            &Vec::<Value>::new(),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_PROCESSED_FILE_NAME),
            &Vec::<Value>::new(),
        )
        .unwrap();
        fs::write(
            staging_dir
                .path()
                .join(ANALYTICS_DIR_NAME)
                .join(ANALYTICS_USAGE_FILE_NAME),
            r#"{"schemaVersion":1}"#,
        )
        .unwrap();
        create_tar_bz2_archive(staging_dir.path(), &archive_path).unwrap();

        let err = prepare_backup_import_inner(&archive_path).unwrap_err();
        assert!(
            err.contains("os error 2")
                || err.contains("No such file")
                || err.contains("找不到指定的文件")
        );
    }

    #[test]
    fn apply_prepared_history_import_replaces_history_and_dispose_cleans_snapshot() {
        let app_data_dir = tempdir().unwrap();
        let history_dir = app_data_dir.path().join(HISTORY_DIR_NAME);
        fs::create_dir_all(&history_dir).unwrap();
        write_json_pretty_atomic(
            &history_dir.join(PROJECTS_INDEX_FILE_NAME),
            &vec![json!({ "id": "old-history" })],
        )
        .unwrap();

        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("valid-backup.tar.bz2");
        create_valid_backup_archive(&archive_path);

        let (prepared, snapshot) = prepare_backup_import_inner(&archive_path).unwrap();
        let state = PreparedBackupImportState::default();
        state.insert(prepared.import_id.clone(), snapshot.clone()).unwrap();

        apply_prepared_history_import_inner(
            app_data_dir.path(),
            &prepared.import_id,
            &snapshot.extraction_dir,
        )
        .unwrap();

        let replaced_items = read_json_value(&history_dir.join(PROJECTS_INDEX_FILE_NAME)).unwrap();
        assert_eq!(
            replaced_items.as_array().unwrap()[0]["id"].as_str().unwrap(),
            "history-1"
        );

        let removed = state.remove(&prepared.import_id).unwrap().unwrap();
        remove_path_if_exists(&removed.extraction_dir).unwrap();
        assert!(!removed.extraction_dir.exists());
    }
}
