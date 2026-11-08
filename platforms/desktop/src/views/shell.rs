//! Complete Main Application Shell for Sona Desktop.
//! Provides strict 1:1 parity with the React frontend, powered by pure Rust and GPUI Kit.
//!
//! Milestone 2 Features:
//! - 56px 3-column header grid (1fr auto 1fr) with 24px padding (Gap G01, G05)
//! - ProjectSelectorDropdown with swatch, pipeline Zap, and counts (Gap G02)
//! - 3-Mode TabNavigation with Lucide SVG icons, sliding pill, and Ctrl+Tab cycling (Gap G03)
//! - SyncHeaderPill with 6 lifecycle states and sync actions (Gap G04)
//! - NotificationCenter with live task ledger grouping and red badge (Gap G04)
//! - Strict 350px left panel layout and sliding Transcript Workbench host (Gap G01, G05)

use std::collections::HashMap;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::IndexPath;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, TextareaState};
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{StatefulInteractiveElement as _, *};
use sona_core::history::HistoryWorkspaceScope;
use sona_core::history_store::HistoryStore;
use sona_core::project::ProjectRecord;
use sona_core::sync::{SyncLifecycleState, SyncStatusSnapshot};

use super::create::{CreateWorkspacePanel, OptionsCapsule, TranscriptionOptionsModel};
use super::dialogs::{
    DiagnosticsModal, ErrorDialog, ExportModal, GlobalDialog, NotificationCenter, OnboardingWizard,
    ProjectDialogs, RecoveryModal,
};
use super::projects::{
    ProjectsHeader, ProjectsRail, ProjectsResults, ProjectsToolbar, SelectionBar,
};
use super::settings::{SettingsModal, SettingsTab, ShortcutTarget};
use super::tools::{
    ApiServerToolPanel, McpToolPanel, MediaServerToolPanel, SubtitlesToolPanel, ToolKind,
    ToolsSidebar, VoiceTypingToolPanel,
};
use super::transcript::TranscriptWorkbench;
use crate::app::{GlobalDesktopServices, GlobalEventBus, GlobalHotkeyController};
use crate::theme::tokens::{
    COLOR_BG_ELEVATED_LIGHT, COLOR_BG_PRIMARY_LIGHT, COLOR_BG_SECONDARY_LIGHT,
    COLOR_BG_TERTIARY_LIGHT, COLOR_BORDER_HOVER_LIGHT, COLOR_BORDER_LIGHT, COLOR_HEADER_BG_LIGHT,
    COLOR_PROJECT_AMBER, COLOR_TEXT_MUTED_LIGHT, COLOR_TEXT_PRIMARY_LIGHT,
    COLOR_TEXT_SECONDARY_LIGHT, HEADER_HEIGHT, PANEL_LEFT_WIDTH, PANEL_RIGHT_MIN_WIDTH, RADIUS_LG,
    RADIUS_MD, RADIUS_SM, project_color_from_index,
};

#[inline]
fn icon(name: LucideIcon) -> Icon {
    Icon::default().path(name.path())
}

fn shortcut_matches(shortcut: &str, key: &str, modifiers: Modifiers) -> bool {
    let parts = shortcut
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let Some(main_key) = parts.last() else {
        return false;
    };

    let needs_ctrl = parts.iter().any(|part| part.eq_ignore_ascii_case("ctrl"));
    let needs_alt = parts.iter().any(|part| part.eq_ignore_ascii_case("alt"));
    let needs_shift = parts.iter().any(|part| part.eq_ignore_ascii_case("shift"));
    let needs_meta = parts.iter().any(|part| part.eq_ignore_ascii_case("meta"));
    let key_matches = if main_key.eq_ignore_ascii_case("space") {
        key == " " || key.eq_ignore_ascii_case("space")
    } else {
        key.eq_ignore_ascii_case(main_key)
    };

    key_matches
        && modifiers.control == needs_ctrl
        && modifiers.alt == needs_alt
        && modifiers.shift == needs_shift
        && modifiers.platform == needs_meta
        && !modifiers.function
}

#[cfg(test)]
mod shortcut_tests {
    use super::shortcut_matches;
    use gpui_kit::Modifiers;

    #[test]
    fn matches_configured_shortcut_with_exact_modifiers() {
        let ctrl = Modifiers {
            control: true,
            ..Default::default()
        };
        assert!(shortcut_matches("Ctrl + Space", "space", ctrl));
        assert!(!shortcut_matches(
            "Ctrl + Space",
            "space",
            Modifiers::default()
        ));
        assert!(!shortcut_matches(
            "Ctrl + Space",
            "space",
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            }
        ));
    }

    #[test]
    fn maps_meta_to_the_platform_modifier() {
        assert!(shortcut_matches(
            "Meta + K",
            "k",
            Modifiers {
                platform: true,
                ..Default::default()
            }
        ));
    }
}

/// 3 Core Application Modes matching React desktop shell.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Create,
    Projects,
    Tools,
}

/// Project selector dropdown state and popover controller.
pub struct ProjectSelectorDropdown {
    pub is_open: bool,
    pub query: String,
    pub active_project_id: Option<String>,
    pub projects: Vec<ProjectRecord>,
    pub count_inbox: usize,
    pub project_counts: HashMap<String, usize>,
    pub is_recording: bool,
}

impl ProjectSelectorDropdown {
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            active_project_id: None,
            projects: Vec::new(),
            count_inbox: 0,
            project_counts: HashMap::new(),
            is_recording: false,
        }
    }

    pub fn load_data(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let projects = services.projects.list().await.unwrap_or_default();
            let history_items = services
                .history
                .query_db(|repo| repo.list_items(sona_core::history::HistoryListOptions::default()))
                .await
                .unwrap_or_default();

            let mut inbox_count = 0;
            let mut counts = HashMap::new();
            for item in history_items {
                if item.deleted_at.is_none() {
                    match &item.project_id {
                        Some(pid) if !pid.is_empty() => {
                            *counts.entry(pid.clone()).or_insert(0) += 1;
                        }
                        _ => inbox_count += 1,
                    }
                }
            }

            this.update(cx, |view, cx| {
                view.project_selector.projects = projects;
                view.project_selector.count_inbox = inbox_count;
                view.project_selector.project_counts = counts;
                view.projects_rail.sync_projects(
                    &view.project_selector.projects,
                    &view.project_selector.project_counts,
                    inbox_count,
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn select_project(&mut self, project_id: Option<String>, cx: &mut Context<MainShell>) {
        self.active_project_id = project_id;
        self.is_open = false;
        self.query.clear();
        cx.notify();
    }

    pub fn toggle(&mut self, cx: &mut Context<MainShell>) {
        if self.is_recording {
            return;
        }
        self.is_open = !self.is_open;
        if self.is_open {
            self.load_data(cx);
        }
        cx.notify();
    }

    pub fn active_project(&self) -> Option<&ProjectRecord> {
        self.active_project_id
            .as_ref()
            .and_then(|id| self.projects.iter().find(|p| &p.id == id))
    }

    pub fn filtered_projects(&self) -> Vec<&ProjectRecord> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.projects.iter().collect()
        } else {
            self.projects
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&q))
                .collect()
        }
    }
}

impl Default for ProjectSelectorDropdown {
    fn default() -> Self {
        Self::new()
    }
}

/// Dynamic cloud sync status pill and popover controller.
pub struct SyncHeaderPill {
    pub is_open: bool,
    pub is_action_busy: bool,
    pub status: Option<SyncStatusSnapshot>,
}

impl SyncHeaderPill {
    pub fn new() -> Self {
        Self {
            is_open: false,
            is_action_busy: false,
            status: None,
        }
    }

