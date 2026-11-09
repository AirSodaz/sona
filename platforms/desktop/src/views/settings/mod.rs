//! Settings modal and its retained data views.

pub(crate) mod about;
pub(crate) mod api_server;
pub(crate) mod automation;
pub(crate) mod dashboard;
pub(crate) mod general;
pub(crate) mod languages;
pub(crate) mod llm_service;
pub(crate) mod mcp;
pub(crate) mod microphone;
pub(crate) mod models;
mod search;
pub(crate) mod shortcuts;
pub(crate) mod speaker_profiles;
pub(crate) mod storage;
pub(crate) mod subtitles;
pub(crate) mod sync;
pub mod types;
pub(crate) mod vocabulary;

pub use about::AboutSettingsTab;
pub use api_server::ApiServerSettingsTab;
pub use automation::AutomationSettingsTab;
pub use dashboard::DashboardSettingsTab;
pub use general::GeneralSettingsTab;
pub use languages::LANGUAGE_OPTIONS;
pub use llm_service::{
    LlmServiceSettingsTab, apply_llm_model_metadata_inputs, llm_slug,
    remove_custom_provider_settings, remove_llm_model_settings,
};
pub use mcp::McpSettingsTab;
pub use microphone::MicrophoneSettingsTab;
pub use models::{
    ModelsSettingsTab, online_provider_added_model_ids, online_provider_default_name,
    online_provider_is_flash_batch_mode, online_provider_languages_tooltip,
    online_provider_logo_path, online_provider_model_options, update_online_provider_model_config,
};
pub use shortcuts::ShortcutsSettingsTab;
pub use storage::StorageSettingsTab;
pub use subtitles::SubtitlesSettingsTab;
pub use sync::SyncSettingsTab;
pub use types::*;
pub use vocabulary::{
    VocabularySettingsTab, add_dictionary_entry_to_scope, legacy_dictionary_sets,
    parse_dictionary_entries, remove_dictionary_line, update_dictionary_line,
};

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use crate::app::GlobalDesktopServices;
use crate::theme::tokens::*;

#[derive(Clone, Debug)]
pub enum SettingsModalEvent {
    OpenDiagnostics,
    RequestApiKeyRegenConfirm,
    RequestCudaDownloadConfirm,
    RequestProviderDeleteConfirm { provider_id: String, name: String },
}

impl EventEmitter<SettingsModalEvent> for SettingsModal {}

pub struct SettingsModal {
    pub is_open: bool,
    pub current_tab: SettingsTab,
    pub model_config: Value,
    pub general: GeneralSettingsTab,
    pub microphone: MicrophoneSettingsTab,
    pub models: ModelsSettingsTab,
    pub subtitles: SubtitlesSettingsTab,
    pub shortcuts: ShortcutsSettingsTab,
    pub storage: StorageSettingsTab,
    pub sync: SyncSettingsTab,
    pub about: AboutSettingsTab,
    pub automation: AutomationSettingsTab,
    pub dashboard: DashboardSettingsTab,
    pub api_server: ApiServerSettingsTab,
    pub mcp: McpSettingsTab,
    pub vocabulary: VocabularySettingsTab,
    pub llm_service: LlmServiceSettingsTab,
    settings_search_state: Entity<InputState>,
    settings_search_query: String,
    settings_search_active_index: usize,
    _settings_search_subscription: Subscription,
}

impl SettingsModal {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let initial_config = services
            .config
            .load()
            .ok()
            .flatten()
            .unwrap_or_else(|| json!({}));

