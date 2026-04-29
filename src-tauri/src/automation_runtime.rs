use log::warn;
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Runtime, State};
use walkdir::WalkDir;

const AUTOMATION_RUNTIME_CANDIDATE_EVENT: &str = "automation-runtime-candidate";
const SUPPORTED_MEDIA_EXTENSIONS: &[&str] = &[
    ".wav", ".mp3", ".m4a", ".aiff", ".flac", ".ogg", ".wma", ".aac", ".opus", ".amr", ".mp4",
    ".webm", ".mov", ".mkv", ".avi", ".wmv", ".flv", ".3gp",
];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeRuleConfig {
    pub rule_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub exclude_directory: String,
    pub debounce_ms: u64,
    pub stable_window_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeReplaceResult {
    pub rule_id: String,
    pub started: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeCandidatePayload {
    pub rule_id: String,
    pub file_path: String,
    pub source_fingerprint: String,
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Clone, Default)]
pub struct AutomationRuntimeState {
    inner: Arc<AutomationRuntimeInner>,
}

#[derive(Default)]
struct AutomationRuntimeInner {
    watchers: Mutex<HashMap<String, AutomationRuleRuntime>>,
    pending_candidates: Mutex<HashMap<String, JoinHandle<()>>>,
}

struct AutomationRuleRuntime {
    _watcher: RecommendedWatcher,
}

trait AutomationRuntimeEventSink: Send + Sync {
    fn emit_candidate(&self, payload: AutomationRuntimeCandidatePayload) -> Result<(), String>;
}

struct TauriAutomationRuntimeEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AutomationRuntimeEventSink for TauriAutomationRuntimeEventSink<R> {
    fn emit_candidate(&self, payload: AutomationRuntimeCandidatePayload) -> Result<(), String> {
        self.app
            .emit(AUTOMATION_RUNTIME_CANDIDATE_EVENT, payload)
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutomationCandidateSnapshot {
    normalized_path: String,
    size: u64,
    mtime_ms: u64,
}

impl AutomationCandidateSnapshot {
    fn into_payload(self, rule_id: &str, file_path: &str) -> AutomationRuntimeCandidatePayload {
        AutomationRuntimeCandidatePayload {
            rule_id: rule_id.to_string(),
            file_path: file_path.to_string(),
            source_fingerprint: format!(
                "{}::{}::{}",
                self.normalized_path, self.size, self.mtime_ms
            ),
            size: self.size,
            mtime_ms: self.mtime_ms,
        }
    }
}

impl AutomationRuntimeState {
    fn abort_all_pending_candidates(&self) {
        self.abort_pending_candidates_for_rule(None);
    }

    fn abort_pending_candidates_for_rule(&self, rule_id: Option<&str>) {
        let mut pending = self.inner.pending_candidates.lock().unwrap();
        let keys_to_remove = pending
            .keys()
            .filter(|key| match rule_id {
                Some(rule_id) => key.starts_with(&format!("{}::", rule_id)),
                None => true,
            })
            .cloned()
            .collect::<Vec<_>>();

        for key in keys_to_remove {
            if let Some(handle) = pending.remove(&key) {
                handle.abort();
            }
        }
    }

    fn clear_watchers(&self) {
        let mut watchers = self.inner.watchers.lock().unwrap();
        watchers.clear();
    }

    fn remove_rule_runtime(&self, rule_id: &str) {
        self.abort_pending_candidates_for_rule(Some(rule_id));
        let mut watchers = self.inner.watchers.lock().unwrap();
        watchers.remove(rule_id);
    }

    fn insert_rule_runtime(&self, rule_id: String, watcher: RecommendedWatcher) {
        let mut watchers = self.inner.watchers.lock().unwrap();
        watchers.insert(rule_id, AutomationRuleRuntime { _watcher: watcher });
    }
}

fn create_event_sink<R: Runtime>(app: AppHandle<R>) -> Arc<dyn AutomationRuntimeEventSink> {
    Arc::new(TauriAutomationRuntimeEventSink { app })
}

fn normalize_automation_path(path: &str) -> String {
    path.trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn is_supported_media_path(file_path: &str) -> bool {
    let normalized = file_path.trim().to_lowercase();
    SUPPORTED_MEDIA_EXTENSIONS
        .iter()
        .any(|extension| normalized.ends_with(extension))
}

fn is_path_inside_directory(file_path: &str, directory_path: &str) -> bool {
    if directory_path.trim().is_empty() {
        return false;
    }

    let normalized_file = normalize_automation_path(file_path);
    let normalized_directory = normalize_automation_path(directory_path);

    normalized_file == normalized_directory
        || normalized_file.starts_with(&format!("{}\\", normalized_directory))
}

fn build_pending_candidate_key(rule_id: &str, normalized_path: &str) -> String {
    format!("{}::{}", rule_id, normalized_path)
}

fn should_consider_candidate_path(rule: &AutomationRuntimeRuleConfig, file_path: &str) -> bool {
    is_supported_media_path(file_path)
        && !is_path_inside_directory(file_path, &rule.exclude_directory)
}

fn snapshot_candidate(file_path: &str) -> Result<Option<AutomationCandidateSnapshot>, String> {
    if !is_supported_media_path(file_path) {
        return Ok(None);
    }

    let metadata = match std::fs::metadata(file_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() {
        return Ok(None);
    }

    let normalized_path = normalize_automation_path(file_path);
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    Ok(Some(AutomationCandidateSnapshot {
        normalized_path,
        size: metadata.len(),
        mtime_ms,
    }))
}

fn collect_candidate_paths(rule: &AutomationRuntimeRuleConfig) -> Result<Vec<String>, String> {
    let watch_directory = rule.watch_directory.trim();
    if watch_directory.is_empty() {
        return Ok(Vec::new());
    }

    let mut walker = WalkDir::new(watch_directory).follow_links(false);
    if !rule.recursive {
        walker = walker.max_depth(1);
    }

    let mut paths = Vec::new();

    for entry in walker.into_iter() {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path().to_string_lossy().into_owned();
        if should_consider_candidate_path(rule, &file_path) {
            paths.push(file_path);
        }
    }

    Ok(paths)
}

fn schedule_candidate(
    state: AutomationRuntimeState,
    event_sink: Arc<dyn AutomationRuntimeEventSink>,
    rule: AutomationRuntimeRuleConfig,
    file_path: PathBuf,
) {
    let file_path = file_path.to_string_lossy().into_owned();
    if !should_consider_candidate_path(&rule, &file_path) {
        return;
    }

    let normalized_path = normalize_automation_path(&file_path);
    let pending_key = build_pending_candidate_key(&rule.rule_id, &normalized_path);

    let mut pending = state.inner.pending_candidates.lock().unwrap();
    if pending.contains_key(&pending_key) {
        return;
    }

    let state_for_task = state.clone();
    let event_sink_for_task = event_sink.clone();
    let rule_for_task = rule.clone();
    let pending_key_for_task = pending_key.clone();
    let file_path_for_task = file_path.clone();
    let handle = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(rule_for_task.debounce_ms)).await;

        let first_snapshot = match snapshot_candidate(&file_path_for_task) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                state_for_task
                    .inner
                    .pending_candidates
                    .lock()
                    .unwrap()
                    .remove(&pending_key_for_task);
                return;
            }
            Err(error) => {
                warn!(
                    "[AutomationRuntime] Failed to stat candidate before stability wait: rule={} path={} error={}",
                    rule_for_task.rule_id, file_path_for_task, error
                );
                state_for_task
                    .inner
                    .pending_candidates
                    .lock()
                    .unwrap()
                    .remove(&pending_key_for_task);
                return;
            }
        };

        tokio::time::sleep(Duration::from_millis(rule_for_task.stable_window_ms)).await;

        let second_snapshot = match snapshot_candidate(&file_path_for_task) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                state_for_task
                    .inner
                    .pending_candidates
                    .lock()
                    .unwrap()
                    .remove(&pending_key_for_task);
                return;
            }
            Err(error) => {
                warn!(
                    "[AutomationRuntime] Failed to stat candidate after stability wait: rule={} path={} error={}",
                    rule_for_task.rule_id, file_path_for_task, error
                );
                state_for_task
                    .inner
                    .pending_candidates
                    .lock()
                    .unwrap()
                    .remove(&pending_key_for_task);
                return;
            }
        };

        if first_snapshot != second_snapshot {
            state_for_task
                .inner
                .pending_candidates
                .lock()
                .unwrap()
                .remove(&pending_key_for_task);
            return;
        }

        if let Err(error) = event_sink_for_task.emit_candidate(
            second_snapshot.into_payload(&rule_for_task.rule_id, &file_path_for_task),
        ) {
            warn!(
                "[AutomationRuntime] Failed to emit candidate: rule={} path={} error={}",
                rule_for_task.rule_id, file_path_for_task, error
            );
        }

        state_for_task
            .inner
            .pending_candidates
            .lock()
            .unwrap()
            .remove(&pending_key_for_task);
    });

    pending.insert(pending_key, handle);
}