    pub fn refresh_status(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let status = services.sync.get_status().await.ok();
            this.update(cx, |view, cx| {
                view.sync_pill.status = status;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn run_sync_now(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.is_action_busy = true;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let _ = services.sync.run_now().await;
            let status = services.sync.get_status().await.ok();
            this.update(cx, |view, cx| {
                view.sync_pill.status = status;
                view.sync_pill.is_action_busy = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_pause(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let is_paused = self
            .status
            .as_ref()
            .is_some_and(|s| s.state == SyncLifecycleState::Paused);
        self.is_action_busy = true;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let _ = services.sync.set_paused(!is_paused).await;
            let status = services.sync.get_status().await.ok();
            this.update(cx, |view, cx| {
                view.sync_pill.status = status;
                view.sync_pill.is_action_busy = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle(&mut self, cx: &mut Context<MainShell>) {
        self.is_open = !self.is_open;
        if self.is_open {
            self.refresh_status(cx);
        }
        cx.notify();
    }
}

impl Default for SyncHeaderPill {
    fn default() -> Self {
        Self::new()
    }
}

/// Root Application Shell view.
pub struct MainShell {
    _voice_typing_event_task: Option<Task<()>>,
    pub mode: AppMode,
    pub status_text: String,
    pub project_count: usize,
    pub loading: bool,
    pub has_active_transcript: bool,

    // Header Components
    pub project_selector: ProjectSelectorDropdown,
    pub sync_pill: SyncHeaderPill,
    pub notification_center: Entity<NotificationCenter>,

    // Sub-views
    pub options_model: Entity<TranscriptionOptionsModel>,
    pub options_capsule: OptionsCapsule,
    pub create_workspace: Entity<CreateWorkspacePanel>,

    pub projects_rail: ProjectsRail,
    pub projects_header: ProjectsHeader,
    pub projects_toolbar: ProjectsToolbar,
    pub projects_results: ProjectsResults,
    pub selection_bar: SelectionBar,
    pub project_search_state: Entity<InputState>,
    _project_search_subscription: Subscription,
    pub project_dropdown_search_state: Entity<InputState>,
    _project_dropdown_search_subscription: Subscription,
    pub history_rename_state: Entity<InputState>,
    pub model_search_state: Entity<InputState>,
    _model_search_subscription: Subscription,
    pub mcp_search_state: Entity<InputState>,
    _mcp_search_subscription: Subscription,
    pub model_engine_select: Entity<SelectState<Vec<&'static str>>>,
    pub model_status_select: Entity<SelectState<Vec<&'static str>>>,
    pub model_label_select: Entity<SelectState<Vec<&'static str>>>,
    _model_filter_subscriptions: [Subscription; 3],
    pub translation_language_select: Entity<SelectState<Vec<&'static str>>>,
    _translation_language_subscription: Subscription,
    pub general_log_level_select: Entity<SelectState<Vec<&'static str>>>,
    _general_log_level_subscription: Subscription,
    pub general_language_select: Entity<SelectState<Vec<&'static str>>>,
    _general_language_subscription: Subscription,
    pub general_font_select: Entity<SelectState<Vec<&'static str>>>,
    _general_font_subscription: Subscription,
    pub online_provider_field_states: HashMap<String, Entity<InputState>>,
    _online_provider_field_subscriptions: Vec<Subscription>,
    pub llm_provider_api_host_state: Entity<InputState>,
    pub llm_provider_api_key_state: Entity<InputState>,
    pub llm_model_draft_state: Entity<InputState>,
    pub llm_custom_provider_name_state: Entity<InputState>,
    _llm_settings_input_subscriptions: [Subscription; 4],
    pub llm_model_metadata_field_states: HashMap<String, Entity<InputState>>,
    _llm_model_metadata_subscriptions: Vec<Subscription>,
    pub api_server_field_states: HashMap<String, Entity<InputState>>,
    _api_server_field_subscriptions: Vec<Subscription>,
    pub shortcut_edit_state: Entity<InputState>,
    _shortcut_edit_subscription: Subscription,
    pub dictionary_textarea_state: Entity<TextareaState>,
    _dictionary_textarea_subscription: Subscription,
    pub dictionary_quick_add_state: Entity<InputState>,
    _dictionary_quick_add_subscription: Subscription,
    pub dictionary_inline_hotword_state: Entity<InputState>,
    _dictionary_inline_hotword_subscription: Subscription,
    pub dictionary_word_state: Entity<InputState>,
    _dictionary_word_subscription: Subscription,
    pub dictionary_weight_state: Entity<InputState>,
    _dictionary_weight_subscription: Subscription,
    pub dictionary_from_state: Entity<InputState>,
    _dictionary_from_subscription: Subscription,
    pub dictionary_to_state: Entity<InputState>,
    _dictionary_to_subscription: Subscription,
    pub dictionary_search_state: Entity<InputState>,
    _dictionary_search_subscription: Subscription,
    pub dictionary_test_state: Entity<InputState>,
    _dictionary_test_subscription: Subscription,
    pub vocabulary_name_state: Entity<InputState>,
    _vocabulary_name_subscription: Subscription,
    pub vocabulary_body_state: Entity<TextareaState>,
    _vocabulary_body_subscription: Subscription,
    pub speaker_profile_name_state: Entity<InputState>,
    _speaker_profile_name_subscription: Subscription,
    pub speaker_profile_search_state: Entity<InputState>,
    _speaker_profile_search_subscription: Subscription,
    pub speaker_profile_name_states: HashMap<String, (Entity<InputState>, Subscription)>,

    pub tools_sidebar: ToolsSidebar,
    pub subtitles_tool: SubtitlesToolPanel,
    pub voice_typing_tool: VoiceTypingToolPanel,
    pub api_server_tool: ApiServerToolPanel,
    pub mcp_tool: McpToolPanel,
    pub media_server_tool: MediaServerToolPanel,

    pub workbench: TranscriptWorkbench,

    // Modals & Overlays
    pub settings_modal: SettingsModal,
    pub diagnostics_modal: DiagnosticsModal,
    pub recovery_modal: RecoveryModal,
    pub global_dialog: GlobalDialog,
    pub export_modal: ExportModal,
    pub onboarding: OnboardingWizard,
    pub project_dialogs: ProjectDialogs,
    pub error_dialog: ErrorDialog,
}

impl MainShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let notification_center = cx.new(|_| NotificationCenter::new());

        let options_model = cx.new(|cx| {
            let mut model = TranscriptionOptionsModel::new();
            model.load_from_services(&services);
            model.refresh_catalog(cx);
            model
        });
        let options_capsule = OptionsCapsule::new(options_model.clone());
        let create_workspace = cx.new(|cx| CreateWorkspacePanel::new(options_model.clone(), cx));
        let projects_toolbar = ProjectsToolbar::new(window, cx);
        let project_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search transcripts"));
        let history_rename_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Transcript title"));
        let project_search_subscription = cx.subscribe(
            &project_search_state,
            |this, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.projects_results
                        .set_search_query(state.read(cx).value().to_string(), cx);
                }
            },
        );
        let project_dropdown_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search projects..."));
        let project_dropdown_search_subscription = cx.subscribe(
            &project_dropdown_search_state,
            |this, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.project_selector.query = state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        let shortcut_edit_state = cx.new(|cx| InputState::new(window, cx));
        let shortcut_edit_subscription = cx.subscribe(
            &shortcut_edit_state,
            |shell, state, event: &InputEvent, cx| {
                if !shell.settings_modal.is_editing_shortcut {
                    return;
                }

                match event {
                    InputEvent::Change => {
                        shell.settings_modal.shortcut_draft = state.read(cx).value().to_string();
                    }
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        let value = state.read(cx).value().to_string();
                        shell.settings_modal.commit_shortcut_edit(value, cx);
                    }
                    InputEvent::Focus => {}
                }
            },
        );
        let model_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search models..."));
        let model_search_subscription = cx.subscribe(
            &model_search_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell.settings_modal.model_search_query = state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        let mcp_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search activity..."));
        let mcp_search_subscription =
            cx.subscribe(&mcp_search_state, |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell.settings_modal.mcp_log_query = state.read(cx).value().to_string();
                    cx.notify();
                }
            });
        let model_engine_select = cx.new(|cx| {
            SelectState::new(
                vec!["All Engines", "ONNX", "GGUF"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let model_status_select = cx.new(|cx| {
            SelectState::new(
                vec!["All Statuses", "Installed", "Not Installed", "Downloading"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let model_label_select = cx.new(|cx| {
            SelectState::new(
                vec!["All Tags", "Accurate", "Lite"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let initial_config = services
            .config
            .load()
            .ok()
            .flatten()
            .unwrap_or_else(|| serde_json::json!({}));
        let configured_theme = initial_config
            .get("theme")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("auto");
        let theme_mode = match configured_theme {
            "light" => gpui_kit::component::theme::ThemeMode::Light,
            "dark" => gpui_kit::component::theme::ThemeMode::Dark,
            _ => gpui_kit::component::theme::ThemeMode::from(window.appearance()),
        };
        gpui_kit::component::theme::Theme::change(theme_mode, None, cx);
        let translation_language = initial_config
            .get("translationLanguage")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("zh");
        let translation_language_index = crate::views::settings::LANGUAGE_OPTIONS
            .iter()
            .position(|(code, _)| *code == translation_language)
            .unwrap_or_else(|| {
                crate::views::settings::LANGUAGE_OPTIONS
                    .iter()
                    .position(|(code, _)| *code == "zh")
                    .unwrap_or(0)
            });
        let translation_language_select = cx.new(|cx| {
            SelectState::new(
                crate::views::settings::LANGUAGE_OPTIONS
                    .iter()
                    .map(|(_, name)| *name)
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(translation_language_index)),
                window,
                cx,
            )
        });
        let log_levels = ["trace", "debug", "info", "warn", "error"];
        let configured_log_level = initial_config
            .get("logLevel")
            .and_then(serde_json::Value::as_str)
            .filter(|level| log_levels.contains(level))
            .unwrap_or("info");
        log::set_max_level(match configured_log_level {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        });
        let log_level_index = log_levels
            .iter()
            .position(|level| *level == configured_log_level)
            .unwrap_or(2);
        let general_log_level_select = cx.new(|cx| {
            SelectState::new(
                log_levels.to_vec(),
                Some(IndexPath::new(log_level_index)),
                window,
                cx,
            )
        });
        let general_log_level_subscription = cx.subscribe(
            &general_log_level_select,
            move |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(level)) = event {
                    log::set_max_level(match *level {
                        "trace" => log::LevelFilter::Trace,
                        "debug" => log::LevelFilter::Debug,
                        "warn" => log::LevelFilter::Warn,
                        "error" => log::LevelFilter::Error,
                        _ => log::LevelFilter::Info,
                    });
                    shell.settings_modal.persist_general_setting(
                        "logLevel",
                        serde_json::Value::String((*level).to_string()),
                        cx,
                    );
                }
            },
        );
        let app_language_options = [
            ("auto", "Automatic"),
            ("en", "English"),
            ("zh", "简体中文"),
            ("zh-TW", "繁體中文"),
            ("ja", "日本語"),
            ("ko", "한국어"),
        ];
        let app_language = initial_config
            .get("appLanguage")
            .and_then(serde_json::Value::as_str)
            .filter(|value| app_language_options.iter().any(|(code, _)| code == value))
            .unwrap_or("auto");
        let app_language_index = app_language_options
            .iter()
            .position(|(code, _)| *code == app_language)
            .unwrap_or(0);
        let general_language_select = cx.new(|cx| {
            SelectState::new(
                app_language_options
                    .iter()
                    .map(|(_, label)| *label)
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(app_language_index)),
                window,
                cx,
            )
        });
        let general_language_subscription = cx.subscribe(
            &general_language_select,
            move |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some((language, _)) = app_language_options
                        .iter()
                        .find(|(_, option_label)| *option_label == *label)
                {
                    shell.settings_modal.selected_lang = (*language).to_string();
                    shell.settings_modal.persist_general_setting(
                        "appLanguage",
                        serde_json::Value::String((*language).to_string()),
                        cx,
                    );
                }
            },
        );
        let font_options = [
            ("system", "System"),
            ("serif", "Serif"),
            ("sans", "Sans-serif"),
            ("mono", "Monospace"),
            ("arial", "Arial"),
            ("georgia", "Georgia"),
        ];
        let font = initial_config
            .get("font")
            .and_then(serde_json::Value::as_str)
            .filter(|value| font_options.iter().any(|(code, _)| code == value))
            .unwrap_or("system");
        let font_index = font_options
            .iter()
            .position(|(code, _)| *code == font)
            .unwrap_or(0);
        let general_font_select = cx.new(|cx| {
            SelectState::new(
                font_options
                    .iter()
                    .map(|(_, label)| *label)
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(font_index)),
                window,
                cx,
            )
        });
        let general_font_subscription = cx.subscribe(
            &general_font_select,
            move |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some((font, _)) = font_options
                        .iter()
                        .find(|(_, option_label)| *option_label == *label)
                {
                    shell.settings_modal.selected_font = (*font).to_string();
                    shell.settings_modal.persist_general_setting(
                        "font",
                        serde_json::Value::String((*font).to_string()),
                        cx,
                    );
                }
            },
        );
        let dictionary_initial_content =
            crate::views::settings::vocabulary::effective_dictionary_content(&initial_config);
        let dictionary_textarea_state =
            cx.new(|cx| TextareaState::new(window, cx).default_value(dictionary_initial_content));
        let dictionary_textarea_subscription = cx.subscribe(
            &dictionary_textarea_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let content = state.read(cx).value().to_string();
                    let (hotword_sets, replacement_sets) =
                        crate::views::settings::vocabulary::legacy_dictionary_sets(&content);
                    shell.settings_modal.persist_model_settings(
                        vec![
                            (
                                "dictionaryContent".to_string(),
                                serde_json::Value::String(content),
                            ),
                            ("hotwordSets".to_string(), hotword_sets),
                            ("textReplacementSets".to_string(), replacement_sets),
                        ],
                        cx,
                    );
                }
            },
        );
        let dictionary_quick_add_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Term (e.g. Sona) or From -> To"));
        let dictionary_quick_add_subscription = cx.subscribe(
            &dictionary_quick_add_state,
            |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let dictionary_inline_hotword_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("+ Add hotword..."));
        let dictionary_inline_hotword_subscription = cx.subscribe(
            &dictionary_inline_hotword_state,
            |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let dictionary_word_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Sona or DeepSeek"));
        let dictionary_word_subscription =
            cx.subscribe(&dictionary_word_state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let dictionary_weight_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 2.0"));
        let dictionary_weight_subscription =
            cx.subscribe(&dictionary_weight_state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let dictionary_from_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Misrecognized word"));
        let dictionary_from_subscription =
            cx.subscribe(&dictionary_from_state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let dictionary_to_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Corrected word"));
        let dictionary_to_subscription =
            cx.subscribe(&dictionary_to_state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let dictionary_search_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search hotwords or replacements...")
        });
        let dictionary_search_subscription = cx.subscribe(
            &dictionary_search_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell.settings_modal.vocabulary_dictionary_search_query =
                        state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        let dictionary_test_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Type or paste text to test replacement...")
        });
        let dictionary_test_subscription = cx.subscribe(
            &dictionary_test_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell.settings_modal.vocabulary_dictionary_test_input =
                        state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        let vocabulary_name_state = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let vocabulary_name_subscription =
            cx.subscribe(&vocabulary_name_state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let vocabulary_body_state = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder("Enter context or instructions...")
        });
        let vocabulary_body_subscription =
            cx.subscribe(&vocabulary_body_state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let speaker_profile_name_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Alice"));
        let speaker_profile_name_subscription = cx.subscribe(
            &speaker_profile_name_state,
            |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let speaker_profile_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search speakers..."));
        let speaker_profile_search_subscription = cx.subscribe(
            &speaker_profile_search_state,
            |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let mut speaker_profile_name_states = HashMap::new();
        if let Some(profiles) = initial_config
            .get("speakerProfiles")
            .and_then(serde_json::Value::as_array)
        {
            for value in profiles {
                let Ok(profile) = serde_json::from_value::<
                    sona_core::transcription::speaker::SpeakerProfile,
                >(value.clone()) else {
                    continue;
                };
                let profile_id = profile.id;
                let subscription_profile_id = profile_id.clone();
                let name_state =
                    cx.new(|cx| InputState::new(window, cx).default_value(profile.name));
                let subscription =
                    cx.subscribe(&name_state, move |shell, state, event: &InputEvent, cx| {
                        if !matches!(event, InputEvent::Change) {
                            return;
                        }
                        let name = state.read(cx).value().to_string();
                        let mut profiles = shell
                            .settings_modal
                            .model_config
                            .get("speakerProfiles")
                            .and_then(serde_json::Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        for value in &mut profiles {
                            if value.get("id").and_then(serde_json::Value::as_str)
                                == Some(subscription_profile_id.as_str())
                            {
                                value["name"] = serde_json::Value::String(name.clone());
                            }
                        }
                        shell.settings_modal.persist_model_setting(
                            "speakerProfiles".to_string(),
                            serde_json::Value::Array(profiles),
                            cx,
                        );
                    });
                speaker_profile_name_states.insert(profile_id, (name_state, subscription));
            }
        }
        let active_llm_provider = initial_config
            .pointer("/llmSettings/activeProvider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("google_translate_free");
        let provider_default_host = match active_llm_provider {
            "google_translate_free" => {
                "https://translate.googleapis.com/translate_a/single".to_string()
            }
            "google_translate" => {
                "https://translation.googleapis.com/language/translate/v2".to_string()
            }
            provider => sona_core::llm::providers::find_llm_provider_by_id_or_alias(provider)
                .map(|provider| provider.defaults.api_host.clone())
                .unwrap_or_default(),
        };
        let api_host = initial_config
            .pointer(&format!(
                "/llmSettings/providers/{active_llm_provider}/apiHost"
            ))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&provider_default_host)
            .to_string();
        let api_key = initial_config
            .pointer(&format!(
                "/llmSettings/providers/{active_llm_provider}/apiKey"
            ))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let llm_provider_api_host_state = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("API host or endpoint");
            state.set_value(api_host, window, cx);
            state
        });
        let llm_provider_api_key_state = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder("API key")
                .masked(true);
            state.set_value(api_key, window, cx);
            state
        });
        let llm_model_draft_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Model ID or deployment name"));
        let llm_custom_provider_name_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Provider name"));
        let metadata_fields = [
            ("displayName", "Display name"),
            ("contextWindow", "Context window"),
            ("maxOutputTokens", "Max output tokens"),
            ("inputPrice", "Input price"),
            ("outputPrice", "Output price"),
            ("cacheReadPrice", "Cache read price"),
            ("cacheWritePrice", "Cache write price"),
            ("knowledgeCutoff", "Knowledge cutoff"),
            ("releaseDate", "Release date"),
            ("lastUpdated", "Last updated"),
        ];
        let mut llm_model_metadata_field_states = HashMap::new();
        let mut llm_model_metadata_subscriptions = Vec::new();
        for (key, placeholder) in metadata_fields {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            let field = key.to_string();
            let subscription = cx.subscribe(&state, move |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell
                        .settings_modal
                        .set_llm_model_metadata_field(&field, state.read(cx).value().to_string());
                }
            });
            llm_model_metadata_field_states.insert(key.to_string(), state);
            llm_model_metadata_subscriptions.push(subscription);
        }
        let mut api_server_field_states = HashMap::new();
        let mut api_server_field_subscriptions = Vec::new();
        for (key, config_key, placeholder, masked) in [
            ("host", "httpServerHost", "127.0.0.1", false),
            ("whitelist", "httpServerIpWhitelist", "localhost", false),
            ("apiKey", "httpServerApiKey", "Optional bearer token", true),
        ] {
            let initial_value = initial_config
                .get(config_key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or(placeholder)
                .to_string();
            let state = cx.new(|cx| {
                let mut state = InputState::new(window, cx)
                    .placeholder(placeholder)
                    .masked(masked);
                state.set_value(initial_value, window, cx);
                state
            });
            let setting_key = config_key;
            let subscription =
                cx.subscribe(&state, move |_shell, state, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = state.read(cx).value().to_string();
                        let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                        if let Err(error) =
                            config.set_setting(setting_key, &serde_json::json!(value))
                        {
                            log::error!(
                                "Failed to update API server setting {setting_key}: {error}"
                            );
                        }
                        cx.notify();
                    }
                });
            api_server_field_states.insert(key.to_string(), state);
            api_server_field_subscriptions.push(subscription);
        }
        for (key, config_key, min, max, default) in [
            ("port", "httpServerPort", 1_i64, 65_535_i64, 14_200_i64),
            ("maxConcurrent", "httpServerMaxConcurrent", 1, 32, 2),
            ("maxQueue", "httpServerMaxQueueSize", 0, 1_000, 100),
            ("maxUpload", "httpServerMaxUploadSizeMB", 0, 10_000, 50),
            ("jobTtl", "httpServerJobTtlMinutes", 0, 10_080, 60),
        ] {
            let initial_value = initial_config
                .get(config_key)
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(default)
                .to_string();
            let state = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder(&initial_value);
                state.set_value(initial_value, window, cx);
                state
            });
            let setting_key = config_key;
            let subscription =
                cx.subscribe(&state, move |_shell, state, event: &InputEvent, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let value = state.read(cx).value().to_string();
                    let Ok(value) = value.parse::<i64>() else {
                        return;
                    };
                    if !(min..=max).contains(&value) {
                        return;
                    }
                    let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                    if let Err(error) = config.set_setting(setting_key, &serde_json::json!(value)) {
                        log::error!("Failed to update API server setting {setting_key}: {error}");
                    }
                    cx.notify();
                });
            api_server_field_states.insert(key.to_string(), state);
            api_server_field_subscriptions.push(subscription);
        }
        let llm_api_host_subscription = cx.subscribe(
            &llm_provider_api_host_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell.settings_modal.set_llm_provider_field(
                        "apiHost",
                        state.read(cx).value().to_string(),
                        cx,
                    );
                }
            },
        );
        let llm_api_key_subscription = cx.subscribe(
            &llm_provider_api_key_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell.settings_modal.set_llm_provider_field(
                        "apiKey",
                        state.read(cx).value().to_string(),
                        cx,
                    );
                }
            },
        );
        let llm_model_draft_subscription = cx.subscribe(
            &llm_model_draft_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell
                        .settings_modal
                        .set_llm_model_draft(state.read(cx).value().to_string());
                }
            },
        );
        let llm_custom_name_subscription = cx.subscribe(
            &llm_custom_provider_name_state,
            |shell, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    shell
                        .settings_modal
                        .set_llm_custom_provider_name(state.read(cx).value().to_string());
                }
            },
        );
        let mut online_provider_field_states = HashMap::new();
        let mut online_provider_field_subscriptions = Vec::new();
        for provider in sona_core::ports::asr::online_asr_providers() {
            let provider_id = provider.id.clone();
            let mut fields = vec![("apiKey".to_string(), "password".to_string())];
            fields.extend(
                provider
                    .ui
                    .fields
                    .iter()
                    .filter(|field| field.name != "apiKey" && field.name != "model")
                    .map(|field| (field.name.clone(), field.field_type.clone())),
            );
            for (field_name, field_type) in fields {
                let state_key = format!("{provider_id}::{field_name}");
                let value = initial_config
                    .pointer(&format!("/asr/providers/online/{provider_id}/{field_name}"))
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        provider
                            .defaults
                            .get(&field_name)
                            .and_then(serde_json::Value::as_str)
                    })
                    .unwrap_or_default()
                    .to_string();
                let placeholder = match (provider_id.as_str(), field_name.as_str()) {
                    ("volcengine-doubao", "apiKey") => "X-Api-Key".to_string(),
                    ("groq-whisper", "apiKey") => "gsk_...".to_string(),
                    (_, "apiKey") => "API Key".to_string(),
                    _ => field_name.clone(),
                };
                let state = cx.new(|cx| {
                    let mut state = InputState::new(window, cx).placeholder(placeholder);
                    if field_type == "password" {
                        state = state.masked(true);
                    }
                    state.set_value(value, window, cx);
                    state
                });
                let subscription_provider_id = provider_id.clone();
                let subscription_field_name = field_name.clone();
                online_provider_field_subscriptions.push(cx.subscribe(
                    &state,
                    move |shell, state, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            shell.settings_modal.set_online_provider_config_field(
                                &subscription_provider_id,
                                &subscription_field_name,
                                state.read(cx).value().to_string(),
                                cx,
                            );
                        }
                    },
                ));
                online_provider_field_states.insert(state_key, state);
            }
        }
        let engine_subscription = cx.subscribe(
            &model_engine_select,
            |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    shell.settings_modal.model_engine_filter = match *value {
                        "ONNX" => "sherpa-onnx",
                        "GGUF" => "llama-cpp",
                        _ => "all",
                    }
                    .to_string();
                    cx.notify();
                }
            },
        );
        let status_subscription = cx.subscribe(
            &model_status_select,
            |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    shell.settings_modal.model_status_filter = match *value {
                        "Installed" => "installed",
                        "Not Installed" => "not-installed",
                        "Downloading" => "downloading",
                        _ => "all",
                    }
                    .to_string();
                    cx.notify();
                }
            },
        );
        let label_subscription = cx.subscribe(
            &model_label_select,
            |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    shell.settings_modal.model_label_filter = match *value {
                        "Accurate" => "accurate",
                        "Lite" => "lite",
                        _ => "all",
                    }
                    .to_string();
                    cx.notify();
                }
            },
        );
        let translation_language_subscription = cx.subscribe(
            &translation_language_select,
            |shell, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event
                    && let Some((code, _)) = crate::views::settings::LANGUAGE_OPTIONS
                        .iter()
                        .find(|(_, name)| *name == *value)
                {
                    shell.settings_modal.persist_model_setting(
                        "translationLanguage".to_string(),
                        serde_json::Value::String((*code).to_string()),
                        cx,
                    );
                }
            },
        );

        let mut settings_modal = SettingsModal::new();
        settings_modal.selected_theme = configured_theme.to_string();
        settings_modal.selected_lang = initial_config
            .get("appLanguage")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("auto")
            .to_string();
        settings_modal.selected_font = initial_config
            .get("font")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("system")
            .to_string();

        let mut shell = Self {
            _voice_typing_event_task: None,
            mode: AppMode::Create,
            status_text: "Connecting to decoupled backend core...".to_string(),
            project_count: 0,
            loading: true,
            has_active_transcript: false,

            project_selector: ProjectSelectorDropdown::new(),
            sync_pill: SyncHeaderPill::new(),
            notification_center,

            options_model,
            options_capsule,
            create_workspace,

            projects_rail: ProjectsRail::new(),
            projects_header: ProjectsHeader::new(),
            projects_toolbar,
            projects_results: ProjectsResults::new(),
            selection_bar: SelectionBar::new(),
            project_search_state,
            _project_search_subscription: project_search_subscription,
            project_dropdown_search_state,
            _project_dropdown_search_subscription: project_dropdown_search_subscription,
            history_rename_state,
            model_search_state,
            _model_search_subscription: model_search_subscription,
            mcp_search_state,
            _mcp_search_subscription: mcp_search_subscription,
            model_engine_select,
            model_status_select,
            model_label_select,
            _model_filter_subscriptions: [
                engine_subscription,
                status_subscription,
                label_subscription,
            ],
            translation_language_select,
            _translation_language_subscription: translation_language_subscription,
            general_log_level_select,
            _general_log_level_subscription: general_log_level_subscription,
            general_language_select,
            _general_language_subscription: general_language_subscription,
            general_font_select,
            _general_font_subscription: general_font_subscription,
            online_provider_field_states,
            _online_provider_field_subscriptions: online_provider_field_subscriptions,
            llm_provider_api_host_state,
            llm_provider_api_key_state,
            llm_model_draft_state,
            llm_custom_provider_name_state,
            _llm_settings_input_subscriptions: [
                llm_api_host_subscription,
                llm_api_key_subscription,
                llm_model_draft_subscription,
                llm_custom_name_subscription,
            ],
            llm_model_metadata_field_states,
            _llm_model_metadata_subscriptions: llm_model_metadata_subscriptions,
            api_server_field_states,
            _api_server_field_subscriptions: api_server_field_subscriptions,
            shortcut_edit_state,
            _shortcut_edit_subscription: shortcut_edit_subscription,
            dictionary_textarea_state,
            _dictionary_textarea_subscription: dictionary_textarea_subscription,
            dictionary_quick_add_state,
            _dictionary_quick_add_subscription: dictionary_quick_add_subscription,
            dictionary_inline_hotword_state,
            _dictionary_inline_hotword_subscription: dictionary_inline_hotword_subscription,
            dictionary_word_state,
            _dictionary_word_subscription: dictionary_word_subscription,
            dictionary_weight_state,
            _dictionary_weight_subscription: dictionary_weight_subscription,
            dictionary_from_state,
            _dictionary_from_subscription: dictionary_from_subscription,
            dictionary_to_state,
            _dictionary_to_subscription: dictionary_to_subscription,
            dictionary_search_state,
            _dictionary_search_subscription: dictionary_search_subscription,
            dictionary_test_state,
            _dictionary_test_subscription: dictionary_test_subscription,
            vocabulary_name_state,
            _vocabulary_name_subscription: vocabulary_name_subscription,
            vocabulary_body_state,
            _vocabulary_body_subscription: vocabulary_body_subscription,
            speaker_profile_name_state,
            _speaker_profile_name_subscription: speaker_profile_name_subscription,
            speaker_profile_search_state,
            _speaker_profile_search_subscription: speaker_profile_search_subscription,
            speaker_profile_name_states,

            tools_sidebar: ToolsSidebar::new(),
            subtitles_tool: SubtitlesToolPanel::new(),
            voice_typing_tool: VoiceTypingToolPanel::new(),
            api_server_tool: ApiServerToolPanel::new(),
            mcp_tool: McpToolPanel::new(),
            media_server_tool: MediaServerToolPanel::new(window, cx),

            workbench: TranscriptWorkbench::new(window, cx),

            settings_modal,
            diagnostics_modal: DiagnosticsModal::new(),
            recovery_modal: RecoveryModal::new(),
            global_dialog: GlobalDialog::new(),
            export_modal: ExportModal::new(window, cx),
            onboarding: OnboardingWizard::new(),
            project_dialogs: ProjectDialogs::new(window, cx),
            error_dialog: ErrorDialog::new(),
        };

        shell.settings_modal.model_config = initial_config;
        let mut voice_typing_events = cx.global::<GlobalEventBus>().0.subscribe();
        shell._voice_typing_event_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            while let Ok(event) = voice_typing_events.recv().await {
                if event.is(sona_core::task_ledger::types::TASK_LEDGER_UPDATED_EVENT) {
                    let Ok(tasks) = serde_json::from_value::<
                        Vec<sona_core::task_ledger::types::TaskLedgerRecord>,
                    >(event.payload["tasks"].clone()) else {
                        continue;
                    };
                    let _ = this.update(cx, |shell, cx| {
                        shell.notification_center.update(cx, |center, cx| {
                            center.set_tasks(tasks, cx);
                        });
                    });
                } else if event.is("voice-typing:show") {
                    let initial_recall = event.payload["recall"].as_bool().unwrap_or(false);
                    let _ = this.update(cx, |_, cx| {
                        crate::app::open_voice_typing_island(cx, initial_recall);
                    });
                } else if event.is("notification:open-recovery") {
                    let _ = this.update(cx, |shell, cx| {
                        shell.notification_center.update(cx, |center, cx| {
                            center.is_open = false;
                            cx.notify();
                        });
                        shell.recovery_modal.open(cx);
                    });
                } else if event.is("notification:open-automation") {
                    let _ = this.update(cx, |shell, cx| {
                        shell.notification_center.update(cx, |center, cx| {
                            center.is_open = false;
                            cx.notify();
                        });
                        shell.settings_modal.set_tab(SettingsTab::Automation, cx);
                        shell.settings_modal.open(cx);
                    });
                } else if event.is("notification:retry-batch") {
                    let file_path = event.payload["filePath"]
                        .as_str()
                        .map(std::path::PathBuf::from);
                    let project_id = event.payload["projectId"].as_str().map(str::to_owned);
                    let task_id = event.payload["taskId"].as_str().map(str::to_owned);
                    if let (Some(file_path), Some(task_id)) = (file_path, task_id) {
                        let services = this.update(cx, |shell, cx| {
                            shell.notification_center.update(cx, |center, cx| {
                                center.is_open = false;
                                cx.notify();
                            });
                            shell.create_workspace.update(cx, |workspace, cx| {
                                workspace.set_tab(
                                    crate::views::create::CreateWorkspaceTab::BatchImport,
                                    cx,
                                );
                                workspace.batch_import.update(cx, |batch, cx| {
                                    batch.enqueue_retry_file(file_path, project_id, cx);
                                });
                            });
                            cx.global::<GlobalDesktopServices>().0.clone()
                        });
                        if let Ok(services) = services {
                            cx.spawn(async move |_cx: &mut AsyncApp| {
                                let _ = services.task_ledger.remove_task(task_id).await;
                            })
                            .detach();
                        }
                    }
                } else if event.is("notification:cancel-task") {
                    if let Some(task_id) = event.payload["taskId"].as_str().map(str::to_owned) {
                        let _ = this.update(cx, |shell, cx| {
                            shell.create_workspace.update(cx, |workspace, cx| {
                                workspace.batch_import.update(cx, |batch, cx| {
                                    batch.cancel_task_instance(&task_id, cx);
                                });
                            });
                        });
                    }
                } else if event.is("live-record:started") {
                    let Some(session_id) = event.payload["sessionId"].as_str().map(str::to_owned)
                    else {
                        continue;
                    };
                    let title = event.payload["title"]
                        .as_str()
                        .unwrap_or("Recording")
                        .to_owned();
                    let _ = this.update(cx, |shell, cx| {
                        shell.projects_results.selected_id = Some(session_id.clone());
                        shell.has_active_transcript = true;
                        shell.workbench.begin_loading(
                            session_id,
                            title,
                            Some("system:mic".to_string()),
                            false,
                        );
                        cx.notify();
                    });
                } else if event.is("live-record:transcript-updated") {
                    let Some(session_id) = event.payload["sessionId"].as_str().map(str::to_owned)
                    else {
                        continue;
                    };
                    let Ok(segments) = serde_json::from_value::<
                        Vec<sona_core::transcription::transcript::TranscriptSegment>,
                    >(event.payload["segments"].clone()) else {
                        continue;
                    };
                    let duration = event.payload["duration"].as_f64().unwrap_or_default();
                    let _ = this.update(cx, |shell, cx| {
                        if shell.workbench.history_id.as_deref() == Some(&session_id) {
                            shell.workbench.set_transcript(segments, duration);
                            cx.notify();
                        }
                    });
                } else if event.is("live-record:stopped") {
                    let session_id = event.payload["sessionId"].as_str().map(str::to_owned);
                    let title = event.payload["title"].as_str().map(str::to_owned);
                    let _ = this.update(cx, |shell, cx| {
                        if let Some(session_id) = session_id.as_deref()
                            && shell.workbench.history_id.as_deref() == Some(session_id)
                            && let Some(title) = title
                        {
                            shell.workbench.session_title = title;
                        }
                        shell.projects_results.load_data(cx);
                        cx.notify();
                    });
                } else if event.is("live-record:error") {
                    let message = event.payload["message"]
                        .as_str()
                        .unwrap_or("Recording failed.")
                        .to_owned();
                    let _ = this.update(cx, |shell, cx| {
                        shell.error_dialog.show(message, "");
                        cx.notify();
                    });
                }
            }
        }));
        shell.project_selector.load_data(cx);
        shell.projects_results.load_data(cx);
        shell.sync_pill.refresh_status(cx);
        shell
            .notification_center
            .update(cx, |nc, cx| nc.load_tasks(cx));
        shell.settings_modal.load_preferences(cx);

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let projects = services.projects.list().await.unwrap_or_default();
            let count = projects.len();

            this.update(cx, |view, cx| {
                view.project_count = count;
                view.loading = false;
                view.status_text =
                    format!("Decoupled core connected: SQLite WAL active, {count} projects loaded");
                cx.notify();
            })
            .ok();
        })
        .detach();

        shell
    }

    pub fn set_mode(&mut self, mode: AppMode, cx: &mut Context<Self>) {
        if self.mode == mode {
            // Re-click: scroll active container to top
            return;
        }
        self.mode = mode;
        cx.notify();
    }

    pub fn open_history_item(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(record) = self
            .projects_results
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.record.clone())
        else {
            return;
        };
        if record.deleted_at.is_some() {
            return;
        }

        self.projects_results.selected_id = Some(id.clone());
        self.has_active_transcript = true;
        let is_batch = matches!(record.kind, sona_core::history::HistoryItemKind::Batch);
        let media_context = self
            .create_workspace
            .read(cx)
            .batch_import
            .read(cx)
            .queue
            .iter()
            .find(|item| item.history_id.as_deref() == Some(&id))
            .and_then(|item| item.media_server_context.clone());
        self.workbench.media_server_context = media_context;
        self.workbench
            .begin_loading(id.clone(), record.title, record.icon.clone(), is_batch);

        let duration = record.duration;
        let selected_id = id.clone();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let query_id = id;
            let snapshots_id = selected_id.clone();
            let result = services
                .history
                .query_db(move |query| query.load_transcript(&query_id))
                .await;
            let snapshots = services
                .history
                .query_db(move |query| query.list_transcript_snapshots(&snapshots_id))
                .await
                .unwrap_or_default();
            let summary_id = selected_id.clone();
            let summary = services
                .history
                .db_task(move |query| query.load_summary(&summary_id))
                .await
                .ok()
                .flatten();
            this.update(cx, |shell, cx| {
                if shell.projects_results.selected_id.as_deref() != Some(&selected_id) {
                    return;
                }
                match result {
                    Ok(Some(segments)) => {
                        shell.workbench.set_transcript(segments, duration);
                        shell.workbench.set_versions(snapshots);
                        shell.workbench.set_summary(summary);
                    }
                    _ => shell.workbench.set_load_error(),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn set_project_scope(
        &mut self,
        scope: HistoryWorkspaceScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.projects_results.set_scope(scope, cx);
        self.projects_toolbar.sync_filter_selects(window, cx);
        self.project_search_state
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    pub fn cycle_mode(&mut self, reverse: bool, cx: &mut Context<Self>) {
        let modes = [AppMode::Create, AppMode::Projects, AppMode::Tools];
        let current_idx = match self.mode {
            AppMode::Create => 0,
            AppMode::Projects => 1,
            AppMode::Tools => 2,
        };
        let next_idx = if reverse {
            (current_idx + modes.len() - 1) % modes.len()
        } else {
            (current_idx + 1) % modes.len()
        };
        self.set_mode(modes[next_idx], cx);
    }

    pub fn should_show_transcript_host(&self) -> bool {
        match self.mode {
            AppMode::Create => true,
            AppMode::Projects => self.has_active_transcript,
            AppMode::Tools => false,
        }
    }

    fn render_project_selector(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active_proj = self.project_selector.active_project().cloned();
        let display_title = active_proj
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Inbox".to_string());
        let has_pipeline = active_proj
            .as_ref()
            .and_then(|p| p.pipeline.as_ref())
            .is_some_and(|p| p.enabled);

        let filtered: Vec<ProjectRecord> = self
            .project_selector
            .filtered_projects()
            .into_iter()
            .cloned()
            .collect();
        let is_inbox_active = self.project_selector.active_project_id.is_none();
        let is_open = self.project_selector.is_open;
        let is_rec = self.project_selector.is_recording;
        let count_inbox = self.project_selector.count_inbox;
        let project_counts = self.project_selector.project_counts.clone();
        let active_pid = self.project_selector.active_project_id.clone();

        div()
            .relative()
            .child(
                // Trigger button (height 30px, max-width 190px, padding 4px 10px, gap 7px)
                div()
                    .id("project-selector-trigger")
                    .flex()
                    .items_center()
                    .h(px(30.0))
                    .max_w(px(190.0))
                    .px(px(10.0))
                    .py(px(4.0))
                    .gap(px(7.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(RADIUS_SM)
                    .bg(cx.theme().background)
                    .text_size(px(13.0))
                    .text_color(cx.theme().foreground)
                    .cursor_pointer()
                    .hover(|style| {
                        style
                            .border_color(COLOR_BORDER_HOVER_LIGHT)
                            .bg(COLOR_BG_SECONDARY_LIGHT)
                    })
                    .when(is_rec, |this| {
                        this.opacity(0.55)
                            .cursor_not_allowed()
                            .tooltip(|window, cx| {
                                Tooltip::new("Cannot switch projects during recording")
                                    .build(window, cx)
                            })
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.project_selector.toggle(cx);
                    }))
                    // Swatch or Inbox Icon
                    .child(if let Some(proj) = active_proj {
                        let swatch_color = proj
                            .color
                            .as_deref()
                            .and_then(crate::theme::tokens::hex_to_hsla)
                            .unwrap_or_else(|| project_color_from_index(proj.sort_order));
                        let custom_emoji = proj
                            .icon
                            .as_ref()
                            .filter(|ic| !ic.is_empty() && !ic.starts_with("system:"));
                        div()
                            .size(px(16.0))
                            .rounded(px(4.0))
                            .bg(swatch_color.opacity(0.14))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(custom_emoji, |this, emoji| {
                                this.child(div().text_size(px(11.0)).child(emoji.clone()))
                            })
                            .when(custom_emoji.is_none(), |this| {
                                this.child(div().size(px(8.0)).rounded_full().bg(swatch_color))
                            })
                            .into_any_element()
                    } else {
                        div()
                            .size(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(LucideIcon::Inbox).size(px(14.0)))
                            .into_any_element()
                    })
                    // Project Title
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(display_title),
                    )
                    // Zap Icon if automation enabled
                    .when(has_pipeline, |this| {
                        this.child(
                            icon(LucideIcon::Zap)
                                .size(px(12.0))
                                .text_color(COLOR_PROJECT_AMBER),
                        )
                    })
                    // Chevron Down Icon
                    .child(
                        icon(LucideIcon::ChevronDown)
                            .size(px(14.0))
                            .text_color(cx.theme().muted),
                    ),
            )
            // Popover panel (width 280px, padding 8px, border-radius 8px, shadow-lg)
            .when(is_open, |this| {
                this.child(
                    div()
                        .id("project-selector-popover")
                        .absolute()
                        .top(px(36.0))
                        .left_0()
                        .w(px(280.0))
                        .p_2()
                        .rounded(RADIUS_MD)
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap_1()
                        // Search bar input
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    icon(LucideIcon::Search)
                                        .size(px(14.0))
                                        .text_color(cx.theme().muted),
                                )
                                .child(
                                    div().flex_1().min_w(px(0.0)).child(
                                        Input::new(&self.project_dropdown_search_state)
                                            .cleanable(true)
                                            .small(),
                                    ),
                                ),
                        )
                        // Inbox item row
                        .child(
                            div()
                                .id("proj-row-inbox")
                                .flex()
                                .items_center()
                                .gap_2()
                                .h(px(32.0))
                                .px_2()
                                .rounded(RADIUS_SM)
                                .bg(if is_inbox_active {
                                    cx.theme().secondary
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .cursor_pointer()
                                .hover(|style| style.bg(COLOR_BG_TERTIARY_LIGHT))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.project_dropdown_search_state.update(cx, |input, cx| {
                                        input.set_value("", window, cx);
                                    });
                                    this.project_selector.select_project(None, cx);
                                }))
                                .child(icon(LucideIcon::Inbox).size(px(14.0)))
                                .child(div().flex_1().text_size(px(13.0)).child("Inbox"))
                                .when(is_inbox_active, |this| {
                                    this.child(icon(LucideIcon::Check).size(px(14.0)))
                                })
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted)
                                        .child(count_inbox.to_string()),
                                ),
                        )
                        // User project rows
                        .child(
                            div()
                                .id("proj-scroll-list")
                                .flex()
                                .flex_col()
                                .max_h(px(220.0))
                                .overflow_y_scroll()
                                .children(filtered.into_iter().enumerate().map(|(i, proj)| {
                                    let is_active = active_pid.as_deref() == Some(&proj.id);
                                    let count = project_counts.get(&proj.id).copied().unwrap_or(0);
                                    let swatch_color = proj
                                        .color
                                        .as_deref()
                                        .and_then(crate::theme::tokens::hex_to_hsla)
                                        .unwrap_or_else(|| {
                                            project_color_from_index(proj.sort_order)
                                        });
                                    let custom_emoji = proj
                                        .icon
                                        .as_ref()
                                        .filter(|ic| !ic.is_empty() && !ic.starts_with("system:"));
                                    let p_has_zap =
                                        proj.pipeline.as_ref().is_some_and(|p| p.enabled);
                                    let pid = proj.id.clone();

                                    div()
                                        .id(ElementId::from(format!("proj-row-{}", i)))
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .h(px(32.0))
                                        .px_2()
                                        .rounded(RADIUS_SM)
                                        .bg(if is_active {
                                            cx.theme().secondary
                                        } else {
                                            gpui_kit::transparent_black()
                                        })
                                        .cursor_pointer()
                                        .hover(|style| style.bg(COLOR_BG_TERTIARY_LIGHT))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.project_dropdown_search_state.update(
                                                cx,
                                                |input, cx| {
                                                    input.set_value("", window, cx);
                                                },
                                            );
                                            this.project_selector
                                                .select_project(Some(pid.clone()), cx);
                                        }))
                                        .child(
                                            div()
                                                .size(px(16.0))
                                                .rounded(px(4.0))
                                                .bg(swatch_color.opacity(0.14))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .when_some(custom_emoji, |this, emoji| {
                                                    this.child(
                                                        div()
                                                            .text_size(px(11.0))
                                                            .child(emoji.clone()),
                                                    )
                                                })
                                                .when(custom_emoji.is_none(), |this| {
                                                    this.child(
                                                        div()
                                                            .size(px(8.0))
                                                            .rounded_full()
                                                            .bg(swatch_color),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .text_size(px(13.0))
                                                .child(proj.name.clone()),
                                        )
                                        .when(p_has_zap, |this| {
                                            this.child(
                                                icon(LucideIcon::Zap)
                                                    .size(px(12.0))
                                                    .text_color(COLOR_PROJECT_AMBER),
                                            )
                                        })
                                        .when(is_active, |this| {
                                            this.child(icon(LucideIcon::Check).size(px(14.0)))
                                        })
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(cx.theme().muted)
                                                .child(count.to_string()),
                                        )
                                })),
                        )
                        // Footer buttons: New Project & Open Project Center
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pt_1p5()
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .child(
                                    Button::new("btn-new-project-dropdown")
                                        .ghost()
                                        .xsmall()
                                        .label("+ New Project")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.project_selector.is_open = false;
                                            this.project_dialogs.open_create(window, cx);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("btn-open-project-center-dropdown")
                                        .ghost()
                                        .xsmall()
                                        .label("Project Center")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.project_selector.is_open = false;
                                            this.set_mode(AppMode::Projects, cx);
                                        })),
                                ),
                        ),
                )
            })
    }

    fn render_tab_navigation(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let modes = [
            (AppMode::Create, "Create", LucideIcon::SquarePen),
            (AppMode::Projects, "Projects", LucideIcon::BookOpen),
            (AppMode::Tools, "Tools", LucideIcon::Wrench),
        ];

        let active_index = match self.mode {
            AppMode::Create => 0,
            AppMode::Projects => 1,
            AppMode::Tools => 2,
        };

        div()
            .id("tab-navigation")
            .relative()
            .flex()
            .items_center()
            .p(px(4.0))
            .rounded(px(12.0))
            .bg(COLOR_BG_TERTIARY_LIGHT)
            .gap(px(4.0))
            // Sliding Pill Background Indicator (width 120px, height 32px, left animated)
            .child(
                div()
                    .id("tab-slider")
                    .absolute()
                    .top(px(4.0))
                    .left(px(4.0 + (active_index as f32) * (120.0 + 4.0)))
                    .w(px(120.0))
                    .h(px(32.0))
                    .rounded(px(6.0))
                    .bg(COLOR_BG_ELEVATED_LIGHT)
                    .shadow_sm(),
            )
            // 3 Mode Tab Buttons with Lucide Icons (matching React TabNavigation.tsx 1:1)
            .children(modes.into_iter().map(|(mode, label, icon_name)| {
                let is_active = self.mode == mode;

                div()
                    .id(ElementId::from(format!("tab-btn-{:?}", mode)))
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .h(px(32.0))
                    .w(px(120.0))
                    .px(px(12.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .text_size(px(14.0))
                    .font_weight(if is_active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(if is_active {
                        COLOR_TEXT_PRIMARY_LIGHT
                    } else {
                        COLOR_TEXT_MUTED_LIGHT
                    })
                    .hover(|style| style.text_color(COLOR_TEXT_SECONDARY_LIGHT))
                    .child(icon(icon_name).size(px(16.0)))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_mode(mode, cx);
                    }))
            }))
    }

    fn render_sync_pill(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.sync_pill.is_open;
        let is_busy = self.sync_pill.is_action_busy;
        let is_loaded = self.sync_pill.status.is_some();
        let lifecycle = self
            .sync_pill
            .status
            .as_ref()
            .map(|s| s.state)
            .unwrap_or(SyncLifecycleState::Disabled);
        let conflict_count = self
            .sync_pill
            .status
            .as_ref()
            .map(|s| s.conflict_count)
            .unwrap_or(0);

        let time_label = if let Some(ref status) = self.sync_pill.status {
            if let Some(ts) = status.last_success_at_ms {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let diff_sec = now.saturating_sub(ts) / 1000;
                if diff_sec < 60 {
                    "Just now".to_string()
                } else if diff_sec < 3600 {
                    format!("{}m ago", diff_sec / 60)
                } else if diff_sec < 86400 {
                    format!("{}h ago", diff_sec / 3600)
                } else {
                    "Synced".to_string()
                }
            } else {
                "Just now".to_string()
            }
        } else {
            "Never".to_string()
        };

        div()
            .relative()
            .child(
                // Status Pill trigger button (matches React sync-header-pill-btn)
                div()
                    .id("sync-header-pill-trigger")
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px(px(12.0))
                    .gap(px(6.0))
                    .border_1()
                    .border_color(COLOR_BORDER_LIGHT)
                    .rounded_full()
                    .bg(COLOR_BG_SECONDARY_LIGHT)
                    .text_size(px(12.8))
                    .cursor_pointer()
                    .hover(|style| style.border_color(COLOR_BORDER_HOVER_LIGHT).bg(COLOR_BG_TERTIARY_LIGHT))
                    .tooltip(|window, cx| Tooltip::new("Cloud Sync").build(window, cx))
                    .child(if !is_loaded || lifecycle == SyncLifecycleState::Disabled {
                        // Real Unconfigured / Empty state: <CloudOff size={14} /> Cloud Sync
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                icon(LucideIcon::CloudOff)
                                    .size(px(14.0))
                                    .text_color(COLOR_TEXT_SECONDARY_LIGHT),
                            )
                            .child(
                                div()
                                    .text_size(px(12.8))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                    .child("Cloud Sync"),
                            )
                            .into_any_element()
                    } else if conflict_count > 0 {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                icon(LucideIcon::TriangleAlert)
                                    .size(px(14.0))
                                    .text_color(COLOR_PROJECT_AMBER),
                            )
                            .child(format!("{conflict_count} conflicts"))
                            .into_any_element()
                    } else {
                        match lifecycle {
                            SyncLifecycleState::Syncing => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::RefreshCw)
                                        .size(px(14.0))
                                        .text_color(cx.theme().primary),
                                )
                                .child("Syncing...")
                                .into_any_element(),
                            SyncLifecycleState::Paused => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::Pause)
                                        .size(px(14.0))
                                        .text_color(COLOR_TEXT_MUTED_LIGHT),
                                )
                                .child("Paused")
                                .into_any_element(),
                            SyncLifecycleState::Locked => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::Lock)
                                        .size(px(14.0))
                                        .text_color(COLOR_TEXT_MUTED_LIGHT),
                                )
                                .child("Locked")
                                .into_any_element(),
                            SyncLifecycleState::Error => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::CircleAlert)
                                        .size(px(14.0))
                                        .text_color(cx.theme().danger),
                                )
                                .child("Sync error")
                                .into_any_element(),
                            _ => div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .size(px(6.0))
                                        .rounded_full()
                                        .bg(hsla(139.0 / 360.0, 0.35, 0.47, 1.0)),
                                )
                                .child(
                                    icon(LucideIcon::Cloud)
                                        .size(px(14.0))
                                        .text_color(COLOR_TEXT_MUTED_LIGHT),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.8))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                        .child(time_label.clone()),
                                )
                                .into_any_element(),
                        }
                    }),
            )
            // Popover dialog (width 300px, rounded-lg, matching React sync-header-popover)
            .when(is_open, |this| {
                this.child(
                    div()
                        .id("sync-header-popover")
                        .absolute()
                        .top(px(40.0))
                        .right_0()
                        .w(px(300.0))
                        .rounded(RADIUS_LG)
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(COLOR_BORDER_LIGHT)
                        .shadow_lg()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        // Popover header
                        .child(
                            div()
                                .id("sync-popover-header")
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_4()
                                .py_3()
                                .bg(COLOR_BG_SECONDARY_LIGHT)
                                .border_b_1()
                                .border_color(COLOR_BORDER_LIGHT)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            if !is_loaded || lifecycle == SyncLifecycleState::Disabled {
                                                icon(LucideIcon::CloudOff).size(px(16.0))
                                            } else {
                                                icon(LucideIcon::Cloud).size(px(16.0))
                                            },
                                        )
                                        .child(
                                            div()
                                                .text_size(px(14.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                                .child("Cloud Sync"),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(if !is_loaded || lifecycle == SyncLifecycleState::Disabled {
                                            COLOR_BG_TERTIARY_LIGHT
                                        } else {
                                            hsla(139.0 / 360.0, 0.35, 0.47, 0.15)
                                        })
                                        .text_size(px(11.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if !is_loaded || lifecycle == SyncLifecycleState::Disabled {
                                            COLOR_TEXT_MUTED_LIGHT
                                        } else {
                                            hsla(139.0 / 360.0, 0.35, 0.47, 1.0)
                                        })
                                        .child(match lifecycle {
                                            SyncLifecycleState::Disabled => "Disabled",
                                            SyncLifecycleState::Idle => "Connected",
                                            SyncLifecycleState::Syncing => "Syncing",
                                            SyncLifecycleState::Paused => "Paused",
                                            SyncLifecycleState::Locked => "Locked",
                                            SyncLifecycleState::Error => "Error",
                                        }),
                                ),
                        )
                        // Popover body
                        .child(
                            div()
                                .id("sync-popover-body")
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .when(!is_loaded || lifecycle == SyncLifecycleState::Disabled, |this| {
                                    this.child(
                                        div()
                                            .text_size(px(13.0))
                                            .line_height(relative(1.5))
                                            .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                            .child("Multi-device sync is not configured yet. Set up WebDAV cloud sync to keep your transcripts safe and in sync across devices."),
                                    )
                                    .child(
                                        Button::new("btn-enable-sync")
                                            .primary()
                                            .small()
                                            .icon(LucideIcon::ShieldCheck)
                                            .label("Enable Cloud Sync")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.sync_pill.is_open = false;
                                                this.settings_modal.set_tab(SettingsTab::Sync, cx);
                                                this.settings_modal.open(cx);
                                                this.settings_modal.load_preferences(cx);
                                                cx.notify();
                                            })),
                                    )
                                })
                                .when(is_loaded && lifecycle != SyncLifecycleState::Disabled, |this| {
                                    let last_err_msg = self.sync_pill.status.as_ref().and_then(|s| s.last_error.as_ref()).map(|e| e.message.clone());
                                    let preset_name = self.sync_pill.status.as_ref().and_then(|s| s.preset).map(|p| format!("{p:?}")).unwrap_or_else(|| "-".to_string());
                                    this.when(conflict_count > 0, |card| {
                                        card.child(
                                            div()
                                                .id("sync-popover-conflict-banner")
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .gap_2()
                                                .p_2()
                                                .rounded(RADIUS_SM)
                                                .bg(hsla(45.0 / 360.0, 0.93, 0.47, 0.12))
                                                .border_1()
                                                .border_color(COLOR_PROJECT_AMBER)
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.sync_pill.is_open = false;
                                                    this.settings_modal.set_tab(SettingsTab::Sync, cx);
                                                    this.settings_modal.open(cx);
                                                    this.settings_modal.load_preferences(cx);
                                                    cx.notify();
                                                }))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(icon(LucideIcon::TriangleAlert).size(px(15.0)).text_color(COLOR_PROJECT_AMBER))
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(COLOR_PROJECT_AMBER)
                                                                .child(format!("{conflict_count} content conflicts need review.")),
                                                        ),
                                                )
                                                .child(icon(LucideIcon::ExternalLink).size(px(13.0)).text_color(COLOR_PROJECT_AMBER)),
                                        )
                                    })
                                    .when_some(last_err_msg, |card, msg| {
                                        card.child(
                                            div()
                                                .id("sync-popover-error-banner")
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .p_2()
                                                .rounded(RADIUS_SM)
                                                .bg(hsla(0.0, 0.84, 0.60, 0.10))
                                                .border_1()
                                                .border_color(cx.theme().danger)
                                                .child(icon(LucideIcon::CircleAlert).size(px(14.0)).text_color(cx.theme().danger))
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(cx.theme().danger)
                                                        .child(msg),
                                                ),
                                        )
                                    })
                                    .child(
                                        div()
                                            .id("sync-popover-metrics")
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .p_2()
                                            .rounded(RADIUS_SM)
                                            .bg(COLOR_BG_SECONDARY_LIGHT)
                                            .border_1()
                                            .border_color(COLOR_BORDER_LIGHT)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .text_size(px(12.0))
                                                    .child(div().text_color(COLOR_TEXT_MUTED_LIGHT).child("Last success:"))
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                                            .child(time_label.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .text_size(px(12.0))
                                                    .child(div().text_color(COLOR_TEXT_MUTED_LIGHT).child("Preset scope:"))
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(COLOR_TEXT_PRIMARY_LIGHT)
                                                            .child(preset_name),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Button::new("btn-sync-run-now")
                                                    .primary()
                                                    .small()
                                                    .icon(LucideIcon::RefreshCw)
                                                    .label(if lifecycle == SyncLifecycleState::Syncing {
                                                        "Syncing..."
                                                    } else {
                                                        "Sync now"
                                                    })
                                                    .disabled(
                                                        is_busy
                                                            || lifecycle == SyncLifecycleState::Syncing
                                                            || lifecycle == SyncLifecycleState::Paused,
                                                    )
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.sync_pill.run_sync_now(cx);
                                                    })),
                                            )
                                            .child(
                                                Button::new("btn-sync-pause")
                                                    .ghost()
                                                    .small()
                                                    .icon(if lifecycle == SyncLifecycleState::Paused {
                                                        LucideIcon::Play
                                                    } else {
                                                        LucideIcon::Pause
                                                    })
                                                    .label(if lifecycle == SyncLifecycleState::Paused {
                                                        "Resume"
                                                    } else {
                                                        "Pause"
                                                    })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.sync_pill.toggle_pause(cx);
                                                    })),
                                            )
                                            .child(
                                                Button::new("btn-sync-settings")
                                                    .ghost()
                                                    .small()
                                                    .icon(LucideIcon::Settings)
                                                    .label("Manage")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.sync_pill.is_open = false;
                                                        this.settings_modal.set_tab(SettingsTab::Sync, cx);
                                                        this.settings_modal.open(cx);
                                                        this.settings_modal.load_preferences(cx);
                                                        cx.notify();
                                                    })),
                                            ),
                                    )
                                }),
                        ),
                )
            })
    }
}

