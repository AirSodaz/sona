use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

const RECOVERY_VERSION: u32 = 1;
const RECOVERY_DIR_NAME: &str = "recovery";
const QUEUE_RECOVERY_FILE_NAME: &str = "queue-recovery.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySnapshot {
    pub version: u32,
    pub updated_at: Option<u64>,
    pub items: Vec<RecoveredQueueItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredQueueItem {
    pub id: String,
    pub filename: String,
    pub file_path: String,
    pub source: String,
    pub resolution: String,
    pub progress: f64,
    pub segments: Vec<RecoveredTranscriptSegment>,
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_title: Option<String>,
    pub last_known_stage: String,
    pub updated_at: u64,
    pub has_source_file: bool,
    pub can_resume: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation_rule_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_config_snapshot: Option<Value>,
    pub export_config: Value,
    pub stage_config: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_stat: Option<RecoveryFileStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_file_name_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryFileStat {
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptSegment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
    #[serde(default = "default_true")]
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<RecoveredTranscriptTiming>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durations: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_attribution: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptTiming {
    pub level: String,
    pub source: String,
    pub units: Vec<RecoveredTranscriptTimingUnit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredTranscriptTimingUnit {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
}

#[derive(Clone, Debug)]
pub struct RecoveryRepository {
    app_local_data_dir: PathBuf,
}

enum SourcePathStatus {
    File,
    Directory,
    Missing,
    Unknown,
}

impl RecoveryRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn recovery_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(RECOVERY_DIR_NAME)
    }

    fn queue_recovery_path(&self) -> PathBuf {
        self.recovery_dir().join(QUEUE_RECOVERY_FILE_NAME)
    }

    pub fn ensure_ready(&self) -> Result<(), String> {
        fs::create_dir_all(self.recovery_dir()).map_err(|error| error.to_string())?;
        let recovery_path = self.queue_recovery_path();
        if !recovery_path.exists() {
            write_json_pretty_atomic(&recovery_path, &empty_snapshot())?;
        }
        Ok(())
    }

    pub fn load_snapshot(&self) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let content =
            fs::read_to_string(self.queue_recovery_path()).map_err(|error| error.to_string())?;
        let value = match serde_json::from_str::<Value>(&content) {
            Ok(value) => value,
            Err(error) => {
                log::error!("[Recovery] Failed to parse recovery snapshot: {}", error);
                return Ok(empty_snapshot());
            }
        };

        Ok(snapshot_from_value(value, false))
    }

    pub fn save_snapshot(&self, items: Vec<Value>) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let now = now_ms();
        let normalized_items = items
            .into_iter()
            .filter_map(|item| normalize_recovered_item(&item, now))
            .filter(|item| item.resolution == "pending")
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_items(normalized_items);
        write_json_pretty_atomic(&self.queue_recovery_path(), &snapshot)?;
        Ok(snapshot)
    }

    pub fn persist_queue_snapshot(
        &self,
        queue_items: Vec<Value>,
    ) -> Result<RecoverySnapshot, String> {
        self.ensure_ready()?;
        let now = now_ms();
        let items = queue_items
            .into_iter()
            .filter_map(|item| recovered_item_from_queue_item(&item, now))
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_items(items);
        write_json_pretty_atomic(&self.queue_recovery_path(), &snapshot)?;
        Ok(snapshot)
    }
}

fn default_true() -> bool {
    true
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn empty_snapshot() -> RecoverySnapshot {
    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: None,
        items: Vec::new(),
    }
}

fn snapshot_from_items(items: Vec<RecoveredQueueItem>) -> RecoverySnapshot {
    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at: (!items.is_empty()).then(now_ms),
        items,
    }
}

fn snapshot_from_value(value: Value, include_only_pending: bool) -> RecoverySnapshot {
    let now = now_ms();
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| normalize_recovered_item(item, now))
                .filter(|item| !include_only_pending || item.resolution == "pending")
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let updated_at = number_field(&value, "updatedAt")
        .map(|value| value.max(0.0) as u64)
        .filter(|_| !items.is_empty());

    RecoverySnapshot {
        version: RECOVERY_VERSION,
        updated_at,
        items,
    }
}