async fn scan_rule_runtime(
    state: AutomationRuntimeState,
    event_sink: Arc<dyn AutomationRuntimeEventSink>,
    rule: AutomationRuntimeRuleConfig,
) -> Result<(), String> {
    let rule_for_scan = rule.clone();
    let paths =
        tauri::async_runtime::spawn_blocking(move || collect_candidate_paths(&rule_for_scan))
            .await
            .map_err(|error| error.to_string())??;

    for file_path in paths {
        schedule_candidate(
            state.clone(),
            event_sink.clone(),
            rule.clone(),
            PathBuf::from(file_path),
        );
    }

    Ok(())
}

fn create_rule_watcher(
    state: AutomationRuntimeState,
    rule: AutomationRuntimeRuleConfig,
    event_sink: Arc<dyn AutomationRuntimeEventSink>,
) -> Result<RecommendedWatcher, String> {
    let callback_rule = rule.clone();
    let mut watcher = RecommendedWatcher::new(
        move |result: notify::Result<Event>| match result {
            Ok(event) => {
                for path in event.paths {
                    schedule_candidate(
                        state.clone(),
                        event_sink.clone(),
                        callback_rule.clone(),
                        path,
                    );
                }
            }
            Err(error) => {
                warn!(
                    "[AutomationRuntime] Watch error for rule {}: {}",
                    callback_rule.rule_id, error
                );
            }
        },
        NotifyConfig::default(),
    )
    .map_err(|error| error.to_string())?;

    watcher
        .watch(
            Path::new(&rule.watch_directory),
            if rule.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            },
        )
        .map_err(|error| error.to_string())?;

    Ok(watcher)
}