        let general = GeneralSettingsTab::new(&initial_config, window, cx);
        let microphone = MicrophoneSettingsTab::new(&initial_config);
        let models = ModelsSettingsTab::new(window, cx);
        let subtitles = SubtitlesSettingsTab::new(&initial_config, window, cx);
        let shortcuts = ShortcutsSettingsTab::new(&initial_config, window, cx);
        let storage = StorageSettingsTab::new();
        let sync = SyncSettingsTab::new();
        let about = AboutSettingsTab::new();
        let automation = AutomationSettingsTab::new();
        let dashboard = DashboardSettingsTab::new();
        let api_server = ApiServerSettingsTab::new(&initial_config, window, cx);
        let mcp = McpSettingsTab::new(window, cx);
        let vocabulary = VocabularySettingsTab::new(&initial_config, window, cx);
        let llm_service = LlmServiceSettingsTab::new(window, cx);
        let settings_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search settings..."));
        let _settings_search_subscription = cx.subscribe(
            &settings_search_state,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    modal.settings_search_query = state.read(cx).value().to_string();
                    modal.settings_search_active_index = 0;
                    cx.notify();
                }
            },
        );

        Self {
            is_open: false,
            current_tab: SettingsTab::General,
            model_config: initial_config,
            general,
            microphone,
            models,
            subtitles,
            shortcuts,
            storage,
            sync,
            about,
            automation,
            dashboard,
            api_server,
            mcp,
            vocabulary,
            llm_service,
            settings_search_state,
            settings_search_query: String::new(),
            settings_search_active_index: 0,
            _settings_search_subscription,
        }
    }

    pub fn open(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        self.is_open = true;
        self.set_tab(tab, cx);
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_open = false;
        self.clear_search(window, cx);
        self.microphone.on_deactivate(cx);
        cx.notify();
    }

    pub fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_search_query.clear();
        self.settings_search_active_index = 0;
        self.settings_search_state
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    fn select_search_hit(
        &mut self,
        hit_id: &'static str,
        tab: SettingsTab,
        cx: &mut Context<Self>,
    ) {
        self.set_tab(tab, cx);
        match search::subsection_for_hit(hit_id) {
            Some(search::SearchSubsection::ModelScenarioBatch(batch)) => {
                self.models.model_scenario_batch = batch;
            }
            Some(search::SearchSubsection::VoiceTyping(voice_typing)) => {
                self.subtitles.subtitle_voice_typing_tab = voice_typing;
            }
            Some(search::SearchSubsection::Vocabulary(subsection)) => {
                if matches!(subsection, "recognition" | "speakers") {
                    self.vocabulary.active_tab = subsection.to_string();
                } else {
                    self.vocabulary.active_tab = "prompts".to_string();
                    self.vocabulary.active_prompt_tab = subsection.to_string();
                }
            }
            None => {}
        }
        cx.notify();
    }

    pub fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_search_state
            .update(cx, |state, cx| state.focus(window, cx));
    }

    pub fn has_search_query(&self) -> bool {
        !self.settings_search_query.is_empty()
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn set_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        if self.current_tab == SettingsTab::Microphone && tab != SettingsTab::Microphone {
            self.microphone.on_deactivate(cx);
        } else if tab == SettingsTab::Microphone {
            self.microphone.load_devices(cx);
            self.microphone.on_activate(cx);
        }

        if tab == SettingsTab::Models {
            self.models.load_model_catalog(cx);
            self.models.load_cuda_addon_status(cx);
        } else if tab == SettingsTab::Storage {
            self.storage.load_storage_state(cx);
        } else if tab == SettingsTab::Sync {
            self.sync.load_sync_state(cx);
        } else if tab == SettingsTab::Automation {
            self.automation.load_automation_state(cx);
        } else if tab == SettingsTab::Dashboard && !self.dashboard.load_attempted {
            self.dashboard.load_dashboard(cx);
        } else if tab == SettingsTab::Mcp && !self.mcp.logs_loaded {
            self.mcp.load_logs(cx);
        }

        self.current_tab = tab;
        cx.notify();
    }

    pub fn next_tab(current_tab: SettingsTab, reverse: bool) -> SettingsTab {
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

    pub fn load_preferences(&mut self, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let loaded = services
                .config
                .load()
                .ok()
                .flatten()
                .unwrap_or_else(|| json!({}));
            this.update(cx, |modal, cx| {
                modal.model_config = loaded.clone();
                modal.general.autostart = loaded
                    .get("autoStart")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                modal.general.minimize_to_tray = loaded
                    .get("minimizeToTrayOnExit")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                modal.general.silent_start = loaded
                    .get("silentStart")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                modal.general.auto_check_updates = loaded
                    .get("autoCheckUpdates")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn remove_llm_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        if remove_llm_model_settings(&mut self.model_config["llmSettings"], model_id) {
            if self.llm_service.editing_model.as_deref() == Some(model_id) {
                self.llm_service.editing_model = None;
            }
            let val = self.model_config["llmSettings"].clone();
            self.persist_model_setting("llmSettings".to_string(), val, cx);
        }
    }

    pub fn persist_audio_value(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.microphone.persist_audio_value(key, value, cx);
    }

    pub fn set_llm_provider_field(
        &mut self,
        field: &'static str,
        value: String,
        cx: &mut Context<Self>,
    ) {
        self.llm_service.ensure_llm_settings(&mut self.model_config);
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
        self.model_config["llmSettings"]["providers"][&provider][field] = Value::String(value);
        let val = self.model_config["llmSettings"].clone();
        self.persist_model_setting("llmSettings".to_string(), val, cx);
    }
    pub fn set_llm_active_provider(&mut self, provider: String, cx: &mut Context<Self>) {
        self.llm_service.ensure_llm_settings(&mut self.model_config);
        if !self.model_config["llmSettings"]["providers"].is_object() {
            self.model_config["llmSettings"]["providers"] = json!({});
        }
        if !self.model_config["llmSettings"]["providers"][&provider].is_object() {
            self.model_config["llmSettings"]["providers"][&provider] = json!({
                "apiHost": "https://api.openai.com",
                "apiKey": ""
            });
        }
        self.model_config["llmSettings"]["activeProvider"] = Value::String(provider);
        let val = self.model_config["llmSettings"].clone();
        self.persist_model_setting("llmSettings".to_string(), val, cx);
    }

    pub fn submit_vocabulary_entry(
        &mut self,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        let current = self
            .vocabulary
            .dictionary_textarea
            .read(cx)
            .value()
            .to_string();
        let target_scope_id = self.vocabulary.add_scope.clone();
        let target_scope_name = if target_scope_id == "voice-typing" {
            "Voice Typing".to_string()
        } else {
            target_scope_id.clone()
        };
        let updated = match self.vocabulary.edit_line {
            Some(line) => {
                let old_scope = parse_dictionary_entries(&current)
                    .into_iter()
                    .find(|entry| entry.line == line)
                    .map(|entry| entry.scope_id)
                    .unwrap_or_else(|| "global".to_string());
                if old_scope == target_scope_id {
                    update_dictionary_line(&current, line, value)
                } else {
                    let without_old = remove_dictionary_line(&current, line);
                    add_dictionary_entry_to_scope(
                        &without_old,
                        value,
                        &target_scope_id,
                        &target_scope_name,
                    )
                }
            }
            None => {
                add_dictionary_entry_to_scope(&current, value, &target_scope_id, &target_scope_name)
            }
        };
        let (hotwords, replacements) = legacy_dictionary_sets(&updated);
        self.vocabulary
            .dictionary_textarea
            .update(cx, |state, cx| state.set_value(updated.clone(), window, cx));
        self.persist_model_settings(
            vec![
                ("dictionaryContent".into(), Value::String(updated)),
                ("hotwordSets".into(), hotwords),
                ("textReplacementSets".into(), replacements),
            ],
            cx,
        );
        self.vocabulary
            .dictionary_quick_add
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.vocabulary.edit_line = None;
        self.vocabulary.dialog_open = false;
        cx.notify();
    }

    pub fn submit_vocabulary_dialog_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = if self.vocabulary.dialog_type == "hotword" {
            let word = self
                .vocabulary
                .word_state
                .read(cx)
                .value()
                .trim()
                .to_string();
            if word.is_empty() {
                return;
            }
            let weight = self
                .vocabulary
                .weight_state
                .read(cx)
                .value()
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|weight| *weight > 0.0);
            weight.map_or(word.clone(), |weight| format!("{word} :{weight:.1}"))
        } else {
            let from = self
                .vocabulary
                .from_state
                .read(cx)
                .value()
                .trim()
                .to_string();
            let to = self.vocabulary.to_state.read(cx).value().trim().to_string();
            if from.is_empty() || to.is_empty() {
                return;
            }
            format!("{from} -> {to}")
        };
        self.vocabulary
            .dictionary_quick_add
            .update(cx, |state, cx| state.set_value(value.clone(), window, cx));
        self.submit_vocabulary_entry(&value, window, cx);
        self.vocabulary
            .word_state
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.vocabulary
            .weight_state
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.vocabulary
            .from_state
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.vocabulary
            .to_state
            .update(cx, |state, cx| state.set_value("", window, cx));
    }

    pub fn persist_model_setting(&mut self, key: String, value: Value, cx: &mut Context<Self>) {
        if !self.model_config.is_object() {
            self.model_config = json!({});
        }
        self.model_config[key.as_str()] = value.clone();
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = services
                .config
                .load()
                .ok()
                .flatten()
                .unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            stored[key.as_str()] = value;
            let _ = services.config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    pub fn persist_model_settings(
        &mut self,
        settings: Vec<(String, Value)>,
        cx: &mut Context<Self>,
    ) {
        if !self.model_config.is_object() {
            self.model_config = json!({});
        }
        for (key, value) in &settings {
            self.model_config[key] = value.clone();
        }
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let mut stored = services
                .config
                .load()
                .ok()
                .flatten()
                .unwrap_or_else(|| json!({}));
            if !stored.is_object() {
                stored = json!({});
            }
            for (key, value) in settings {
                stored[key] = value;
            }
            let _ = services.config.save(&stored);
        })
        .detach();
        cx.notify();
    }

    pub fn persist_general_setting(
        &mut self,
        key: &'static str,
        value: Value,
        cx: &mut Context<Self>,
    ) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = services.config.set_setting(key, &value);
        })
        .detach();
    }

    pub fn persist_preference(&mut self, key: &'static str, value: bool, cx: &mut Context<Self>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = services.config.set_setting(key, &Value::Bool(value));
        })
        .detach();
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
}