impl Render for MainShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_mode = self.mode;
        let show_transcript_host = self.should_show_transcript_host();
        let project_inbox_count = self.project_selector.count_inbox;
        let project_all_count =
            project_inbox_count + self.project_selector.project_counts.values().sum::<usize>();
        let project_trash_count = self.projects_results.trash_count;

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(COLOR_BG_SECONDARY_LIGHT)
            .text_color(COLOR_TEXT_PRIMARY_LIGHT)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.is_held {
                    return;
                }

                if event.keystroke.key.eq_ignore_ascii_case("escape")
                    && this.workbench.search.is_visible
                {
                    this.workbench.search.is_visible = false;
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }

                if event.keystroke.key.eq_ignore_ascii_case("f")
                    && (event.keystroke.modifiers.control || event.keystroke.modifiers.platform)
                    && this.should_show_transcript_host()
                {
                    this.workbench.search.open(window, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }

                if this.settings_modal.is_editing_shortcut {
                    if event.keystroke.key.eq_ignore_ascii_case("escape") {
                        let current = this.settings_modal.live_record_shortcut.clone();
                        this.settings_modal.shortcut_draft = current.clone();
                        this.settings_modal.is_editing_shortcut = false;
                        this.shortcut_edit_state.update(cx, |state, cx| {
                            state.set_value(current, window, cx);
                        });
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                    }
                    return;
                }

                if event.keystroke.key.eq_ignore_ascii_case("escape")
                    && this.notification_center.read(cx).is_open
                {
                    this.notification_center.update(cx, |center, cx| {
                        if center.clear_menu_open {
                            center.clear_menu_open = false;
                        } else {
                            center.is_open = false;
                        }
                        cx.notify();
                    });
                    window.prevent_default();
                    cx.stop_propagation();
                    return;
                }

                if this.settings_modal.is_recording_shortcut {
                    let key = event.keystroke.key.as_str();
                    if key.eq_ignore_ascii_case("escape") {
                        this.settings_modal.is_recording_shortcut = false;
                        this.settings_modal.shortcut_recording_target = None;
                    } else if !["control", "ctrl", "alt", "shift", "meta", "super"]
                        .iter()
                        .any(|modifier| key.eq_ignore_ascii_case(modifier))
                    {
                        let modifiers = event.keystroke.modifiers;
                        let mut parts = Vec::new();
                        if modifiers.control {
                            parts.push("Ctrl");
                        }
                        if modifiers.alt {
                            parts.push("Alt");
                        }
                        if modifiers.shift {
                            parts.push("Shift");
                        }
                        if modifiers.platform {
                            parts.push("Meta");
                        }
                        let main_key = if key == " " || key.eq_ignore_ascii_case("space") {
                            "Space".to_string()
                        } else if key.len() == 1 {
                            key.to_uppercase()
                        } else {
                            key.to_string()
                        };
                        parts.push(&main_key);
                        let shortcut = parts.join(" + ");
                        let target = this
                            .settings_modal
                            .shortcut_recording_target
                            .take()
                            .unwrap_or(ShortcutTarget::LiveRecord);
                        this.settings_modal.is_recording_shortcut = false;
                        match target {
                            ShortcutTarget::LiveRecord => {
                                this.settings_modal.live_record_shortcut = shortcut.clone();
                                this.settings_modal.persist_shortcut(shortcut, cx);
                            }
                            ShortcutTarget::VoiceTyping => {
                                let controller = cx.global::<GlobalHotkeyController>().0.clone();
                                let result = controller
                                    .write()
                                    .rebind_shortcut(
                                        crate::platform::hotkey::HotkeyAction::VoiceTyping,
                                        &shortcut,
                                    );
                                match result {
                                    Ok(()) => {
                                        let services = &cx.global::<GlobalDesktopServices>().0;
                                        if let Err(error) = crate::platform::agent_control::facade::settings::update_setting(
                                            services,
                                            "voiceTypingShortcut".to_string(),
                                            serde_json::Value::String(shortcut),
                                        ) {
                                            this.global_dialog.show(
                                                "Shortcut Save Failed",
                                                error,
                                                crate::views::dialogs::global_dialog::DialogKind::Alert,
                                            );
                                        }
                                    }
                                    Err(error) => this.global_dialog.show(
                                        "Shortcut Registration Failed",
                                        error,
                                        crate::views::dialogs::global_dialog::DialogKind::Alert,
                                    ),
                                }
                            }
                            ShortcutTarget::VoiceTypingTranslate => {
                                let controller = cx.global::<GlobalHotkeyController>().0.clone();
                                let result = controller.write().rebind_shortcut(
                                    crate::platform::hotkey::HotkeyAction::VoiceTypingTranslate,
                                    &shortcut,
                                );
                                match result {
                                    Ok(()) => {
                                        let services = &cx.global::<GlobalDesktopServices>().0;
                                        if let Err(error) = crate::platform::agent_control::facade::settings::update_setting(
                                            services,
                                            "voiceTypingTranslateShortcut".to_string(),
                                            serde_json::Value::String(shortcut),
                                        ) {
                                            this.global_dialog.show(
                                                "Shortcut Save Failed",
                                                error,
                                                crate::views::dialogs::global_dialog::DialogKind::Alert,
                                            );
                                        }
                                    }
                                    Err(error) => this.global_dialog.show(
                                        "Shortcut Registration Failed",
                                        error,
                                        crate::views::dialogs::global_dialog::DialogKind::Alert,
                                    ),
                                }
                            }
                            ShortcutTarget::VoiceTypingQuickRecall => {
                                let controller = cx.global::<GlobalHotkeyController>().0.clone();
                                let result = controller
                                    .write()
                                    .rebind_shortcut(
                                        crate::platform::hotkey::HotkeyAction::QuickRecall,
                                        &shortcut,
                                    );
                                match result {
                                    Ok(()) => {
                                        let services = &cx.global::<GlobalDesktopServices>().0;
                                        if let Err(error) = crate::platform::agent_control::facade::settings::update_setting(
                                            services,
                                            "voiceTypingQuickRecallShortcut".to_string(),
                                            serde_json::Value::String(shortcut),
                                        ) {
                                            this.global_dialog.show(
                                                "Shortcut Save Failed",
                                                error,
                                                crate::views::dialogs::global_dialog::DialogKind::Alert,
                                            );
                                        }
                                    }
                                    Err(error) => this.global_dialog.show(
                                        "Shortcut Registration Failed",
                                        error,
                                        crate::views::dialogs::global_dialog::DialogKind::Alert,
                                    ),
                                }
                            }
                        }
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }

                if this.recovery_modal.is_open && event.keystroke.key.eq_ignore_ascii_case("escape")
                {
                    this.recovery_modal.close();
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }

                if this.diagnostics_modal.is_open
                    && event.keystroke.key.eq_ignore_ascii_case("escape")
                {
                    this.diagnostics_modal.close();
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }

                if this.global_dialog.is_open && event.keystroke.key.eq_ignore_ascii_case("escape")
                {
                    this.global_dialog.close();
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }

                if this.settings_modal.is_open {
                    if event.keystroke.key.eq_ignore_ascii_case("escape") {
                        if this.settings_modal.vocabulary_dictionary_dialog_open {
                            this.settings_modal.vocabulary_dictionary_dialog_open = false;
                            this.settings_modal.vocabulary_dictionary_edit_line = None;
                            window.prevent_default();
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                        this.settings_modal.close(cx);
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if event.keystroke.key.eq_ignore_ascii_case("tab")
                        && event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.platform
                    {
                        this.settings_modal
                            .cycle_tab(event.keystroke.modifiers.shift, cx);
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                } else if event.keystroke.key.eq_ignore_ascii_case("tab")
                    && event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.platform
                    && !this.diagnostics_modal.is_open
                    && !this.recovery_modal.is_open
                    && !this.global_dialog.is_open
                    && !this.export_modal.is_open
                    && !this.onboarding.is_open
                    && !this.project_dialogs.is_create_open
                    && !this.project_dialogs.is_delete_open
                    && !this.error_dialog.is_open
                {
                    this.cycle_mode(event.keystroke.modifiers.shift, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                let batch_import = this.create_workspace.read(cx).batch_import.clone();
                let batch_import = batch_import.read(cx);
                let text_input_focused = this
                    .project_search_state
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
                    || this
                        .model_search_state
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || this
                        .mcp_search_state
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || this
                        .shortcut_edit_state
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                    || batch_import
                        .stream_url_state
                        .as_ref()
                        .is_some_and(|state| state.read(cx).focus_handle(cx).is_focused(window))
                    || batch_import
                        .stream_alias_state
                        .as_ref()
                        .is_some_and(|state| state.read(cx).focus_handle(cx).is_focused(window));
                if text_input_focused {
                    return;
                }

                let live_record = this.create_workspace.read(cx).live_record.clone();
                let (is_recording, is_initializing) = {
                    let panel = live_record.read(cx);
                    (panel.is_recording, panel.is_initializing)
                };
                let is_start_stop = shortcut_matches(
                    &this.settings_modal.live_record_shortcut,
                    &event.keystroke.key,
                    event.keystroke.modifiers,
                );
                let is_plain_space = (event.keystroke.key == " "
                    || event.keystroke.key.eq_ignore_ascii_case("space"))
                    && !event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.shift
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.function;

                if is_start_stop {
                    if !is_initializing {
                        live_record.update(cx, |panel, cx| {
                            if is_recording {
                                panel.stop_recording(cx);
                            } else {
                                panel.start_recording(cx);
                            }
                        });
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                } else if is_plain_space && is_recording {
                    live_record.update(cx, |panel, cx| panel.toggle_pause(cx));
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .child(
                // ==========================================
                // Top Application Header Bar (56px 3-column 1fr auto 1fr grid)
                // ==========================================
                div()
                    .id("app-header")
                    .flex()
                    .items_center()
                    .h(HEADER_HEIGHT)
                    .w_full()
                    .px(px(24.0)) // padding: 8px 24px
                    .bg(COLOR_HEADER_BG_LIGHT)
                    .border_b_1()
                    .border_color(COLOR_BORDER_LIGHT)
                    // Column 1: Left Logo & ProjectSelectorDropdown (1fr)
                    .child(
                        div()
                            .id("app-logo")
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_start()
                            .gap(px(8.0))
                            .child(
                                img("sona.png")
                                    .id("app-logo-img")
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .rounded(px(6.0))
                                    .opacity(0.92)
                                    .hover(|style| style.opacity(1.0)),
                            )
                            .child(self.render_project_selector(window, cx)),
                    )
                    // Column 2: Center 3-Mode TabNavigation (auto)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(self.render_tab_navigation(cx)),
                    )
                    // Column 3: Right Header Actions: SyncPill, NotificationCenter, Settings (1fr)
                    .child(
                        div()
                            .id("header-actions")
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.0))
                            .child(self.render_sync_pill(cx))
                            .child(self.notification_center.clone())
                            .child(
                                // Icon-only Settings button (36x36px)
                                div()
                                    .id("btn-open-settings")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(36.0))
                                    .rounded(RADIUS_MD)
                                    .text_color(COLOR_TEXT_SECONDARY_LIGHT)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(COLOR_BG_TERTIARY_LIGHT).text_color(COLOR_TEXT_PRIMARY_LIGHT))
                                    .active(|s| s.bg(COLOR_BG_SECONDARY_LIGHT))
                                    .tooltip(|window, cx| Tooltip::new("Settings").build(window, cx))
                                    .child(icon(LucideIcon::Settings).size(px(18.0)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_modal.open(cx);
                                        this.settings_modal.load_preferences(cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                // ==========================================
                // Main Viewport Container (.app-main)
                // ==========================================
                div()
                    .id("app-main")
                    .relative()
                    .flex()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .child(match current_mode {
                        AppMode::Create => div()
                            .id("workspace-mode-shell")
                            .flex()
                            .flex_1()
                            .size_full()
                            .child(
                                // Two-panel workspace container
                                div()
                                    .id("panel-container")
                                    .flex()
                                    .flex_1()
                                    .size_full()
                                    .overflow_hidden()
                                    // Left Panel (strict 350px width)
                                    .child(
                                        div()
                                            .id("panel-left")
                                            .flex_none()
                                            .w(PANEL_LEFT_WIDTH)
                                            .min_w(PANEL_LEFT_WIDTH)
                                            .max_w(PANEL_LEFT_WIDTH)
                                            .h_full()
                                            .flex()
                                            .flex_col()
                                            .bg(COLOR_BG_TERTIARY_LIGHT)
                                            .child(
                                                // .panel-content (holding CreateWorkspacePanel)
                                                div()
                                                    .id("panel-content")
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .child(self.create_workspace.clone()),
                                            )
                                            .child(
                                                // .panel-footer (holding TranscriptionOptions capsule)
                                                div()
                                                    .id("panel-footer")
                                                    .relative()
                                                    .flex_none()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .px(px(20.0))
                                                    .pt(px(16.0))
                                                    .pb(px(20.0))
                                                    .child(
                                                        self.options_capsule.render(
                                                            self.create_workspace
                                                                .read(cx)
                                                                .live_record
                                                                .read(cx)
                                                                .is_recording,
                                                            cx,
                                                        ),
                                                    ),
                                            ),
                                    )
                                    // Right Panel (.panel-right: flex 1, min-width 500px, rounded-tl 16px)
                                    .child(
                                        div()
                                            .id("panel-right")
                                            .flex_1()
                                            .min_w(PANEL_RIGHT_MIN_WIDTH)
                                            .h_full()
                                            .bg(gpui_kit::white())
                                            .border_t_1()
                                            .border_l_1()
                                            .border_color(COLOR_BORDER_LIGHT)
                                            .rounded_tl(px(16.0))
                                            .overflow_hidden()
                                            .child(self.workbench.render(window, cx)),
                                    ),
                            )
                            .into_any_element(),
                        AppMode::Projects => div()
                            .id("projects-mode-shell")
                            .flex()
                            .size_full()
                            .child(self.projects_rail.render(
                                &self.projects_results.scope,
                                project_inbox_count,
                                project_all_count,
                                project_trash_count,
                                cx,
                            ))
                            .child({
                                let browse_project = match &self.projects_results.scope {
                                    HistoryWorkspaceScope::Project { project_id } => {
                                        self.project_selector.projects.iter().find(|p| &p.id == project_id)
                                    }
                                    _ => None,
                                };
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .h_full()
                                    .child(self.projects_header.render(
                                        &self.projects_results.scope,
                                        browse_project,
                                        self.projects_results.summary.as_ref(),
                                        project_trash_count,
                                        cx,
                                    ))
                                    .child(
                                        self.projects_toolbar.render(
                                            &self.project_search_state,
                                            self.projects_results.filter_type,
                                            self.projects_results.date_filter,
                                            self.projects_results.filtered_count,
                                            self.projects_results.scoped_count,
                                            cx,
                                        ))
                                    .child(
                                        div()
                                            .relative()
                                            .flex_1()
                                            .min_h_0()
                                            .overflow_hidden()
                                            .child(self.projects_results.render(
                                                self.projects_toolbar.view_mode,
                                                self.projects_toolbar.is_selection_active,
                                                &self.project_selector.projects,
                                                &self.history_rename_state,
                                                cx,
                                            ))
                                            .when(
                                                self.projects_toolbar.is_selection_active,
                                                |this| {
                                                    let is_trash_scope = matches!(
                                                        self.projects_results.scope,
                                                        HistoryWorkspaceScope::Trash
                                                    );
                                                    let selected_count =
                                                        self.projects_results.selected_ids.len();
                                                    let visible_count =
                                                        self.projects_results.items.len();
                                                    let projects =
                                                        self.project_selector.projects.clone();
                                                    this.child(self.selection_bar.render(
                                                        is_trash_scope,
                                                        selected_count,
                                                        visible_count,
                                                        &projects,
                                                        cx,
                                                    ))
                                                },
                                            ),
                                    )
                            })
                            .into_any_element(),
                        AppMode::Tools => {
                            let media_server_name = self.media_server_tool.server_name();
                            let is_recording_shortcut = self.settings_modal.is_recording_shortcut;
                            let shortcut_target = self.settings_modal.shortcut_recording_target;
                            let queued_media_ids = {
                                let profile_id = self.media_server_tool.active_profile_id.as_deref();
                                self.create_workspace
                                    .read(cx)
                                    .batch_import
                                    .read(cx)
                                    .queue
                                    .iter()
                                    .filter(|item| {
                                        matches!(
                                            item.status,
                                            crate::views::create::BatchItemStatus::Pending
                                                | crate::views::create::BatchItemStatus::Processing
                                        )
                                    })
                                    .filter_map(|item| item.media_server_context.as_ref())
                                    .filter(|context| profile_id.is_none_or(|id| context.profile_id == id))
                                    .map(|context| context.item_id.clone())
                                    .collect::<std::collections::HashSet<_>>()
                            };

                            let tool_panel: AnyElement = match self.tools_sidebar.active_tool {
                                ToolKind::Subtitles => {
                                    self.subtitles_tool.render(cx).into_any_element()
                                }
                                ToolKind::VoiceTyping => {
                                    self.voice_typing_tool.render(is_recording_shortcut, shortcut_target, cx).into_any_element()
                                }
                                ToolKind::ApiServer => {
                                    self.api_server_tool.render(&self.api_server_field_states, cx).into_any_element()
                                }
                                ToolKind::Mcp => self.mcp_tool.render(cx).into_any_element(),
                                ToolKind::MediaServer => {
                                    self.media_server_tool.render(&queued_media_ids, cx).into_any_element()
                                }
                            };

                            div()
                                .id("tools-mode-shell")
                                .flex()
                                .size_full()
                                .bg(cx.theme().secondary)
                                .child(self.tools_sidebar.render(media_server_name.as_deref(), cx))
                                .child(
                                    div()
                                        .id("tools-detail-pane")
                                        .flex_1()
                                        .min_w(PANEL_RIGHT_MIN_WIDTH)
                                        .h_full()
                                        .bg(cx.theme().background)
                                        .border_t_1()
                                        .border_l_1()
                                        .border_color(cx.theme().border)
                                        .rounded_tl(RADIUS_LG)
                                        .overflow_y_scroll()
                                        .p_8()
                                        .px(px(48.0))
                                        .child(tool_panel),
                                )
                                .into_any_element()
                        }
                    })
                    // Persistent Transcript Host overlay (sliding over inset: 0 0 0 350px)
                    .when(
                        show_transcript_host && current_mode == AppMode::Projects,
                        |this| {
                            this.child(
                                div()
                                    .id("persistent-transcript-host")
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .right_0()
                                    .left(PANEL_LEFT_WIDTH)
                                    .border_t_1()
                                    .border_l_1()
                                    .border_color(COLOR_BORDER_LIGHT)
                                    .rounded_tl(RADIUS_LG)
                                    .bg(COLOR_BG_PRIMARY_LIGHT)
                                    .overflow_hidden()
                                    .child(self.workbench.render(window, cx)),
                            )
                        },
                    ),
            )
            // Modals & Dialogs (conditionally mounted to prevent stack overflow and frame overhead)
            .when(self.settings_modal.is_open, |this| {
                this.child(self.settings_modal.render(
                    &self.shortcut_edit_state,
                    &self.model_search_state,
                    &self.mcp_search_state,
                    &mut self.voice_typing_tool,
                    &mut self.subtitles_tool,
                    &self.api_server_field_states,
                    &self.model_engine_select,
                    &self.model_status_select,
                    &self.model_label_select,
                    &self.online_provider_field_states,
                    &self.llm_provider_api_host_state,
                    &self.llm_provider_api_key_state,
                    &self.llm_model_draft_state,
                    &self.llm_custom_provider_name_state,
                    &self.llm_model_metadata_field_states,
                    &self.dictionary_textarea_state,
                    &self.dictionary_quick_add_state,
                    &self.dictionary_search_state,
                    &self.dictionary_test_state,
                    &self.vocabulary_name_state,
                    &self.vocabulary_body_state,
                    &self.translation_language_select,
                    &self.general_log_level_select,
                    &self.general_language_select,
                    &self.general_font_select,
                    &self.project_selector.projects,
                    self.workbench.history_id.as_deref(),
                    &self.dictionary_word_state,
                    &self.dictionary_weight_state,
                    &self.dictionary_from_state,
                    &self.dictionary_to_state,
                    &self.dictionary_inline_hotword_state,
                    &self.speaker_profile_name_state,
                    &self.speaker_profile_search_state,
                    &self.speaker_profile_name_states,
                    window,
                    cx,
                ))
            })
            .when(self.diagnostics_modal.is_open, |this| {
                this.child(self.diagnostics_modal.render(cx))
            })
            .when(self.recovery_modal.is_open, |this| {
                this.child(self.recovery_modal.render(cx))
            })
            .when(self.global_dialog.is_open, |this| {
                this.child(self.global_dialog.render(cx))
            })
            .when(self.export_modal.is_open, |this| {
                this.child(self.export_modal.render(cx))
            })
            .when(self.onboarding.is_open, |this| {
                this.child(self.onboarding.render(cx))
            })
            .when(
                self.project_dialogs.is_create_open
                    || self.project_dialogs.is_settings_open
                    || self.project_dialogs.is_delete_open,
                |this| this.child(self.project_dialogs.render(cx)),
            )
            .when(self.error_dialog.is_open, |this| {
                this.child(self.error_dialog.render(cx))
            })
    }
}