async fn start_rule_runtime<R: Runtime>(
    app: AppHandle<R>,
    state: AutomationRuntimeState,
    rule: AutomationRuntimeRuleConfig,
) -> Result<(), String> {
    let event_sink = create_event_sink(app.clone());
    let watcher = create_rule_watcher(state.clone(), rule.clone(), event_sink.clone())?;
    state.insert_rule_runtime(rule.rule_id.clone(), watcher);

    if let Err(error) = scan_rule_runtime(state.clone(), event_sink, rule.clone()).await {
        state.remove_rule_runtime(&rule.rule_id);
        return Err(error);
    }

    Ok(())
}

async fn replace_rule_runtimes_with<StartFn, StartFuture>(
    state: AutomationRuntimeState,
    rules: Vec<AutomationRuntimeRuleConfig>,
    mut start_rule: StartFn,
) -> Vec<AutomationRuntimeReplaceResult>
where
    StartFn: FnMut(AutomationRuntimeRuleConfig) -> StartFuture,
    StartFuture: Future<Output = Result<(), String>>,
{
    state.clear_watchers();
    state.abort_all_pending_candidates();

    let mut results = Vec::with_capacity(rules.len());
    for rule in rules {
        let rule_id = rule.rule_id.clone();
        match start_rule(rule).await {
            Ok(()) => results.push(AutomationRuntimeReplaceResult {
                rule_id,
                started: true,
                error: None,
            }),
            Err(error) => results.push(AutomationRuntimeReplaceResult {
                rule_id,
                started: false,
                error: Some(error),
            }),
        }
    }

    results
}

