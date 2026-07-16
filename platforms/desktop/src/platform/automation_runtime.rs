use log::warn;
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
pub use sona_core::automation::{
    AutomationRuntimeCandidatePayload, AutomationRuntimePathCollectionOutcome,
    AutomationRuntimePathCollectionResult, AutomationRuntimeReplaceResult,
    AutomationRuntimeRuleConfig, collect_runtime_rule_path_result, normalize_automation_path,
    should_consider_runtime_candidate_path,
};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Runtime};

const AUTOMATION_RUNTIME_CANDIDATE_EVENT: &str = "automation-runtime-candidate";

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

pub trait AutomationRuntimeEventSink: Send + Sync {
    fn emit_candidate(&self, payload: AutomationRuntimeCandidatePayload) -> Result<(), String>;
}

struct TauriAutomationRuntimeEventSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> AutomationRuntimeEventSink for TauriAutomationRuntimeEventSink<R> {
    fn emit_candidate(&self, payload: AutomationRuntimeCandidatePayload) -> Result<(), String> {
        sona_ts_bind::validate_typescript_safe_integers(&payload)?;
        self.app
            .emit(AUTOMATION_RUNTIME_CANDIDATE_EVENT, payload)
            .map_err(|error| error.to_string())
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

pub fn create_event_sink<R: Runtime>(app: AppHandle<R>) -> Arc<dyn AutomationRuntimeEventSink> {
    Arc::new(TauriAutomationRuntimeEventSink { app })
}

fn build_pending_candidate_key(rule_id: &str, normalized_path: &str) -> String {
    format!("{}::{}", rule_id, normalized_path)
}

fn candidate_payload_for_rule(
    rule: &AutomationRuntimeRuleConfig,
    file_path: &str,
) -> Result<Option<AutomationRuntimeCandidatePayload>, String> {
    let result = collect_runtime_rule_path_result(
        rule,
        file_path,
        sona_runtime_fs::automation_runtime_path_metadata(file_path),
    );
    match result.outcome {
        AutomationRuntimePathCollectionOutcome::Candidate => Ok(result.candidate),
        AutomationRuntimePathCollectionOutcome::Error => {
            Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
        }
        AutomationRuntimePathCollectionOutcome::Missing
        | AutomationRuntimePathCollectionOutcome::Unsupported
        | AutomationRuntimePathCollectionOutcome::Excluded
        | AutomationRuntimePathCollectionOutcome::NotFile => Ok(None),
    }
}

pub fn collect_rule_path_result(
    rule: &AutomationRuntimeRuleConfig,
    file_path: &str,
) -> AutomationRuntimePathCollectionResult {
    collect_runtime_rule_path_result(
        rule,
        file_path,
        sona_runtime_fs::automation_runtime_path_metadata(file_path),
    )
}

pub async fn collect_rule_path_results(
    rule: AutomationRuntimeRuleConfig,
    file_paths: Vec<String>,
) -> Result<Vec<AutomationRuntimePathCollectionResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        file_paths
            .into_iter()
            .map(|file_path| collect_rule_path_result(&rule, &file_path))
            .collect()
    })
    .await
    .map_err(|error| error.to_string())
}

fn collect_candidate_paths(rule: &AutomationRuntimeRuleConfig) -> Result<Vec<String>, String> {
    sona_runtime_fs::collect_automation_runtime_candidate_paths(rule)
}

