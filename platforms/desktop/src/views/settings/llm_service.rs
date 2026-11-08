use std::collections::{HashMap, HashSet};

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, StatefulInteractiveElement as _, *};
use serde_json::{Value, json};

use super::SettingsModal;
use super::types::OnlineProviderTestState;

pub struct LlmServiceSettingsTab {
    pub api_host_state: Entity<InputState>,
    pub api_key_state: Entity<InputState>,
    pub model_draft_state: Entity<InputState>,
    pub custom_provider_name_state: Entity<InputState>,
    pub model_metadata_field_states: HashMap<String, Entity<InputState>>,
    pub active_feature: String,
    pub model_draft: String,
    pub custom_provider_name: String,
    pub custom_provider_strategy: String,
    pub edit_provider_strategy: String,
    pub show_custom_provider_form: bool,
    pub editing_provider: Option<String>,
    pub pending_delete_provider: Option<String>,
    pub discovery_provider: Option<String>,
    pub discovery_loading: bool,
    pub discovery_error: Option<String>,
    pub discovered_models: Vec<sona_core::llm::provider_protocol::LlmModelSummary>,
    pub discovery_generation: u64,
    pub provider_test_states: HashMap<String, OnlineProviderTestState>,
    pub editing_model: Option<String>,
    pub model_metadata_draft: Value,
    pub model_metadata_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl LlmServiceSettingsTab {
    pub fn new(window: &mut Window, cx: &mut Context<SettingsModal>) -> Self {
        let api_host_state = cx.new(|cx| InputState::new(window, cx).placeholder("API Host"));
        let api_key_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("API Key")
                .masked(true)
        });
        let model_draft_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Model name (e.g. gpt-4o)"));
        let custom_provider_name_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Provider name"));

