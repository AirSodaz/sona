//! Settings modal and its retained data views.

mod about;
pub(crate) mod api_server;
mod automation;
mod dashboard;
mod languages;
mod llm_service;
mod mcp;
mod speaker_profiles;
mod storage;
mod sync;
pub(crate) mod vocabulary;

use crate::theme::tokens::*;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, StatefulInteractiveElement as _, *};
use serde_json::{Value, json};
use sona_core::automation::repository::AutomationRepositoryState;
use sona_core::dashboard::models::DashboardSnapshotDomainModel;
use sona_core::runtime::app_update::{AppUpdateCheckResult, UpdateChannel};
use sona_core::sync::{SyncConflictResolution, SyncConflictSummary, SyncStatusSnapshot};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::{Notify, watch};

use super::shell::MainShell;
use about::about_panel;
use api_server::api_server_settings_panel;
use automation::automation_panel;
use dashboard::{dashboard_panel, dashboard_snapshot_time};
pub(crate) use languages::LANGUAGE_OPTIONS;
use llm_service::llm_service_panel;
use mcp::mcp_settings_panel;
use storage::storage_panel;
use sync::sync_panel;
use vocabulary::vocabulary_panel;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SettingsTab {
    General,
    Dashboard,
    Shortcuts,
    Microphone,
    Models,
    Subtitle,
    Vocabulary,
    LlmService,
    Automation,
    Storage,
    Sync,
    About,
    ApiServer,
    Mcp,
}

type TabItem = (SettingsTab, &'static str, LucideIcon);
type TabGroup = (&'static str, &'static [TabItem]);
fn storage_cleanup_summary(report: &sona_core::history::HistoryAudioCleanupReport) -> String {
    let mut parts = Vec::new();
    if report.removed_count > 0 {
        parts.push(format!("{} files removed", report.removed_count));
    }
    if report.removed_bytes > 0 {
        parts.push(format!(
            "{} freed",
            crate::views::settings::storage::format_bytes(report.removed_bytes)
        ));
    }
    if report.missing_marked_count > 0 {
        parts.push(format!("{} missing", report.missing_marked_count));
    }
    if report.failed_count > 0 {
        parts.push(format!("{} failed", report.failed_count));
    }
    if report.skipped_active_count > 0 {
        parts.push(format!("{} active skipped", report.skipped_active_count));
    }
    if parts.is_empty() {
        "No audio files need cleanup.".to_string()
    } else {
        parts.join(" · ")
    }
}

fn safe_update_version(version: &str) -> String {
    let sanitized = version
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "latest".to_string()
    } else {
        sanitized
    }
}

pub(super) fn llm_slug(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn llm_provider_default_host(provider: &str) -> String {
    match provider {
        "google_translate_free" => "https://translate.googleapis.com/translate_a/single".into(),
        "google_translate" => "https://translation.googleapis.com/language/translate/v2".into(),
        _ => sona_core::llm::providers::find_llm_provider_by_id_or_alias(provider)
            .map(|provider| provider.defaults.api_host.clone())
            .unwrap_or_default(),
    }
}

fn llm_provider_test_model(provider: &str) -> &'static str {
    match provider {
        "azure_openai" => "gpt-4o-deployment",
        "anthropic" => "claude-sonnet-4-20250514",
        "gemini" => "gemini-2.5-flash",
        "ollama" => "qwen3:8b",
        "deep_seek" => "deepseek-chat",
        "moonshot_cn" | "moonshot_ai" | "kimi" => "moonshot-v1-8k",
        "chatglm" => "glm-4-flash",
        "qwen" | "qwen_portal" => "qwen-max",
        "groq" => "llama-3.3-70b-versatile",
        "x_ai" => "grok-3-mini",
        "mistral_ai" => "mistral-large-latest",
        "perplexity" => "sonar",
        "cohere" => "command-r-plus",
        "together" => "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        "venice" => "llama-3.3-70b",
        "hyperbolic" => "meta-llama/Llama-3.3-70B-Instruct",
        "llamafile" => "default",
        "local" => "Qwen/Qwen3.5-4B",
        "google_translate" | "google_translate_free" => "default",
        _ => "gpt-4o-mini",
    }
}

fn llm_custom_provider_default_host(strategy: &str) -> String {
    match strategy {
        "openai_responses" => String::new(),
        "anthropic" | "gemini" | "openai_compatible" => String::new(),
        _ => String::new(),
    }
}

fn remove_custom_provider_settings(settings: &mut Value, provider_id: &str) -> bool {
    let Some(custom_providers) = settings
        .get_mut("customProviders")
        .and_then(Value::as_object_mut)
    else {
        return false;
    };
    if custom_providers.remove(provider_id).is_none() {
        return false;
    }

    if let Some(providers) = settings.get_mut("providers").and_then(Value::as_object_mut) {
        providers.remove(provider_id);
    }
    let removed_ids = settings
        .get("models")
        .and_then(Value::as_object)
        .map(|models| {
            models
                .iter()
                .filter(|&(_id, model)| {
                    model.get("provider").and_then(Value::as_str) == Some(provider_id)
                })
                .map(|(id, _model)| id.clone())
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();
    if let Some(models) = settings.get_mut("models").and_then(Value::as_object_mut) {
        for id in &removed_ids {
            models.remove(id);
        }
    }
    if let Some(order) = settings.get_mut("modelOrder").and_then(Value::as_array_mut) {
        order.retain(|id| id.as_str().is_none_or(|id| !removed_ids.contains(id)));
    }
    if let Some(selections) = settings
        .get_mut("selections")
        .and_then(Value::as_object_mut)
    {
        selections.retain(|_, id| id.as_str().is_none_or(|id| !removed_ids.contains(id)));
    }
    if settings.get("activeProvider").and_then(Value::as_str) == Some(provider_id) {
        settings["activeProvider"] = Value::String("google_translate_free".into());
    }
    true
}

fn remove_llm_model_settings(settings: &mut Value, model_id: &str) -> bool {
    if settings
        .get_mut("models")
        .and_then(Value::as_object_mut)
        .and_then(|models| models.remove(model_id))
        .is_none()
    {
        return false;
    }
    if let Some(order) = settings.get_mut("modelOrder").and_then(Value::as_array_mut) {
        order.retain(|id| id.as_str() != Some(model_id));
    }
    if let Some(selections) = settings
        .get_mut("selections")
        .and_then(Value::as_object_mut)
    {
        for feature in ["polish", "translation", "summary"] {
            let key = format!("{feature}ModelId");
            if selections.get(&key).and_then(Value::as_str) == Some(model_id) {
                selections.remove(&key);
            }
        }
    }
    true
}

fn apply_llm_model_metadata_inputs(
    metadata: &Value,
    fields: &HashMap<String, String>,
) -> Result<Value, String> {
    let mut updated = metadata
        .as_object()
        .cloned()
        .map(Value::Object)
        .unwrap_or_else(|| json!({}));
    for key in [
        "displayName",
        "knowledgeCutoff",
        "releaseDate",
        "lastUpdated",
    ] {
        if let Some(value) = fields.get(key) {
            let value = value.trim();
            if value.is_empty() {
                updated.as_object_mut().unwrap().remove(key);
            } else {
                updated[key] = Value::String(value.to_string());
            }
        }
    }
    for key in [
        "contextWindow",
        "maxOutputTokens",
        "inputPrice",
        "outputPrice",
        "cacheReadPrice",
        "cacheWritePrice",
    ] {
        let Some(value) = fields.get(key) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            updated.as_object_mut().unwrap().remove(key);
            continue;
        }
        let number = value
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite() && *number >= 0.0)
            .and_then(serde_json::Number::from_f64)
            .ok_or_else(|| format!("{key} must be a nonnegative number."))?;
        updated[key] = Value::Number(number);
    }
    Ok(updated)
}

#[cfg(target_os = "windows")]
fn update_package_extension() -> &'static str {
    ".exe"
}

#[cfg(target_os = "macos")]
fn update_package_extension() -> &'static str {
    ".tar.gz"
}

#[cfg(target_os = "linux")]
fn update_package_extension() -> &'static str {
    ".AppImage"
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn update_package_extension() -> &'static str {
    ".package"
}