impl Render for SettingsModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_open {
            return div().into_any_element();
        }

        let tab_groups: &[TabGroup] = &[
            (
                "APPLICATION",
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
            .id("settings-modal-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(hsla(0.0, 0.0, 0.0, 0.5))
            .on_click(cx.listener(|this, _, window, cx| {
                this.close(window, cx);
            }))
            .child(
                div()
                    .id("settings-modal-content")
                    .flex()
                    .w(px(1080.0))
                    .h(px(720.0))
                    .rounded_xl()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_2xl()
                    .overflow_hidden()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        // Left sidebar navigation
                        div()
                            .flex()
                            .flex_col()
                            .w(px(240.0))
                            .h_full()
                            .py_3()
                            .px_2()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .overflow_y_scrollbar()
                            .gap_2()
                            .on_key_down(cx.listener(|modal, event: &KeyDownEvent, window, cx| {
                                if modal.settings_search_query.trim().is_empty() || event.is_held {
                                    return;
                                }
                                let hits = search::search_settings(&modal.settings_search_query);
                                if hits.is_empty() {
                                    return;
                                }
                                match event.keystroke.key.to_ascii_lowercase().as_str() {
                                    "arrowdown" | "down" => {
                                        modal.settings_search_active_index =
                                            (modal.settings_search_active_index + 1) % hits.len();
                                    }
                                    "arrowup" | "up" => {
                                        modal.settings_search_active_index =
                                            (modal.settings_search_active_index + hits.len() - 1)
                                                % hits.len();
                                    }
                                    "enter" => {
                                        let hit = hits[modal
                                            .settings_search_active_index
                                            .min(hits.len() - 1)];
                                        modal.select_search_hit(hit.id, hit.tab, cx);
                                        window.prevent_default();
                                        cx.stop_propagation();
                                        return;
                                    }
                                    _ => return,
                                }
                                window.prevent_default();
                                cx.stop_propagation();
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .px_2()
                                    .child(Input::new(&self.settings_search_state))
                                    .when(!self.settings_search_query.is_empty(), |row| {
                                        row.child(
                                            Button::new("settings-search-clear")
                                                .ghost()
                                                .small()
                                                .icon(LucideIcon::X)
                                                .on_click(cx.listener(|modal, _, window, cx| {
                                                    modal.clear_search(window, cx);
                                                })),
                                        )
                                    }),
                            )
                            .when(self.settings_search_query.trim().is_empty(), |sidebar| {
                                sidebar.children(tab_groups.iter().map(
                                    |(group_name, group_tabs)| {
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
                                            .children(group_tabs.iter().map(
                                                |(tab, label, icon_name)| {
                                                    let is_selected = self.current_tab == *tab;
                                                    let tab_val = *tab;
                                                    div()
                                                        .id(ElementId::from(format!(
                                                            "tab-btn-{:?}",
                                                            tab
                                                        )))
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
                                                        .hover(|style| {
                                                            style.bg(if is_selected {
                                                                cx.theme().background
                                                            } else {
                                                                cx.theme().accent
                                                            })
                                                        })
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.set_tab(tab_val, cx);
                                                            },
                                                        ))
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
                                                },
                                            ))
                                    },
                                ))
                            })
                            .when(!self.settings_search_query.trim().is_empty(), |sidebar| {
                                let hits = search::search_settings(&self.settings_search_query);
                                if hits.is_empty() {
                                    sidebar.child(
                                        div()
                                            .id("settings-search-empty")
                                            .px_3()
                                            .py_2()
                                            .text_size(px(12.0))
                                            .text_color(cx.theme().muted_foreground)
                                            .child("No matching settings"),
                                    )
                                } else {
                                    sidebar.child(
                                        div()
                                            .id("settings-search-results")
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .overflow_y_scrollbar()
                                            .children(hits.into_iter().map(|hit| {
                                                let tab = hit.tab;
                                                let hit_id = hit.id;
                                                let is_active = self.settings_search_active_index
                                                    < search::search_settings(
                                                        &self.settings_search_query,
                                                    )
                                                    .len()
                                                    && search::search_settings(
                                                        &self.settings_search_query,
                                                    )[self.settings_search_active_index]
                                                        .id
                                                        == hit.id;
                                                Button::new(ElementId::from(format!(
                                                    "settings-search-{}",
                                                    hit.id
                                                )))
                                                .when(is_active, |button| button.secondary())
                                                .when(!is_active, |button| button.ghost())
                                                .w_full()
                                                .justify_start()
                                                .label(hit.title)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.select_search_hit(hit_id, tab, cx);
                                                }))
                                                .into_any_element()
                                            })),
                                    )
                                }
                            }),
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
                                            .text_size(px(20.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .child(format!(
                                                "{} Settings",
                                                Self::tab_title(self.current_tab)
                                            )),
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
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.close(window, cx);
                                            })),
                                    ),
                            )
                            .child(match self.current_tab {
                                SettingsTab::General => {
                                    self.general.render(window, cx).into_any_element()
                                }
                                SettingsTab::Dashboard => {
                                    self.dashboard.render(cx).into_any_element()
                                }
                                SettingsTab::Shortcuts => {
                                    self.shortcuts.render(window, cx).into_any_element()
                                }
                                SettingsTab::Microphone => {
                                    self.microphone.render(cx).into_any_element()
                                }
                                SettingsTab::Models => self
                                    .models
                                    .render(&self.model_config, cx)
                                    .into_any_element(),
                                SettingsTab::Subtitle => {
                                    let is_rec = self.shortcuts.is_recording_shortcut;
                                    let target = self.shortcuts.shortcut_recording_target;
                                    self.subtitles.render(is_rec, target, cx).into_any_element()
                                }
                                SettingsTab::Vocabulary => self
                                    .vocabulary
                                    .render(&self.model_config, cx)
                                    .into_any_element(),
                                SettingsTab::LlmService => self
                                    .llm_service
                                    .render(&self.model_config, window, cx)
                                    .into_any_element(),
                                SettingsTab::Automation => {
                                    self.automation.render(cx).into_any_element()
                                }
                                SettingsTab::Storage => self.storage.render(cx).into_any_element(),
                                SettingsTab::Sync => self.sync.render(true, cx).into_any_element(),
                                SettingsTab::About => self.about.render(cx).into_any_element(),
                                SettingsTab::ApiServer => {
                                    self.api_server.render(cx).into_any_element()
                                }
                                SettingsTab::Mcp => self.mcp.render(cx).into_any_element(),
                            }),
                    ),
            )
            .into_any_element()
    }
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