fn schedule_candidate(
    state: AutomationRuntimeState,
    event_sink: Arc<dyn AutomationRuntimeEventSink>,
    rule: AutomationRuntimeRuleConfig,
    file_path: PathBuf,
) {
    let file_path = file_path.to_string_lossy().into_owned();
    if !should_consider_runtime_candidate_path(&rule, &file_path) {
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

        let first_snapshot = match candidate_payload_for_rule(&rule_for_task, &file_path_for_task) {
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

        let second_snapshot = match candidate_payload_for_rule(&rule_for_task, &file_path_for_task)
        {
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

        if let Err(error) = event_sink_for_task.emit_candidate(second_snapshot) {
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

pub async fn scan_rule_runtime(
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

pub async fn start_rule_runtime<R: Runtime>(
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

pub async fn replace_rule_runtimes_with<StartFn, StartFuture>(
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

#[cfg(test)]
mod tests {
    use super::{
        AutomationRuntimeCandidatePayload, AutomationRuntimePathCollectionOutcome,
        AutomationRuntimeRuleConfig, AutomationRuntimeState, collect_candidate_paths,
        collect_rule_path_result, replace_rule_runtimes_with, schedule_candidate,
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

    async fn wait_for_payload_count(
        state: &AutomationRuntimeState,
        sink: &RecordingSink,
        expected: usize,
    ) {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let has_expected_payloads = sink.payloads.lock().unwrap().len() >= expected;
                let scheduler_is_idle = state.inner.pending_candidates.lock().unwrap().is_empty();
                if has_expected_payloads && scheduler_is_idle {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timed out waiting for automation candidate payload");
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

    #[test]
    fn collect_rule_path_result_returns_candidate_for_current_file_snapshot() {
        let dir = tempdir().unwrap();
        let watch_dir = dir.path().join("watch");
        fs::create_dir_all(&watch_dir).unwrap();
        let file_path = watch_dir.join("meeting.wav");
        fs::write(&file_path, b"one").unwrap();

        let rule = sample_rule(|rule| {
            rule.watch_directory = watch_dir.to_string_lossy().into_owned();
            rule.exclude_directory = watch_dir.join("exports").to_string_lossy().into_owned();
        });

        let result = collect_rule_path_result(&rule, file_path.to_string_lossy().as_ref());

        assert_eq!(
            result.outcome,
            AutomationRuntimePathCollectionOutcome::Candidate
        );
        assert_eq!(
            result
                .candidate
                .as_ref()
                .map(|candidate| candidate.file_path.as_str()),
            Some(file_path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn collect_rule_path_result_rejects_retry_sources_outside_watch_directory() {
        let dir = tempdir().unwrap();
        let watch_dir = dir.path().join("watch");
        let outside_dir = dir.path().join("outside");
        fs::create_dir_all(&watch_dir).unwrap();
        fs::create_dir_all(&outside_dir).unwrap();
        let outside_file = outside_dir.join("meeting.wav");
        fs::write(&outside_file, b"one").unwrap();

        let rule = sample_rule(|rule| {
            rule.watch_directory = watch_dir.to_string_lossy().into_owned();
            rule.exclude_directory = watch_dir.join("exports").to_string_lossy().into_owned();
        });

        let result = collect_rule_path_result(&rule, outside_file.to_string_lossy().as_ref());

        assert_eq!(
            result.outcome,
            AutomationRuntimePathCollectionOutcome::Excluded
        );
        assert!(result.candidate.is_none());
    }

    #[test]
    fn collect_rule_path_result_rejects_nested_retry_sources_for_non_recursive_rules() {
        let dir = tempdir().unwrap();
        let watch_dir = dir.path().join("watch");
        let nested_dir = watch_dir.join("nested");
        fs::create_dir_all(&nested_dir).unwrap();
        let nested_file = nested_dir.join("meeting.wav");
        fs::write(&nested_file, b"one").unwrap();

        let rule = sample_rule(|rule| {
            rule.watch_directory = watch_dir.to_string_lossy().into_owned();
            rule.exclude_directory = watch_dir.join("exports").to_string_lossy().into_owned();
            rule.recursive = false;
        });

        let result = collect_rule_path_result(&rule, nested_file.to_string_lossy().as_ref());

        assert_eq!(
            result.outcome,
            AutomationRuntimePathCollectionOutcome::Excluded
        );
        assert!(result.candidate.is_none());
    }

    #[test]
    fn collect_rule_path_result_returns_missing_for_deleted_retry_source() {
        let dir = tempdir().unwrap();
        let watch_dir = dir.path().join("watch");
        fs::create_dir_all(&watch_dir).unwrap();
        let file_path = watch_dir.join("missing.wav");

        let rule = sample_rule(|rule| {
            rule.watch_directory = watch_dir.to_string_lossy().into_owned();
            rule.exclude_directory = watch_dir.join("exports").to_string_lossy().into_owned();
        });

        let result = collect_rule_path_result(&rule, file_path.to_string_lossy().as_ref());

        assert_eq!(
            result.outcome,
            AutomationRuntimePathCollectionOutcome::Missing
        );
        assert!(result.candidate.is_none());
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

        wait_for_payload_count(&state, &sink, 1).await;

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

        wait_for_payload_count(&state, &sink, 1).await;

        let payloads = sink.payloads.lock().unwrap().clone();
        assert_eq!(payloads.len(), 1);
        assert!(payloads[0].source_fingerprint.contains("::"));
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
        assert!(results[0].started);
        assert!(!results[1].started);
        assert_eq!(results[1].error.as_deref(), Some("watch failed"));
    }
}