fn recovered_item_from_queue_item(value: &Value, now: u64) -> Option<RecoveredQueueItem> {
    let status = string_field(value, "status")?;
    if status != "pending" && status != "processing" {
        return None;
    }

    let queue_id = non_empty_string_field(value, "id")?;
    let id = non_empty_string_field(value, "recoveryId").unwrap_or(queue_id);
    let file_path = string_field(value, "filePath").unwrap_or_default();
    let automation_rule_id = string_field(value, "automationRuleId");
    let source = if string_field(value, "origin").as_deref() == Some("automation")
        || automation_rule_id.is_some()
    {
        "automation"
    } else {
        "batch_import"
    }
    .to_string();
    let resolution = "pending".to_string();
    let (has_source_file, can_resume) = resolve_source_flags(&file_path, &resolution, true, true);

    Some(RecoveredQueueItem {
        id,
        filename: string_field(value, "filename").unwrap_or_else(|| filename_from_path(&file_path)),
        file_path,
        source,
        resolution,
        progress: number_field(value, "progress").unwrap_or(0.0),
        segments: segments_from_value(value.get("segments")),
        project_id: optional_string_field(value, "projectId"),
        history_id: string_field(value, "historyId"),
        history_title: string_field(value, "historyTitle"),
        last_known_stage: normalize_stage(string_field(value, "lastKnownStage").as_deref()),
        updated_at: now,
        has_source_file,
        can_resume,
        automation_rule_id,
        automation_rule_name: string_field(value, "automationRuleName"),
        resolved_config_snapshot: optional_value_field(value, "resolvedConfigSnapshot"),
        export_config: value_field_or_null(value, "exportConfig"),
        stage_config: value_field_or_null(value, "stageConfig"),
        source_fingerprint: string_field(value, "sourceFingerprint"),
        file_stat: file_stat_field(value, "fileStat"),
        export_file_name_prefix: optional_string_field(value, "exportFileNamePrefix"),
    })
}

fn normalize_recovered_item(value: &Value, now: u64) -> Option<RecoveredQueueItem> {
    let id = non_empty_string_field(value, "id")?;
    let file_path = string_field(value, "filePath").unwrap_or_default();
    let automation_rule_id = string_field(value, "automationRuleId");
    let resolution = normalize_resolution(string_field(value, "resolution").as_deref());
    let saved_has_source_file = bool_field(value, "hasSourceFile").unwrap_or(true);
    let saved_can_resume =
        bool_field(value, "canResume").unwrap_or(saved_has_source_file && resolution == "pending");
    let (has_source_file, can_resume) = resolve_source_flags(
        &file_path,
        &resolution,
        saved_has_source_file,
        saved_can_resume,
    );

    Some(RecoveredQueueItem {
        id,
        filename: string_field(value, "filename").unwrap_or_else(|| filename_from_path(&file_path)),
        file_path,
        source: normalize_source(
            string_field(value, "source").as_deref(),
            automation_rule_id.as_ref(),
        ),
        resolution,
        progress: number_field(value, "progress").unwrap_or(0.0),
        segments: segments_from_value(value.get("segments")),
        project_id: optional_string_field(value, "projectId"),
        history_id: string_field(value, "historyId"),
        history_title: string_field(value, "historyTitle"),
        last_known_stage: normalize_stage(string_field(value, "lastKnownStage").as_deref()),
        updated_at: number_field(value, "updatedAt")
            .map(|value| value.max(0.0) as u64)
            .unwrap_or(now),
        has_source_file,
        can_resume,
        automation_rule_id,
        automation_rule_name: string_field(value, "automationRuleName"),
        resolved_config_snapshot: optional_value_field(value, "resolvedConfigSnapshot"),
        export_config: value_field_or_null(value, "exportConfig"),
        stage_config: value_field_or_null(value, "stageConfig"),
        source_fingerprint: string_field(value, "sourceFingerprint"),
        file_stat: file_stat_field(value, "fileStat"),
        export_file_name_prefix: optional_string_field(value, "exportFileNamePrefix"),
    })
}