#[tauri::command]
pub async fn replace_automation_runtime_rules<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AutomationRuntimeState>,
    rules: Vec<AutomationRuntimeRuleConfig>,
) -> Result<Vec<AutomationRuntimeReplaceResult>, String> {
    let runtime_state = state.inner().clone();
    Ok(
        replace_rule_runtimes_with(runtime_state.clone(), rules, move |rule| {
            start_rule_runtime(app.clone(), runtime_state.clone(), rule)
        })
        .await,
    )
}

#[tauri::command]
pub async fn scan_automation_runtime_rule<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AutomationRuntimeState>,
    rule: AutomationRuntimeRuleConfig,
) -> Result<(), String> {
    scan_rule_runtime(state.inner().clone(), create_event_sink(app), rule).await
}

#[cfg(test)]
mod tests {
    use super::{
        collect_candidate_paths, is_path_inside_directory, is_supported_media_path,
        normalize_automation_path, replace_rule_runtimes_with, schedule_candidate,
        snapshot_candidate, AutomationRuntimeCandidatePayload, AutomationRuntimeRuleConfig,
        AutomationRuntimeState,
    };
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::tempdir;

    #[derive(Default)]
    struct RecordingSink {
        payloads: Mutex<Vec<AutomationRuntimeCandidatePayload>>,
    }

    impl super::AutomationRuntimeEventSink for RecordingSink {
        fn emit_candidate(&self, payload: AutomationRuntimeCandidatePayload) -> Result<(), String> {
            self.payloads.lock().unwrap().push(payload);
            Ok(())
        }
    }

    fn sample_rule(
        overrides: impl FnOnce(&mut AutomationRuntimeRuleConfig),
    ) -> AutomationRuntimeRuleConfig {
        let mut rule = AutomationRuntimeRuleConfig {
            rule_id: "rule-1".to_string(),
            watch_directory: "C:\\watch".to_string(),
            recursive: true,
            exclude_directory: "C:\\exports".to_string(),
            debounce_ms: 5,
            stable_window_ms: 10,
        };
        overrides(&mut rule);
        rule
    }

    #[test]
    fn supported_media_path_recognizes_audio_and_video_extensions() {
        assert!(is_supported_media_path("C:\\watch\\meeting.wav"));
        assert!(is_supported_media_path("C:\\watch\\clip.MP4"));
        assert!(!is_supported_media_path("C:\\watch\\notes.txt"));
    }

    #[test]
    fn exclude_directory_filter_matches_directory_prefixes() {
        assert!(is_path_inside_directory(
            "C:\\exports\\meeting.txt",
            "C:\\exports"
        ));
        assert!(is_path_inside_directory(
            "C:/exports/sub/meeting.txt",
            "C:\\exports"
        ));
        assert!(!is_path_inside_directory(
            "C:\\watch\\meeting.wav",
            "C:\\exports"
        ));
    }

    #[test]
    fn snapshot_candidate_uses_normalized_path_in_fingerprint_inputs() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("Meeting.WAV");
        fs::write(&file_path, b"hello").unwrap();

        let snapshot = snapshot_candidate(file_path.to_string_lossy().as_ref())
            .unwrap()
            .expect("snapshot should exist");