        let mut model_metadata_field_states = HashMap::new();
        for field in [
            "displayName",
            "contextWindow",
            "maxOutputTokens",
            "inputPrice",
            "outputPrice",
            "cacheReadPrice",
            "cacheWritePrice",
            "knowledgeCutoff",
            "releaseDate",
            "lastUpdated",
        ] {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(field));
            model_metadata_field_states.insert(field.to_string(), state);
        }

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &api_host_state,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let val = state.read(cx).value().to_string();
                    modal.set_llm_provider_field("apiHost", val, cx);
                }
            },
        ));

        subscriptions.push(
            cx.subscribe(&api_key_state, |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let val = state.read(cx).value().to_string();
                    modal.set_llm_provider_field("apiKey", val, cx);
                }
            }),
        );

        subscriptions.push(cx.subscribe(
            &model_draft_state,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    modal.llm_service.model_draft = state.read(cx).value().to_string();
                }
            },
        ));

        subscriptions.push(cx.subscribe(
            &custom_provider_name_state,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    modal.llm_service.custom_provider_name = state.read(cx).value().to_string();
                }
            },
        ));

        Self {
            api_host_state,
            api_key_state,
            model_draft_state,
            custom_provider_name_state,
            model_metadata_field_states,
            active_feature: "polish".to_string(),
            model_draft: String::new(),
            custom_provider_name: String::new(),
            custom_provider_strategy: "openai_compatible".to_string(),
            edit_provider_strategy: "openai_compatible".to_string(),
            show_custom_provider_form: false,
            editing_provider: None,
            pending_delete_provider: None,
            discovery_provider: None,
            discovery_loading: false,
            discovery_error: None,
            discovered_models: Vec::new(),
            discovery_generation: 0,
            provider_test_states: HashMap::new(),
            editing_model: None,
            model_metadata_draft: json!({}),
            model_metadata_error: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn ensure_llm_settings(&mut self, config: &mut Value) {
        if !config.is_object() {
            *config = json!({});
        }
        if !config.get("llmSettings").is_some_and(Value::is_object) {
            config["llmSettings"] = json!({
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

    pub fn set_llm_provider_field(
        &mut self,
        field: &'static str,
        value: String,
        modal: &mut SettingsModal,
        cx: &mut Context<SettingsModal>,
    ) {
        self.ensure_llm_settings(&mut modal.model_config);
        let provider = modal
            .model_config
            .pointer("/llmSettings/activeProvider")
            .and_then(Value::as_str)
            .unwrap_or("google_translate_free")
            .to_string();
        if !modal.model_config["llmSettings"]["providers"].is_object() {
            modal.model_config["llmSettings"]["providers"] = json!({});
        }
        if !modal.model_config["llmSettings"]["providers"][&provider].is_object() {
            modal.model_config["llmSettings"]["providers"][&provider] = json!({});
        }
        modal.model_config["llmSettings"]["providers"][&provider][field] = Value::String(value);
        let val = modal.model_config["llmSettings"].clone();
        modal.persist_model_setting("llmSettings".to_string(), val, cx);
    }

    pub fn set_llm_active_provider(
        &mut self,
        provider: String,
        modal: &mut SettingsModal,
        cx: &mut Context<SettingsModal>,
    ) {
        self.ensure_llm_settings(&mut modal.model_config);
        if !modal.model_config["llmSettings"]["providers"].is_object() {
            modal.model_config["llmSettings"]["providers"] = json!({});
        }
        if !modal.model_config["llmSettings"]["providers"][&provider].is_object() {
            modal.model_config["llmSettings"]["providers"][&provider] = json!({
                "apiHost": llm_provider_default_host(&provider),
                "apiKey": ""
            });
        }
        modal.model_config["llmSettings"]["activeProvider"] = Value::String(provider);
        let val = modal.model_config["llmSettings"].clone();
        modal.persist_model_setting("llmSettings".to_string(), val, cx);
    }

    pub fn active_llm_provider_values(&self, config: &Value) -> (String, String) {
        let provider = config
            .pointer("/llmSettings/activeProvider")
            .and_then(Value::as_str)
            .unwrap_or("google_translate_free");
        let setting = config.pointer(&format!("/llmSettings/providers/{provider}"));
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

    pub fn select_llm_model(
        &mut self,
        model_id: String,
        modal: &mut SettingsModal,
        cx: &mut Context<SettingsModal>,
    ) {
        self.ensure_llm_settings(&mut modal.model_config);
        let selection_key = format!("{}ModelId", self.active_feature);
        modal.model_config["llmSettings"]["selections"][selection_key] = Value::String(model_id);
        let val = modal.model_config["llmSettings"].clone();
        modal.persist_model_setting("llmSettings".to_string(), val, cx);
    }

    pub fn remove_llm_model(
        &mut self,
        model_id: &str,
        modal: &mut SettingsModal,
        cx: &mut Context<SettingsModal>,
    ) {
        if remove_llm_model_settings(&mut modal.model_config["llmSettings"], model_id) {
            if self.editing_model.as_deref() == Some(model_id) {
                self.editing_model = None;
            }
            let val = modal.model_config["llmSettings"].clone();
            modal.persist_model_setting("llmSettings".to_string(), val, cx);
        }
    }

    pub fn begin_llm_model_metadata_edit(&mut self, model_id: String, config: &Value) {
        self.model_metadata_draft = config
            .pointer(&format!("/llmSettings/models/{model_id}/metadata"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        self.model_metadata_error = None;
        self.editing_model = Some(model_id);
    }

    pub fn set_llm_model_metadata_field(&mut self, key: &str, value: String) {
        self.model_metadata_draft[key] = Value::String(value);
        self.model_metadata_error = None;
    }

    pub fn set_llm_model_metadata_flag(&mut self, key: &str, value: bool) {
        self.model_metadata_draft[key] = Value::Bool(value);
        self.model_metadata_error = None;
    }

    pub fn set_llm_model_metadata_modality(&mut self, field: &str, modality: &str, checked: bool) {
        if !matches!(field, "inputModalities" | "outputModalities")
            || !matches!(modality, "text" | "image" | "audio" | "video" | "pdf")
        {
            return;
        }
        if !self.model_metadata_draft[field].is_array() {
            self.model_metadata_draft[field] = json!([]);
        }
        let modalities = self.model_metadata_draft[field]
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

    pub fn save_llm_model_metadata(
        &mut self,
        metadata: Value,
        modal: &mut SettingsModal,
        cx: &mut Context<SettingsModal>,
    ) {
        let Some(model_id) = self.editing_model.take() else {
            return;
        };
        let Some(model) = modal
            .model_config
            .pointer_mut(&format!("/llmSettings/models/{model_id}"))
        else {
            return;
        };
        model["metadata"] = metadata;
        self.model_metadata_error = None;
        let val = modal.model_config["llmSettings"].clone();
        modal.persist_model_setting("llmSettings".to_string(), val, cx);
    }

    pub fn set_llm_feature_option(
        &mut self,
        feature: &str,
        option: &str,
        value: Value,
        modal: &mut SettingsModal,
        cx: &mut Context<SettingsModal>,
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
        self.ensure_llm_settings(&mut modal.model_config);
        modal.model_config["llmSettings"]["selections"][key] = value;
        let val = modal.model_config["llmSettings"].clone();
        modal.persist_model_setting("llmSettings".to_string(), val, cx);
    }

    pub fn render(
        &mut self,
        config: &Value,
        _window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> impl IntoElement {
        let active_feature = self.active_feature.clone();
        let _model_draft = self.model_draft.clone();
        let _custom_provider_name = self.custom_provider_name.clone();
        let _custom_provider_strategy = self.custom_provider_strategy.clone();
        let _edit_provider_strategy = self.edit_provider_strategy.clone();
        let _show_custom_provider_form = self.show_custom_provider_form;
        let editing_provider = self.editing_provider.clone();
        let _discovery_provider = self.discovery_provider.clone();
        let _discovery_loading = self.discovery_loading;
        let _discovery_error = self.discovery_error.clone();
        let _discovered_models = self.discovered_models.clone();
        let _editing_model = self.editing_model.clone();
        let _metadata_draft = self.model_metadata_draft.clone();
        let _metadata_error = self.model_metadata_error.clone();

        let llm = config.get("llmSettings").unwrap_or(&Value::Null);
        let active_provider = llm
            .get("activeProvider")
            .and_then(Value::as_str)
            .unwrap_or("google_translate_free")
            .to_string();
        let custom_providers = llm.get("customProviders");
        let _active_custom_provider = custom_providers
            .and_then(|providers| providers.get(&active_provider))
            .is_some();
        let _editing_active_provider =
            editing_provider.as_deref() == Some(active_provider.as_str());
        let _models = llm.get("models").and_then(Value::as_object);
        let _model_order = llm
            .get("modelOrder")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let _selected_model = llm
            .pointer(&format!("/selections/{active_feature}ModelId"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let provider_ids = {
            let mut ids = sona_core::llm::providers::llm_providers()
                .iter()
                .map(|p| p.id.clone())
                .collect::<Vec<_>>();
            ids.extend([
                "google_translate_free".to_string(),
                "google_translate".to_string(),
            ]);
            if let Some(custom) = custom_providers.and_then(Value::as_object) {
                ids.extend(custom.keys().cloned());
            }
            let mut seen = HashSet::new();
            ids.retain(|p| seen.insert(p.clone()));
            ids
        };

        div()
            .id("settings-llm-service-panel")
            .flex().flex_col().gap(px(24.0)).size_full().min_h_0().overflow_y_scroll().px(px(32.0)).py(px(24.0))
            .child(
                div().w_full().max_w(px(900.0)).mx_auto().flex().flex_col().gap_6()
                    .child(
                        div().flex().flex_col().gap_1().pb_4().border_b_1().border_color(cx.theme().border)
                            .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("LLM Service"))
                            .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Configure AI text enhancement, translation, and summary models.")),
                    )
                    .child(
                        div().flex().gap_2().children([
                            ("polish", "Polish", "Improve wording and readability"),
                            ("translation", "Translation", "Translate transcript text"),
                            ("summary", "Summary", "Create concise transcript summaries"),
                        ].into_iter().map(|(id, label, desc)| {
                            let selected = active_feature == id;
                            div().id(ElementId::Name(format!("llm-feature-{id}").into())).flex_1().min_w_0().flex().flex_col().gap_1().p_3().rounded_md().border_1()
                                .border_color(if selected { cx.theme().accent } else { cx.theme().border })
                                .bg(if selected { cx.theme().accent.opacity(0.10) } else { cx.theme().group_box })
                                .cursor_pointer().hover(|s| s.border_color(cx.theme().accent))
                                .on_click(cx.listener(move |modal, _, _, cx| {
                                    modal.llm_service.active_feature = id.to_string();
                                    cx.notify();
                                }))
                                .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child(label))
                                .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child(desc))
                        })),
                    )
                    .child(
                        div().flex().flex_col().gap_3().p_4().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                            .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Provider Selection"))
                            .child(
                                div().flex().flex_wrap().gap_2().children(provider_ids.iter().map(|provider| {
                                    let provider_id = provider.clone();
                                    let is_active = active_provider == *provider;
                                    let label = provider_label(provider, custom_providers);
                                    Button::new(ElementId::Name(format!("llm-provider-{provider}").into()))
                                        .when(is_active, |b| b.primary())
                                        .when(!is_active, |b| b.outline())
                                        .small().label(label)
                                        .on_click(cx.listener(move |modal, _, window, cx| {
                                            modal.set_llm_active_provider(provider_id.clone(), cx);
                                            let (host, key) = modal.llm_service.active_llm_provider_values(&modal.model_config);
                                            modal.llm_service.api_host_state.update(cx, |s, cx| s.set_value(host, window, cx));
                                            modal.llm_service.api_key_state.update(cx, |s, cx| s.set_value(key, window, cx));
                                            cx.notify();
                                        }))
                                })),
                            )
                            .child(
                                div().flex().flex_col().gap_3().pt_3().border_t_1().border_color(cx.theme().border)
                                    .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("API Credentials"))
                                    .child(Input::new(&self.api_host_state))
                                    .child(Input::new(&self.api_key_state)),
                            ),
                    )
            )
            .into_any_element()
    }
}

pub fn llm_slug(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn remove_custom_provider_settings(settings: &mut Value, provider_id: &str) -> bool {
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
                .map(|(id, _)| id.clone())
                .collect::<HashSet<_>>()
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

pub fn remove_llm_model_settings(settings: &mut Value, model_id: &str) -> bool {
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

pub fn apply_llm_model_metadata_inputs(
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
            .filter(|n| n.is_finite() && *n >= 0.0)
            .and_then(serde_json::Number::from_f64)
            .ok_or_else(|| format!("{key} must be a nonnegative number."))?;
        updated[key] = Value::Number(number);
    }
    Ok(updated)
}

fn provider_label(provider: &str, custom_providers: Option<&Value>) -> String {
    if let Some(name) = custom_providers
        .and_then(|providers| providers.get(provider))
        .and_then(|provider| provider.get("name"))
        .and_then(Value::as_str)
    {
        return name.to_string();
    }
    provider
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn llm_provider_default_host(provider: &str) -> &'static str {
    match provider {
        "open_ai" => "https://api.openai.com",
        "anthropic" => "https://api.anthropic.com",
        "deepseek" => "https://api.deepseek.com",
        "gemini" => "https://generativelanguage.googleapis.com",
        "groq" => "https://api.groq.com/openai",
        "mistral" => "https://api.mistral.ai",
        "xai" => "https://api.x.ai",
        "together" => "https://api.together.xyz",
        "fireworks" => "https://api.fireworks.ai/inference",
        "siliconflow" => "https://api.siliconflow.cn",
        "volcengine" => "https://ark.cn-beijing.volces.com/api/v3",
        "bailian" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "tencent_hunyuan" => "https://api.hunyuan.cloud.tencent.com",
        "zhipu" => "https://open.bigmodel.cn/api/paas/v4",
        "lingyi" => "https://api.lingyiwanwu.com",
        "baichuan" => "https://api.baichuan-ai.com",
        "minimax" => "https://api.minimax.chat",
        "moonshot" => "https://api.moonshot.cn",
        "openrouter" => "https://openrouter.ai/api",
        "ollama" => "http://localhost:11434",
        "lm_studio" => "http://localhost:1234",
        _ => "https://api.openai.com",
    }
}