fn segments_from_value(value: Option<&Value>) -> Vec<RecoveredTranscriptSegment> {
    value
        .and_then(Value::as_array)
        .map(|segments| {
            segments
                .iter()
                .filter_map(|segment| {
                    serde_json::from_value::<RecoveredTranscriptSegment>(segment.clone())
                        .ok()
                        .map(normalize_transcript_segment)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_transcript_segment(
    mut segment: RecoveredTranscriptSegment,
) -> RecoveredTranscriptSegment {
    segment.start = segment.start.max(0.0);
    segment.end = segment.end.max(segment.start);
    segment.timing = normalize_existing_timing(&segment)
        .or_else(|| build_token_timing_from_legacy(&segment))
        .or_else(|| Some(build_segment_timing(&segment, "derived")));
    segment
}

fn normalize_existing_timing(
    segment: &RecoveredTranscriptSegment,
) -> Option<RecoveredTranscriptTiming> {
    let timing = segment.timing.as_ref()?;
    if !is_valid_timing_source(&timing.source) {
        return None;
    }

    if timing.level == "segment" {
        return Some(build_segment_timing(segment, &timing.source));
    }

    if timing.level != "token" {
        return None;
    }

    let units = normalize_timing_units(timing.units.clone(), segment.start, segment.end);
    (!units.is_empty()).then(|| RecoveredTranscriptTiming {
        level: "token".to_string(),
        source: timing.source.clone(),
        units,
    })
}

fn build_token_timing_from_legacy(
    segment: &RecoveredTranscriptSegment,
) -> Option<RecoveredTranscriptTiming> {
    let tokens = segment.tokens.as_ref()?;
    let timestamps = segment.timestamps.as_ref()?;
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let durations = segment
        .durations
        .as_ref()
        .filter(|values| values.len() == tokens.len());
    let units = tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            let start = timestamps[index];
            let explicit_end = durations
                .and_then(|values| values.get(index))
                .map(|duration| start + (*duration).max(0.0));
            let next_start = timestamps.get(index + 1).copied();
            let end = explicit_end
                .or(next_start)
                .unwrap_or(segment.end)
                .max(start);
            RecoveredTranscriptTimingUnit {
                text: token.clone(),
                start,
                end,
            }
        })
        .collect::<Vec<_>>();
    let units = normalize_timing_units(units, segment.start, segment.end);

    (!units.is_empty()).then(|| RecoveredTranscriptTiming {
        level: "token".to_string(),
        source: "model".to_string(),
        units,
    })
}

fn build_segment_timing(
    segment: &RecoveredTranscriptSegment,
    source: &str,
) -> RecoveredTranscriptTiming {
    RecoveredTranscriptTiming {
        level: "segment".to_string(),
        source: source.to_string(),
        units: vec![RecoveredTranscriptTimingUnit {
            text: segment.text.clone(),
            start: segment.start,
            end: segment.end,
        }],
    }
}

fn normalize_timing_units(
    units: Vec<RecoveredTranscriptTimingUnit>,
    start: f64,
    end: f64,
) -> Vec<RecoveredTranscriptTimingUnit> {
    let safe_start = start.max(0.0);
    let safe_end = end.max(safe_start);
    let unit_count = units.len();

    units
        .into_iter()
        .enumerate()
        .filter_map(|(index, unit)| {
            if unit.text.is_empty() {
                return None;
            }

            let unit_start = unit.start.max(safe_start).min(safe_end);
            let fallback_end = if index + 1 == unit_count {
                safe_end
            } else {
                unit_start
            };
            let unit_end = unit.end.max(fallback_end).min(safe_end).max(unit_start);

            Some(RecoveredTranscriptTimingUnit {
                text: unit.text,
                start: unit_start,
                end: unit_end,
            })
        })
        .collect()
}

fn is_valid_timing_source(source: &str) -> bool {
    source == "model" || source == "derived"
}

fn normalize_source(raw_source: Option<&str>, automation_rule_id: Option<&String>) -> String {
    match raw_source {
        Some("automation") => "automation".to_string(),
        Some("batch_import") => "batch_import".to_string(),
        _ if automation_rule_id.is_some() => "automation".to_string(),
        _ => "batch_import".to_string(),
    }
}

fn normalize_resolution(raw_resolution: Option<&str>) -> String {
    match raw_resolution {
        Some("resumed") => "resumed".to_string(),
        Some("discarded") => "discarded".to_string(),
        _ => "pending".to_string(),
    }
}

fn normalize_stage(raw_stage: Option<&str>) -> String {
    match raw_stage {
        Some("transcribing") => "transcribing".to_string(),
        Some("polishing") => "polishing".to_string(),
        Some("translating") => "translating".to_string(),
        Some("exporting") => "exporting".to_string(),
        _ => "queued".to_string(),
    }
}

fn resolve_source_flags(
    file_path: &str,
    resolution: &str,
    default_has_source_file: bool,
    default_can_resume: bool,
) -> (bool, bool) {
    match resolve_source_path_status(file_path) {
        SourcePathStatus::File => (true, resolution == "pending"),
        SourcePathStatus::Directory | SourcePathStatus::Missing => (false, false),
        SourcePathStatus::Unknown => (default_has_source_file, default_can_resume),
    }
}

fn resolve_source_path_status(path: &str) -> SourcePathStatus {
    if path.trim().is_empty() {
        return SourcePathStatus::Unknown;
    }

    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => SourcePathStatus::File,
        Ok(metadata) if metadata.is_dir() => SourcePathStatus::Directory,
        Ok(_) => SourcePathStatus::Unknown,
        Err(error) if error.kind() == ErrorKind::NotFound => SourcePathStatus::Missing,
        Err(_) => SourcePathStatus::Unknown,
    }
}

fn filename_from_path(path: &str) -> String {
    path.rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn non_empty_string_field(value: &Value, key: &str) -> Option<String> {
    string_field(value, key).filter(|value| !value.trim().is_empty())
}

fn optional_string_field(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn optional_value_field(value: &Value, key: &str) -> Option<Value> {
    match value.get(key) {
        Some(Value::Null) | None => None,
        Some(value) => Some(value.clone()),
    }
}

fn value_field_or_null(value: &Value, key: &str) -> Value {
    value.get(key).cloned().unwrap_or(Value::Null)
}

fn file_stat_field(value: &Value, key: &str) -> Option<RecoveryFileStat> {
    value
        .get(key)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
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
        let mut writer =
            BufWriter::new(File::create(&temp_path).map_err(|error| error.to_string())?);
        writer
            .write_all(contents)
            .map_err(|error| error.to_string())?;
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
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        }
        Ok(_) => fs::remove_file(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

async fn run_repository_task<R, T, F>(app: AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(RecoveryRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || task(RecoveryRepository::new(app_local_data_dir)))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn recovery_load_snapshot<R: Runtime>(
    app: AppHandle<R>,
) -> Result<RecoverySnapshot, String> {
    run_repository_task(app, |repository| repository.load_snapshot()).await
}

#[tauri::command]
pub async fn recovery_save_snapshot<R: Runtime>(
    app: AppHandle<R>,
    items: Vec<Value>,
) -> Result<RecoverySnapshot, String> {
    run_repository_task(app, move |repository| repository.save_snapshot(items)).await
}

#[tauri::command]
pub async fn recovery_persist_queue_snapshot<R: Runtime>(
    app: AppHandle<R>,
    queue_items: Vec<Value>,
) -> Result<(), String> {
    run_repository_task(app, move |repository| {
        repository.persist_queue_snapshot(queue_items).map(|_| ())
    })
    .await
}