        assert_eq!(
            snapshot.normalized_path,
            normalize_automation_path(file_path.to_string_lossy().as_ref())
        );
        assert_eq!(snapshot.size, 5);
    }

    #[test]
    fn collect_candidate_paths_skips_files_inside_excluded_directory() {
        let dir = tempdir().unwrap();
        let watch_dir = dir.path().join("watch");
        let export_dir = watch_dir.join("exports");
        fs::create_dir_all(&export_dir).unwrap();
        fs::write(watch_dir.join("meeting.wav"), b"one").unwrap();
        fs::write(export_dir.join("skip.wav"), b"two").unwrap();
        fs::write(watch_dir.join("notes.txt"), b"three").unwrap();

        let rule = sample_rule(|rule| {
            rule.watch_directory = watch_dir.to_string_lossy().into_owned();
            rule.exclude_directory = export_dir.to_string_lossy().into_owned();
        });

        let paths = collect_candidate_paths(&rule).unwrap();

        assert_eq!(
            paths,
            vec![watch_dir.join("meeting.wav").to_string_lossy().into_owned()]
        );
    }

    #[tokio::test]
    async fn schedule_candidate_coalesces_repeated_events_for_the_same_rule_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("meeting.wav");
        fs::write(&file_path, b"one").unwrap();
        let sink = Arc::new(RecordingSink::default());
        let state = AutomationRuntimeState::default();
        let rule = sample_rule(|rule| {
            rule.watch_directory = dir.path().to_string_lossy().into_owned();
            rule.exclude_directory = dir.path().join("exports").to_string_lossy().into_owned();
        });

        schedule_candidate(state.clone(), sink.clone(), rule.clone(), file_path.clone());
        schedule_candidate(state.clone(), sink.clone(), rule.clone(), file_path.clone());

        tokio::time::sleep(Duration::from_millis(50)).await;

        let payloads = sink.payloads.lock().unwrap().clone();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].rule_id, rule.rule_id);
        assert_eq!(payloads[0].file_path, file_path.to_string_lossy());
    }

    #[tokio::test]
    async fn schedule_candidate_coalesces_scan_and_watch_races_for_the_same_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("meeting.wav");
        fs::write(&file_path, b"one").unwrap();
        let sink = Arc::new(RecordingSink::default());
        let state = AutomationRuntimeState::default();
        let rule = sample_rule(|rule| {
            rule.watch_directory = dir.path().to_string_lossy().into_owned();
            rule.exclude_directory = dir.path().join("exports").to_string_lossy().into_owned();
        });

        schedule_candidate(state.clone(), sink.clone(), rule.clone(), file_path.clone());
        tokio::time::sleep(Duration::from_millis(1)).await;
        schedule_candidate(state.clone(), sink.clone(), rule.clone(), file_path.clone());

        tokio::time::sleep(Duration::from_millis(50)).await;

        let payloads = sink.payloads.lock().unwrap().clone();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].source_fingerprint.contains("::"), true);
    }

    #[tokio::test]
    async fn replace_rule_runtimes_reports_partial_start_failures_without_stopping_other_rules() {
        let state = AutomationRuntimeState::default();
        let started_rules = Arc::new(Mutex::new(Vec::new()));
        let started_rules_for_closure = started_rules.clone();
        let rules = vec![
            sample_rule(|rule| {
                rule.rule_id = "rule-ok".to_string();
            }),
            sample_rule(|rule| {
                rule.rule_id = "rule-fail".to_string();
            }),
        ];

        let results = replace_rule_runtimes_with(state, rules, move |rule| {
            let started_rules = started_rules_for_closure.clone();
            async move {
                started_rules.lock().unwrap().push(rule.rule_id.clone());
                if rule.rule_id == "rule-fail" {
                    Err("watch failed".to_string())
                } else {
                    Ok(())
                }
            }
        })
        .await;

        assert_eq!(
            started_rules.lock().unwrap().as_slice(),
            &["rule-ok".to_string(), "rule-fail".to_string()]
        );
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].started, true);
        assert_eq!(results[1].started, false);
        assert_eq!(results[1].error.as_deref(), Some("watch failed"));
    }
}