fn spawn_updated_app(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(path).spawn();
    #[cfg(not(target_os = "macos"))]
    let result = Command::new(path).spawn();

    result
        .map(|_| ())
        .map_err(|error| format!("Failed to relaunch the updated application: {error}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutTarget {
    LiveRecord,
    VoiceTyping,
    VoiceTypingTranslate,
    VoiceTypingQuickRecall,
}

#[derive(Clone)]
enum SyncAction {
    Run,
    Pause(bool),
    Lock,
    Resolve(String, SyncConflictResolution),
}

#[derive(Clone, Debug, PartialEq)]
pub enum OnlineProviderTestState {
    Loading,
    Success { latency_ms: u64 },
    Error { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AboutUpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Installing,
    Downloaded,
    Error(String),
}

pub struct SettingsModal {
    pub is_open: bool,
    pub current_tab: SettingsTab,
    pub autostart: bool,
    pub minimize_to_tray: bool,
    pub silent_start: bool,
    pub auto_check_updates: bool,
    pub selected_theme: String,
    pub selected_lang: String,
    pub selected_font: String,
    pub selected_mic: String,
    pub selected_system_audio: String,
    pub microphone_devices: Vec<String>,
    pub system_audio_devices: Vec<String>,
    pub audio_devices_loading: bool,
    pub audio_devices_error: Option<String>,
    pub microphone_menu_open: bool,
    pub system_audio_menu_open: bool,
    pub microphone_boost: f64,
    pub keep_microphone_active: bool,
    pub mute_during_recording: bool,
    pub ffmpeg_enabled: bool,
    pub ffmpeg_path: String,
    pub microphone_peak: f32,
    pub system_audio_peak: f32,
    pub audio_peak_listener_started: bool,
    pub audio_preview_started: bool,
    pub live_record_shortcut: String,
    pub is_recording_shortcut: bool,
    pub shortcut_recording_target: Option<ShortcutTarget>,
    pub is_editing_shortcut: bool,
    pub shortcut_draft: String,
    pub model_catalog: Option<Value>,
    pub model_catalog_loading: bool,
    pub model_catalog_error: Option<String>,
    pub model_download_ids: HashMap<String, String>,
    pub model_download_progress: HashMap<String, f32>,
    pub pending_model_delete: Option<(String, String, String)>,
    pub model_config: Value,
    pub model_scenario_batch: bool,
    pub model_selector_open: Option<String>,
    pub model_engine_filter: String,
    pub model_status_filter: String,
    pub model_label_filter: String,
    pub model_search_query: String,
    pub expanded_online_provider: Option<String>,
    pub online_provider_api_key_revealed: std::collections::HashSet<String>,
    pub online_provider_test_states: HashMap<String, OnlineProviderTestState>,
    pub cuda_addon_status: Option<sona_core::runtime::cuda_addon::CudaAddonInspection>,
    pub cuda_addon_loading: bool,
    pub cuda_addon_downloading: bool,
    pub cuda_addon_progress: f32,
    pub cuda_addon_error: Option<String>,
    dashboard_snapshot: Option<DashboardSnapshotDomainModel>,
    dashboard_loading: bool,
    dashboard_deep_loading: bool,
    dashboard_error: Option<String>,
    dashboard_load_attempted: bool,
    dashboard_request_generation: u64,
    automation_state: Option<AutomationRepositoryState>,
    automation_loading: bool,
    automation_error: Option<String>,
    sync_status: Option<SyncStatusSnapshot>,
    sync_conflicts: Vec<SyncConflictSummary>,
    sync_loading: bool,
    sync_busy: bool,
    sync_error: Option<String>,
    storage_usage: Option<crate::platform::storage::usage::StorageUsageSnapshot>,
    storage_directories: Option<crate::platform::storage::location::StorageDirectoriesInfo>,
    storage_loading: bool,
    storage_error: Option<String>,
    pub(crate) storage_pending_migration: Option<(bool, String)>,
    storage_migration_progress:
        Option<crate::platform::storage::location::StorageMigrationProgress>,
    storage_migration_busy: bool,
    pub(crate) storage_cleanup_pending: Option<sona_core::history::HistoryAudioCleanupRequest>,
    pub(crate) storage_pending_reset: Option<bool>,
    storage_cleanup_busy: bool,
    storage_cleanup_result: Option<String>,
    about_update_status: AboutUpdateStatus,
    about_update_result: Option<AppUpdateCheckResult>,
    about_update_progress: f32,
    about_installed_app: Option<PathBuf>,
    llm_active_feature: String,
    llm_model_draft: String,
    llm_custom_provider_name: String,
    llm_custom_provider_strategy: String,
    llm_edit_provider_strategy: String,
    llm_show_custom_provider_form: bool,
    llm_editing_provider: Option<String>,
    pub(crate) llm_pending_delete_provider: Option<String>,
    pub(crate) api_server_pending_key: Option<String>,
    llm_discovery_provider: Option<String>,
    llm_discovery_loading: bool,
    llm_discovery_error: Option<String>,
    llm_discovered_models: Vec<sona_core::llm::provider_protocol::LlmModelSummary>,
    llm_discovery_generation: u64,
    llm_provider_test_states: HashMap<String, OnlineProviderTestState>,
    llm_editing_model: Option<String>,
    llm_model_metadata_draft: Value,
    llm_model_metadata_error: Option<String>,
    mcp_logs: Vec<crate::platform::agent_control::server::McpActivityLogEntry>,
    mcp_logs_loading: bool,
    mcp_logs_loaded: bool,
    mcp_log_filter_status: String,
    pub(crate) mcp_log_query: String,
    mcp_selected_guide: String,
    subtitle_voice_typing_tab: bool,
    vocabulary_tab: String,
    vocabulary_prompt_tab: String,
    vocabulary_dictionary_mode: String,
    vocabulary_dictionary_scope: String,
    pub(crate) vocabulary_dictionary_search_query: String,
    pub(crate) vocabulary_dictionary_test_input: String,
    pub(crate) vocabulary_dictionary_edit_line: Option<usize>,
    pub(crate) vocabulary_dictionary_tutorial_open: bool,
    pub(crate) vocabulary_dictionary_add_scope: String,
    pub(crate) vocabulary_dictionary_scope_menu_open: bool,
    pub(crate) vocabulary_dictionary_dialog_open: bool,
    pub(crate) vocabulary_dictionary_dialog_type: String,
    vocabulary_polish_edit_id: Option<String>,
    vocabulary_summary_edit_id: Option<String>,
    speaker_scope_filter: String,
    speaker_new_scope: String,
    speaker_new_scope_open: bool,
    speaker_expanded_ids: HashSet<String>,
    speaker_edit_scope: Option<String>,
    speaker_scope_modal_type: String,
    speaker_scope_project_ids: Vec<String>,
    speaker_import_error: Option<String>,
}

impl SettingsModal {
    pub fn new() -> Self {
        Self {
            is_open: false,
            current_tab: SettingsTab::General,
            autostart: false,
            minimize_to_tray: true,
            silent_start: false,
            auto_check_updates: true,
            selected_theme: "Dark (System)".to_string(),
            selected_lang: "auto".to_string(),
            selected_font: "system".to_string(),
            selected_mic: "default".to_string(),
            selected_system_audio: "default".to_string(),
            microphone_devices: Vec::new(),
            system_audio_devices: Vec::new(),
            audio_devices_loading: false,
            audio_devices_error: None,
            microphone_menu_open: false,
            system_audio_menu_open: false,
            microphone_boost: 1.0,
            keep_microphone_active: false,
            mute_during_recording: false,
            ffmpeg_enabled: false,
            ffmpeg_path: String::new(),
            microphone_peak: 0.0,
            system_audio_peak: 0.0,
            audio_peak_listener_started: false,
            audio_preview_started: false,
            live_record_shortcut: "Ctrl + Space".to_string(),
            is_recording_shortcut: false,
            shortcut_recording_target: None,
            is_editing_shortcut: false,
            shortcut_draft: "Ctrl + Space".to_string(),
            model_catalog: None,
            model_catalog_loading: false,
            model_catalog_error: None,
            model_download_ids: HashMap::new(),
            model_download_progress: HashMap::new(),
            pending_model_delete: None,
            model_config: json!({}),
            model_scenario_batch: false,
            model_selector_open: None,
            model_engine_filter: "all".to_string(),
            model_status_filter: "all".to_string(),
            model_label_filter: "all".to_string(),
            model_search_query: String::new(),
            expanded_online_provider: None,
            online_provider_api_key_revealed: std::collections::HashSet::new(),
            online_provider_test_states: HashMap::new(),
            cuda_addon_status: None,
            cuda_addon_loading: false,
            cuda_addon_downloading: false,
            cuda_addon_progress: 0.0,
            cuda_addon_error: None,
            dashboard_snapshot: None,
            dashboard_loading: false,
            dashboard_deep_loading: false,
            dashboard_error: None,
            dashboard_load_attempted: false,
            dashboard_request_generation: 0,
            automation_state: None,
            automation_loading: false,
            automation_error: None,
            sync_status: None,
            sync_conflicts: Vec::new(),
            sync_loading: false,
            sync_busy: false,
            sync_error: None,
            storage_usage: None,
            storage_directories: None,
            storage_loading: false,
            storage_error: None,
            storage_pending_migration: None,
            storage_migration_progress: None,
            storage_migration_busy: false,
            storage_cleanup_pending: None,
            storage_pending_reset: None,
            storage_cleanup_busy: false,
            storage_cleanup_result: None,
            about_update_status: AboutUpdateStatus::Idle,
            about_update_result: None,
            about_update_progress: 0.0,
            about_installed_app: None,
            llm_active_feature: "polish".to_string(),
            llm_model_draft: String::new(),
            llm_custom_provider_name: String::new(),
            llm_custom_provider_strategy: "openai_compatible".to_string(),
            llm_edit_provider_strategy: "openai_compatible".to_string(),
            llm_show_custom_provider_form: false,
            llm_editing_provider: None,
            llm_pending_delete_provider: None,
            api_server_pending_key: None,
            llm_discovery_provider: None,
            llm_discovery_loading: false,
            llm_discovery_error: None,
            llm_discovered_models: Vec::new(),
            llm_discovery_generation: 0,
            llm_provider_test_states: HashMap::new(),
            llm_editing_model: None,
            llm_model_metadata_draft: json!({}),
            llm_model_metadata_error: None,
            mcp_logs: Vec::new(),
            mcp_logs_loading: false,
            mcp_logs_loaded: false,
            mcp_log_filter_status: "all".to_string(),
            mcp_log_query: String::new(),
            mcp_selected_guide: "claude-desktop".to_string(),
            subtitle_voice_typing_tab: true,
            vocabulary_tab: "recognition".to_string(),
            vocabulary_prompt_tab: "polish".to_string(),
            vocabulary_dictionary_mode: "visual".to_string(),
            vocabulary_dictionary_scope: "all".to_string(),
            vocabulary_dictionary_search_query: String::new(),
            vocabulary_dictionary_test_input: String::new(),
            vocabulary_dictionary_edit_line: None,
            vocabulary_dictionary_tutorial_open: false,
            vocabulary_dictionary_add_scope: "global".to_string(),
            vocabulary_dictionary_scope_menu_open: false,
            vocabulary_dictionary_dialog_open: false,
            vocabulary_dictionary_dialog_type: "hotword".to_string(),
            vocabulary_polish_edit_id: None,
            vocabulary_summary_edit_id: None,
            speaker_scope_filter: "all".to_string(),
            speaker_new_scope: "global".to_string(),
            speaker_new_scope_open: false,
            speaker_expanded_ids: HashSet::new(),
            speaker_edit_scope: None,
            speaker_scope_modal_type: "global".to_string(),
            speaker_scope_project_ids: Vec::new(),
            speaker_import_error: None,
        }
    }

    pub fn open(&mut self, cx: &mut Context<MainShell>) {
        self.is_open = true;
        if self.current_tab == SettingsTab::Microphone {
            self.start_audio_preview(cx);
        }
    }

    pub fn close(&mut self, cx: &mut Context<MainShell>) {
        self.is_open = false;
        self.stop_audio_preview(cx);
    }

    pub fn set_tab(&mut self, tab: SettingsTab, cx: &mut Context<MainShell>) {
        if self.current_tab == SettingsTab::About
            && tab != SettingsTab::About
            && self.about_update_status == AboutUpdateStatus::UpToDate
        {
            self.about_update_status = AboutUpdateStatus::Idle;
        }
        self.current_tab = tab;
        if tab == SettingsTab::Vocabulary {
            let projects = cx
                .global::<crate::app::GlobalDesktopServices>()
                .0
                .projects
                .clone();
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let projects = projects.list().await.unwrap_or_default();
                this.update(cx, |shell, cx| {
                    shell.project_selector.projects = projects;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        if tab == SettingsTab::Microphone {
            self.load_audio_devices(cx);
            self.start_audio_preview(cx);
        } else {
            self.stop_audio_preview(cx);
        }
        if tab == SettingsTab::Models {
            self.load_model_config(cx);
            self.load_model_catalog(cx);
            self.load_cuda_addon_status(cx);
        }
        if tab == SettingsTab::LlmService {
            self.load_model_config(cx);
        }
        if tab == SettingsTab::Vocabulary {
            self.load_model_config(cx);
        }
        if tab == SettingsTab::Dashboard && !self.dashboard_load_attempted {
            self.load_dashboard(cx);
        }
        if tab == SettingsTab::Mcp && !self.mcp_logs_loaded {
            self.load_mcp_logs(cx);
        }
        if tab == SettingsTab::Automation && self.automation_state.is_none() {
            self.load_automation_state(cx);
        }
        if tab == SettingsTab::Sync && self.sync_status.is_none() {
            self.load_sync_state(cx);
        }
        if tab == SettingsTab::Storage {
            self.load_storage_state(cx);
        }
    }

    fn load_storage_state(&mut self, cx: &mut Context<MainShell>) {
        self.storage_loading = true;
        self.storage_error = None;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = async {
                let directories = crate::platform::storage::location::get_storage_directories_info(
                    &crate::platform::storage::location::default_app_local_data_dir(),
                )?;
                let context = services.sqlite.current_context()?;
                let usage = crate::platform::storage::usage::get_usage_snapshot(
                    &context,
                    Some(PathBuf::from(&directories.models_dir)),
                )
                .await?;
                Ok::<_, String>((directories, usage))
            }
            .await;
            let _ = this.update(cx, |shell, cx| {
                shell.settings_modal.storage_loading = false;
                match result {
                    Ok((directories, usage)) => {
                        shell.settings_modal.storage_directories = Some(directories);
                        shell.settings_modal.storage_usage = Some(usage);
                        shell.settings_modal.storage_error = None;
                    }
                    Err(error) => shell.settings_modal.storage_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn set_storage_retention(&mut self, days: Option<u64>, cx: &mut Context<MainShell>) {
        let value = days.map_or(Value::Null, Value::from);
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        if let Err(error) = config.set_setting("historyAudioRetentionDays", &value) {
            self.storage_error = Some(error);
        }
        self.storage_cleanup_result = None;
        cx.notify();
    }

    fn preview_storage_cleanup(
        &mut self,
        days: u64,
        active_history_id: Option<String>,
        cx: &mut Context<MainShell>,
    ) {
        if self.storage_cleanup_busy {
            return;
        }
        self.storage_cleanup_busy = true;
        self.storage_cleanup_result = None;
        let history = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .history
            .clone();
        let this = cx.entity();
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let request = sona_core::history::HistoryAudioCleanupRequest {
                retention_days: Some(days),
                exclude_history_id: active_history_id,
            };
            let result = history.preview_audio_cleanup(request.clone()).await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.storage_cleanup_busy = false;
                match result {
                    Ok(report) if report.eligible_count > 0 || report.removed_count > 0 || report.missing_marked_count > 0 => {
                        shell.settings_modal.storage_cleanup_pending = Some(request);
                        shell.global_dialog.show(
                            "Clean History Audio?",
                            format!("This removes managed history audio only. Transcript text and history records remain.\\n\\n{}", storage_cleanup_summary(&report)),
                            crate::views::dialogs::global_dialog::DialogKind::Confirm,
                        );
                    }
                    Ok(report) => shell.settings_modal.storage_cleanup_result = Some(storage_cleanup_summary(&report)),
                    Err(error) => shell.global_dialog.show("Audio Cleanup Failed", error, crate::views::dialogs::global_dialog::DialogKind::Alert),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    pub(crate) fn confirm_storage_cleanup(&mut self, cx: &mut Context<MainShell>) {
        let Some(request) = self.storage_cleanup_pending.take() else {
            return;
        };
        self.storage_cleanup_busy = true;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let this = cx.entity();
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let result = services.history.cleanup_audio(request).await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.storage_cleanup_busy = false;
                match result {
                    Ok(report) => {
                        shell.settings_modal.storage_cleanup_result =
                            Some(storage_cleanup_summary(&report));
                        shell.settings_modal.load_storage_state(cx);
                    }
                    Err(error) => shell.global_dialog.show(
                        "Audio Cleanup Failed",
                        error,
                        crate::views::dialogs::global_dialog::DialogKind::Alert,
                    ),
                }
                cx.notify();
            })
        })
        .detach();
        cx.notify();
    }

    fn choose_storage_directory(&mut self, data_directory: bool, cx: &mut Context<MainShell>) {
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let this = cx.entity();
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let selected = rfd::AsyncFileDialog::new().pick_folder().await
                .map(|handle| handle.path().to_path_buf());
            let Some(path) = selected else { return };
            if let Err(error) = crate::platform::storage::location::check_storage_can_migrate(&services).await {
                this.update(cx, |shell, cx| {
                    shell.global_dialog.show("Storage Migration Unavailable", error, crate::views::dialogs::global_dialog::DialogKind::Alert);
                    cx.notify();
                });
                return;
            }
            this.update(cx, |shell, cx| {
                let path = path.to_string_lossy().into_owned();
                let title = if data_directory { "Change Data Directory?" } else { "Change Models Directory?" };
                let body = if data_directory {
                    format!("Switch data directory to:\n{path}\n\nSona will migrate files and reload its database connections.")
                } else {
                    format!("Switch model storage directory to:\n{path}\n\nSona will move model files and use the new directory for future downloads and recognition.")
                };
                shell.settings_modal.storage_pending_migration = Some((data_directory, path));
                shell.global_dialog.show(title, body, crate::views::dialogs::global_dialog::DialogKind::Confirm);
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn confirm_storage_migration(&mut self, cx: &mut Context<MainShell>) {
        let Some((data_directory, path)) = self.storage_pending_migration.take() else {
            return;
        };
        self.storage_migration_busy = true;
        self.storage_migration_progress = None;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let mut events = cx.global::<crate::app::GlobalEventBus>().subscribe();
        let this = cx.entity();
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let target = PathBuf::from(path);
            let migration = async {
                if data_directory {
                    crate::platform::storage::location::migrate_data_directory(&services, target, true).await
                } else {
                    crate::platform::storage::location::set_models_directory(&services, target, true).await
                }
            };
            let mut migration = std::pin::pin!(migration);
            let result = loop {
                tokio::select! {
                    result = &mut migration => break result,
                    event = events.recv() => {
                        if let Ok(event) = event
                            && event.name == crate::platform::storage::location::STORAGE_MIGRATION_PROGRESS_EVENT
                            && let Ok(progress) = serde_json::from_value::<crate::platform::storage::location::StorageMigrationProgress>(event.payload)
                        {
                            this.update(cx, |shell, cx| {
                                shell.settings_modal.storage_migration_progress = Some(progress);
                                cx.notify();
                            });
                        }
                    }
                }
            };
            this.update(cx, |shell, cx| {
                shell.settings_modal.storage_migration_busy = false;
                match result {
                    Ok(directories) => {
                        shell.settings_modal.storage_directories = Some(directories);
                        shell.settings_modal.storage_pending_migration = None;
                        shell.settings_modal.load_storage_state(cx);
                    }
                    Err(error) => {
                        shell.settings_modal.storage_error = Some(error.clone());
                        shell.global_dialog.show("Storage Migration Failed", error, crate::views::dialogs::global_dialog::DialogKind::Alert);
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn reset_storage_directory(&mut self, data_directory: bool, cx: &mut Context<MainShell>) {
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let Some(directories) = self.storage_directories.as_ref() else {
            return;
        };
        let target = if data_directory {
            directories.default_data_dir.clone()
        } else {
            directories.default_models_dir.clone()
        };
        let title = if data_directory {
            "Restore Default Data Directory?"
        } else {
            "Restore Default Models Directory?"
        };
        let this = cx.entity();
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            match crate::platform::storage::location::check_storage_can_migrate(&services).await {
                Err(error) => {
                    this.update(cx, |shell, cx| {
                        shell.global_dialog.show(
                            "Storage Migration Unavailable",
                            error,
                            crate::views::dialogs::global_dialog::DialogKind::Alert,
                        );
                        cx.notify();
                    });
                }
                Ok(()) => this.update(cx, |shell, cx| {
                    shell.settings_modal.storage_pending_reset = Some(data_directory);
                    shell.global_dialog.show(
                        title,
                        format!("Restore the default storage location?\\n\\n{target}"),
                        crate::views::dialogs::global_dialog::DialogKind::Confirm,
                    );
                    cx.notify();
                }),
            }
        })
        .detach();
    }

    pub(crate) fn confirm_storage_directory_reset(&mut self, cx: &mut Context<MainShell>) {
        let Some(data_directory) = self.storage_pending_reset.take() else {
            return;
        };
        self.storage_migration_busy = true;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let this = cx.entity();
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let result = if data_directory {
                crate::platform::storage::location::reset_data_directory(&services).await
            } else {
                crate::platform::storage::location::reset_models_directory(&services).await
            };
            this.update(cx, |shell, cx| {
                shell.settings_modal.storage_migration_busy = false;
                match result {
                    Ok(directories) => {
                        shell.settings_modal.storage_directories = Some(directories);
                        shell.settings_modal.load_storage_state(cx);
                    }
                    Err(error) => shell.global_dialog.show(
                        "Storage Location Reset Failed",
                        error,
                        crate::views::dialogs::global_dialog::DialogKind::Alert,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn load_automation_state(&mut self, cx: &mut Context<MainShell>) {
        self.automation_loading = true;
        self.automation_error = None;
        let sqlite = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sqlite
            .clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result =
                crate::platform::repositories::automation::load_repository_state(&sqlite).await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.automation_loading = false;
                match result {
                    Ok(state) => shell.settings_modal.automation_state = Some(state),
                    Err(error) => shell.settings_modal.automation_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_automation_rule_enabled(
        &mut self,
        rule_id: String,
        enabled: bool,
        cx: &mut Context<MainShell>,
    ) {
        let Some(state) = self.automation_state.clone() else {
            return;
        };
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let converted = state
            .rules
            .into_iter()
            .map(|rule| {
                let mut value = serde_json::to_value(rule).map_err(|error| error.to_string())?;
                if value.get("id").and_then(Value::as_str) == Some(rule_id.as_str()) {
                    value["enabled"] = Value::Bool(enabled);
                }
                serde_json::from_value(value).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<sona_core::automation::repository::AutomationRuleInput>, String>>(
            );
        let Ok(rules) = converted else {
            return;
        };
        self.automation_loading = true;
        self.automation_error = None;
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = async {
                crate::platform::repositories::automation::persist_rules(&services.sqlite, rules)
                    .await?;
                crate::platform::repositories::automation::load_repository_state(&services.sqlite)
                    .await
            }
            .await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.automation_loading = false;
                match result {
                    Ok(state) => shell.settings_modal.automation_state = Some(state),
                    Err(error) => shell.settings_modal.automation_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_automation_profile_dependency(
        &mut self,
        dependency: &'static str,
        dependency_id: String,
        cx: &mut Context<MainShell>,
    ) {
        let sqlite = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sqlite
            .clone();
        self.automation_loading = true;
        self.automation_error = None;
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = async {
                let state =
                    crate::platform::repositories::automation::load_repository_state(&sqlite)
                        .await?;
                let profiles = state
                    .profiles
                    .into_iter()
                    .map(|profile| {
                        let mut input: sona_core::automation::repository::AutomationProfileInput =
                            serde_json::from_value(
                                serde_json::to_value(profile).map_err(|error| error.to_string())?,
                            )
                            .map_err(|error| error.to_string())?;
                        let value = match dependency {
                            "polishPreset" if input.polish_preset_id == dependency_id => {
                                input.polish_preset_id =
                                    sona_core::config::DEFAULT_POLISH_PRESET_ID.to_string();
                                true
                            }
                            "summaryTemplate" if input.summary_template_id == dependency_id => {
                                input.summary_template_id =
                                    sona_core::config::DEFAULT_SUMMARY_TEMPLATE_ID.to_string();
                                true
                            }
                            "speakerProfile"
                                if input.enabled_speaker_profile_ids.contains(&dependency_id) =>
                            {
                                input
                                    .enabled_speaker_profile_ids
                                    .retain(|profile_id| profile_id != &dependency_id);
                                true
                            }
                            _ => false,
                        };
                        Ok::<_, String>((input, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let changed = profiles.iter().any(|(_, changed)| *changed);
                if changed {
                    crate::platform::repositories::automation::persist_profiles(
                        &sqlite,
                        profiles.into_iter().map(|(profile, _)| profile).collect(),
                    )
                    .await?;
                }
                crate::platform::repositories::automation::load_repository_state(&sqlite).await
            }
            .await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.automation_loading = false;
                match result {
                    Ok(state) => shell.settings_modal.automation_state = Some(state),
                    Err(error) => shell.settings_modal.automation_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_sync_state(&mut self, cx: &mut Context<MainShell>) {
        self.sync_loading = true;
        self.sync_error = None;
        let sync = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sync
            .clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = async {
                let status = sync.get_status().await?;
                let conflicts = sync.list_conflicts().await?;
                Ok::<_, String>((status, conflicts))
            }
            .await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.sync_loading = false;
                match result {
                    Ok((status, conflicts)) => {
                        shell.settings_modal.sync_status = Some(status);
                        shell.settings_modal.sync_conflicts = conflicts;
                    }
                    Err(error) => shell.settings_modal.sync_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn run_sync_action(&mut self, action: SyncAction, cx: &mut Context<MainShell>) {
        if self.sync_busy {
            return;
        }
        self.sync_busy = true;
        self.sync_error = None;
        let sync = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sync
            .clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = match action {
                SyncAction::Run => sync.run_now().await.map(|_| ()),
                SyncAction::Pause(paused) => sync.set_paused(paused).await.map(|_| ()),
                SyncAction::Lock => sync.lock().await.map(|_| ()),
                SyncAction::Resolve(conflict_id, resolution) => {
                    sync.resolve_conflict(&conflict_id, resolution).await
                }
            };
            let refreshed = if result.is_ok() {
                Some((sync.get_status().await, sync.list_conflicts().await))
            } else {
                None
            };
            this.update(cx, |shell, cx| {
                shell.settings_modal.sync_busy = false;
                match (result, refreshed) {
                    (Err(error), _) => shell.settings_modal.sync_error = Some(error),
                    (Ok(()), Some((Ok(status), Ok(conflicts)))) => {
                        shell.settings_modal.sync_status = Some(status);
                        shell.settings_modal.sync_conflicts = conflicts;
                    }
                    (Ok(()), Some((Err(error), _))) | (Ok(()), Some((_, Err(error)))) => {
                        shell.settings_modal.sync_error = Some(error);
                    }
                    _ => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_mcp_logs(&mut self, cx: &mut Context<MainShell>) {
        self.mcp_logs_loading = true;
        self.mcp_logs_loaded = true;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let logs = services.agent_control.get_activity_logs().await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.mcp_logs = logs;
                shell.settings_modal.mcp_logs_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn retry_dashboard(&mut self, cx: &mut Context<MainShell>) {
        self.dashboard_load_attempted = false;
        self.dashboard_snapshot = None;
        self.dashboard_error = None;
        self.load_dashboard(cx);
    }

    fn load_dashboard(&mut self, cx: &mut Context<MainShell>) {
        let context = match cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sqlite
            .current_context()
        {
            Ok(context) => context,
            Err(error) => {
                self.dashboard_error = Some(error);
                self.dashboard_load_attempted = true;
                return;
            }
        };
        let app_local_data_dir = context.app_data_dir().to_path_buf();
        self.dashboard_request_generation += 1;
        let generation = self.dashboard_request_generation;
        self.dashboard_loading = true;
        self.dashboard_deep_loading = false;
        self.dashboard_error = None;
        self.dashboard_load_attempted = true;

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let fast_snapshot = sona_sqlite::load_dashboard_snapshot(
                app_local_data_dir.clone(),
                false,
                dashboard_snapshot_time(),
            )
            .await
            .map_err(|error| error.to_string());
            let has_items = fast_snapshot
                .as_ref()
                .is_ok_and(|snapshot| snapshot.content.overview.item_count > 0);
            let fast_ok = fast_snapshot.is_ok();
            this.update(cx, |shell, cx| {
                if shell.settings_modal.dashboard_request_generation != generation {
                    return;
                }
                shell.settings_modal.dashboard_loading = false;
                match fast_snapshot {
                    Ok(snapshot) => shell.settings_modal.dashboard_snapshot = Some(snapshot),
                    Err(error) => shell.settings_modal.dashboard_error = Some(error),
                }
                shell.settings_modal.dashboard_deep_loading = fast_ok && has_items;
                cx.notify();
            })
            .ok();

            if !has_items {
                return;
            }

            let deep_snapshot = sona_sqlite::load_dashboard_snapshot(
                app_local_data_dir,
                true,
                dashboard_snapshot_time(),
            )
            .await
            .map_err(|error| error.to_string());
            this.update(cx, |shell, cx| {
                if shell.settings_modal.dashboard_request_generation != generation {
                    return;
                }
                shell.settings_modal.dashboard_deep_loading = false;
                match deep_snapshot {
                    Ok(snapshot) => shell.settings_modal.dashboard_snapshot = Some(snapshot),
                    Err(error) => shell.settings_modal.dashboard_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn check_about_updates(&mut self, cx: &mut Context<MainShell>) {
        if matches!(
            self.about_update_status,
            AboutUpdateStatus::Checking
                | AboutUpdateStatus::Downloading
                | AboutUpdateStatus::Installing
        ) {
            return;
        }
        let channel = match env!("SONA_BUILD_CHANNEL") {
            "stable" => UpdateChannel::Stable,
            "nightly" => UpdateChannel::Nightly,
            _ => return,
        };
        self.about_update_status = AboutUpdateStatus::Checking;
        self.about_update_result = None;
        self.about_update_progress = 0.0;
        self.about_installed_app = None;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let config = services
                .config
                .load()
                .ok()
                .flatten()
                .unwrap_or_else(|| json!({}));
            let mirror = sona_model_downloads::parse_download_mirror(
                config
                    .get("modelDownloadMirror")
                    .and_then(Value::as_str)
                    .unwrap_or("auto"),
            );
            let result = sona_model_downloads::check_app_update(
                services.downloads.client(),
                None,
                channel,
                mirror,
            )
            .await
            .map_err(|error| error.to_string());
            this.update(cx, |shell, cx| {
                match result {
                    Ok(result) => {
                        shell.settings_modal.about_update_status = if result.update_available {
                            AboutUpdateStatus::Available
                        } else {
                            AboutUpdateStatus::UpToDate
                        };
                        shell.settings_modal.about_update_result = Some(result);
                    }
                    Err(error) => {
                        shell.settings_modal.about_update_status = AboutUpdateStatus::Error(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn install_about_update(&mut self, cx: &mut Context<MainShell>) {
        if self.about_update_status != AboutUpdateStatus::Available {
            return;
        }
        let Some(update) = self.about_update_result.clone() else {
            self.about_update_status = AboutUpdateStatus::Error(
                "The update information is no longer available. Check again.".into(),
            );
            cx.notify();
            return;
        };
        let context = match cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sqlite
            .current_context()
        {
            Ok(context) => context,
            Err(error) => {
                self.about_update_status = AboutUpdateStatus::Error(error);
                cx.notify();
                return;
            }
        };
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        let client = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .downloads
            .client()
            .clone();
        let update_dir = context.app_data_dir().join("updates");
        let version = safe_update_version(&update.latest_version);
        let package_path = update_dir.join(format!("sona-{version}{}", update_package_extension()));
        let running_exe = std::env::current_exe().unwrap_or_default();
        let update = update.clone();
        self.about_update_status = AboutUpdateStatus::Downloading;
        self.about_update_progress = 0.0;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let config = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            let mirror = sona_model_downloads::parse_download_mirror(
                config
                    .get("modelDownloadMirror")
                    .and_then(Value::as_str)
                    .unwrap_or("auto"),
            );
            let cancel = Arc::new(Notify::new());
            let (progress_tx, mut progress_rx) = watch::channel((0_u64, 0_u64));
            let progress_callback: Box<dyn FnMut(u64, u64) + Send> = Box::new(move |current, total| {
                progress_tx.send_replace((current, total));
            });
            let download = sona_model_downloads::download_update_package(
                &client,
                &update.asset_url,
                &package_path,
                &update.signature,
                None,
                mirror,
                cancel,
                Some(progress_callback),
            );
            tokio::pin!(download);
            let download_result = loop {
                tokio::select! {
                    result = &mut download => break result.map_err(|error| error.to_string()),
                    changed = progress_rx.changed() => {
                        if changed.is_ok() {
                            let (current, total) = *progress_rx.borrow_and_update();
                            let progress = if total == 0 {
                                0.0
                            } else {
                                (current as f64 / total as f64 * 100.0) as f32
                            };
                            this.update(cx, |shell, cx| {
                                shell.settings_modal.about_update_progress = progress.clamp(0.0, 100.0);
                                cx.notify();
                            }).ok();
                        }
                    }
                }
            };

            if let Err(error) = download_result {
                this.update(cx, |shell, cx| {
                    shell.settings_modal.about_update_status = AboutUpdateStatus::Error(error);
                    shell.settings_modal.about_update_progress = 0.0;
                    cx.notify();
                }).ok();
                return;
            }

            let _ = this.update(cx, |shell, cx| {
                shell.settings_modal.about_update_status = AboutUpdateStatus::Installing;
                shell.settings_modal.about_update_progress = 100.0;
                cx.notify();
            });

            let install_package_path = package_path.clone();
            let install = tokio::task::spawn_blocking(move || {
                sona_model_downloads::install_update_package(
                    &install_package_path,
                    &running_exe,
                    false,
                )
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result);
            this.update(cx, |shell, cx| {
                match install {
                    Ok(installed_app) => {
                        shell.settings_modal.about_installed_app = Some(installed_app);
                        shell.settings_modal.about_update_status = AboutUpdateStatus::Downloaded;
                        shell.settings_modal.about_update_progress = 100.0;
                    }
                    Err(error) => {
                        shell.settings_modal.about_update_status = AboutUpdateStatus::Error(error);
                        shell.settings_modal.about_update_progress = 0.0;
                    }
                }
                cx.notify();
            }).ok();
        })
        .detach();
    }

    fn relaunch_about_update(&mut self, cx: &mut Context<MainShell>) {
        if self.about_update_status != AboutUpdateStatus::Downloaded {
            return;
        }
        let Some(installed_app) = self.about_installed_app.clone() else {
            self.about_update_status = AboutUpdateStatus::Error(
                "The installed application could not be located. Open the release page to continue.".into(),
            );
            cx.notify();
            return;
        };
        match spawn_updated_app(&installed_app) {
            Ok(()) => cx.quit(),
            Err(error) => {
                self.about_update_status = AboutUpdateStatus::Error(error);
                cx.notify();
            }
        }
    }

    fn load_cuda_addon_status(&mut self, cx: &mut Context<MainShell>) {
        if self.cuda_addon_loading {
            return;
        }
        self.cuda_addon_loading = true;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = services
                .sqlite
                .current_context()
                .map_err(|error| error.to_string())
                .map(|context| {
                    crate::platform::models::downloads::get_cuda_addon_status(
                        context.app_data_dir(),
                    )
                })
                .and_then(|result| result);
            this.update(cx, |shell, cx| {
                shell.settings_modal.cuda_addon_loading = false;
                match result {
                    Ok(status) => {
                        shell.settings_modal.cuda_addon_status = Some(status);
                        shell.settings_modal.cuda_addon_error = None;
                    }
                    Err(error) => shell.settings_modal.cuda_addon_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn begin_cuda_addon_download(&mut self, cx: &mut Context<MainShell>) {
        if self.cuda_addon_downloading {
            return;
        }
        self.cuda_addon_downloading = true;
        self.cuda_addon_progress = 0.0;
        self.cuda_addon_error = None;
        let download_id = format!("settings-cuda-{}", uuid::Uuid::new_v4());
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let mut events = cx.global::<crate::app::GlobalEventBus>().0.subscribe();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let base_dir = services
                .sqlite
                .current_context()
                .map(|context| context.app_data_dir().to_path_buf())
                .map_err(|error| error.to_string());
            let result = if let Ok(base_dir) = base_dir {
                let config = services.config.load().ok().flatten().unwrap_or_else(|| json!({}));
                let mirror = config
                    .get("modelDownloadMirror")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let download = crate::platform::models::downloads::download_and_install_cuda_addon(
                    &base_dir,
                    Some(services.emitter.clone()),
                    &services.downloads,
                    download_id.clone(),
                    mirror,
                    None,
                    None,
                    None,
                );
                tokio::pin!(download);
                loop {
                    tokio::select! {
                        result = &mut download => break result,
                        event = events.recv() => {
                            let Ok(event) = event else { continue };
                            if event.name != "download-progress" { continue; }
                            let Some(values) = event.payload.as_array() else { continue };
                            if values.get(2).and_then(Value::as_str) != Some(download_id.as_str()) { continue; }
                            let downloaded = values.first().and_then(Value::as_f64).unwrap_or_default();
                            let total = values.get(1).and_then(Value::as_f64).unwrap_or_default();
                            let progress = if total > 0.0 { (downloaded / total * 100.0) as f32 } else { 0.0 };
                            this.update(cx, |shell, cx| {
                                shell.settings_modal.cuda_addon_progress = progress.clamp(0.0, 100.0);
                                cx.notify();
                            }).ok();
                        }
                    }
                }
            } else {
                Err(base_dir.unwrap_err())
            };
            this.update(cx, |shell, cx| {
                shell.settings_modal.cuda_addon_downloading = false;
                match result {
                    Ok(status) => {
                        shell.settings_modal.cuda_addon_status = Some(status);
                        shell.settings_modal.persist_model_setting(
                            "gpuAcceleration".to_string(),
                            Value::String("cuda".to_string()),
                            cx,
                        );
                        shell.global_dialog.show(
                            "CUDA Add-on Installed",
                            "The CUDA runtime is ready to use.",
                            crate::views::dialogs::global_dialog::DialogKind::Alert,
                        );
                    }
                    Err(error) => {
                        shell.settings_modal.cuda_addon_error = Some(error.clone());
                        shell.global_dialog.show(
                            "CUDA Add-on Download Failed",
                            error,
                            crate::views::dialogs::global_dialog::DialogKind::Alert,
                        );
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn load_model_config(&mut self, cx: &mut Context<MainShell>) {
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let loaded = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            this.update(cx, |shell, cx| {
                shell.settings_modal.model_config = loaded;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn persist_model_setting(
        &mut self,
        key: String,
        value: Value,
        cx: &mut Context<MainShell>,
    ) {
        if !self.model_config.is_object() {
            self.model_config = json!({});
        }
        self.model_config[key.as_str()] = value.clone();
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            stored[key.as_str()] = value;
            let _ = config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    pub(super) fn persist_model_settings(
        &mut self,
        settings: Vec<(String, Value)>,
        cx: &mut Context<MainShell>,
    ) {
        if !self.model_config.is_object() {
            self.model_config = json!({});
        }
        for (key, value) in &settings {
            self.model_config[key] = value.clone();
        }
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            for (key, value) in settings {
                stored[key] = value;
            }
            let _ = config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    fn ensure_llm_settings(&mut self) {
        if !self.model_config.is_object() {
            self.model_config = json!({});
        }
        if !self
            .model_config
            .get("llmSettings")
            .is_some_and(Value::is_object)
        {
            self.model_config["llmSettings"] = json!({
                "activeProvider": "google_translate_free",
                "customProviders": {},
                "providers": {
                    "google_translate_free": {
                        "apiHost": "https://translate.googleapis.com/translate_a/single",
                        "apiKey": ""
                    }
                },
                "models": {},
                "modelOrder": [],
                "selections": {}
            });
        }
    }

    fn persist_llm_settings(&mut self, cx: &mut Context<MainShell>) {
        let value = self
            .model_config
            .get("llmSettings")
            .cloned()
            .unwrap_or_else(|| json!({}));
        self.persist_model_setting("llmSettings".to_string(), value, cx);
    }

    pub(super) fn set_llm_provider_field(
        &mut self,
        field: &'static str,
        value: String,
        cx: &mut Context<MainShell>,
    ) {
        self.ensure_llm_settings();
        let provider = self
            .model_config
            .pointer("/llmSettings/activeProvider")
            .and_then(Value::as_str)
            .unwrap_or("google_translate_free")
            .to_string();
        if !self.model_config["llmSettings"]["providers"].is_object() {
            self.model_config["llmSettings"]["providers"] = json!({});
        }
        if !self.model_config["llmSettings"]["providers"][&provider].is_object() {
            self.model_config["llmSettings"]["providers"][&provider] = json!({});
        }
        self.model_config["llmSettings"]["providers"][&provider][field] =
            Value::String(value.clone());

        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if !stored.get("llmSettings").is_some_and(Value::is_object) {
                stored["llmSettings"] = json!({});
            }
            if !stored["llmSettings"]["providers"].is_object() {
                stored["llmSettings"]["providers"] = json!({});
            }
            if !stored["llmSettings"]["providers"][&provider].is_object() {
                stored["llmSettings"]["providers"][&provider] = json!({});
            }
            stored["llmSettings"]["providers"][&provider][field] = Value::String(value);
            let _ = config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    fn set_llm_active_provider(&mut self, provider: String, cx: &mut Context<MainShell>) {
        self.ensure_llm_settings();
        if !self.model_config["llmSettings"]["providers"].is_object() {
            self.model_config["llmSettings"]["providers"] = json!({});
        }
        if !self.model_config["llmSettings"]["providers"][&provider].is_object() {
            self.model_config["llmSettings"]["providers"][&provider] = json!({
                "apiHost": llm_provider_default_host(&provider),
                "apiKey": ""
            });
        }
        self.model_config["llmSettings"]["activeProvider"] = Value::String(provider);
        self.persist_llm_settings(cx);
    }

    fn active_llm_provider_values(&self) -> (String, String) {
        let provider = self
            .model_config
            .pointer("/llmSettings/activeProvider")
            .and_then(Value::as_str)
            .unwrap_or("google_translate_free");
        let setting = self
            .model_config
            .pointer(&format!("/llmSettings/providers/{provider}"));
        let host = setting
            .and_then(|value| value.get("apiHost"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| llm_provider_default_host(provider).to_string());
        let key = setting
            .and_then(|value| value.get("apiKey"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        (host, key)
    }

    fn set_llm_active_feature(&mut self, feature: &str) {
        if matches!(feature, "polish" | "translation" | "summary") {
            self.llm_active_feature = feature.to_string();
        }
    }

    pub(super) fn set_llm_model_draft(&mut self, value: String) {
        self.llm_model_draft = value;
    }

    pub(super) fn set_llm_custom_provider_name(&mut self, value: String) {
        self.llm_custom_provider_name = value;
    }

    fn set_llm_custom_provider_strategy(&mut self, value: &str) {
        if matches!(
            value,
            "openai_compatible" | "openai_responses" | "anthropic" | "gemini"
        ) {
            self.llm_custom_provider_strategy = value.to_string();
        }
    }

    fn set_llm_edit_provider_strategy(&mut self, value: &str) {
        if matches!(
            value,
            "openai_compatible" | "openai_responses" | "anthropic" | "gemini"
        ) {
            self.llm_edit_provider_strategy = value.to_string();
        }
    }

    fn add_llm_custom_provider(&mut self, cx: &mut Context<MainShell>) {
        let name = self.llm_custom_provider_name.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.ensure_llm_settings();
        let base = llm_slug(&name);
        let base = if base.is_empty() {
            "provider"
        } else {
            base.as_str()
        };
        let mut provider_id = format!("custom-{base}");
        let mut suffix = 2;
        while self
            .model_config
            .pointer("/llmSettings/customProviders")
            .and_then(|providers| providers.get(&provider_id))
            .is_some()
        {
            provider_id = format!("custom-{base}-{suffix}");
            suffix += 1;
        }
        if !self.model_config["llmSettings"]["customProviders"].is_object() {
            self.model_config["llmSettings"]["customProviders"] = json!({});
        }
        self.model_config["llmSettings"]["customProviders"][&provider_id] = json!({
            "id": provider_id,
            "name": name,
            "strategy": self.llm_custom_provider_strategy,
            "createdAt": chrono::Utc::now().to_rfc3339()
        });
        self.model_config["llmSettings"]["providers"][&provider_id] = json!({
            "apiHost": llm_custom_provider_default_host(&self.llm_custom_provider_strategy),
            "apiKey": ""
        });
        self.model_config["llmSettings"]["activeProvider"] = Value::String(provider_id);
        self.llm_custom_provider_name.clear();
        self.llm_show_custom_provider_form = false;
        self.persist_llm_settings(cx);
    }

    fn save_llm_custom_provider_edit(&mut self, cx: &mut Context<MainShell>) {
        let Some(provider_id) = self.llm_editing_provider.take() else {
            return;
        };
        let name = self.llm_custom_provider_name.trim().to_string();
        if name.is_empty() {
            self.llm_editing_provider = Some(provider_id);
            return;
        }
        let Some(provider) = self
            .model_config
            .pointer_mut(&format!("/llmSettings/customProviders/{provider_id}"))
        else {
            return;
        };
        let old_strategy = provider
            .get("strategy")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        provider["name"] = Value::String(name);
        provider["strategy"] = Value::String(self.llm_edit_provider_strategy.clone());
        if old_strategy != self.llm_edit_provider_strategy
            && let Some(setting) = self
                .model_config
                .pointer_mut(&format!("/llmSettings/providers/{provider_id}"))
                .and_then(Value::as_object_mut)
        {
            setting.remove("apiPath");
            setting.remove("apiVersion");
        }
        self.llm_custom_provider_name.clear();
        self.persist_llm_settings(cx);
    }

    pub(crate) fn confirm_delete_llm_custom_provider(&mut self, cx: &mut Context<MainShell>) {
        let Some(provider_id) = self.llm_pending_delete_provider.take() else {
            return;
        };
        if !remove_custom_provider_settings(&mut self.model_config["llmSettings"], &provider_id) {
            return;
        }
        self.llm_provider_test_states.remove(&provider_id);
        if self.llm_discovery_provider.as_deref() == Some(provider_id.as_str()) {
            self.llm_discovery_generation += 1;
            self.llm_discovery_provider = None;
            self.llm_discovery_loading = false;
            self.llm_discovery_error = None;
            self.llm_discovered_models.clear();
        }
        self.persist_llm_settings(cx);
    }

    fn add_llm_model(&mut self, cx: &mut Context<MainShell>) {
        let model = self.llm_model_draft.trim().to_string();
        if model.is_empty() {
            return;
        }
        self.ensure_llm_settings();
        let provider = self
            .model_config
            .pointer("/llmSettings/activeProvider")
            .and_then(Value::as_str)
            .unwrap_or("google_translate_free")
            .to_string();
        let base = llm_slug(&model);
        let base = if base.is_empty() {
            "model"
        } else {
            base.as_str()
        };
        let mut id = format!("{provider}-{base}");
        let mut suffix = 2;
        while self
            .model_config
            .pointer(&format!("/llmSettings/models/{id}"))
            .is_some()
        {
            id = format!("{provider}-{base}-{suffix}");
            suffix += 1;
        }
        if !self.model_config["llmSettings"]["models"].is_object() {
            self.model_config["llmSettings"]["models"] = json!({});
        }
        self.model_config["llmSettings"]["models"][&id] = json!({
            "id": id,
            "provider": provider,
            "model": model,
            "source": "manual"
        });
        if !self.model_config["llmSettings"]["modelOrder"].is_array() {
            self.model_config["llmSettings"]["modelOrder"] = json!([]);
        }
        self.model_config["llmSettings"]["modelOrder"]
            .as_array_mut()
            .expect("model order initialized")
            .push(Value::String(id.clone()));
        let selection_key = format!("{}ModelId", self.llm_active_feature);
        self.model_config["llmSettings"]["selections"][selection_key] = Value::String(id);
        self.llm_model_draft.clear();
        self.persist_llm_settings(cx);
    }

    fn add_discovered_llm_model(
        &mut self,
        provider: &str,
        summary: sona_core::llm::provider_protocol::LlmModelSummary,
        cx: &mut Context<MainShell>,
    ) {
        let model = summary.model.trim().to_string();
        if model.is_empty() {
            return;
        }
        self.ensure_llm_settings();
        let existing_id = self
            .model_config
            .pointer("/llmSettings/models")
            .and_then(Value::as_object)
            .and_then(|models| {
                models.iter().find_map(|(id, entry)| {
                    (entry.get("provider").and_then(Value::as_str) == Some(provider)
                        && entry.get("model").and_then(Value::as_str) == Some(model.as_str()))
                    .then(|| id.clone())
                })
            });
        let model_id = if let Some(id) = existing_id {
            id
        } else {
            let base = llm_slug(&model);
            let base = if base.is_empty() {
                "model"
            } else {
                base.as_str()
            };
            let mut id = format!("{provider}-{base}");
            let mut suffix = 2;
            while self
                .model_config
                .pointer(&format!("/llmSettings/models/{id}"))
                .is_some()
            {
                id = format!("{provider}-{base}-{suffix}");
                suffix += 1;
            }
            let mut metadata = serde_json::to_value(summary).unwrap_or_else(|_| json!({}));
            if let Some(object) = metadata.as_object_mut() {
                object.remove("model");
            }
            self.model_config["llmSettings"]["models"][&id] = json!({
                "id": id,
                "provider": provider,
                "model": model,
                "source": "discovered",
                "metadata": metadata
            });
            if !self.model_config["llmSettings"]["modelOrder"].is_array() {
                self.model_config["llmSettings"]["modelOrder"] = json!([]);
            }
            self.model_config["llmSettings"]["modelOrder"]
                .as_array_mut()
                .expect("model order initialized")
                .push(Value::String(id.clone()));
            id
        };
        let selection_key = format!("{}ModelId", self.llm_active_feature);
        self.model_config["llmSettings"]["selections"][selection_key] = Value::String(model_id);
        self.persist_llm_settings(cx);
    }

    fn discover_llm_models(&mut self, provider_id: String, cx: &mut Context<MainShell>) {
        let provider_setting = self
            .model_config
            .pointer(&format!("/llmSettings/providers/{provider_id}"));
        let api_host = provider_setting
            .and_then(|setting| setting.get("apiHost"))
            .and_then(Value::as_str)
            .filter(|host| !host.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| llm_provider_default_host(&provider_id));
        let api_key = provider_setting
            .and_then(|setting| setting.get("apiKey"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let custom = self
            .model_config
            .pointer(&format!(
                "/llmSettings/customProviders/{provider_id}/strategy"
            ))
            .and_then(Value::as_str);
        let strategy = custom
            .and_then(|strategy| match strategy {
                "openai_compatible" => Some(sona_core::llm::tasks::LlmProviderStrategy::OpenAi),
                "openai_responses" => {
                    Some(sona_core::llm::tasks::LlmProviderStrategy::OpenAiResponses)
                }
                "anthropic" => Some(sona_core::llm::tasks::LlmProviderStrategy::Anthropic),
                "gemini" => Some(sona_core::llm::tasks::LlmProviderStrategy::Gemini),
                _ => None,
            })
            .or_else(|| {
                let provider = serde_json::from_value::<sona_core::domain::LlmProvider>(
                    Value::String(provider_id.clone()),
                )
                .ok()?;
                Some(sona_core::llm::tasks::LlmProviderStrategy::from_provider(
                    &provider,
                ))
            });
        let Some(strategy) = strategy else {
            self.llm_discovery_error =
                Some("This provider does not support model discovery.".into());
            self.llm_discovered_models.clear();
            return;
        };
        let provider = serde_json::from_value::<sona_core::domain::LlmProvider>(Value::String(
            provider_id.clone(),
        ))
        .unwrap_or_else(|_| sona_core::domain::LlmProvider::Custom(provider_id.clone()));
        self.llm_discovery_generation += 1;
        let generation = self.llm_discovery_generation;
        self.llm_discovery_provider = Some(provider_id);
        self.llm_discovery_loading = true;
        self.llm_discovery_error = None;
        self.llm_discovered_models.clear();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let adapter = crate::integrations::llm::DesktopLlmAdapter::default();
            let result = sona_application::llm::LlmRuntimeService::new(&adapter, adapter.clone())
                .list_models(sona_core::llm::requests::LlmModelsRequest {
                    provider,
                    strategy: Some(strategy),
                    base_url: api_host,
                    api_key,
                })
                .await
                .map_err(|error| error.to_string());
            this.update(cx, |shell, cx| {
                let settings = &mut shell.settings_modal;
                if settings.llm_discovery_generation != generation {
                    return;
                }
                settings.llm_discovery_loading = false;
                match result {
                    Ok(models) => settings.llm_discovered_models = models,
                    Err(error) => settings.llm_discovery_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn test_llm_provider(&mut self, provider_id: String, cx: &mut Context<MainShell>) {
        self.test_llm_model(provider_id, None, cx);
    }

    fn test_llm_model(
        &mut self,
        provider_id: String,
        model_override: Option<String>,
        cx: &mut Context<MainShell>,
    ) {
        let state_key = model_override
            .as_ref()
            .map(|model| format!("{provider_id}::{model}"))
            .unwrap_or_else(|| provider_id.clone());
        let provider_setting = self
            .model_config
            .pointer(&format!("/llmSettings/providers/{provider_id}"));
        let api_host = provider_setting
            .and_then(|setting| setting.get("apiHost"))
            .and_then(Value::as_str)
            .filter(|host| !host.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| llm_provider_default_host(&provider_id));
        let api_key = provider_setting
            .and_then(|setting| setting.get("apiKey"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let api_path = provider_setting
            .and_then(|setting| setting.get("apiPath"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let api_version = provider_setting
            .and_then(|setting| setting.get("apiVersion"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let custom_strategy = self
            .model_config
            .pointer(&format!(
                "/llmSettings/customProviders/{provider_id}/strategy"
            ))
            .and_then(Value::as_str);
        let strategy = custom_strategy
            .and_then(|strategy| match strategy {
                "openai_compatible" => Some(sona_core::llm::tasks::LlmProviderStrategy::OpenAi),
                "openai_responses" => {
                    Some(sona_core::llm::tasks::LlmProviderStrategy::OpenAiResponses)
                }
                "anthropic" => Some(sona_core::llm::tasks::LlmProviderStrategy::Anthropic),
                "gemini" => Some(sona_core::llm::tasks::LlmProviderStrategy::Gemini),
                _ => None,
            })
            .or_else(|| {
                let provider = serde_json::from_value::<sona_core::domain::LlmProvider>(
                    Value::String(provider_id.clone()),
                )
                .ok()?;
                Some(sona_core::llm::tasks::LlmProviderStrategy::from_provider(
                    &provider,
                ))
            });
        let Some(strategy) = strategy else {
            self.llm_provider_test_states.insert(
                state_key,
                OnlineProviderTestState::Error {
                    message: "This provider does not support connection testing.".into(),
                },
            );
            cx.notify();
            return;
        };
        let provider = serde_json::from_value::<sona_core::domain::LlmProvider>(Value::String(
            provider_id.clone(),
        ))
        .unwrap_or_else(|_| sona_core::domain::LlmProvider::Custom(provider_id.clone()));
        let model = model_override.unwrap_or_else(|| {
            self.model_config
                .pointer("/llmSettings/models")
                .and_then(Value::as_object)
                .and_then(|models| {
                    models.values().find_map(|entry| {
                        (entry.get("provider").and_then(Value::as_str)
                            == Some(provider_id.as_str()))
                        .then(|| entry.get("model").and_then(Value::as_str))
                        .flatten()
                        .map(str::to_string)
                    })
                })
                .unwrap_or_else(|| llm_provider_test_model(&provider_id).to_string())
        });
        let timeout_seconds = self
            .model_config
            .get("llmRequestTimeoutSeconds")
            .and_then(Value::as_u64)
            .unwrap_or(180);
        self.llm_provider_test_states
            .insert(state_key.clone(), OnlineProviderTestState::Loading);
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let started = std::time::Instant::now();
            let adapter = crate::integrations::llm::DesktopLlmAdapter::default();
            let result = sona_application::llm::LlmRuntimeService::new(&adapter, adapter.clone())
                .complete(sona_core::llm::runtime::LlmCompletionRequest {
                    config: sona_core::llm::requests::LlmConfig {
                        provider,
                        strategy,
                        base_url: api_host,
                        api_key,
                        model,
                        api_path,
                        api_version,
                        temperature: None,
                        reasoning_enabled: None,
                        reasoning_level: None,
                        timeout_seconds: Some(timeout_seconds),
                    },
                    system_prompt: None,
                    input: "Hello, this is a connection test.".into(),
                    options: sona_core::llm::runtime::LlmCompletionOptions::default(),
                    source: Some(sona_core::llm::usage::LlmGenerateSource::ConnectionTest),
                })
                .await
                .map(|_| started.elapsed().as_millis().min(u64::MAX as u128) as u64)
                .map_err(|error| error.to_string());
            this.update(cx, |shell, cx| {
                shell.settings_modal.llm_provider_test_states.insert(
                    state_key,
                    match result {
                        Ok(latency_ms) => OnlineProviderTestState::Success { latency_ms },
                        Err(message) => OnlineProviderTestState::Error { message },
                    },
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn select_llm_model(&mut self, model_id: String, cx: &mut Context<MainShell>) {
        self.ensure_llm_settings();
        let selection_key = format!("{}ModelId", self.llm_active_feature);
        self.model_config["llmSettings"]["selections"][selection_key] = Value::String(model_id);
        self.persist_llm_settings(cx);
    }

    fn remove_llm_model(&mut self, model_id: &str, cx: &mut Context<MainShell>) {
        if remove_llm_model_settings(&mut self.model_config["llmSettings"], model_id) {
            if self.llm_editing_model.as_deref() == Some(model_id) {
                self.llm_editing_model = None;
            }
            self.persist_llm_settings(cx);
        }
    }

    fn begin_llm_model_metadata_edit(&mut self, model_id: String) {
        self.llm_model_metadata_draft = self
            .model_config
            .pointer(&format!("/llmSettings/models/{model_id}/metadata"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        self.llm_model_metadata_error = None;
        self.llm_editing_model = Some(model_id);
    }

    pub(super) fn set_llm_model_metadata_flag(&mut self, key: &str, value: bool) {
        self.llm_model_metadata_draft[key] = Value::Bool(value);
        self.llm_model_metadata_error = None;
    }

    pub(super) fn set_llm_model_metadata_field(&mut self, key: &str, value: String) {
        self.llm_model_metadata_draft[key] = Value::String(value);
        self.llm_model_metadata_error = None;
    }

    pub(super) fn set_llm_model_metadata_modality(
        &mut self,
        field: &str,
        modality: &str,
        checked: bool,
    ) {
        if !matches!(field, "inputModalities" | "outputModalities")
            || !matches!(modality, "text" | "image" | "audio" | "video" | "pdf")
        {
            return;
        }
        if !self.llm_model_metadata_draft[field].is_array() {
            self.llm_model_metadata_draft[field] = json!([]);
        }
        let modalities = self.llm_model_metadata_draft[field]
            .as_array_mut()
            .expect("metadata modality array initialized");
        if checked {
            if !modalities
                .iter()
                .any(|value| value.as_str() == Some(modality))
            {
                modalities.push(Value::String(modality.to_string()));
            }
        } else {
            modalities.retain(|value| value.as_str() != Some(modality));
        }
    }

    fn save_llm_model_metadata(&mut self, metadata: Value, cx: &mut Context<MainShell>) {
        let Some(model_id) = self.llm_editing_model.take() else {
            return;
        };
        let Some(model) = self
            .model_config
            .pointer_mut(&format!("/llmSettings/models/{model_id}"))
        else {
            return;
        };
        model["metadata"] = metadata;
        self.llm_model_metadata_error = None;
        self.persist_llm_settings(cx);
    }

    fn set_llm_feature_option(
        &mut self,
        feature: &str,
        option: &str,
        value: Value,
        cx: &mut Context<MainShell>,
    ) {
        if !matches!(feature, "polish" | "translation" | "summary") {
            return;
        }
        let key = match option {
            "temperature" => format!("{feature}Temperature"),
            "reasoningEnabled" => format!("{feature}ReasoningEnabled"),
            "reasoningLevel" => format!("{feature}ReasoningLevel"),
            "reasoningBudget" => format!("{feature}ReasoningBudget"),
            _ => return,
        };
        match option {
            "temperature"
                if value
                    .as_f64()
                    .is_none_or(|v| !v.is_finite() || !(0.0..=2.0).contains(&v)) =>
            {
                return;
            }
            "reasoningEnabled" if !value.is_boolean() => return,
            "reasoningLevel"
                if !value.as_str().is_some_and(|v| {
                    matches!(v, "minimal" | "low" | "medium" | "high" | "xhigh" | "max")
                }) =>
            {
                return;
            }
            "reasoningBudget" if value.as_u64().is_none_or(|v| v == 0) => return,
            _ => {}
        }
        self.ensure_llm_settings();
        if !self.model_config["llmSettings"]["selections"].is_object() {
            self.model_config["llmSettings"]["selections"] = json!({});
        }
        self.model_config["llmSettings"]["selections"][key] = value;
        self.persist_llm_settings(cx);
    }

    fn adjust_llm_timeout(&mut self, delta: i64, cx: &mut Context<MainShell>) {
        let current = self
            .model_config
            .get("llmRequestTimeoutSeconds")
            .and_then(Value::as_i64)
            .unwrap_or(180);
        let next = (current + delta).clamp(1, 3600);
        self.persist_model_setting("llmRequestTimeoutSeconds".to_string(), json!(next), cx);
    }

    pub fn set_online_provider_config_field(
        &mut self,
        provider_id: &str,
        field_name: &str,
        field_value: String,
        cx: &mut Context<MainShell>,
    ) {
        set_json_path(
            &mut self.model_config,
            &["asr", "providers", "online", provider_id, field_name],
            Value::String(field_value.clone()),
        );
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        let provider_id = provider_id.to_string();
        let field_name = field_name.to_string();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            set_json_path(
                &mut stored,
                &["asr", "providers", "online", &provider_id, &field_name],
                Value::String(field_value),
            );
            let _ = config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    fn set_online_provider_config_fields(
        &mut self,
        provider_id: &str,
        fields: &[(&str, &str)],
        cx: &mut Context<MainShell>,
    ) {
        for (field_name, field_value) in fields {
            set_json_path(
                &mut self.model_config,
                &["asr", "providers", "online", provider_id, field_name],
                Value::String((*field_value).to_string()),
            );
        }
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        let provider_id = provider_id.to_string();
        let fields = fields
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<Vec<_>>();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            for (field_name, field_value) in fields {
                set_json_path(
                    &mut stored,
                    &["asr", "providers", "online", &provider_id, &field_name],
                    Value::String(field_value),
                );
            }
            let _ = config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    pub fn set_online_provider_model_added(
        &mut self,
        provider_id: &str,
        model_id: &str,
        added: bool,
        is_default: bool,
        cx: &mut Context<MainShell>,
    ) {
        update_online_provider_model_config(
            &mut self.model_config,
            provider_id,
            model_id,
            added,
            is_default,
        );
        let persisted = self.model_config.clone();
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if let Some(stored_asr) = persisted.get("asr") {
                stored["asr"] = stored_asr.clone();
            }
            let _ = config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    fn test_online_provider_model(
        &mut self,
        provider_id: String,
        model_id: String,
        cx: &mut Context<MainShell>,
    ) {
        let state_key = format!("{provider_id}::{model_id}");
        let mut provider_config = self
            .model_config
            .pointer(&format!("/asr/providers/online/{provider_id}"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !provider_config.is_object() {
            provider_config = json!({});
        }
        provider_config["model"] = Value::String(model_id.clone());
        self.online_provider_test_states
            .insert(state_key.clone(), OnlineProviderTestState::Loading);
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = sona_online_asr::test_online_asr_provider(&provider_id, &provider_config)
                .await
                .map_err(|error| error.to_string());
            let state = match result {
                Ok(latency_ms) => OnlineProviderTestState::Success { latency_ms },
                Err(message) => OnlineProviderTestState::Error { message },
            };
            let clear_after_delay = matches!(state, OnlineProviderTestState::Success { .. });
            this.update(cx, |shell, cx| {
                shell
                    .settings_modal
                    .online_provider_test_states
                    .insert(state_key.clone(), state.clone());
                cx.notify();
            })
            .ok();

            if clear_after_delay {
                tokio::time::sleep(std::time::Duration::from_millis(3500)).await;
                this.update(cx, |shell, cx| {
                    if matches!(
                        shell
                            .settings_modal
                            .online_provider_test_states
                            .get(&state_key),
                        Some(OnlineProviderTestState::Success { .. })
                    ) {
                        shell
                            .settings_modal
                            .online_provider_test_states
                            .remove(&state_key);
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn select_asr_model(&mut self, model_id: &str, cx: &mut Context<MainShell>) {
        let batch = self.model_scenario_batch;
        let online_selection = model_id
            .split_once("::")
            .map(|(provider_id, model_id)| (provider_id, Some(model_id)))
            .or_else(|| {
                sona_core::ports::asr::find_online_asr_provider(model_id)
                    .map(|provider| (provider.id.as_str(), None))
            });
        if let Some((provider_id, model_id)) = online_selection {
            let Some(provider) = sona_core::ports::asr::find_online_asr_provider(provider_id)
            else {
                return;
            };
            let selection = json!({
                "engine": "online",
                "mode": if batch { "batch" } else { "streaming" },
                "modelId": model_id,
                "modelPath": "",
                "providerId": provider_id,
                "profileId": provider.profile_id,
            });
            if !self.model_config.is_object() {
                self.model_config = json!({});
            }
            if !self.model_config.get("asr").is_some_and(Value::is_object) {
                self.model_config["asr"] = json!({});
            }
            if !self.model_config["asr"]
                .get("selections")
                .is_some_and(Value::is_object)
            {
                self.model_config["asr"]["selections"] = json!({});
            }
            let selections = &mut self.model_config["asr"]["selections"];
            if batch {
                selections["batch"] = selection;
                self.model_config["batchModelPath"] = Value::String(String::new());
            } else {
                for slot in ["live", "caption", "voiceTyping"] {
                    selections[slot] = selection.clone();
                }
                self.model_config["streamingModelPath"] = Value::String(String::new());
            }
            let persisted = self.model_config.clone();
            self.model_selector_open = None;
            let service = cx
                .global::<crate::app::GlobalDesktopServices>()
                .0
                .config
                .clone();
            cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                let _ = service.save(&persisted);
            })
            .detach();
            cx.notify();
            return;
        }
        let snapshot = self.model_catalog.as_ref();
        let model = snapshot
            .and_then(|snapshot| snapshot.get("models"))
            .and_then(Value::as_array)
            .and_then(|models| {
                models
                    .iter()
                    .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
            });
        let path = model
            .and_then(|model| model.get("installPath"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let modes = model
            .and_then(|model| model.get("modes"))
            .and_then(Value::as_array);
        let selection = json!({
            "engine": "local",
            "mode": if batch { "batch" } else { "streaming" },
            "modelId": if model_id.is_empty() { Value::Null } else { Value::String(model_id.to_string()) },
            "modelPath": path,
        });
        let config = &mut self.model_config;
        if !config.is_object() {
            *config = json!({});
        }
        if !config.get("asr").is_some_and(Value::is_object) {
            config["asr"] = json!({});
        }
        if !config["asr"]
            .get("selections")
            .is_some_and(Value::is_object)
        {
            config["asr"]["selections"] = json!({});
        }
        let asr = &mut config["asr"]["selections"];
        if batch {
            asr["batch"] = selection.clone();
            config["batchModelPath"] = selection["modelPath"].clone();
        } else {
            for slot in ["live", "caption", "voiceTyping"] {
                asr[slot] = selection.clone();
            }
            config["streamingModelPath"] = selection["modelPath"].clone();
        }
        if !model_id.is_empty()
            && let Some(modes) = modes
        {
            for mode in modes.iter().filter_map(Value::as_str) {
                let field = match mode {
                    "live" | "streaming" => {
                        if batch {
                            None
                        } else {
                            Some("streamingModelPath")
                        }
                    }
                    "batch" => {
                        if batch {
                            Some("batchModelPath")
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(field) = field {
                    config[field] = Value::String(path.clone());
                }
            }
        }
        let persisted = self.model_config.clone();
        self.model_selector_open = None;
        let service = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = service.save(&persisted);
        })
        .detach();
        cx.notify();
    }

    fn choose_model_option(
        &mut self,
        global_dialog: &mut crate::views::dialogs::global_dialog::GlobalDialog,
        field: &'static str,
        value: String,
        cx: &mut Context<MainShell>,
    ) {
        if field == "gpu-acceleration" {
            self.model_selector_open = None;
            if value == "cuda"
                && !self
                    .cuda_addon_status
                    .as_ref()
                    .is_some_and(|status| status.is_installed)
            {
                global_dialog.show(
                    "Download CUDA Add-on",
                    "CUDA acceleration requires an additional runtime package. Download and install it now?",
                    crate::views::dialogs::global_dialog::DialogKind::Confirm,
                );
                self.cuda_addon_error = None;
                cx.notify();
                return;
            }
            self.persist_model_setting("gpuAcceleration".to_string(), Value::String(value), cx);
            return;
        }
        if field == "asr" {
            self.select_asr_model(&value, cx);
            return;
        }
        let scenario = if self.model_scenario_batch {
            "batch"
        } else {
            "live"
        };
        let config_key = match field {
            "vad" => format!("{scenario}VadModelPath"),
            "punctuation" => format!("{scenario}PunctuationModelPath"),
            "speaker-segmentation" => format!("{scenario}SpeakerSegmentationModelPath"),
            "speaker-embedding" => format!("{scenario}SpeakerEmbeddingModelPath"),
            "alignment" => format!("{scenario}AlignmentModelPath"),
            _ => return,
        };
        let path = self
            .model_catalog
            .as_ref()
            .and_then(|snapshot| snapshot.get("modelPathById"))
            .and_then(|paths| paths.get(&value))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.model_selector_open = None;
        self.persist_model_setting(config_key, Value::String(path), cx);
    }

    fn restore_model_defaults(&mut self, cx: &mut Context<MainShell>) {
        let Some(defaults) = self
            .model_catalog
            .as_ref()
            .and_then(|snapshot| snapshot.get("restoreDefaults"))
            .cloned()
        else {
            return;
        };
        if !self.model_config.is_object() {
            self.model_config = json!({});
        }
        for (config_key, default_key) in [
            ("streamingModelPath", "streamingModelPath"),
            ("batchModelPath", "batchModelPath"),
            ("liveVadModelPath", "vadModelPath"),
            ("batchVadModelPath", "vadModelPath"),
            ("livePunctuationModelPath", "punctuationModelPath"),
            ("batchPunctuationModelPath", "punctuationModelPath"),
            (
                "liveSpeakerSegmentationModelPath",
                "speakerSegmentationModelPath",
            ),
            (
                "batchSpeakerSegmentationModelPath",
                "speakerSegmentationModelPath",
            ),
            ("liveSpeakerEmbeddingModelPath", "speakerEmbeddingModelPath"),
            (
                "batchSpeakerEmbeddingModelPath",
                "speakerEmbeddingModelPath",
            ),
            ("liveAlignmentModelPath", "alignmentModelPath"),
            ("batchAlignmentModelPath", "alignmentModelPath"),
        ] {
            if let Some(value) = defaults.get(default_key) {
                self.model_config[config_key] = value.clone();
            }
        }
        for key in ["enableITN", "batchVadEnabled", "maxConcurrent"] {
            if let Some(value) = defaults.get(key) {
                self.model_config[key] = value.clone();
            }
        }
        if let Some(buffer) = defaults.get("vadBufferSize") {
            self.model_config["liveVadBufferSize"] = buffer.clone();
            self.model_config["batchVadBufferSize"] = buffer.clone();
        }
        let persisted = self.model_config.clone();
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = config.save(&persisted);
        })
        .detach();
        cx.notify();
    }

    fn load_model_catalog(&mut self, cx: &mut Context<MainShell>) {
        if self.model_catalog_loading {
            return;
        }
        self.model_catalog_loading = true;
        self.model_catalog_error = None;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result =
                crate::platform::agent_control::facade::models::get_model_catalog(&services).await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.model_catalog_loading = false;
                match result {
                    Ok(snapshot) => {
                        shell.settings_modal.model_catalog = Some(snapshot);
                        shell.settings_modal.model_catalog_error = None;
                    }
                    Err(error) => shell.settings_modal.model_catalog_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn download_model(&mut self, model_id: String, cx: &mut Context<MainShell>) {
        if self.model_download_ids.contains_key(&model_id) {
            return;
        }

        let download_id = format!("settings-{model_id}-{}", uuid::Uuid::new_v4());
        self.model_download_ids
            .insert(model_id.clone(), download_id.clone());
        self.model_download_progress.insert(model_id.clone(), 0.0);

        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let mut events = cx.global::<crate::app::GlobalEventBus>().0.subscribe();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let config = services.config.load().ok().flatten().unwrap_or_else(|| json!({}));
            let mirror = config
                .get("modelDownloadMirror")
                .and_then(Value::as_str)
                .map(str::to_string);
            let models_dir = services
                .sqlite
                .current_context()
                .map(|context| context.app_data_dir().join("models"))
                .map_err(|error| error.to_string());
            let result = if let Ok(models_dir) = models_dir {
                let cancel_notify = std::sync::Arc::new(tokio::sync::Notify::new());
                let _download_guard = services
                    .downloads
                    .register_download(download_id.clone(), cancel_notify);
                let download = crate::platform::model_downloads::download_preset_model(
                    &models_dir,
                    Some(services.emitter.clone()),
                    &services.downloads,
                    model_id.clone(),
                    download_id.clone(),
                    mirror,
                );
                tokio::pin!(download);
                loop {
                    tokio::select! {
                        result = &mut download => break result,
                        event = events.recv() => {
                            let Ok(event) = event else { continue };
                            if event.name != "download-progress" { continue; }
                            let Some(values) = event.payload.as_array() else { continue };
                            if values.get(2).and_then(Value::as_str) != Some(download_id.as_str()) { continue; }
                            let downloaded = values.first().and_then(Value::as_f64).unwrap_or_default();
                            let total = values.get(1).and_then(Value::as_f64).unwrap_or_default();
                            let progress = if total > 0.0 { (downloaded / total * 100.0) as f32 } else { 0.0 };
                            this.update(cx, |shell, cx| {
                                shell.settings_modal.model_download_progress.insert(model_id.clone(), progress.clamp(0.0, 100.0));
                                cx.notify();
                            }).ok();
                        }
                    }
                }
            } else {
                Err(models_dir.unwrap_err())
            };

            let catalog = crate::platform::agent_control::facade::models::get_model_catalog(&services).await;
            this.update(cx, |shell, cx| {
                shell.settings_modal.model_download_ids.remove(&model_id);
                shell.settings_modal.model_download_progress.remove(&model_id);
                match catalog {
                    Ok(snapshot) => shell.settings_modal.model_catalog = Some(snapshot),
                    Err(error) => shell.settings_modal.model_catalog_error = Some(error),
                }
                if let Err(error) = result {
                    shell.settings_modal.model_catalog_error = Some(error);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn cancel_model_download(&mut self, model_id: String, cx: &mut Context<MainShell>) {
        let Some(download_id) = self.model_download_ids.get(&model_id).cloned() else {
            return;
        };
        let downloads = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .downloads
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = downloads.notify_download(&download_id).await;
        })
        .detach();
    }

    fn request_model_delete(&mut self, model_id: String, model_name: String, path: String) {
        self.pending_model_delete = Some((model_id, model_name, path));
    }

    fn confirm_model_delete(&mut self, cx: &mut Context<MainShell>) {
        let Some((model_id, _model_name, path)) = self.pending_model_delete.take() else {
            return;
        };
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = crate::platform::agent_control::facade::models::delete_preset_model(
                &services, model_id,
            )
            .await;
            let catalog = if result.is_ok() {
                crate::platform::agent_control::facade::models::get_model_catalog(&services).await
            } else {
                result.as_ref().map(|_| Value::Null).map_err(Clone::clone)
            };
            this.update(cx, |shell, cx| {
                if let Err(error) = result {
                    shell.settings_modal.model_catalog_error = Some(error);
                } else if let Ok(snapshot) = catalog {
                    shell.settings_modal.model_catalog = Some(snapshot);
                    let config = services
                        .config
                        .load()
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| json!({}));
                    let mut config = config;
                    clear_model_path_references(&mut config, &path);
                    let _ = services.config.save(&config);
                } else if let Err(error) = catalog {
                    shell.settings_modal.model_catalog_error = Some(error);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn start_audio_preview(&mut self, cx: &mut Context<MainShell>) {
        if self.audio_preview_started {
            return;
        }
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        if services.audio.has_active_captures() {
            return;
        }
        self.audio_preview_started = true;
        let microphone = (self.selected_mic != "default").then(|| self.selected_mic.clone());
        let system =
            (self.selected_system_audio != "default").then(|| self.selected_system_audio.clone());
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let app_data_dir = std::env::temp_dir().join("sona");
            let mic_result = crate::integrations::audio::start_microphone_capture(
                services.audio.clone(),
                services.asr.clone(),
                services.emitter.clone(),
                app_data_dir.clone(),
                microphone,
                "test_mic".to_string(),
                None,
            )
            .await;
            let system_result = crate::integrations::audio::start_system_audio_capture(
                services.audio.clone(),
                services.asr.clone(),
                services.emitter.clone(),
                app_data_dir,
                system,
                "test_system".to_string(),
                None,
            )
            .await;
            let errors = [mic_result.err(), system_result.err()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let should_stop = this
                .update(cx, |shell, cx| {
                    if !errors.is_empty() {
                        shell.settings_modal.audio_devices_error = Some(errors.join("; "));
                    }
                    let should_stop = !shell.settings_modal.is_open
                        || shell.settings_modal.current_tab != SettingsTab::Microphone;
                    if errors.len() == 2 {
                        shell.settings_modal.audio_preview_started = false;
                    }
                    cx.notify();
                    should_stop
                })
                .unwrap_or(true);
            if should_stop {
                for path in [
                    crate::integrations::audio::stop_microphone_capture(
                        &services.audio,
                        "test_mic".to_string(),
                    )
                    .await,
                    crate::integrations::audio::stop_system_audio_capture(
                        &services.audio,
                        "test_system".to_string(),
                    )
                    .await,
                ]
                .into_iter()
                .flatten()
                {
                    let _ = tokio::fs::remove_file(path).await;
                }
            }
        })
        .detach();
    }

    fn stop_audio_preview(&mut self, cx: &mut Context<MainShell>) {
        if !self.audio_preview_started {
            return;
        }
        self.audio_preview_started = false;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            for (kind, id) in [
                (
                    crate::integrations::audio::CaptureKind::Microphone,
                    "test_mic",
                ),
                (
                    crate::integrations::audio::CaptureKind::System,
                    "test_system",
                ),
            ] {
                let result = match kind {
                    crate::integrations::audio::CaptureKind::Microphone => {
                        crate::integrations::audio::stop_microphone_capture(
                            &services.audio,
                            id.to_string(),
                        )
                        .await
                    }
                    crate::integrations::audio::CaptureKind::System => {
                        crate::integrations::audio::stop_system_audio_capture(
                            &services.audio,
                            id.to_string(),
                        )
                        .await
                    }
                };
                if let Ok(path) = result {
                    let _ = tokio::fs::remove_file(path).await;
                }
            }
        })
        .detach();
    }

    pub fn cycle_tab(&mut self, reverse: bool, cx: &mut Context<MainShell>) {
        self.set_tab(Self::next_tab(self.current_tab, reverse), cx);
    }

    fn next_tab(current_tab: SettingsTab, reverse: bool) -> SettingsTab {
        let tabs = [
            SettingsTab::General,
            SettingsTab::Dashboard,
            SettingsTab::Shortcuts,
            SettingsTab::About,
            SettingsTab::Microphone,
            SettingsTab::Models,
            SettingsTab::Subtitle,
            SettingsTab::Vocabulary,
            SettingsTab::LlmService,
            SettingsTab::Automation,
            SettingsTab::Storage,
            SettingsTab::Sync,
            SettingsTab::ApiServer,
            SettingsTab::Mcp,
        ];
        let index = tabs
            .iter()
            .position(|tab| *tab == current_tab)
            .unwrap_or_default();
        let next = if reverse {
            (index + tabs.len() - 1) % tabs.len()
        } else {
            (index + 1) % tabs.len()
        };
        tabs[next]
    }

    pub fn load_preferences(&mut self, cx: &mut Context<MainShell>) {
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let loaded = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            this.update(cx, |shell, cx| {
                shell.settings_modal.autostart = loaded
                    .get("autoStart")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                shell.settings_modal.minimize_to_tray = loaded
                    .get("minimizeToTrayOnExit")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                shell.settings_modal.silent_start = loaded
                    .get("silentStart")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                shell.settings_modal.auto_check_updates = loaded
                    .get("autoCheckUpdates")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                shell.settings_modal.selected_theme = loaded
                    .get("theme")
                    .and_then(Value::as_str)
                    .unwrap_or("auto")
                    .to_string();
                shell.settings_modal.selected_lang = loaded
                    .get("appLanguage")
                    .and_then(Value::as_str)
                    .unwrap_or("auto")
                    .to_string();
                shell.settings_modal.selected_font = loaded
                    .get("font")
                    .and_then(Value::as_str)
                    .unwrap_or("system")
                    .to_string();
                match shell.settings_modal.selected_theme.as_str() {
                    "light" => gpui_kit::component::theme::Theme::change(
                        gpui_kit::component::theme::ThemeMode::Light,
                        None,
                        cx,
                    ),
                    "dark" => gpui_kit::component::theme::Theme::change(
                        gpui_kit::component::theme::ThemeMode::Dark,
                        None,
                        cx,
                    ),
                    _ => {}
                }
                shell.settings_modal.selected_mic = loaded
                    .get("microphoneId")
                    .and_then(Value::as_str)
                    .unwrap_or("default")
                    .to_string();
                shell.settings_modal.selected_system_audio = loaded
                    .get("systemAudioDeviceId")
                    .and_then(Value::as_str)
                    .unwrap_or("default")
                    .to_string();
                shell.settings_modal.microphone_boost = loaded
                    .get("microphoneBoost")
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0)
                    .clamp(1.0, 5.0);
                shell.settings_modal.keep_microphone_active = loaded
                    .get("keepMicrophoneActive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                shell.settings_modal.mute_during_recording = loaded
                    .get("muteDuringRecording")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                shell.settings_modal.ffmpeg_enabled = loaded
                    .get("ffmpegEnabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                shell.settings_modal.ffmpeg_path = loaded
                    .get("ffmpegPath")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let shortcut = loaded
                    .get("liveRecordShortcut")
                    .and_then(Value::as_str)
                    .unwrap_or("Ctrl + Space")
                    .to_string();
                if !shell.settings_modal.is_editing_shortcut {
                    shell.settings_modal.live_record_shortcut = shortcut.clone();
                    shell.settings_modal.shortcut_draft = shortcut;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn load_audio_devices(&mut self, cx: &mut Context<MainShell>) {
        if self.audio_devices_loading {
            return;
        }
        self.audio_devices_loading = true;
        self.audio_devices_error = None;
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = tokio::task::spawn_blocking(|| {
                let microphones = crate::integrations::audio::get_microphone_devices()?;
                let system_devices = crate::integrations::audio::get_system_audio_devices()?;
                Ok::<_, String>((microphones, system_devices))
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result);
            this.update(cx, |shell, cx| {
                shell.settings_modal.audio_devices_loading = false;
                match result {
                    Ok((microphones, system_devices)) => {
                        shell.settings_modal.microphone_devices =
                            microphones.into_iter().map(|device| device.name).collect();
                        shell.settings_modal.system_audio_devices = system_devices
                            .into_iter()
                            .map(|device| device.name)
                            .collect();
                    }
                    Err(error) => shell.settings_modal.audio_devices_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn persist_audio_value(
        &mut self,
        key: &'static str,
        value: Value,
        cx: &mut Context<MainShell>,
    ) {
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            stored[key] = value;
            let _ = config.save(&stored);
        })
        .detach();
    }

    fn browse_ffmpeg(&mut self, cx: &mut Context<MainShell>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let selected = rfd::AsyncFileDialog::new()
                .add_filter("FFmpeg executable", &["exe", "*"])
                .pick_file()
                .await
                .map(|file| file.path().to_string_lossy().into_owned());
            if let Some(path) = selected {
                this.update(cx, |shell, cx| {
                    shell.settings_modal.ffmpeg_path = path.clone();
                    shell
                        .settings_modal
                        .persist_audio_value("ffmpegPath", Value::String(path), cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn reset_ffmpeg_path(&mut self, cx: &mut Context<MainShell>) {
        self.ffmpeg_path.clear();
        self.persist_audio_value("ffmpegPath", Value::String(String::new()), cx);
        cx.notify();
    }

    fn persist_preference(&mut self, key: &'static str, value: bool, cx: &mut Context<MainShell>) {
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            stored[key] = Value::Bool(value);
            if !value && matches!(key, "autoStart" | "minimizeToTrayOnExit") {
                stored["silentStart"] = Value::Bool(false);
            }
            if config.save(&stored).is_ok() && key == "autoStart" {
                let _ = crate::platform::auto_launch::set_auto_start(value);
            }
        })
        .detach();
    }

    pub(crate) fn persist_general_setting(
        &mut self,
        key: &'static str,
        value: Value,
        cx: &mut Context<MainShell>,
    ) {
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            stored[key] = value;
            let _ = config.save(&stored);
        })
        .detach();
    }

    pub fn persist_shortcut(&mut self, value: String, cx: &mut Context<MainShell>) {
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            stored["liveRecordShortcut"] = Value::String(value);
            let _ = config.save(&stored);
        })
        .detach();
    }

    pub fn commit_shortcut_edit(&mut self, value: String, cx: &mut Context<MainShell>) {
        let value = if value.trim().is_empty() {
            "Ctrl + Space".to_string()
        } else {
            value.trim().to_string()
        };
        self.live_record_shortcut = value.clone();
        self.shortcut_draft = value.clone();
        self.is_editing_shortcut = false;
        self.persist_shortcut(value, cx);
        cx.notify();
    }

    fn tab_title(tab: SettingsTab) -> &'static str {
        match tab {
            SettingsTab::General => "General",
            SettingsTab::Dashboard => "Dashboard",
            SettingsTab::Shortcuts => "Shortcuts",
            SettingsTab::Microphone => "Input Device",
            SettingsTab::Models => "Models",
            SettingsTab::Subtitle => "Subtitles & Voice Typing",
            SettingsTab::Vocabulary => "Vocabulary",
            SettingsTab::LlmService => "LLM Service",
            SettingsTab::Automation => "Automation",
            SettingsTab::Storage => "Storage",
            SettingsTab::Sync => "Cloud Sync",
            SettingsTab::About => "About",
            SettingsTab::ApiServer => "API Server",
            SettingsTab::Mcp => "MCP Agents",
        }
    }

    pub fn render(
        &mut self,
        shortcut_edit_state: &Entity<InputState>,
        model_search_state: &Entity<InputState>,
        mcp_search_state: &Entity<InputState>,
        voice_typing_tool: &mut crate::views::tools::VoiceTypingToolPanel,
        subtitles_tool: &mut crate::views::tools::SubtitlesToolPanel,
        api_server_field_states: &HashMap<String, Entity<InputState>>,
        model_engine_select: &Entity<SelectState<Vec<&'static str>>>,
        model_status_select: &Entity<SelectState<Vec<&'static str>>>,
        model_label_select: &Entity<SelectState<Vec<&'static str>>>,
        online_provider_field_states: &HashMap<String, Entity<InputState>>,
        llm_provider_api_host_state: &Entity<InputState>,
        llm_provider_api_key_state: &Entity<InputState>,
        llm_model_draft_state: &Entity<InputState>,
        llm_custom_provider_name_state: &Entity<InputState>,
        llm_model_metadata_field_states: &HashMap<String, Entity<InputState>>,
        dictionary_textarea_state: &Entity<gpui_kit::component::input::TextareaState>,
        dictionary_quick_add_state: &Entity<InputState>,
        dictionary_search_state: &Entity<InputState>,
        dictionary_test_state: &Entity<InputState>,
        vocabulary_name_state: &Entity<InputState>,
        vocabulary_body_state: &Entity<gpui_kit::component::input::TextareaState>,
        translation_language_select: &Entity<SelectState<Vec<&'static str>>>,
        general_log_level_select: &Entity<SelectState<Vec<&'static str>>>,
        general_language_select: &Entity<SelectState<Vec<&'static str>>>,
        general_font_select: &Entity<SelectState<Vec<&'static str>>>,
        projects: &[sona_core::project::ProjectRecord],
        active_history_id: Option<&str>,
        dictionary_word_state: &Entity<InputState>,
        dictionary_weight_state: &Entity<InputState>,
        dictionary_from_state: &Entity<InputState>,
        dictionary_to_state: &Entity<InputState>,
        dictionary_inline_hotword_state: &Entity<InputState>,
        speaker_profile_name_state: &Entity<InputState>,
        speaker_profile_search_state: &Entity<InputState>,
        speaker_profile_name_states: &HashMap<String, (Entity<InputState>, Subscription)>,
        window: &mut Window,
        cx: &mut Context<MainShell>,
    ) -> impl IntoElement {
        if !self.is_open {
            return div();
        }
        if !self.audio_peak_listener_started {
            self.audio_peak_listener_started = true;
            let mut events = cx.global::<crate::app::GlobalEventBus>().0.subscribe();
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                while let Ok(event) = events.recv().await {
                    let peak = event.decode::<i16>().ok();
                    if event.name == crate::integrations::audio::MICROPHONE_PEAK_EVENT {
                        if let Some(peak) = peak {
                            this.update(cx, |shell, cx| {
                                if !shell.settings_modal.is_open
                                    || shell.settings_modal.current_tab != SettingsTab::Microphone
                                {
                                    return;
                                }
                                shell.settings_modal.microphone_peak =
                                    ((peak as f32).abs() / 32767.0).clamp(0.0, 1.0);
                                cx.notify();
                            })
                            .ok();
                        }
                    } else if event.name == crate::integrations::audio::SYSTEM_PEAK_EVENT
                        && let Some(peak) = peak
                    {
                        this.update(cx, |shell, cx| {
                            if !shell.settings_modal.is_open
                                || shell.settings_modal.current_tab != SettingsTab::Microphone
                            {
                                return;
                            }
                            shell.settings_modal.system_audio_peak =
                                ((peak as f32).abs() / 32767.0).clamp(0.0, 1.0);
                            cx.notify();
                        })
                        .ok();
                    }
                }
            })
            .detach();
        }
        let viewport = window.viewport_size();
        let wide_layout = viewport.width >= px(1400.0);
        let modal_width = if wide_layout {
            (viewport.width * 0.90).min(px(1200.0))
        } else {
            (viewport.width * 0.92).min(SETTINGS_MODAL_WIDTH)
        };
        let modal_height = if viewport.height <= px(640.0) {
            viewport.height - px(8.0)
        } else {
            let preferred = (viewport.height * 0.88).min(SETTINGS_MODAL_HEIGHT);
            let minimum = (viewport.height - px(32.0)).min(px(550.0));
            preferred.max(minimum)
        };
        let sidebar_width = (viewport.width * 0.22).max(px(220.0)).min(px(280.0));
        let model_sections = self
            .model_catalog
            .as_ref()
            .and_then(|snapshot| snapshot.get("sections"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let model_search_query = self.model_search_query.trim().to_lowercase();
        let model_engine_filter = self.model_engine_filter.as_str();
        let model_status_filter = self.model_status_filter.as_str();
        let model_label_filter = self.model_label_filter.as_str();
        let mut gpu_options = vec![
            ("auto".to_string(), "Auto".to_string()),
            ("cpu".to_string(), "Off".to_string()),
            ("vulkan".to_string(), "Vulkan".to_string()),
            ("metal".to_string(), "Metal".to_string()),
        ];
        if !cfg!(target_os = "macos") {
            let cuda_label = if self
                .cuda_addon_status
                .as_ref()
                .is_some_and(|status| status.is_installed)
            {
                "CUDA (Ready)"
            } else {
                "CUDA (Not installed)"
            };
            gpu_options.push(("cuda".to_string(), cuda_label.to_string()));
        }
        let selection_options = self
            .model_catalog
            .as_ref()
            .and_then(|snapshot| snapshot.get("selectionOptions"));
        let mut asr_options = selection_options
            .and_then(|options| {
                options.get(if self.model_scenario_batch {
                    "batch"
                } else {
                    "streaming"
                })
            })
            .and_then(Value::as_array)
            .map(|options| {
                options
                    .iter()
                    .filter_map(|option| {
                        Some((
                            option.get("id")?.as_str()?.to_string(),
                            option.get("label")?.as_str()?.to_string(),
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let active_asr_selection = self.model_config.pointer(if self.model_scenario_batch {
            "/asr/selections/batch"
        } else {
            "/asr/selections/live"
        });
        let selected_online_id = active_asr_selection
            .filter(|selection| selection.get("engine").and_then(Value::as_str) == Some("online"))
            .and_then(|selection| {
                let provider_id = selection.get("providerId")?.as_str()?;
                Some(match selection.get("modelId").and_then(Value::as_str) {
                    Some(model_id) => format!("{provider_id}::{model_id}"),
                    None => provider_id.to_string(),
                })
            });
        if let Some(selected_id) = selected_online_id.as_deref() {
            asr_options.retain(|(id, _)| id != selected_id);
        }
        asr_options.extend(online_provider_model_options(
            &self.model_config,
            self.model_scenario_batch,
            selected_online_id.as_deref(),
        ));
        let companion_options = |section_type: &str| {
            model_sections
                .iter()
                .filter(|section| section.get("type").and_then(Value::as_str) == Some(section_type))
                .flat_map(|section| {
                    section
                        .get("groups")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                })
                .flat_map(|group| {
                    group
                        .get("models")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                })
                .filter(|model| model.get("isInstalled").and_then(Value::as_bool) == Some(true))
                .filter_map(|model| {
                    Some((
                        model.get("id")?.as_str()?.to_string(),
                        model.get("name")?.as_str()?.to_string(),
                    ))
                })
                .collect::<Vec<_>>()
        };
        let selected_model_id = |field: &str| {
            let scenario = if self.model_scenario_batch {
                "batch"
            } else {
                "live"
            };
            let key = match field {
                "asr" => {
                    if self.model_scenario_batch {
                        "batchModelPath"
                    } else {
                        "streamingModelPath"
                    }
                }
                "vad" => {
                    if self.model_scenario_batch {
                        "batchVadModelPath"
                    } else {
                        "liveVadModelPath"
                    }
                }
                "punctuation" => {
                    if self.model_scenario_batch {
                        "batchPunctuationModelPath"
                    } else {
                        "livePunctuationModelPath"
                    }
                }
                "speaker-segmentation" => {
                    if self.model_scenario_batch {
                        "batchSpeakerSegmentationModelPath"
                    } else {
                        "liveSpeakerSegmentationModelPath"
                    }
                }
                "speaker-embedding" => {
                    if self.model_scenario_batch {
                        "batchSpeakerEmbeddingModelPath"
                    } else {
                        "liveSpeakerEmbeddingModelPath"
                    }
                }
                "alignment" => {
                    if self.model_scenario_batch {
                        "batchAlignmentModelPath"
                    } else {
                        "liveAlignmentModelPath"
                    }
                }
                _ => {
                    let _ = scenario;
                    ""
                }
            };
            let selected_path = if field == "asr" {
                self.model_config
                    .pointer(if self.model_scenario_batch {
                        "/asr/selections/batch/modelPath"
                    } else {
                        "/asr/selections/live/modelPath"
                    })
                    .and_then(Value::as_str)
                    .or_else(|| self.model_config.get(key).and_then(Value::as_str))
            } else {
                self.model_config.get(key).and_then(Value::as_str)
            }
            .unwrap_or_default();
            self.model_catalog
                .as_ref()
                .and_then(|snapshot| snapshot.get("modelPathById"))
                .and_then(Value::as_object)
                .and_then(|paths| {
                    paths
                        .iter()
                        .find(|(_, path)| path.as_str() == Some(selected_path))
                })
                .map(|(id, _)| id.clone())
                .unwrap_or_default()
        };
        let asr_selected = selected_online_id.unwrap_or_else(|| selected_model_id("asr"));
        let vad_selected = selected_model_id("vad");
        let punctuation_selected = selected_model_id("punctuation");
        let speaker_segmentation_selected = selected_model_id("speaker-segmentation");
        let speaker_embedding_selected = selected_model_id("speaker-embedding");
        let alignment_selected = selected_model_id("alignment");
        let vad_options = companion_options("vad");
        let punctuation_options = companion_options("punctuation");
        let speaker_segmentation_options = companion_options("speaker-segmentation");
        let speaker_embedding_options = companion_options("speaker-embedding");
        let alignment_options = companion_options("alignment");
        let tab_groups: &[TabGroup] = &[
            (
                "GENERAL",
                &[
                    (
                        SettingsTab::General,
                        "General",
                        LucideIcon::SlidersHorizontal,
                    ),
                    (SettingsTab::Dashboard, "Dashboard", LucideIcon::Activity),
                    (SettingsTab::Shortcuts, "Shortcuts", LucideIcon::Keyboard),
                    (SettingsTab::About, "About", LucideIcon::Info),
                ],
            ),
            (
                "AUDIO",
                &[
                    (SettingsTab::Microphone, "Input Device", LucideIcon::Mic),
                    (SettingsTab::Models, "Models", LucideIcon::Box),
                    (SettingsTab::Vocabulary, "Vocabulary", LucideIcon::BookOpen),
                ],
            ),
            (
                "INTELLIGENCE",
                &[
                    (SettingsTab::LlmService, "LLM Service", LucideIcon::Bot),
                    (SettingsTab::Automation, "Automation", LucideIcon::Zap),
                ],
            ),
            (
                "INFRASTRUCTURE",
                &[
                    (SettingsTab::Storage, "Storage", LucideIcon::HardDrive),
                    (SettingsTab::Sync, "Cloud Sync", LucideIcon::Cloud),
                ],
            ),
            (
                "TOOLS",
                &[
                    (
                        SettingsTab::Subtitle,
                        "Subtitles & Voice Typing",
                        LucideIcon::Captions,
                    ),
                    (SettingsTab::ApiServer, "API Server", LucideIcon::Server),
                    (SettingsTab::Mcp, "MCP Agents", LucideIcon::Bot),
                ],
            ),
        ];

        div()
            .when(self.is_open, |this| {
                this.child(
                    // Modal backdrop overlay
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .id("settings-modal-backdrop")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings_modal.close(cx);
                            cx.notify();
                        }))
                        .child(
                            // Modal card
                            div()
                                .flex()
                                .w(modal_width)
                                .h(modal_height)
                                .rounded_xl()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_xl()
                                .id("settings-modal-content")
                                .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                                .child(
                                    // Left tabs sidebar
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w(sidebar_width)
                                        .h_full()
                                        .py_3()
                                        .px_2()
                                        .border_r_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().secondary)
                                        .overflow_y_scrollbar()
                                        .gap_2()
                                        .child(
                                            div()
                                                .px_3()
                                                .py_1()
                                                .text_size(px(15.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child("Settings"),
                                        )
                                        .children(tab_groups.iter().map(|(group_name, group_tabs)| {
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .px_3()
                                                        .pt_2()
                                                        .pb_1()
                                                        .text_size(px(11.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(cx.theme().muted)
                                                        .child(*group_name),
                                                )
                                                .children(group_tabs.iter().map(|(tab, label, icon_name)| {
                                                    let is_selected = self.current_tab == *tab;
                                                    let tab_val = *tab;
                                                    div()
                                                        .id(ElementId::from(format!("tab-btn-{:?}", tab)))
                                                        .flex()
                                                        .items_center()
                                                        .h(px(40.0))
                                                        .px_3()
                                                        .py_2()
                                                        .gap(px(12.0))
                                                        .rounded(RADIUS_MD)
                                                        .bg(if is_selected {
                                                            cx.theme().background
                                                        } else {
                                                            gpui_kit::transparent_black()
                                                        })
                                                        .border_1()
                                                        .border_color(if is_selected {
                                                            cx.theme().border
                                                        } else {
                                                            gpui_kit::transparent_black()
                                                        })
                                                        .cursor_pointer()
                                                        .hover(|style| style.bg(if is_selected { cx.theme().background } else { cx.theme().accent }))
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.settings_modal.set_tab(tab_val, cx);
                                                            cx.notify();
                                                        }))
                                                        .child(
                                                            Icon::new(*icon_name)
                                                                .size(px(18.0))
                                                                .text_color(if is_selected {
                                                                    cx.theme().primary
                                                                } else {
                                                                    cx.theme().muted
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .text_size(px(14.0))
                                                                .font_weight(if is_selected {
                                                                    FontWeight::SEMIBOLD
                                                                } else {
                                                                    FontWeight::MEDIUM
                                                                })
                                                                .text_color(if is_selected {
                                                                    cx.theme().foreground
                                                                } else {
                                                                    cx.theme().muted_foreground
                                                                })
                                                                .overflow_hidden()
                                                                .whitespace_nowrap()
                                                                .child(*label),
                                                        )
                                                }))
                                        })),
                                )
                                .child(
                                    // Right content area
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .h_full()
                                        .p_6()
                                        .gap_4()
                                        .overflow_y_scrollbar()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_size(px(20.0))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(cx.theme().foreground)
                                                                .child(format!("{} Settings", Self::tab_title(self.current_tab))),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id("btn-close-settings-modal")
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .size(px(32.0))
                                                        .rounded(RADIUS_MD)
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(cx.theme().accent))
                                                        .child(Icon::new(LucideIcon::X).size(px(18.0)))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.settings_modal.close(cx);
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                        .when(self.current_tab == SettingsTab::General, |this| this.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_4()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .pb_3()
                                                        .border_b_1()
                                                        .border_color(cx.theme().border)
                                                        .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("General"))
                                                        .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Configure application startup, updates, and environment diagnostics.")),
                                                )
                                                .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Appearance"))
                                                .child(
                                                    div().flex().items_center().justify_between().gap_4().py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Language"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Choose the language used by Sona.")))
                                                        .child(Select::new(general_language_select).w(px(200.0)).menu_width(px(200.0))),
                                                )
                                                .child(
                                                    div().flex().flex_col().gap_2().py_2()
                                                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Theme"))
                                                        .child(div().flex().gap_2().children(["light", "dark", "auto"].into_iter().map(|theme| {
                                                            let selected = self.selected_theme == theme;
                                                            let label = match theme { "light" => "Light", "dark" => "Dark", _ => "Automatic" };
                                                            Button::new(format!("settings-theme-{theme}"))
                                                                .when(selected, |button| button.primary())
                                                                .when(!selected, |button| button.outline())
                                                                .small()
                                                                .label(label)
                                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                                    this.settings_modal.selected_theme = theme.to_string();
                                                                    let mode = match theme {
                                                                        "light" => gpui_kit::component::theme::ThemeMode::Light,
                                                                        "dark" => gpui_kit::component::theme::ThemeMode::Dark,
                                                                        _ => gpui_kit::component::theme::ThemeMode::from(window.appearance()),
                                                                    };
                                                                    gpui_kit::component::theme::Theme::change(mode, None, cx);
                                                                    this.settings_modal.persist_general_setting(
                                                                        "theme",
                                                                        Value::String(theme.to_string()),
                                                                        cx,
                                                                    );
                                                                    cx.notify();
                                                                }))
                                                        }))),
                                                )
                                                .child(
                                                    div().flex().items_center().justify_between().gap_4().py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Font"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Choose the font used by Sona.")))
                                                        .child(Select::new(general_font_select).w(px(240.0)).menu_width(px(240.0))),
                                                )
                                                .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Startup and updates"))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_4()
                                                        .py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Log level"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Set the minimum level written to the application log.")))
                                                        .child(Select::new(general_log_level_select).w(px(180.0)).menu_width(px(180.0))),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_4()
                                                        .py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Launch on startup"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Start Sona automatically when you sign in to Windows.")))
                                                        .child(
                                                            Switch::new("btn-toggle-autostart")
                                                                .checked(self.autostart)
                                                                .on_change(cx.listener(|this, checked, _, cx| {
                                                                    let value = *checked;
                                                                    this.settings_modal.autostart = value;
                                                                    if !value {
                                                                        this.settings_modal.silent_start = false;
                                                                    }
                                                                    this.settings_modal.persist_preference("autoStart", value, cx);
                                                                    cx.notify();
                                                                })),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_4()
                                                        .py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Minimize to tray on exit"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Keep Sona running in the system tray when the main window closes.")))
                                                        .child(
                                                            Switch::new("btn-toggle-tray")
                                                                .checked(self.minimize_to_tray)
                                                                .on_change(cx.listener(|this, checked, _, cx| {
                                                                    let value = *checked;
                                                                    this.settings_modal.minimize_to_tray = value;
                                                                    if !value {
                                                                        this.settings_modal.silent_start = false;
                                                                    }
                                                                    this.settings_modal.persist_preference("minimizeToTrayOnExit", value, cx);
                                                                    cx.notify();
                                                                })),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_4()
                                                        .py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Start minimized"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child(if !self.autostart || !self.minimize_to_tray { "Requires Launch on startup and Minimize to tray on exit to be enabled." } else { "Open Sona directly in the system tray at sign-in." })))
                                                        .child(
                                                            Switch::new("btn-toggle-silent-start")
                                                                .checked(self.autostart && self.minimize_to_tray && self.silent_start)
                                                                .disabled(!self.autostart || !self.minimize_to_tray)
                                                                .on_change(cx.listener(|this, checked, _, cx| {
                                                                    if !this.settings_modal.autostart || !this.settings_modal.minimize_to_tray {
                                                                        return;
                                                                    }
                                                                    let value = *checked;
                                                                    this.settings_modal.silent_start = value;
                                                                    this.settings_modal.persist_preference("silentStart", value, cx);
                                                                    cx.notify();
                                                                })),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_4()
                                                        .py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Check for updates automatically"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Check for new Sona releases automatically.")))
                                                        .child(
                                                            Switch::new("btn-toggle-auto-check-updates")
                                                                .checked(self.auto_check_updates)
                                                                .on_change(cx.listener(|this, checked, _, cx| {
                                                                    let value = *checked;
                                                                    this.settings_modal.auto_check_updates = value;
                                                                    this.settings_modal.persist_preference("autoCheckUpdates", value, cx);
                                                                    cx.notify();
                                                                })),
                                                        ),
                                                )
                                                .child(div().flex().flex_col().gap_1().pt_3().border_t_1().border_color(cx.theme().border)
                                                    .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Model & Environment Diagnostics"))
                                                    .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Inspect the local transcription path, runtime readiness, and packaged environment checks.")))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_4()
                                                        .py_2()
                                                        .child(div().flex_1().flex().flex_col().gap_1()
                                                            .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Diagnostics"))
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Review the current local setup and jump to the clearest fix when something is off.")))
                                                        .child(
                                                            Button::new("btn-open-diagnostics")
                                                                .outline()
                                                                .small()
                                                                .label("Open Diagnostics")
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.settings_modal.close(cx);
                                                                    this.diagnostics_modal.open(cx);
                                                                })),
                                                        ),
                                                ),
                                        ))
                                        .when(self.current_tab == SettingsTab::Dashboard, |this| {
                                            this.child(dashboard_panel(
                                                self.dashboard_snapshot.as_ref(),
                                                self.dashboard_loading,
                                                self.dashboard_deep_loading,
                                                self.dashboard_error.as_deref(),
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::About, |this| {
                                            this.child(about_panel(
                                                &self.about_update_status,
                                                self.about_update_result.as_ref(),
                                                self.about_update_progress,
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::Automation, |this| {
                                            this.child(automation_panel(
                                                self.automation_state
                                                    .as_ref()
                                                    .map(|state| state.rules.as_slice())
                                                    .unwrap_or_default(),
                                                self.automation_loading,
                                                self.automation_error.as_deref(),
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::Sync, |this| {
                                            let enabled = cx
                                                .global::<crate::app::GlobalDesktopServices>()
                                                .0
                                                .config
                                                .get_setting("enableCloudSync")
                                                .ok()
                                                .flatten()
                                                .and_then(|value| value.as_bool())
                                                .unwrap_or(false);
                                            this.child(sync_panel(
                                                enabled,
                                                self.sync_status.as_ref(),
                                                &self.sync_conflicts,
                                                self.sync_loading,
                                                self.sync_busy,
                                                self.sync_error.as_deref(),
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::Storage, |this| {
                                            let retention_days = cx
                                                .global::<crate::app::GlobalDesktopServices>()
                                                .0
                                                .config
                                                .get_setting("historyAudioRetentionDays")
                                                .ok()
                                                .flatten()
                                                .and_then(|value| value.as_u64());
                                            this.child(storage_panel(
                                                self.storage_directories.as_ref(),
                                                self.storage_usage.as_ref(),
                                                self.storage_loading,
                                                self.storage_migration_busy,
                                                self.storage_migration_progress.as_ref(),
                                                self.storage_error.as_deref(),
                                                retention_days,
                                                active_history_id,
                                                self.storage_cleanup_busy,
                                                self.storage_cleanup_result.as_deref(),
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::Subtitle, |this| {
                                            this.child(div().flex().flex_col().gap_4().size_full().min_h_0()
                                                .child(div().flex().gap_2().px(px(32.0)).pt(px(20.0))
                                                    .child(Button::new("settings-subtitle-voice-typing")
                                                        .when(self.subtitle_voice_typing_tab, |button| button.primary())
                                                        .when(!self.subtitle_voice_typing_tab, |button| button.outline())
                                                        .small().label("Voice Typing")
                                                        .on_click(cx.listener(|shell, _, _, cx| {
                                                            shell.settings_modal.subtitle_voice_typing_tab = true;
                                                            cx.notify();
                                                        })))
                                                    .child(Button::new("settings-subtitle-caption-settings")
                                                        .when(!self.subtitle_voice_typing_tab, |button| button.primary())
                                                        .when(self.subtitle_voice_typing_tab, |button| button.outline())
                                                        .small().label("Subtitle Settings")
                                                        .on_click(cx.listener(|shell, _, _, cx| {
                                                            shell.settings_modal.subtitle_voice_typing_tab = false;
                                                            cx.notify();
                                                        }))))
                                                .child(if self.subtitle_voice_typing_tab {
                                                    voice_typing_tool.render(self.is_recording_shortcut, self.shortcut_recording_target, cx).into_any_element()
                                                } else {
                                                    subtitles_tool.render(cx).into_any_element()
                                                }))
                                        })
                                        .when(self.current_tab == SettingsTab::ApiServer, |this| {
                                            this.child(api_server_settings_panel(api_server_field_states, cx))
                                        })
                                        .when(self.current_tab == SettingsTab::Mcp, |this| {
                                            this.child(mcp_settings_panel(
                                                &self.mcp_logs,
                                                self.mcp_logs_loading,
                                                &self.mcp_log_filter_status,
                                                &self.mcp_log_query,
                                                &self.mcp_selected_guide,
                                                mcp_search_state,
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::LlmService, |this| {
                                            this.child(llm_service_panel(
                                                &self.model_config,
                                                &self.llm_active_feature,
                                                &self.llm_model_draft,
                                                &self.llm_custom_provider_name,
                                                &self.llm_custom_provider_strategy,
                                                &self.llm_edit_provider_strategy,
                                                self.llm_show_custom_provider_form,
                                                self.llm_editing_provider.as_deref(),
                                                self.llm_discovery_provider.as_deref(),
                                                self.llm_discovery_loading,
                                                self.llm_discovery_error.as_deref(),
                                                &self.llm_discovered_models,
                                                self.llm_editing_model.as_deref(),
                                                &self.llm_model_metadata_draft,
                                                self.llm_model_metadata_error.as_deref(),
                                                llm_model_metadata_field_states,
                                                self.llm_provider_test_states.get(
                                                    self.model_config
                                                        .pointer("/llmSettings/activeProvider")
                                                        .and_then(Value::as_str)
                                                        .unwrap_or("google_translate_free"),
                                                ),
                                                &self.llm_provider_test_states,
                                                llm_provider_api_host_state,
                                                llm_provider_api_key_state,
                                                llm_model_draft_state,
                                                llm_custom_provider_name_state,
                                                window,
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::Vocabulary, |this| {
                                            this.child(vocabulary_panel(
                                                &self.model_config,
                                                self,
                                                projects,
                                                dictionary_word_state,
                                                dictionary_weight_state,
                                                dictionary_from_state,
                                                dictionary_to_state,
                                                dictionary_inline_hotword_state,
                                                speaker_profile_name_state,
                                                speaker_profile_search_state,
                                                speaker_profile_name_states,
                                                &self.vocabulary_tab,
                                                &self.vocabulary_prompt_tab,
                                                self.vocabulary_polish_edit_id.as_deref(),
                                                self.vocabulary_summary_edit_id.as_deref(),
                                                &self.vocabulary_dictionary_mode,
                                                &self.vocabulary_dictionary_scope,
                                                dictionary_textarea_state,
                                                dictionary_quick_add_state,
                                                dictionary_search_state,
                                                dictionary_test_state,
                                                vocabulary_name_state,
                                                vocabulary_body_state,
                                                translation_language_select,
                                                cx,
                                            ))
                                        })
                                        .when(self.current_tab == SettingsTab::Microphone, |this| this.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_4()
                                                .child(audio_section_title("Microphone", "Select the input device used for recording.", cx))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .child(audio_row("Input device", "Choose the microphone used for recording.", cx)
                                                            .child(
                                                                div().flex().flex_col().gap_1().w(px(280.0))
                                                                    .child(Button::new("settings-mic-device")
                                                                        .outline().small()
                                                                        .label(if self.selected_mic == "default" { "System default" } else { self.selected_mic.as_str() })
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.settings_modal.microphone_menu_open = !this.settings_modal.microphone_menu_open;
                                                                            cx.notify();
                                                                        })))
                                                                    .when(self.microphone_menu_open, |menu| menu.children(
                                                                        std::iter::once("default".to_string())
                                                                            .chain(self.microphone_devices.iter().cloned())
                                                                            .map(|device| {
                                                                                let selected = device.clone();
                                                                                let label = if device == "default" { "System default".to_string() } else { device };
                                                                                div().child(Button::new(ElementId::Name(format!("mic-device-{selected}").into()))
                                                                                    .ghost().small().label(label)
                                                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                                                        this.settings_modal.selected_mic = selected.clone();
                                                                                        this.settings_modal.microphone_menu_open = false;
                                                                                        this.settings_modal.persist_audio_value("microphoneId", Value::String(selected.clone()), cx);
                                                                                        cx.notify();
                                                                                    })))
                                                                            })
                                                                    ))
                                                                    .child(audio_meter(self.microphone_peak, cx))
                                                            ),
                                                        )
                                                        .child(audio_row("Microphone boost", "Input gain multiplier from 1.0x to 5.0x.", cx)
                                                            .child(div().flex().items_center().gap_2()
                                                                .child(Button::new("settings-mic-boost-down").outline().small().label("-")
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        let value = (this.settings_modal.microphone_boost - 0.1).max(1.0);
                                                                        this.settings_modal.microphone_boost = value;
                                                                        this.settings_modal.persist_audio_value("microphoneBoost", json!(value), cx);
                                                                        cx.notify();
                                                                    })))
                                                                .child(div().w(px(48.0)).text_center().child(format!("{:.1}x", self.microphone_boost)))
                                                                .child(Button::new("settings-mic-boost-up").outline().small().label("+")
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        let value = (this.settings_modal.microphone_boost + 0.1).min(5.0);
                                                                        this.settings_modal.microphone_boost = value;
                                                                        this.settings_modal.persist_audio_value("microphoneBoost", json!(value), cx);
                                                                        cx.notify();
                                                                    }))),
                                                            ),
                                                        )
                                                        .child(audio_toggle_row("Keep microphone active", "Keep the microphone ready for voice typing.", self.keep_microphone_active, "settings-keep-mic-active", cx, "keepMicrophoneActive")),
                                                )
                                                .child(audio_section_title("System audio", "Choose desktop audio capture behavior.", cx))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .child(audio_row("Output device", "Choose the output device to capture.", cx)
                                                            .child(
                                                                div().flex().flex_col().gap_1().w(px(280.0))
                                                                    .child(Button::new("settings-system-device").outline().small()
                                                                        .label(if self.selected_system_audio == "default" { "System default" } else { self.selected_system_audio.as_str() })
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.settings_modal.system_audio_menu_open = !this.settings_modal.system_audio_menu_open;
                                                                            cx.notify();
                                                                        })))
                                                                    .when(self.system_audio_menu_open, |menu| menu.children(
                                                                        std::iter::once("default".to_string())
                                                                            .chain(self.system_audio_devices.iter().cloned())
                                                                            .map(|device| {
                                                                                let selected = device.clone();
                                                                                let label = if device == "default" { "System default".to_string() } else { device };
                                                                                div().child(Button::new(ElementId::Name(format!("system-device-{selected}").into()))
                                                                                    .ghost().small().label(label)
                                                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                                                        this.settings_modal.selected_system_audio = selected.clone();
                                                                                        this.settings_modal.system_audio_menu_open = false;
                                                                                        this.settings_modal.persist_audio_value("systemAudioDeviceId", Value::String(selected.clone()), cx);
                                                                                        cx.notify();
                                                                                    })))
                                                                            })
                                                                    ))
                                                                    .child(audio_meter(self.system_audio_peak, cx))
                                                            ),
                                                        )
                                                        .child(audio_toggle_row("Mute during recording", "Mute system output while recording.", self.mute_during_recording, "settings-mute-recording", cx, "muteDuringRecording")),
                                                )
                                                .when(self.audio_devices_loading, |this| this.child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("Loading audio devices...")))
                                                .when(self.audio_devices_error.is_some(), |this| this.child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(self.audio_devices_error.clone().unwrap_or_default())))
                                                .child(audio_section_title("Audio decoding", "Configure FFmpeg for imported audio and video files.", cx))
                                                .child(audio_toggle_row("Enable FFmpeg decoder", "Use FFmpeg when decoding imported media.", self.ffmpeg_enabled, "settings-ffmpeg-enabled", cx, "ffmpegEnabled"))
                                                .when(self.ffmpeg_enabled, |this| this.child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_3()
                                                        .child(div().flex_1().text_size(px(12.0)).text_color(cx.theme().muted)
                                                            .child(if self.ffmpeg_path.trim().is_empty() { "Using system PATH or detected FFmpeg".to_string() } else { self.ffmpeg_path.clone() }))
                                                        .child(Button::new("settings-ffmpeg-browse").outline().small().label("Change Path...")
                                                            .on_click(cx.listener(|this, _, _, cx| this.settings_modal.browse_ffmpeg(cx))))
                                                        .when(!self.ffmpeg_path.trim().is_empty(), |row| row.child(
                                                            Button::new("settings-ffmpeg-reset").ghost().small().label("Restore Default")
                                                                .on_click(cx.listener(|this, _, _, cx| this.settings_modal.reset_ffmpeg_path(cx)))
                                                        )),
                                                )),
                                        ))
                                        .when(self.current_tab == SettingsTab::Shortcuts, |this| this.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_4()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_size(px(14.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .child("Live Recording"),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .gap_3()
                                                                .py_1()
                                                                .child(
                                                                    div()
                                                                        .text_size(px(13.0))
                                                                        .child("Start / Stop Recording"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap_1()
                                                                        .when(self.is_editing_shortcut, |this| {
                                                                            this.child(
                                                                                div()
                                                                                    .w(px(120.0))
                                                                                    .h(px(28.0))
                                                                                    .px_2()
                                                                                    .py_1()
                                                                                    .text_center()
                                                                                    .rounded_sm()
                                                                                    .border_1()
                                                                                    .border_color(cx.theme().border)
                                                                                    .bg(cx.theme().input)
                                                                                    .text_size(px(14.0))
                                                                                    .text_color(cx.theme().foreground)
                                                                                    .child(Input::new(shortcut_edit_state)),
                                                                            )
                                                                        })
                                                                        .when(!self.is_editing_shortcut, |this| {
                                                                            this.child(
                                                                                Button::new("shortcut-live-record-capture")
                                                                                    .outline()
                                                                                    .small()
                                                                                    .label(if self.is_recording_shortcut {
                                                                                        "Recording..."
                                                                                    } else {
                                                                                        self.live_record_shortcut.as_str()
                                                                                    })
                                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                                        this.settings_modal.is_recording_shortcut = true;
                                                                                        cx.notify();
                                                                                    })),
                                                                            )
                                                                            .child(
                                                                                Button::new("shortcut-live-record-edit")
                                                                                    .ghost()
                                                                                    .small()
                                                                                    .label("Edit manually")
                                                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                                                        let value = this.settings_modal.live_record_shortcut.clone();
                                                                                        this.settings_modal.shortcut_draft = value.clone();
                                                                                        this.settings_modal.is_editing_shortcut = true;
                                                                                        this.shortcut_edit_state.update(cx, |state, cx| {
                                                                                            state.set_value(value, window, cx);
                                                                                            state.focus(window, cx);
                                                                                        });
                                                                                        cx.notify();
                                                                                    })),
                                                                            )
                                                                        }),
                                                                ),
                                                        )
                                                        .child(shortcut_section(
                                                            "",
                                                            vec![("Pause / Resume Recording", "Space")],
                                                            cx,
                                                        )),
                                                )
                                                .child(shortcut_section(
                                                    "Playback",
                                                    vec![
                                                        ("Play / Pause", "Space / K"),
                                                        ("Seek Backward 5s / Seek Forward 5s", "← / →"),
                                                        ("Volume Up / Volume Down", "↑ / ↓"),
                                                        ("Toggle Mute", "M"),
                                                    ],
                                                    cx,
                                                ))
                                                .child(shortcut_section(
                                                    "Search",
                                                    vec![
                                                        ("Find in current context", "Ctrl + F"),
                                                        ("Next / previous transcript match", "Enter / Shift + Enter"),
                                                        ("Navigate workspace search results", "↑ / ↓"),
                                                        ("Clear or close current search", "Esc"),
                                                    ],
                                                    cx,
                                                ))
                                                .child(shortcut_section(
                                                    "Editor",
                                                    vec![
                                                        ("Bold", "Ctrl + B"),
                                                        ("Italic", "Ctrl + I"),
                                                        ("Underline", "Ctrl + U"),
                                                        ("Strikethrough", "Ctrl + Shift + S"),
                                                        ("Split Segment", "Shift + Enter"),
                                                    ],
                                                    cx,
                                                )),
                                        ))
                                        .when(self.current_tab == SettingsTab::Models, |this| this.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_4()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Model Hub"))
                                                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("Select recognition models and configure transcription behavior.")),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_3()
                                                        .p_3()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(cx.theme().border)
                                                        .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Model Selection"))
                                                        .child(div().flex().gap_2().children([
                                                            Button::new("settings-model-scenario-live").outline().small().label("Real-time Live").on_click(cx.listener(|this, _, _, cx| { this.settings_modal.model_scenario_batch = false; cx.notify(); })),
                                                            Button::new("settings-model-scenario-batch").outline().small().label("Batch Import").on_click(cx.listener(|this, _, _, cx| { this.settings_modal.model_scenario_batch = true; cx.notify(); })),
                                                        ]))
                                                        .child(model_selector_row("Recognition Model", "asr", &asr_selected, &asr_options, self.model_selector_open.as_deref() == Some("asr"), self.model_catalog_loading, cx))
                                                        .child(model_toggle_row("Speaker diarization and recognition", if self.model_scenario_batch { "batchSpeakerDiarizationEnabled" } else { "liveSpeakerDiarizationEnabled" }, self.model_config.get(if self.model_scenario_batch { "batchSpeakerDiarizationEnabled" } else { "liveSpeakerDiarizationEnabled" }).and_then(Value::as_bool).unwrap_or(false), self.model_catalog_loading, cx))
                                                        .when(self.model_config.get(if self.model_scenario_batch { "batchSpeakerDiarizationEnabled" } else { "liveSpeakerDiarizationEnabled" }).and_then(Value::as_bool).unwrap_or(false), |panel| panel
                                                            .child(model_selector_row("Speaker Embedding Model", "speaker-embedding", &speaker_embedding_selected, &speaker_embedding_options, self.model_selector_open.as_deref() == Some("speaker-embedding"), self.model_catalog_loading, cx))
                                                            .child(model_selector_row("Speaker Segmentation Model", "speaker-segmentation", &speaker_segmentation_selected, &speaker_segmentation_options, self.model_selector_open.as_deref() == Some("speaker-segmentation"), self.model_catalog_loading, cx))
                                                            .child(model_sensitivity_row(self.model_config.get("speakerDiarizationSensitivity").and_then(Value::as_str).unwrap_or("balanced"), self.model_catalog_loading, cx))
                                                        )
                                                        .child(model_selector_row("CTC Alignment Model", "alignment", &alignment_selected, &alignment_options, self.model_selector_open.as_deref() == Some("alignment"), self.model_catalog_loading, cx))
                                                        .child(model_selector_row("Punctuation Model", "punctuation", &punctuation_selected, &punctuation_options, self.model_selector_open.as_deref() == Some("punctuation"), self.model_catalog_loading, cx))
                                                        .child(model_selector_row("VAD Model", "vad", &vad_selected, &vad_options, self.model_selector_open.as_deref() == Some("vad"), self.model_catalog_loading, cx))
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .p_3()
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(cx.theme().border)
                                                        .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Transcription Settings"))
                                                        .child(model_toggle_row("Enable inverse text normalization", "enableITN", self.model_config.get("enableITN").and_then(Value::as_bool).unwrap_or(true), false, cx))
                                                        .when(self.model_scenario_batch, |panel| panel.child(model_toggle_row("Use VAD speech segmentation", "batchVadEnabled", self.model_config.get("batchVadEnabled").and_then(Value::as_bool).unwrap_or(true), false, cx)))
                                                        .when(!self.model_scenario_batch, |panel| panel.child(model_toggle_row("Low-power sentence-only mode", "liveSentenceOnly", self.model_config.get("liveSentenceOnly").and_then(Value::as_bool).unwrap_or(false), false, cx)))
                                                        .child(model_numeric_row("Maximum concurrent transcriptions", "maxConcurrent", self.model_config.get("maxConcurrent").and_then(Value::as_f64).unwrap_or(2.0), 1.0, 4.0, 1.0, cx))
                                                        .child(model_numeric_row(if self.model_scenario_batch { "Batch VAD buffer (seconds)" } else { "Live VAD buffer (seconds)" }, if self.model_scenario_batch { "batchVadBufferSize" } else { "liveVadBufferSize" }, self.model_config.get(if self.model_scenario_batch { "batchVadBufferSize" } else { "liveVadBufferSize" }).and_then(Value::as_f64).unwrap_or(5.0), 0.0, 30.0, 0.5, cx))
                                                        .child(model_selector_row("GPU Acceleration", "gpu-acceleration", self.model_config.get("gpuAcceleration").and_then(Value::as_str).unwrap_or("auto"), &gpu_options, self.model_selector_open.as_deref() == Some("gpu-acceleration"), self.cuda_addon_downloading, cx))
                                                        .when(self.cuda_addon_loading, |panel| panel.child(div().text_size(px(11.0)).text_color(cx.theme().muted).child("Checking CUDA runtime...")))
                                                        .when(self.cuda_addon_status.is_some(), |panel| panel.child(div().text_size(px(11.0)).text_color(cx.theme().muted).child(if self.cuda_addon_status.as_ref().is_some_and(|status| status.is_installed) { "CUDA runtime ready" } else { "CUDA runtime not installed" })))
                                                        .when(self.cuda_addon_downloading, |panel| panel.child(
                                                            div().flex().flex_col().gap_1().w(px(220.0))
                                                                .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(format!("Downloading CUDA runtime ({:.0}%)...", self.cuda_addon_progress)))
                                                                .child(div().w_full().h(px(4.0)).rounded_sm().bg(cx.theme().border)
                                                                    .child(div().h(px(4.0)).w(px(220.0 * self.cuda_addon_progress.clamp(0.0, 100.0) / 100.0)).rounded_sm().bg(cx.theme().accent)))
                                                        ))
                                                        .when(self.cuda_addon_error.is_some(), |panel| panel.child(div().text_size(px(12.0)).text_color(cx.theme().danger).child(format!("CUDA installation failed: {}", self.cuda_addon_error.as_deref().unwrap_or_default()))))
                                                )
                                                .child(Button::new("settings-model-restore-defaults").ghost().small().label("Restore defaults").disabled(self.model_catalog.is_none()).on_click(cx.listener(|this, _, _, cx| this.settings_modal.restore_model_defaults(cx))))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .child(div().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Local Model Management"))
                                                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("Download and manage local recognition models.")),
                                                )
                                                .when(self.model_catalog.is_some(), |body| body.child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(Select::new(model_engine_select).w(px(130.0)).menu_width(px(170.0)))
                                                        .child(Select::new(model_status_select).w(px(130.0)).menu_width(px(190.0)))
                                                        .child(Select::new(model_label_select).w(px(120.0)).menu_width(px(150.0)))
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap_1()
                                                                .flex_1()
                                                                .min_w(px(180.0))
                                                                .px_2()
                                                                .h(px(32.0))
                                                                .rounded_md()
                                                                .border_1()
                                                                .border_color(cx.theme().border)
                                                                .bg(cx.theme().input)
                                                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                                                                    if event.keystroke.key.eq_ignore_ascii_case("escape") && !this.settings_modal.model_search_query.is_empty() {
                                                                        this.model_search_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                                        this.settings_modal.model_search_query.clear();
                                                                        window.prevent_default();
                                                                        cx.stop_propagation();
                                                                        cx.notify();
                                                                    }
                                                                }))
                                                                .child(Input::new(model_search_state).small())
                                                                .when(!self.model_search_query.is_empty(), |search| search.child(
                                                                    Button::new("settings-model-search-clear")
                                                                        .ghost()
                                                                        .small()
                                                                        .label("×")
                                                                        .on_click(cx.listener(|this, _, window, cx| {
                                                                            this.model_search_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                                            this.settings_modal.model_search_query.clear();
                                                                            cx.notify();
                                                                        })),
                                                                )),
                                                        ),
                                                ))
                                                .when(self.model_catalog_loading && self.model_catalog.is_none(), |body| body.child(
                                                    div().text_size(px(12.0)).text_color(cx.theme().muted).child("Checking local models..."),
                                                ))
                                                .when(self.model_catalog_error.is_some(), |body| body.child(
                                                    div().text_size(px(12.0)).text_color(cx.theme().muted).child(format!("Could not check local models: {}. Download status may be stale.", self.model_catalog_error.as_deref().unwrap_or_default())),
                                                ))
                                                .children(model_sections.into_iter().filter_map(|section| {
                                                    let section_type = section.get("type").and_then(Value::as_str).unwrap_or_default();
                                                    let title = match section_type {
                                                        "asr" => "Recognition Models",
                                                        "punctuation" => "Punctuation Models",
                                                        "vad" => "VAD Models",
                                                        "speaker-segmentation" => "Speaker Segmentation Models",
                                                        "speaker-embedding" => "Speaker Embedding Models",
                                                        "alignment" => "CTC Alignment Models",
                                                        _ => return None,
                                                    };
                                                    let groups = section
                                                        .get("groups")
                                                        .and_then(Value::as_array)
                                                        .cloned()
                                                        .unwrap_or_default()
                                                        .into_iter()
                                                        .filter_map(|mut group| {
                                                            let models = group.get("models").and_then(Value::as_array).cloned().unwrap_or_default();
                                                            let filtered = models.into_iter().filter(|model| {
                                                                let id = model.get("id").and_then(Value::as_str).unwrap_or_default();
                                                                let engine = model.get("engine").and_then(Value::as_str).unwrap_or_default();
                                                                let installed = model.get("isInstalled").and_then(Value::as_bool).unwrap_or(false);
                                                                let downloading = self.model_download_ids.contains_key(id);
                                                                if model_engine_filter != "all" && model_engine_filter != engine { return false; }
                                                                match model_status_filter {
                                                                    "installed" if !installed => return false,
                                                                    "not-installed" if installed || downloading => return false,
                                                                    "downloading" if !downloading => return false,
                                                                    _ => {}
                                                                }
                                                                if model_label_filter != "all" {
                                                                    let has_label = sona_core::models::preset_models::preset_models()
                                                                        .iter()
                                                                        .find(|preset| preset.id == id)
                                                                        .is_some_and(|preset| preset.labels.iter().any(|label| label == model_label_filter));
                                                                    if !has_label { return false; }
                                                                }
                                                                if !model_search_query.is_empty() {
                                                                    let name = model.get("name").and_then(Value::as_str).unwrap_or_default().to_lowercase();
                                                                    let version = model.get("versionLabel").and_then(Value::as_str).unwrap_or_default().to_lowercase();
                                                                    if !name.contains(&model_search_query) && !version.contains(&model_search_query) { return false; }
                                                                }
                                                                true
                                                            }).collect::<Vec<_>>();
                                                            if filtered.is_empty() { return None; }
                                                            group["models"] = Value::Array(filtered);
                                                            Some(group)
                                                        })
                                                        .collect::<Vec<_>>();
                                                    let cards = groups.into_iter().map(|group| {
                                                        let models = group
                                                            .get("models")
                                                            .and_then(Value::as_array)
                                                            .cloned()
                                                            .unwrap_or_default();
                                                        let versions = models.iter().map(|model| {
                                                            let installed = model
                                                                .get("isInstalled")
                                                                .and_then(Value::as_bool)
                                                                .unwrap_or(false);
                                                            let interrupted = model
                                                                .get("interruptedDownload")
                                                                .is_some_and(|value| !value.is_null());
                                                            let model_name = model
                                                                .get("versionLabel")
                                                                .and_then(Value::as_str)
                                                                .or_else(|| model.get("name").and_then(Value::as_str))
                                                                .unwrap_or("");
                                                            let size = model
                                                                .get("size")
                                                                .and_then(Value::as_str)
                                                                .unwrap_or_default();
                                                            let model_id = model
                                                                .get("id")
                                                                .and_then(Value::as_str)
                                                                .unwrap_or_default();
                                                        let model_name_full = model
                                                            .get("name")
                                                            .and_then(Value::as_str)
                                                            .unwrap_or(model_name)
                                                            .to_string();
                                                        let install_path = model
                                                            .get("installPath")
                                                            .and_then(Value::as_str)
                                                            .unwrap_or_default()
                                                            .to_string();
                                                        let active_download = self.model_download_ids.contains_key(model_id);
                                                        let progress = self.model_download_progress.get(model_id).copied().unwrap_or_default();
                                                        let is_delete_pending = self
                                                            .pending_model_delete
                                                            .as_ref()
                                                            .is_some_and(|(pending_id, _, _)| pending_id == model_id);
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .gap_3()
                                                                .py_2()
                                                                .border_t_1()
                                                                .border_color(cx.theme().border)
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap_2()
                                                                        .child(div().text_size(px(13.0)).child(model_name.to_string()))
                                                                        .child(div().text_size(px(10.0)).text_color(cx.theme().muted).child(if installed { "Installed" } else if active_download { "Downloading" } else if interrupted { "Paused" } else { "Not installed" })),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap_2()
                                                                        .child(div().text_size(px(11.0)).text_color(cx.theme().muted).child(size.to_string()))
                                                                        .when(installed, |row| {
                                                                            let model_id = model_id.to_string();
                                                                            let model_name = model_name_full.clone();
                                                                            let path = install_path.clone();
                                                                            row.child(Button::new(ElementId::Name(format!("model-delete-{model_id}").into())).ghost().small().label("Delete").on_click(cx.listener(move |this, _, _, cx| {
                                                                                this.settings_modal.request_model_delete(model_id.clone(), model_name.clone(), path.clone());
                                                                                cx.notify();
                                                                            })))
                                                                        })
                                                                        .when(!installed && !active_download, |row| {
                                                                            let model_id = model_id.to_string();
                                                                            row.child(Button::new(ElementId::Name(format!("model-download-{model_id}").into())).outline().small().label(if interrupted { "Resume" } else { "Download" }).on_click(cx.listener(move |this, _, _, cx| this.settings_modal.download_model(model_id.clone(), cx))))
                                                                        })
                                                                        .when(active_download, |row| {
                                                                            let model_id = model_id.to_string();
                                                                            row.child(div().flex().items_center().gap_2().child(div().w(px(72.0)).h(px(4.0)).rounded_sm().bg(cx.theme().border).child(div().h(px(4.0)).w(px(72.0 * progress.clamp(0.0, 100.0) / 100.0)).rounded_sm().bg(cx.theme().accent))).child(Button::new(ElementId::Name(format!("model-cancel-{model_id}").into())).ghost().small().label("Cancel").on_click(cx.listener(move |this, _, _, cx| this.settings_modal.cancel_model_download(model_id.clone(), cx)))))
                                                                        }),
                                                                )
                                                                .when(is_delete_pending, |row| {
                                                                    row.child(
                                                                        div()
                                                                            .flex()
                                                                            .items_center()
                                                                            .gap_1()
                                                                            .child(div().text_size(px(11.0)).text_color(cx.theme().muted).child("Delete this model?"))
                                                                            .child(Button::new(ElementId::Name(format!("model-delete-confirm-{model_id}").into())).primary().small().label("Delete").on_click(cx.listener(|this, _, _, cx| this.settings_modal.confirm_model_delete(cx))))
                                                                            .child(Button::new(ElementId::Name(format!("model-delete-cancel-{model_id}").into())).ghost().small().label("Cancel").on_click(cx.listener(|this, _, _, cx| { this.settings_modal.pending_model_delete = None; cx.notify(); }))),
                                                                    )
                                                                })
                                                        }).collect::<Vec<_>>();
                                                        let name = models
                                                            .first()
                                                            .and_then(|model| model.get("name"))
                                                            .and_then(Value::as_str)
                                                            .unwrap_or("Model");
                                                        let description = models
                                                            .first()
                                                            .and_then(|model| model.get("description"))
                                                            .and_then(Value::as_str)
                                                            .unwrap_or_default();
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_2()
                                                            .p_3()
                                                            .rounded_md()
                                                            .border_1()
                                                            .border_color(cx.theme().border)
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .justify_between()
                                                                    .items_center()
                                                                    .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child(name.to_string()))
                                                                    .child(div().text_size(px(11.0)).text_color(cx.theme().muted).child(format!("{} version{}", models.len(), if models.len() == 1 { "" } else { "s" }))),
                                                            )
                                                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(description.to_string()))
                                                            .children(versions)
                                                    }).collect::<Vec<_>>();
                                                    let section_has_models = !cards.is_empty();
                                                    Some(div().flex().flex_col().gap_2().child(div().flex().items_center().justify_between().child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child(title)).child(div().text_size(px(11.0)).text_color(cx.theme().muted).child(format!("{} models", cards.len())))).when(section_has_models, |section| section.children(cards)).when(!section_has_models && self.model_catalog.is_some(), |section| section.child(div().py_2().text_size(px(12.0)).text_color(cx.theme().muted).child("No matching models"))))
                                                })).child(div().pt_3().text_size(px(16.0)).font_weight(FontWeight::SEMIBOLD).child("Online Model Management"))
                                                .child(online_provider_grid(
                                                    self.expanded_online_provider.as_deref(),
                                                    &self.model_config,
                                                    &self.online_provider_api_key_revealed,
                                                    &self.online_provider_test_states,
                                                    online_provider_field_states,
                                                    cx,
                                                )),
                                        )),
                                ),
                        ),
                )
            })
    }
}

impl Default for SettingsModal {
    fn default() -> Self {
        Self::new()
    }
}

fn audio_meter(level: f32, cx: &Context<MainShell>) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .w(px(120.0))
        .h(px(36.0))
        .px_1()
        .rounded_sm()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().input)
        .child(
            div()
                .h(px(6.0))
                .w(px(116.0 * level.clamp(0.0, 1.0)))
                .rounded_sm()
                .bg(cx.theme().accent),
        )
}

fn clear_model_path_references(value: &mut Value, deleted_path: &str) {
    match value {
        Value::String(path) if path == deleted_path => *path = String::new(),
        Value::Array(items) => {
            for item in items {
                clear_model_path_references(item, deleted_path);
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                clear_model_path_references(field, deleted_path);
            }
        }
        _ => {}
    }
}

fn set_json_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }
    if !root.is_object() {
        *root = json!({});
    }
    let mut current = root;
    for key in &path[..path.len() - 1] {
        if !current.get(*key).is_some_and(Value::is_object) {
            current[*key] = json!({});
        }
        current = &mut current[*key];
    }
    current[path[path.len() - 1]] = value;
}

fn update_online_provider_model_config(
    config: &mut Value,
    provider_id: &str,
    model_id: &str,
    added: bool,
    is_default: bool,
) {
    let Some(provider) = sona_core::ports::asr::find_online_asr_provider(provider_id) else {
        return;
    };
    let provider_config = config
        .pointer("/asr/providers/online")
        .and_then(|providers| providers.get(provider_id));
    let mut ids = online_provider_added_model_ids(provider, provider_config);
    if added {
        ids.insert(model_id.to_string());
    } else {
        ids.remove(model_id);
    }
    let next_added = ids.clone();
    set_json_path(
        config,
        &["asr", "providers", "online", provider_id, "addedModels"],
        Value::Array(ids.into_iter().map(Value::String).collect()),
    );
    if !added
        && let Some(selections) = config
            .pointer_mut("/asr/selections")
            .and_then(Value::as_object_mut)
    {
        for slot in ["live", "caption", "voiceTyping", "batch"] {
            let Some(selection) = selections.get_mut(slot).and_then(Value::as_object_mut) else {
                continue;
            };
            if selection.get("engine").and_then(Value::as_str) != Some("online")
                || selection.get("providerId").and_then(Value::as_str) != Some(provider_id)
            {
                continue;
            }
            let selected_model = selection.get("modelId").and_then(Value::as_str);
            if selected_model == Some(model_id)
                || (selected_model.is_none() && is_default)
                || next_added.is_empty()
            {
                let mode = selection.get("mode").cloned().unwrap_or(Value::Null);
                *selection = serde_json::Map::from_iter([
                    ("engine".to_string(), Value::String("local".to_string())),
                    ("mode".to_string(), mode),
                    ("modelId".to_string(), Value::Null),
                    ("modelPath".to_string(), Value::String(String::new())),
                ]);
            }
        }
    }
}

fn online_provider_model_options(
    model_config: &Value,
    batch: bool,
    selected_id: Option<&str>,
) -> Vec<(String, String)> {
    let mode = if batch { "batch" } else { "streaming" };
    let mut options = Vec::new();
    for provider in sona_core::ports::asr::online_asr_providers() {
        if (batch && !provider.batch.local_file_mode.supported)
            || (!batch && provider.streaming.supported == Some(false))
        {
            continue;
        }
        let provider_config = model_config
            .pointer("/asr/providers/online")
            .and_then(|providers| providers.get(&provider.id));
        let has_api_key = provider_config
            .and_then(|config| config.get("apiKey"))
            .and_then(Value::as_str)
            .is_some_and(|key| !key.trim().is_empty());
        let selected_provider = selected_id.is_some_and(|selected| {
            selected == provider.id || selected.starts_with(&format!("{}::", provider.id))
        });
        if !has_api_key && !selected_provider {
            continue;
        }

        let added_models = online_provider_added_model_ids(provider, provider_config);
        let selected_model =
            selected_id.and_then(|selected| selected.strip_prefix(&format!("{}::", provider.id)));
        let active_models = provider
            .models
            .iter()
            .filter(|model| model.modes.iter().any(|model_mode| model_mode == mode))
            .filter(|model| {
                added_models.contains(model.id.as_str())
                    || selected_model == Some(model.id.as_str())
                    || (selected_id == Some(provider.id.as_str()) && model.is_default == Some(true))
            })
            .collect::<Vec<_>>();
        let provider_name = online_provider_default_name(&provider.id);
        if active_models.is_empty() {
            if selected_provider {
                options.push((provider.id.clone(), provider_name.to_string()));
            }
            continue;
        }
        for model in active_models.iter() {
            let explicit_id = format!("{}::{}", provider.id, model.id);
            let value = if active_models.len() == 1 && selected_id != Some(explicit_id.as_str()) {
                provider.id.clone()
            } else {
                explicit_id
            };
            options.push((value, format!("{} · {}", model.name, provider_name)));
        }
    }
    options
}

fn online_provider_added_model_ids(
    provider: &sona_core::ports::asr::OnlineAsrProvider,
    config: Option<&Value>,
) -> std::collections::HashSet<String> {
    if let Some(models) = config
        .and_then(|config| config.get("addedModels"))
        .and_then(Value::as_array)
    {
        return models
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    let has_api_key = config
        .and_then(|config| config.get("apiKey"))
        .and_then(Value::as_str)
        .is_some_and(|key| !key.trim().is_empty());
    if !has_api_key {
        return std::collections::HashSet::new();
    }
    let defaults = provider
        .models
        .iter()
        .filter(|model| model.is_default == Some(true))
        .map(|model| model.id.clone())
        .collect::<Vec<_>>();
    if !defaults.is_empty() {
        return defaults.into_iter().collect();
    }
    provider
        .models
        .first()
        .map(|model| [model.id.clone()].into_iter().collect())
        .unwrap_or_default()
}

fn online_provider_default_name(provider_id: &str) -> &'static str {
    match provider_id {
        "volcengine-doubao" => "Volcengine",
        "groq-whisper" => "Groq",
        "mistral-voxtral" => "Mistral",
        "openai-whisper" => "OpenAI",
        "deepgram" => "Deepgram",
        "assemblyai" => "AssemblyAI",
        "elevenlabs" => "ElevenLabs",
        "azure-speech" => "Azure Speech",
        "google-gemini" => "Google Gemini",
        "xai" => "xAI Audio",
        "gladia" => "Gladia",
        "revai" => "Rev.ai",
        "fal-whisper" => "FAL AI Whisper",
        _ => "Online Provider",
    }
}

fn online_provider_logo_path(provider_id: &str, provider_name: &str) -> Option<&'static str> {
    let token = format!("{provider_id} {provider_name}").to_lowercase();
    let path = if token.contains("volcengine") || token.contains("doubao") {
        "models/volcengine.png"
    } else if token.contains("groq") {
        "models/groq.png"
    } else if token.contains("mistral") || token.contains("voxtral") {
        "models/mistral.png"
    } else if token.contains("openai") {
        "models/openai.png"
    } else if token.contains("deepgram") {
        "models/deepgram.png"
    } else if token.contains("assemblyai") || token.contains("assembly") {
        "models/assemblyai.png"
    } else if token.contains("elevenlabs") {
        "models/elevenlabs.png"
    } else if token.contains("azure") {
        "providers/azure.svg"
    } else if token.contains("gemini") {
        "providers/gemini.svg"
    } else if token.contains("fal-whisper") || token.contains(" fal") {
        "providers/fal.svg"
    } else if token.contains("gladia") {
        "providers/gladia.svg"
    } else if token.contains("revai") || token.contains("rev.ai") {
        "providers/revai.svg"
    } else if token.contains("xai") || token.contains("x.ai") {
        "providers/xai.svg"
    } else if token.contains("r2t2") {
        "models/r2t2.png"
    } else if token.contains("qwen")
        || token.contains("sense-voice")
        || token.contains("sensevoice")
    {
        "models/sensevoice.png"
    } else if token.contains("fire-red") || token.contains("firered") {
        "models/firered.png"
    } else if token.contains("whisper") {
        "models/whisper.png"
    } else if token.contains("omnilingual") {
        "models/meta.png"
    } else if token.contains("paraformer") {
        "models/paraformer.png"
    } else if token.contains("funasr") {
        "models/funasr.png"
    } else if token.contains("zipformer") || token.contains("icefall") || token.contains("k2") {
        "models/zipformer.png"
    } else if token.contains("parakeet") || token.contains("nemo") || token.contains("canary") {
        "models/nvidia.png"
    } else if token.contains("dolphin") {
        "models/dolphin.png"
    } else if token.contains("moonshine") {
        "models/moonshine.png"
    } else {
        return None;
    };
    Some(path)
}

fn online_provider_languages_tooltip(languages: &[String]) -> String {
    const MAX_TOOLTIP_LANGUAGES: usize = 24;
    let sample = languages
        .iter()
        .take(MAX_TOOLTIP_LANGUAGES)
        .map(|code| online_language_display_name(code))
        .collect::<Vec<_>>()
        .join(", ");
    if languages.len() > MAX_TOOLTIP_LANGUAGES {
        format!("{sample} ... ({} languages in total)", languages.len())
    } else {
        sample
    }
}

fn online_language_display_name(code: &str) -> &str {
    match code {
        "af" => "Afrikaans",
        "am" => "Amharic",
        "ar" => "Arabic",
        "as" => "Assamese",
        "az" => "Azerbaijani",
        "ba" => "Bashkir",
        "be" => "Belarusian",
        "bg" => "Bulgarian",
        "bn" => "Bengali",
        "bo" => "Tibetan",
        "br" => "Breton",
        "bs" => "Bosnian",
        "ca" => "Catalan",
        "cs" => "Czech",
        "cy" => "Welsh",
        "da" => "Danish",
        "de" => "German",
        "el" => "Greek",
        "en" => "English",
        "es" => "Spanish",
        "et" => "Estonian",
        "eu" => "Basque",
        "fa" => "Persian",
        "fi" => "Finnish",
        "fil" => "Filipino",
        "fr" => "French",
        "gl" => "Galician",
        "gu" => "Gujarati",
        "ha" => "Hausa",
        "haw" => "Hawaiian",
        "he" => "Hebrew",
        "hi" => "Hindi",
        "hr" => "Croatian",
        "ht" => "Haitian Creole",
        "hu" => "Hungarian",
        "hy" => "Armenian",
        "id" => "Indonesian",
        "is" => "Icelandic",
        "it" => "Italian",
        "ja" => "Japanese",
        "jv" | "jw" => "Javanese",
        "ka" => "Georgian",
        "kk" => "Kazakh",
        "km" => "Khmer",
        "kn" => "Kannada",
        "ko" => "Korean",
        "la" => "Latin",
        "lb" => "Luxembourgish",
        "ln" => "Lingala",
        "lo" => "Lao",
        "lt" => "Lithuanian",
        "lv" => "Latvian",
        "mg" => "Malagasy",
        "mi" => "Maori",
        "mk" => "Macedonian",
        "ml" => "Malayalam",
        "mn" => "Mongolian",
        "mr" => "Marathi",
        "ms" => "Malay",
        "mt" => "Maltese",
        "my" => "Myanmar",
        "ne" => "Nepali",
        "nl" => "Dutch",
        "nn" => "Nynorsk",
        "no" => "Norwegian",
        "oc" => "Occitan",
        "pa" => "Punjabi",
        "pl" => "Polish",
        "ps" => "Pashto",
        "pt" => "Portuguese",
        "ro" => "Romanian",
        "ru" => "Russian",
        "sa" => "Sanskrit",
        "sd" => "Sindhi",
        "si" => "Sinhala",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "sn" => "Shona",
        "so" => "Somali",
        "sq" => "Albanian",
        "sr" => "Serbian",
        "su" => "Sundanese",
        "sv" => "Swedish",
        "sw" => "Swahili",
        "ta" => "Tamil",
        "te" => "Telugu",
        "tg" => "Tajik",
        "th" => "Thai",
        "tk" => "Turkmen",
        "tl" => "Tagalog",
        "tr" => "Turkish",
        "tt" => "Tatar",
        "uk" => "Ukrainian",
        "ur" => "Urdu",
        "uz" => "Uzbek",
        "vi" => "Vietnamese",
        "yi" => "Yiddish",
        "yo" => "Yoruba",
        "yue" => "Cantonese",
        "zh" => "Chinese",
        _ => code,
    }
}

fn online_provider_description(provider_id: &str) -> &'static str {
    match provider_id {
        "volcengine-doubao" => {
            "ByteDance in-house speech LLM specialized in Chinese, dialects, and mixed language with rapid Flash batch output."
        }
        "groq-whisper" => {
            "Whisper inference powered by Groq LPU hardware, delivering hundreds of times real-time speed with open-source robustness."
        }
        "mistral-voxtral" => {
            "Mistral multimodal speech architecture specialized in European languages and Chinese, featuring well-formatted punctuation and paragraphs."
        }
        "openai-whisper" => {
            "OpenAI official flagship speech recognition model with strong generalization across specialized terminology and rare words."
        }
        "deepgram" => {
            "Global benchmark for speed and cost efficiency with minimal word error rates in English/Western European languages and built-in diarization."
        }
        "assemblyai" => {
            "Enterprise-grade speech analysis engineered for noisy environments, domain-specific terminology, and multi-speaker overlapping turns."
        }
        "elevenlabs" => {
            "Engineered by leading voice AI lab with high robustness against non-native accents and precise millisecond-level word timestamps."
        }
        "azure-speech" => "Microsoft enterprise speech-to-text and Whisper model service.",
        "google-gemini" => "Google native Gemini multimodal audio understanding and recognition.",
        "xai" => "xAI official speech recognition and transcription endpoint.",
        "gladia" => "High-performance speech-to-text API v2 optimized for real-time and batch.",
        "revai" => "Industry-leading speech-to-text engine for English and global languages.",
        "fal-whisper" => "Serverless low-latency Whisper inference on FAL AI infrastructure.",
        _ => "Audio will be sent to the cloud provider for recognition.",
    }
}

fn online_model_description(provider_id: &str, model_id: &str) -> &'static str {
    match (provider_id, model_id) {
        ("volcengine-doubao", "volc.bigasr.auc_turbo") => {
            "Synchronous ultra-fast response with sub-second latency, ideal for batch audio imports."
        }
        ("volcengine-doubao", "volc.seedasr.sauc.duration") => {
            "Bidirectional real-time streaming with high noise robustness, ideal for live microphone transcription."
        }
        ("groq-whisper", "whisper-large-v3-turbo") => {
            "Powered by Groq LPU hardware inference for ultra-fast real-time transcription speeds."
        }
        ("groq-whisper", "whisper-large-v3") => {
            "Full-precision Whisper v3 model delivering top accuracy and multilingual performance."
        }
        ("groq-whisper", "distil-whisper-large-v3-en") => {
            "Distilled lightweight English Whisper model with ultra-low latency and high throughput."
        }
        ("mistral-voxtral", "mistral-small-latest") => {
            "Mistral multilingual multimodal speech recognition supporting both live streaming and batch imports."
        }
        ("openai-whisper", "whisper-1") => {
            "OpenAI official universal speech recognition model, setting the benchmark for multilingual robustness."
        }
        ("deepgram", "nova-3") => {
            "Deepgram latest flagship speech model with ultra-low latency and top accuracy."
        }
        ("deepgram", "nova-2") => {
            "Production-grade general speech model balancing high stability with speaker diarization."
        }
        ("assemblyai", "enhanced") => {
            "Enhanced audio recognition engine tailored for complex background noise environments."
        }
        ("assemblyai", "best") => {
            "AssemblyAI flagship high-accuracy model specialized for long-form speech and multi-speaker meetings."
        }
        ("assemblyai", "nano") => {
            "Lightweight and cost-efficient model delivering rapid responses with low latency."
        }
        ("elevenlabs", "scribe_v1") => {
            "ElevenLabs speech model featuring fine phoneme alignment across 99+ languages."
        }
        ("azure-speech", "whisper-1") => {
            "Azure Cognitive Services enterprise speech-to-text with precision speaker diarization."
        }
        ("google-gemini", "gemini-2.0-flash") => {
            "Google native Gemini multimodal audio recognition with deep acoustic scene understanding."
        }
        ("google-gemini", "gemini-1.5-flash") => {
            "Google ultra-lightweight high-concurrency audio recognition with low latency and high throughput."
        }
        ("xai", "default") => {
            "Official xAI Grok speech transcription model featuring high concurrency and robustness."
        }
        ("gladia", "default") => {
            "Gladia v2 multilingual engine featuring fast audio transcription and speaker diarization."
        }
        ("revai", "default") => {
            "Rev.ai high-accuracy speech engine with professional accent and speaker diarization support."
        }
        ("fal-whisper", "whisper") => {
            "FAL.ai cloud serverless Whisper inference delivering elastic sub-second responses."
        }
        _ => "Cloud speech recognition model.",
    }
}

fn online_provider_status_badge(
    has_api_key: bool,
    added_count: usize,
    total_models: usize,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    let is_added = total_models > 0 && added_count == total_models;
    let is_partial = added_count > 0 && added_count < total_models;
    let label = if !has_api_key && added_count == 0 {
        "Not Configured".to_string()
    } else if added_count == 0 {
        "Not added".to_string()
    } else if is_added {
        "Added".to_string()
    } else {
        format!("{added_count}/{total_models} added")
    };
    let is_installed = is_added || is_partial;

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(10.0))
        .py(px(2.0))
        .rounded_full()
        .border_1()
        .border_color(if is_installed {
            cx.theme().border.opacity(0.0)
        } else {
            cx.theme().border
        })
        .bg(if is_installed {
            cx.theme().success.opacity(0.14)
        } else {
            cx.theme().background
        })
        .text_size(px(11.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if is_installed {
            cx.theme().success
        } else {
            cx.theme().muted
        })
        .when(is_installed, |badge| {
            badge.child(
                Icon::new(LucideIcon::Check)
                    .size(px(11.0))
                    .text_color(cx.theme().success),
            )
        })
        .child(label)
}

fn online_provider_is_flash_batch_mode(
    provider: &sona_core::ports::asr::OnlineAsrProvider,
    config: Option<&Value>,
) -> bool {
    let endpoint = config
        .and_then(|config| config.get("batchEndpoint"))
        .and_then(Value::as_str)
        .or_else(|| {
            provider
                .defaults
                .get("batchEndpoint")
                .and_then(Value::as_str)
        })
        .unwrap_or(&provider.batch.local_file_mode.endpoint);
    let resource_id = config
        .and_then(|config| config.get("batchResourceId"))
        .and_then(Value::as_str)
        .or_else(|| {
            provider
                .defaults
                .get("batchResourceId")
                .and_then(Value::as_str)
        })
        .unwrap_or(&provider.batch.local_file_mode.resource_id);
    endpoint == provider.batch.local_file_mode.endpoint
        && resource_id == provider.batch.local_file_mode.resource_id
}

fn model_selector_row(
    title: &'static str,
    field: &'static str,
    selected: &str,
    options: &[(String, String)],
    open: bool,
    disabled: bool,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    let selected_label = if selected.is_empty() {
        "Off".to_string()
    } else {
        options
            .iter()
            .find(|(id, _)| id == selected)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| "Select model...".to_string())
    };
    let options = options.to_vec();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w(px(260.0))
                .child(
                    Button::new(ElementId::Name(
                        format!("settings-model-select-{field}").into(),
                    ))
                    .outline()
                    .small()
                    .label(selected_label)
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_modal.model_selector_open =
                            (this.settings_modal.model_selector_open.as_deref() != Some(field))
                                .then_some(field.to_string());
                        cx.notify();
                    })),
                )
                .when(open, |menu| {
                    menu.children(
                        std::iter::once((String::new(), "Off".to_string()))
                            .chain(options)
                            .map(|(value, label)| {
                                let selected_value = value.clone();
                                div().child(
                                    Button::new(ElementId::Name(
                                        format!("settings-model-option-{field}-{value}").into(),
                                    ))
                                    .ghost()
                                    .small()
                                    .label(label)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.settings_modal.choose_model_option(
                                                &mut this.global_dialog,
                                                field,
                                                selected_value.clone(),
                                                cx,
                                            );
                                        },
                                    )),
                                )
                            }),
                    )
                }),
        )
}

fn online_provider_grid(
    expanded_provider_id: Option<&str>,
    model_config: &Value,
    revealed_api_keys: &std::collections::HashSet<String>,
    test_states: &HashMap<String, OnlineProviderTestState>,
    field_states: &HashMap<String, Entity<InputState>>,
    cx: &mut Context<MainShell>,
) -> impl IntoElement {
    let providers = sona_core::ports::asr::online_asr_providers();
    let provider_count = providers.len();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().border)
        .children(providers.iter().enumerate().map(|(provider_index, provider)| {
        let provider_id = provider.id.clone();
        let provider_config = model_config
            .pointer("/asr/providers/online")
            .and_then(|providers| providers.get(&provider_id))
            .or_else(|| model_config.pointer(&format!("/asr/providers/{provider_id}")));
        let has_api_key = provider_config
            .and_then(|config| config.get("apiKey"))
            .and_then(Value::as_str)
            .is_some_and(|key| !key.trim().is_empty());
        let added_ids = online_provider_added_model_ids(provider, provider_config);
        let total_models = provider.models.len();
        let added_count = provider
            .models
            .iter()
            .filter(|model| added_ids.contains(model.id.as_str()))
            .count();
        let is_expanded = expanded_provider_id == Some(provider_id.as_str());
        let title = online_provider_default_name(&provider_id).to_string();
        let logo_path = online_provider_logo_path(&provider_id, &title);
        let description = online_provider_description(&provider_id).to_string();
        let language_tooltip = online_provider_languages_tooltip(&provider.languages);
        let supports_streaming = provider.streaming.supported != Some(false);
        let supports_batch = provider.batch.local_file_mode.supported;
        let supports_speaker_diarization = matches!(
            provider_id.as_str(),
            "volcengine-doubao"
                | "deepgram"
                | "assemblyai"
                | "elevenlabs"
                | "openai-whisper"
                | "azure-speech"
                | "gladia"
                | "revai"
        );
        let mut common_tag_tokens = vec!["cloud".to_string()];
        if supports_streaming {
            common_tag_tokens.extend(["live".to_string(), "streaming".to_string()]);
        }
        if supports_batch {
            common_tag_tokens.push("batch".to_string());
        }
        if supports_speaker_diarization {
            common_tag_tokens.extend([
                "diarization".to_string(),
                "speaker diarization".to_string(),
                "speaker labels".to_string(),
                "smart diarization".to_string(),
                "区分说话人".to_string(),
            ]);
        }
        let tooltip_languages = language_tooltip.clone();
        let language_badges = div()
                .id(ElementId::Name(
                    format!("online-provider-languages-{provider_id}").into(),
                ))
                .tooltip(move |window, cx| {
                    Tooltip::new(tooltip_languages.clone()).build(window, cx)
                })
                .tab_index(0)
                .aria_label(language_tooltip.clone())
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
                .children(provider.languages.iter().take(4).map(|language| {
                    div()
                        .px_2()
                        .h(px(20.0))
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().background.opacity(0.0))
                        .bg(cx.theme().secondary)
                        .text_size(px(11.5))
                        .text_color(cx.theme().muted)
                        .child(language.to_uppercase())
                }))
                .when(provider.languages.len() > 4, |row| {
                    row.child(
                        div()
                            .px_2()
                            .h(px(20.0))
                            .rounded_full()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .text_size(px(11.5))
                            .text_color(cx.theme().muted)
                            .child(format!("+{}", provider.languages.len() - 4)),
                    )
                });
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .py(px(16.0))
            .px(px(24.0))
            .bg(cx.theme().background)
            .when(provider_index == 0, |card| {
                card.rounded_tl(px(15.0)).rounded_tr(px(15.0))
            })
            .when(provider_index + 1 == provider_count, |card| {
                card.rounded_bl(px(15.0)).rounded_br(px(15.0))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        Button::new(ElementId::Name(
                            format!("online-provider-toggle-{provider_id}").into(),
                        ))
                            .ghost()
                            .small()
                            .w_full()
                            .h(px(36.0))
                            .px_0()
                            .py_0()
                            .flex()
                            .items_center()
                            .justify_between()
                            .accessibility_label(format!(
                                "{} — {added_count}/{total_models}",
                                online_provider_default_name(&provider_id)
                            ))
                            .hover(|style| style.text_color(cx.theme().accent))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .rounded_md()
                                            .overflow_hidden()
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().secondary)
                                            .shadow_sm()
                                            .when_some(logo_path, |badge, path| {
                                                badge.child(
                                                    img(path)
                                                        .w(px(36.0))
                                                        .h(px(36.0))
                                                        .object_fit(ObjectFit::Cover),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .flex_wrap()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_size(px(16.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(title),
                                            )
                                            .child(language_badges.into_any_element()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(online_provider_status_badge(
                                        has_api_key,
                                        added_count,
                                        total_models,
                                        cx,
                                    ))
                                    .child(
                                        Icon::new(if is_expanded {
                                            LucideIcon::ChevronDown
                                        } else {
                                            LucideIcon::ChevronRight
                                        })
                                        .size(px(16.0))
                                        .text_color(cx.theme().muted),
                                    ),
                            )
                            .on_click(cx.listener({
                                let provider_id = provider_id.clone();
                                move |this, _, _, cx| {
                                    this.settings_modal.expanded_online_provider =
                                        (this.settings_modal.expanded_online_provider.as_deref()
                                            != Some(provider_id.as_str()))
                                        .then_some(provider_id.clone());
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .mt(px(10.0))
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(12.8))
                                    .text_color(cx.theme().muted)
                                    .child(description),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(6.0))
                                    .when(supports_streaming, |row| {
                                        row.child(
                                            div()
                                                .px_2()
                                                .py(px(2.0))
                                                .rounded_full()
                                                .bg(cx.theme().info.opacity(0.1))
                                                .text_size(px(11.5))
                                                .text_color(cx.theme().info)
                                                .child("Live"),
                                        )
                                    })
                                    .when(supports_batch, |row| {
                                        row.child(
                                            div()
                                                .px_2()
                                                .py(px(2.0))
                                                .rounded_full()
                                                .bg(cx.theme().info.opacity(0.1))
                                                .text_size(px(11.5))
                                                .text_color(cx.theme().info)
                                                .child("Batch"),
                                        )
                                    })
                                    .when(supports_speaker_diarization, |row| {
                                        row.child(
                                            div()
                                                .px_2()
                                                .py(px(2.0))
                                                .rounded_full()
                                                .bg(cx.theme().accent.opacity(0.08))
                                                .text_size(px(11.5))
                                                .text_color(cx.theme().accent)
                                                .child("Speaker Diarization"),
                                        )
                                    }),
                            ),
                    )
                    .when(is_expanded, |row| {
                        let provider_id = provider_id.clone();
                        row.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .w_full()
                                .mt(px(16.0))
                                .mx(px(-24.0))
                                .mb(px(-16.0))
                                .p(px(16.0))
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().group_box)
                                .when_some(field_states.get(&format!("{provider_id}::apiKey")), |panel, state| {
                                    let api_key_state = state.clone();
                                    panel.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .w_full()
                                            .max_w(px(480.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .text_size(px(14.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().muted)
                                                    .child("API Key")
                                                    .when_some(provider.spec.as_ref().and_then(|spec| spec.console_url.clone()), |row, url| {
                                                        let console_url = url.clone();
                                                        row.child(
                                                            Button::new(ElementId::Name(format!("online-provider-console-{provider_id}").into()))
                                                                .ghost()
                                                                .small()
                                                                .label("Get API Key from Console")
                                                                .tooltip("Get API Key from Console")
                                                                .on_click(cx.listener(move |_, _, _, cx| {
                                                                    let url = console_url.clone();
                                                                    cx.spawn(async move |_, _cx: &mut AsyncApp| {
                                                                        if let Err(error) = crate::platform::open_url_async(url).await {
                                                                            log::warn!("Failed to open online provider console: {error}");
                                                                        }
                                                                    }).detach();
                                                                })),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .h(px(32.0))
                                                    .px_2()
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(cx.theme().border)
                                                    .bg(cx.theme().input)
                                                    .child(Input::new(state).small())
                                                    .child(
                                                        Button::new(ElementId::Name(
                                                            format!("online-provider-key-visibility-{provider_id}").into(),
                                                        ))
                                                        .ghost()
                                                        .small()
                                                        .w(px(28.0))
                                                        .h(px(28.0))
                                                        .icon(if revealed_api_keys.contains(&provider_id) {
                                                            LucideIcon::EyeOff
                                                        } else {
                                                            LucideIcon::Eye
                                                        })
                                                        .tooltip(if revealed_api_keys.contains(&provider_id) {
                                                            "Hide password"
                                                        } else {
                                                            "Show password"
                                                        })
                                                        .accessibility_label(if revealed_api_keys.contains(&provider_id) {
                                                            "Hide password"
                                                        } else {
                                                            "Show password"
                                                        })
                                                        .on_click(cx.listener({
                                                            let provider_id = provider_id.clone();
                                                            move |this, _, window, cx| {
                                                            let revealed = this.settings_modal.online_provider_api_key_revealed.contains(&provider_id);
                                                            if revealed {
                                                                this.settings_modal.online_provider_api_key_revealed.remove(&provider_id);
                                                            } else {
                                                                this.settings_modal.online_provider_api_key_revealed.insert(provider_id.clone());
                                                            }
                                                            api_key_state.update(cx, |state, cx| {
                                                                state.set_masked(revealed, window, cx);
                                                            });
                                                            cx.notify();
                                                            }
                                                        })),
                                                    ),
                                            ),
                                    )
                                })
                                .when(provider_id == "volcengine-doubao", |panel| {
                                    let endpoint = provider.batch.local_file_mode.endpoint.clone();
                                    let resource_id = provider.batch.local_file_mode.resource_id.clone();
                                    let is_flash = online_provider_is_flash_batch_mode(provider, provider_config);
                                    panel.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Recording File Mode"))
                                            .child(div().text_size(px(11.0)).text_color(cx.theme().muted).child("Local batch import supports Flash only. Other modes require a public audio URL."))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        Button::new(ElementId::Name("online-provider-volcengine-mode-flash".into()))
                                                            .when(is_flash, |button| button.primary())
                                                            .when(!is_flash, |button| button.outline())
                                                            .small()
                                                            .label("Flash (Sync Direct)")
                                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                                this.settings_modal.set_online_provider_config_fields(
                                                                    "volcengine-doubao",
                                                                    &[("batchEndpoint", endpoint.as_str()), ("batchResourceId", resource_id.as_str())],
                                                                    cx,
                                                                );
                                                            })),
                                                    )
                                                    .child(
                                                        Button::new(ElementId::Name("online-provider-volcengine-mode-standard".into()))
                                                            .outline()
                                                            .small()
                                                            .label("Standard (Async Polling)")
                                                            .disabled(true)
                                                            .tooltip("Requires a public audio URL; local batch import does not support it yet."),
                                                    )
                                                    .child(
                                                        Button::new(ElementId::Name("online-provider-volcengine-mode-offpeak".into()))
                                                            .outline()
                                                            .small()
                                                            .label("Off-peak (Discount Async)")
                                                            .disabled(true)
                                                            .tooltip("Requires a public audio URL; local batch import does not support it yet."),
                                                    ),
                                            ),
                                    )
                                })
                                .children(provider.ui.fields.iter().filter(|field| field.name != "apiKey" && field.name != "model").filter_map(|field| {
                                    let state_key = format!("{provider_id}::{}", field.name);
                                    let state = field_states.get(&state_key)?;
                                    let label = field.label_key.rsplit('.').next().unwrap_or(&field.name).replace('_', " ");
                                    Some(div().flex().flex_col().gap_1().w_full().max_w(px(480.0))
                                        .child(div().text_size(px(11.0)).text_color(cx.theme().muted).child(label))
                                        .child(div().flex().items_center().h(px(32.0)).px_2().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().input).child(Input::new(state).small())))
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .mt(px(16.0))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .px(px(2.0))
                                                .child(
                                                    div()
                                                        .text_size(px(12.8))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(cx.theme().muted)
                                                        .child("SUPPORTED MODELS"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(cx.theme().muted)
                                                        .child(format!("{added_count}/{total_models} added")),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .rounded_lg()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .bg(cx.theme().popover)
                                                .shadow_sm()
                                                .children(provider.models.iter().enumerate().map(|(model_index, model)| {
                                            let model_id = model.id.clone();
                                            let model_name = model.name.clone();
                                            let is_default = model.is_default == Some(true);
                                            let is_added = added_ids.contains(model.id.as_str());
                                            let description = online_model_description(&provider_id, &model_id).to_string();
                                            let display_tags = model
                                                .tags
                                                .iter()
                                                .filter(|tag| !common_tag_tokens.contains(&tag.to_lowercase()))
                                                .cloned()
                                                .collect::<Vec<_>>();
                                            let test_key = format!("{provider_id}::{model_id}");
                                            let test_state = test_states.get(&test_key);
                                            let is_testing = matches!(test_state, Some(OnlineProviderTestState::Loading));
                                            let (test_icon, test_tooltip) = match test_state {
                                                Some(OnlineProviderTestState::Loading) => (LucideIcon::Zap, "Verifying...".to_string()),
                                                Some(OnlineProviderTestState::Success { latency_ms }) => (
                                                    LucideIcon::Check,
                                                    format!("✓ Connection verified ({latency_ms}ms)"),
                                                ),
                                                Some(OnlineProviderTestState::Error { message }) => (
                                                    LucideIcon::CircleAlert,
                                                    format!("✕ {message}"),
                                                ),
                                                None => (LucideIcon::Zap, "Test Connection".to_string()),
                                            };
                                            let test_provider_id = provider_id.clone();
                                            let test_model_id = model_id.clone();
                                            let provider_id = provider_id.clone();
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .px(px(16.0))
                                                .py(px(12.0))
                                                .when(model_index > 0, |row| {
                                                    row.border_t_1().border_color(cx.theme().border)
                                                })
                                                .hover(|style| style.bg(cx.theme().secondary.opacity(0.3)))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .flex_wrap()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .flex()
                                                            .flex_row()
                                                            .flex_wrap()
                                                            .items_center()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .id(ElementId::Name(
                                                                        format!("online-provider-model-name-{provider_id}-{model_id}").into(),
                                                                    ))
                                                                    .tooltip(move |window, cx| {
                                                                        Tooltip::new(description.clone()).build(window, cx)
                                                                    })
                                                                    .text_size(px(14.0))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .child(model_name.clone()),
                                                            )
                                                            .children(display_tags.iter().map(|tag| {
                                                                div()
                                                    .h(px(20.0))
                                                                    .px_2()
                                                                    .rounded_full()
                                                    .border_1()
                                                    .border_color(cx.theme().background.opacity(0.0))
                                                    .bg(cx.theme().secondary)
                                                                    .text_size(px(11.5))
                                                                    .text_color(cx.theme().muted)
                                                                    .child(tag.clone())
                                                            }))
                                                            .when(is_added, |row| {
                                                                row.child(
                                                                    Icon::new(LucideIcon::Check)
                                                                        .size(px(13.0))
                                                                        .text_color(cx.theme().success),
                                                                )
                                                            }),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap_1()
                                                                .child(
                                                                    Button::new(ElementId::Name(
                                                                        format!("online-provider-test-{provider_id}-{model_id}").into(),
                                                                    ))
                                                                    .ghost()
                                                                    .small()
                                                                    .w(px(28.0))
                                                                    .h(px(28.0))
                                                                    .icon(test_icon)
                                                                    .loading(is_testing)
                                                                    .when(
                                                                        matches!(test_state, Some(OnlineProviderTestState::Success { .. })),
                                                                        |button| button.text_color(cx.theme().success),
                                                                    )
                                                                    .when(
                                                                        matches!(test_state, Some(OnlineProviderTestState::Error { .. })),
                                                                        |button| button.text_color(cx.theme().danger),
                                                                    )
                                                                    .tooltip(test_tooltip)
                                                                    .accessibility_label(format!("Test Connection {}", model_name))
                                                                    .disabled(!has_api_key || is_testing)
                                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                                        this.settings_modal.test_online_provider_model(
                                                                            test_provider_id.clone(),
                                                                            test_model_id.clone(),
                                                                            cx,
                                                                        );
                                                                    })),
                                                                )
                                                                .child(
                                                                    Button::new(ElementId::Name(
                                                                        format!("online-provider-model-{provider_id}-{model_id}").into(),
                                                                    ))
                                                                    .ghost()
                                                                    .small()
                                                                    .w(px(28.0))
                                                                    .h(px(28.0))
                                                                    .icon(if is_added { LucideIcon::Trash } else { LucideIcon::Plus })
                                                                    .tooltip(if is_added {
                                                                        "Delete Model"
                                                                    } else if !has_api_key {
                                                                        "Configure API Key first"
                                                                    } else {
                                                                        "Add Model"
                                                                    })
                                                                    .accessibility_label(format!(
                                                                        "{} {}",
                                                                        if is_added { "Delete" } else { "Add" },
                                                                        model_name
                                                                    ))
                                                                    .when(is_added, |button| {
                                                                        button.hover(|style| {
                                                                            style
                                                                                .bg(cx.theme().danger.opacity(0.1))
                                                                                .text_color(cx.theme().danger)
                                                                        })
                                                                    })
                                                                    .when(!is_added && has_api_key, |button| {
                                                                        button.hover(|style| {
                                                                            style
                                                                                .bg(cx.theme().success.opacity(0.1))
                                                                                .text_color(cx.theme().success)
                                                                        })
                                                                    })
                                                                    .disabled(!has_api_key && !is_added)
                                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                                        this.settings_modal.set_online_provider_model_added(
                                                                            &provider_id,
                                                                            &model_id,
                                                                            !is_added,
                                                                            is_default,
                                                                            cx,
                                                                        );
                                                                    })),
                                                                ),
                                                        ),
                                                )
                                        })),
                                        ),
                                )
                        )
                    }),
            )
    }))
}

fn model_toggle_row(
    title: &'static str,
    key: &'static str,
    checked: bool,
    disabled: bool,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().text_size(px(13.0)).child(title))
        .child(
            Switch::new(ElementId::Name(
                format!("settings-model-toggle-{key}").into(),
            ))
            .checked(checked)
            .disabled(disabled)
            .on_change(cx.listener(move |this, value, _, cx| {
                this.settings_modal
                    .persist_model_setting(key.to_string(), Value::Bool(*value), cx);
            })),
        )
}

fn model_numeric_row(
    title: &'static str,
    key: &'static str,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().text_size(px(13.0)).child(title))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new(ElementId::Name(format!("settings-model-dec-{key}").into()))
                        .outline()
                        .small()
                        .label(if step.fract() == 0.0 { "-" } else { "-0.5" })
                        .disabled(value <= min)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let current = this
                                .settings_modal
                                .model_config
                                .get(key)
                                .and_then(Value::as_f64)
                                .unwrap_or(value);
                            this.settings_modal.persist_model_setting(
                                key.to_string(),
                                Value::from((current - step).max(min)),
                                cx,
                            );
                        })),
                )
                .child(
                    div()
                        .min_w(px(28.0))
                        .text_center()
                        .text_size(px(13.0))
                        .child(if value.fract() == 0.0 {
                            format!("{value:.0}")
                        } else {
                            value.to_string()
                        }),
                )
                .child(
                    Button::new(ElementId::Name(format!("settings-model-inc-{key}").into()))
                        .outline()
                        .small()
                        .label(if step.fract() == 0.0 { "+" } else { "+0.5" })
                        .disabled(value >= max)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let current = this
                                .settings_modal
                                .model_config
                                .get(key)
                                .and_then(Value::as_f64)
                                .unwrap_or(value);
                            this.settings_modal.persist_model_setting(
                                key.to_string(),
                                Value::from((current + step).min(max)),
                                cx,
                            );
                        })),
                ),
        )
}

fn model_sensitivity_row(
    selected: &str,
    disabled: bool,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().text_size(px(13.0)).child("Separation Sensitivity"))
        .child(
            div().flex().items_center().gap_1().children(
                ["permissive", "balanced", "strict"]
                    .into_iter()
                    .map(|value| {
                        let is_selected = value == selected;
                        let label = match value {
                            "permissive" => "Permissive",
                            "strict" => "Strict",
                            _ => "Balanced",
                        };
                        Button::new(ElementId::Name(
                            format!("settings-sensitivity-{value}").into(),
                        ))
                        .when(is_selected, |button| button.primary())
                        .when(!is_selected, |button| button.ghost())
                        .small()
                        .label(label)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings_modal.persist_model_setting(
                                "speakerDiarizationSensitivity".to_string(),
                                Value::String(value.to_string()),
                                cx,
                            );
                        }))
                    }),
            ),
        )
}

fn audio_section_title(
    title: &'static str,
    description: &'static str,
    cx: &Context<MainShell>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(cx.theme().muted)
                .child(description),
        )
}

fn audio_row(title: &'static str, hint: &'static str, cx: &Context<MainShell>) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted)
                        .child(hint),
                ),
        )
}

fn audio_toggle_row(
    title: &'static str,
    hint: &'static str,
    enabled: bool,
    id: &'static str,
    cx: &Context<MainShell>,
    key: &'static str,
) -> gpui_kit::Div {
    audio_row(title, hint, cx).child(
        Switch::new(id)
            .checked(enabled)
            .accessibility_label(title)
            .on_change(cx.listener(move |this, checked, _, cx| {
                let value = *checked;
                match key {
                    "keepMicrophoneActive" => this.settings_modal.keep_microphone_active = value,
                    "muteDuringRecording" => this.settings_modal.mute_during_recording = value,
                    "ffmpegEnabled" => this.settings_modal.ffmpeg_enabled = value,
                    _ => return,
                }
                this.settings_modal
                    .persist_audio_value(key, Value::Bool(value), cx);
                cx.notify();
            })),
    )
}

fn shortcut_section(
    title: &'static str,
    shortcuts: Vec<(&'static str, &str)>,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .when(!title.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
        })
        .children(shortcuts.into_iter().map(|(description, keys)| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .py_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(cx.theme().foreground)
                        .child(description),
                )
                .child(div().flex().items_center().gap_1().children(
                    keys.split(" / ").enumerate().flat_map(|(index, key)| {
                        let mut elements = vec![
                            div()
                                .min_w(px(24.0))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().input)
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_center()
                                .child(key.to_string())
                                .into_any_element(),
                        ];
                        if index + 1 < keys.split(" / ").count() {
                            elements.push(
                                div()
                                    .mx_1()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted)
                                    .child("/")
                                    .into_any_element(),
                            );
                        }
                        elements
                    }),
                ))
        }))
}

#[cfg(test)]
mod tests {
    use super::{
        SettingsModal, SettingsTab, apply_llm_model_metadata_inputs,
        online_provider_added_model_ids, online_provider_default_name,
        online_provider_is_flash_batch_mode, online_provider_languages_tooltip,
        online_provider_logo_path, online_provider_model_options, remove_custom_provider_settings,
        remove_llm_model_settings, update_online_provider_model_config,
    };
    use gpui_kit::AssetSource as _;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn deleting_custom_provider_cleans_its_models_and_feature_selections() {
        let mut settings = json!({
            "activeProvider": "custom-acme",
            "customProviders": {
                "custom-acme": { "name": "Acme", "strategy": "openai_compatible" }
            },
            "providers": {
                "custom-acme": { "apiHost": "https://acme.example", "apiKey": "secret" },
                "open_ai": { "apiHost": "https://api.openai.com", "apiKey": "key" }
            },
            "models": {
                "acme-a": { "provider": "custom-acme", "model": "a" },
                "openai-b": { "provider": "open_ai", "model": "b" }
            },
            "modelOrder": ["acme-a", "openai-b"],
            "selections": {
                "polishModelId": "acme-a",
                "summaryModelId": "openai-b",
                "polishTemperature": 0.4
            }
        });

        assert!(remove_custom_provider_settings(
            &mut settings,
            "custom-acme"
        ));
        assert_eq!(settings["activeProvider"], "google_translate_free");
        assert!(settings["customProviders"].get("custom-acme").is_none());
        assert!(settings["providers"].get("custom-acme").is_none());
        assert!(settings["models"].get("acme-a").is_none());
        assert_eq!(settings["models"]["openai-b"]["model"], "b");
        assert_eq!(settings["modelOrder"], json!(["openai-b"]));
        assert!(settings["selections"].get("polishModelId").is_none());
        assert_eq!(settings["selections"]["summaryModelId"], "openai-b");
        assert_eq!(settings["selections"]["polishTemperature"], 0.4);
        assert!(!remove_custom_provider_settings(
            &mut settings,
            "custom-acme"
        ));
    }

    #[test]
    fn deleting_llm_model_removes_only_its_feature_selections() {
        let mut settings = json!({
            "models": {
                "model-a": { "provider": "open_ai", "model": "a" },
                "model-b": { "provider": "anthropic", "model": "b" }
            },
            "modelOrder": ["model-a", "model-b"],
            "selections": {
                "polishModelId": "model-a",
                "translationModelId": "model-a",
                "summaryModelId": "model-b",
                "polishTemperature": 0.8
            }
        });

        assert!(remove_llm_model_settings(&mut settings, "model-a"));
        assert!(settings["models"].get("model-a").is_none());
        assert_eq!(settings["modelOrder"], json!(["model-b"]));
        assert!(settings["selections"].get("polishModelId").is_none());
        assert!(settings["selections"].get("translationModelId").is_none());
        assert_eq!(settings["selections"]["summaryModelId"], "model-b");
        assert_eq!(settings["selections"]["polishTemperature"], 0.8);
        assert!(!remove_llm_model_settings(&mut settings, "missing"));
    }

    #[test]
    fn llm_metadata_editor_validates_numbers_and_preserves_catalog_fields() {
        let current = json!({
            "supportsReasoning": true,
            "reasoningMode": { "type": "effort", "levels": ["low", "high"] },
            "metadataSources": ["provider"]
        });
        let valid = HashMap::from([
            ("displayName".to_string(), "  Model X  ".to_string()),
            ("contextWindow".to_string(), "128000".to_string()),
            ("inputPrice".to_string(), "0.25".to_string()),
        ]);
        let updated = apply_llm_model_metadata_inputs(&current, &valid).unwrap();
        assert_eq!(updated["displayName"], "Model X");
        assert_eq!(updated["contextWindow"].as_f64(), Some(128000.0));
        assert_eq!(updated["inputPrice"].as_f64(), Some(0.25));
        assert_eq!(updated["supportsReasoning"], true);
        assert_eq!(updated["reasoningMode"]["type"], "effort");
        assert_eq!(updated["metadataSources"][0], "provider");

        let invalid = HashMap::from([("outputPrice".to_string(), "-1".to_string())]);
        assert!(apply_llm_model_metadata_inputs(&current, &invalid).is_err());
    }

    #[test]
    fn settings_tab_navigation_wraps_in_both_directions() {
        assert_eq!(
            SettingsModal::next_tab(SettingsTab::General, true),
            SettingsTab::Mcp
        );
        let cycle = [
            SettingsTab::General,
            SettingsTab::Dashboard,
            SettingsTab::Shortcuts,
            SettingsTab::About,
            SettingsTab::Microphone,
            SettingsTab::Models,
            SettingsTab::Subtitle,
            SettingsTab::Vocabulary,
            SettingsTab::LlmService,
            SettingsTab::Automation,
            SettingsTab::Storage,
            SettingsTab::Sync,
            SettingsTab::ApiServer,
            SettingsTab::Mcp,
        ];
        for (index, tab) in cycle.iter().enumerate() {
            assert_eq!(
                SettingsModal::next_tab(*tab, false),
                cycle[(index + 1) % cycle.len()],
            );
        }
    }

    #[test]
    fn every_online_provider_uses_its_original_brand_asset() {
        let assets = crate::assets::AppAssets;
        for provider in sona_core::ports::asr::online_asr_providers() {
            let name = online_provider_default_name(&provider.id);
            let path = online_provider_logo_path(&provider.id, name)
                .unwrap_or_else(|| panic!("missing brand mapping for {}", provider.id));
            assert!(
                assets.load(path).unwrap().is_some(),
                "missing embedded brand asset {path} for {}",
                provider.id
            );
        }
    }

    #[test]
    fn online_provider_language_tooltips_localize_and_summarize_languages() {
        assert_eq!(
            online_provider_languages_tooltip(&["en".into(), "zh".into()]),
            "English, Chinese"
        );
        let long_list = (0..25).map(|_| "en".to_string()).collect::<Vec<_>>();
        assert_eq!(
            online_provider_languages_tooltip(&long_list),
            format!(
                "{} ... (25 languages in total)",
                vec!["English"; 24].join(", ")
            )
        );
    }

    #[test]
    fn online_provider_add_preserves_implicit_defaults() {
        let provider = sona_core::ports::asr::find_online_asr_provider("groq-whisper").unwrap();
        let provider_config = json!({ "apiKey": "test-key" });
        let defaults = online_provider_added_model_ids(provider, Some(&provider_config));
        assert!(!defaults.is_empty());
        assert!(defaults.iter().all(|id| {
            provider
                .models
                .iter()
                .any(|model| model.id == *id && model.is_default == Some(true))
        }));
        let additional_model = provider
            .models
            .iter()
            .find(|model| model.is_default != Some(true))
            .expect("Groq exposes optional non-default models")
            .id
            .clone();
        let mut config = json!({
            "asr": { "providers": { "online": { "groq-whisper": { "apiKey": "test-key" } } } }
        });

        update_online_provider_model_config(
            &mut config,
            &provider.id,
            &additional_model,
            true,
            false,
        );

        let added = online_provider_added_model_ids(
            provider,
            config.pointer("/asr/providers/online/groq-whisper"),
        );
        assert!(defaults.iter().all(|id| added.contains(id)));
        assert!(added.contains(&additional_model));
    }

    #[test]
    fn explicit_empty_online_model_list_stays_empty() {
        let provider = sona_core::ports::asr::find_online_asr_provider("groq-whisper").unwrap();
        let config = json!({ "apiKey": "test-key", "addedModels": [] });
        assert!(online_provider_added_model_ids(provider, Some(&config)).is_empty());
        assert!(
            online_provider_model_options(
                &json!({ "asr": { "providers": { "online": { "groq-whisper": config } } } }),
                true,
                None,
            )
            .is_empty()
        );
    }

    #[test]
    fn volcengine_batch_mode_uses_manifest_defaults_and_saved_configuration() {
        let provider = sona_core::ports::asr::find_online_asr_provider("volcengine-doubao")
            .expect("Volcengine provider is in the shared manifest");
        assert!(online_provider_is_flash_batch_mode(provider, None));

        let non_flash = json!({
            "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit",
            "batchResourceId": "volc.bigasr.auc"
        });
        assert!(!online_provider_is_flash_batch_mode(
            provider,
            Some(&non_flash)
        ));

        let flash = json!({
            "batchEndpoint": provider.batch.local_file_mode.endpoint,
            "batchResourceId": provider.batch.local_file_mode.resource_id
        });
        assert!(online_provider_is_flash_batch_mode(provider, Some(&flash)));
    }

    #[test]
    fn removing_online_model_clears_only_its_active_selection() {
        let provider = sona_core::ports::asr::online_asr_providers()
            .iter()
            .find(|provider| {
                provider
                    .models
                    .iter()
                    .any(|model| model.is_default == Some(true))
                    && provider
                        .models
                        .iter()
                        .any(|model| model.is_default != Some(true))
            })
            .expect("at least one online provider has optional models");
        let default_model = provider
            .models
            .iter()
            .find(|model| model.is_default == Some(true))
            .unwrap();
        let other_model = provider
            .models
            .iter()
            .find(|model| model.is_default != Some(true))
            .unwrap();
        let mut config = json!({
            "asr": {
                "providers": { "online": { (provider.id.clone()): { "apiKey": "test-key", "addedModels": [default_model.id, other_model.id] } } },
                "selections": {
                    "live": { "engine": "online", "providerId": provider.id, "mode": "streaming", "modelId": null },
                    "batch": { "engine": "online", "providerId": provider.id, "mode": "batch", "modelId": other_model.id }
                }
            }
        });

        update_online_provider_model_config(
            &mut config,
            &provider.id,
            &default_model.id,
            false,
            true,
        );

        assert_eq!(
            config
                .pointer("/asr/selections/live/engine")
                .and_then(serde_json::Value::as_str),
            Some("local")
        );
        assert_eq!(
            config
                .pointer("/asr/selections/batch/modelId")
                .and_then(serde_json::Value::as_str),
            Some(other_model.id.as_str())
        );
    }
}
