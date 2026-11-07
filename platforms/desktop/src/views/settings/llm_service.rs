use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::{MainShell, OnlineProviderTestState, llm_slug};

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

fn metadata_number(metadata: &Value, key: &str) -> String {
    metadata
        .get(key)
        .and_then(Value::as_number)
        .map(ToString::to_string)
        .unwrap_or_else(|| "—".to_string())
}

fn metadata_list(metadata: &Value, key: &str) -> Option<String> {
    let values = metadata.get(key)?.as_array()?;
    let values = values.iter().filter_map(Value::as_str).collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.join(", "))
}

fn section(
    title: &str,
    description: &str,
    content: AnyElement,
    cx: &Context<MainShell>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted)
                        .child(description.to_string()),
                ),
        )
        .child(content)
        .into_any_element()
}

pub(super) fn llm_service_panel(
    config: &Value,
    active_feature: &str,
    model_draft: &str,
    custom_provider_name: &str,
    custom_provider_strategy: &str,
    edit_provider_strategy: &str,
    show_custom_provider_form: bool,
    editing_provider: Option<&str>,
    discovery_provider: Option<&str>,
    discovery_loading: bool,
    discovery_error: Option<&str>,
    discovered_models: &[sona_core::llm::provider_protocol::LlmModelSummary],
    editing_model: Option<&str>,
    metadata_draft: &Value,
    metadata_error: Option<&str>,
    metadata_field_states: &std::collections::HashMap<String, Entity<InputState>>,
    provider_test_state: Option<&OnlineProviderTestState>,
    model_test_states: &std::collections::HashMap<String, OnlineProviderTestState>,
    api_host_state: &Entity<InputState>,
    api_key_state: &Entity<InputState>,
    model_draft_state: &Entity<InputState>,
    custom_provider_name_state: &Entity<InputState>,
    _window: &Window,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let active_feature: &'static str = match active_feature {
        "translation" => "translation",
        "summary" => "summary",
        _ => "polish",
    };
    let llm = config.get("llmSettings").unwrap_or(&Value::Null);
    let active_provider = llm
        .get("activeProvider")
        .and_then(Value::as_str)
        .unwrap_or("google_translate_free")
        .to_string();
    let custom_providers = llm.get("customProviders");
    let active_custom_provider = custom_providers
        .and_then(|providers| providers.get(&active_provider))
        .is_some();
    let editing_active_provider = editing_provider == Some(active_provider.as_str());
    let models = llm.get("models").and_then(Value::as_object);
    let model_order = llm
        .get("modelOrder")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selected_model = llm
        .pointer(&format!("/selections/{active_feature}ModelId"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let selected_entry = models.and_then(|models| models.get(&selected_model));
    let selected_provider = selected_entry
        .and_then(|entry| entry.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or(&active_provider);
    let selections = llm.get("selections").unwrap_or(&Value::Null);
    let temperature_key = format!("{active_feature}Temperature");
    let reasoning_enabled_key = format!("{active_feature}ReasoningEnabled");
    let reasoning_level_key = format!("{active_feature}ReasoningLevel");
    let reasoning_budget_key = format!("{active_feature}ReasoningBudget");
    let temperature = selections
        .get(&temperature_key)
        .and_then(Value::as_f64)
        .unwrap_or(0.7)
        .clamp(0.0, 2.0);
    let reasoning_enabled = selections
        .get(&reasoning_enabled_key)
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let reasoning_level = selections
        .get(&reasoning_level_key)
        .and_then(Value::as_str)
        .unwrap_or("medium");
    let metadata_reasoning_mode = selected_entry
        .and_then(|entry| entry.pointer("/metadata/reasoningMode/type"))
        .and_then(Value::as_str);
    let normalized_model = selected_entry
        .and_then(|entry| entry.get("model"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let model_core = normalized_model
        .rsplit('/')
        .next()
        .unwrap_or(&normalized_model)
        .replace('.', "-");
    let supports_reasoning = selected_entry
        .and_then(|entry| entry.pointer("/metadata/supportsReasoning"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            matches!(
                metadata_reasoning_mode,
                Some("effort" | "budget" | "hybrid")
            ) || [
                "o1",
                "o3",
                "o4",
                "gpt-5",
                "gpt-6",
                "deepseek-reasoner",
                "deepseek-r1",
                "claude-3-7",
                "claude-opus-5",
                "claude-5",
                "claude-sonnet-5",
                "claude-fable-5",
                "claude-sonnet-4",
                "claude-opus-4",
                "claude-4",
                "gemini-2-5",
                "gemini-3",
                "gemma-4",
                "qwq",
                "qwen3",
                "qwen-3",
                "thinking",
                "reasoner",
            ]
            .iter()
            .any(|pattern| model_core.contains(pattern))
        });
    let reasoning_mode = metadata_reasoning_mode.or(if supports_reasoning {
        Some("effort")
    } else {
        None
    });
    let supports_temperature = selected_entry
        .and_then(|entry| entry.pointer("/metadata/supportsTemperature"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            !normalized_model.trim().is_empty()
                && !matches!(
                    selected_provider,
                    "google_translate" | "google_translate_free"
                )
                && !(selected_provider == "local"
                    && active_feature == "translation"
                    && (normalized_model == "auto" || normalized_model.starts_with("opus-mt-")))
                && ![
                    "o1",
                    "o3",
                    "o4",
                    "gpt-5",
                    "gpt-6",
                    "deepseek-reasoner",
                    "deepseek-r1",
                    "claude-opus-5",
                    "claude-5",
                    "claude-sonnet-5",
                    "claude-fable-5",
                    "claude-sonnet-4-6",
                    "claude-opus-4-6",
                    "claude-3-7",
                    "claude-4-5",
                    "qwq",
                    "reasoner",
                ]
                .iter()
                .any(|pattern| model_core.contains(pattern))
        });
    let feature_rows = [
        ("polish", "Polish", "Improve wording and readability"),
        ("translation", "Translation", "Translate transcript text"),
        ("summary", "Summary", "Create concise transcript summaries"),
    ];

    let mut provider_ids = sona_core::llm::providers::llm_providers()
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    provider_ids.extend([
        "google_translate_free".to_string(),
        "google_translate".to_string(),
    ]);
    if let Some(custom) = custom_providers.and_then(Value::as_object) {
        provider_ids.extend(custom.keys().cloned());
    }
    let mut seen = std::collections::HashSet::new();
    provider_ids.retain(|provider| seen.insert(provider.clone()));

    let feature_tabs = feature_rows
        .iter()
        .map(|(id, label, description)| {
            let is_selected = active_feature == *id;
            let tab = (*id).to_string();
            div()
                .id(ElementId::Name(format!("llm-feature-{id}").into()))
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(if is_selected {
                    cx.theme().accent
                } else {
                    cx.theme().group_box
                })
                .cursor_pointer()
                .hover(|style| style.bg(cx.theme().accent))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.settings_modal.set_llm_active_feature(&tab);
                    cx.notify();
                }))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child((*label).to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted)
                        .child((*description).to_string()),
                )
                .into_any_element()
        })
        .collect::<Vec<_>>();

    let provider_buttons = provider_ids.iter().map(|provider| {
        let provider_id = provider.clone();
        let label = provider_label(provider, custom_providers);
        let is_active = active_provider == *provider;
        let host_state = api_host_state.clone();
        let key_state = api_key_state.clone();
        div().child(
            Button::new(ElementId::Name(format!("llm-provider-{provider}").into()))
                .when(is_active, |button| button.primary())
                .when(!is_active, |button| button.outline())
                .small()
                .label(label)
                .on_click(cx.listener(move |shell, _, window, cx| {
                    shell
                        .settings_modal
                        .set_llm_active_provider(provider_id.clone(), cx);
                    let (host, key) = shell.settings_modal.active_llm_provider_values();
                    host_state.update(cx, |state, cx| state.set_value(host, window, cx));
                    key_state.update(cx, |state, cx| state.set_value(key, window, cx));
                    cx.notify();
                })),
        )
    });

    let model_rows = model_order
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|model_id| {
            models.and_then(|models| models.get(model_id).map(|model| (model_id, model)))
        })
        .collect::<Vec<_>>();
    let model_rows = if model_rows.is_empty() {
        models
            .map(|models| {
                models
                    .iter()
                    .map(|(id, model)| (id.as_str(), model))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        model_rows
    };
    let model_list = if model_rows.is_empty() {
        div()
            .p_3()
            .rounded_md()
            .bg(cx.theme().input)
            .text_color(cx.theme().muted)
            .child(
                "No models have been added yet. Add a model ID below to assign it to this feature.",
            )
            .into_any_element()
    } else {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(model_rows.into_iter().map(|(model_id, model)| {
                let model_id_owned = model_id.to_string();
                let provider = model
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let model_name = model
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or(model_id);
                let provider_name = provider_label(provider, custom_providers);
                let metadata = model.get("metadata").unwrap_or(&Value::Null);
                let display_name = metadata
                    .get("displayName")
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty());
                let capability_labels = [
                    ("supportsMultimodal", "Multimodal"),
                    ("supportsTools", "Tools"),
                    ("supportsReasoning", "Reasoning"),
                    ("supportsStructuredOutput", "Structured output"),
                    ("supportsPromptCaching", "Prompt caching"),
                ]
                .into_iter()
                .filter_map(|(key, label)| {
                    metadata
                        .get(key)
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                        .then_some(label)
                })
                .collect::<Vec<_>>();
                let mut metadata_lines = vec![
                    format!(
                        "Context window: {}",
                        metadata_number(metadata, "contextWindow")
                    ),
                    format!(
                        "Max output tokens: {}",
                        metadata_number(metadata, "maxOutputTokens")
                    ),
                    format!("Input price: {}", metadata_number(metadata, "inputPrice")),
                    format!("Output price: {}", metadata_number(metadata, "outputPrice")),
                    format!(
                        "Cache read price: {}",
                        metadata_number(metadata, "cacheReadPrice")
                    ),
                    format!(
                        "Cache write price: {}",
                        metadata_number(metadata, "cacheWritePrice")
                    ),
                ];
                for (key, label) in [
                    ("knowledgeCutoff", "Knowledge cutoff"),
                    ("releaseDate", "Release date"),
                    ("lastUpdated", "Last updated"),
                ] {
                    if let Some(value) = metadata.get(key).and_then(Value::as_str) {
                        if !value.is_empty() {
                            metadata_lines.push(format!("{label}: {value}"));
                        }
                    }
                }
                for (key, label) in [
                    ("inputModalities", "Input modalities"),
                    ("outputModalities", "Output modalities"),
                    ("metadataSources", "Metadata sources"),
                ] {
                    if let Some(value) = metadata_list(metadata, key) {
                        metadata_lines.push(format!("{label}: {value}"));
                    }
                }
                let selected = selected_model == model_id;
                let remove_model_id = model_id.to_string();
                let edit_model_id = model_id.to_string();
                let is_editing_model = editing_model == Some(model_id);
                let test_key = format!("{provider}::{model_name}");
                let (test_label, test_disabled, test_latency, test_error) =
                    match model_test_states.get(&test_key) {
                        Some(OnlineProviderTestState::Loading) => ("Testing…", true, None, None),
                        Some(OnlineProviderTestState::Success { latency_ms }) => {
                            ("Connection successful", false, Some(*latency_ms), None)
                        }
                        Some(OnlineProviderTestState::Error { message }) => {
                            ("Test connection again", false, None, Some(message.clone()))
                        }
                        None => ("Test connection", false, None, None),
                    };
                let test_provider_id = provider.to_string();
                let test_model_name = model_name.to_string();
                let metadata_states = metadata_field_states.clone();
                let metadata_states_for_save = metadata_field_states.clone();
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(if selected {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .bg(if selected {
                        cx.theme().accent
                    } else {
                        cx.theme().group_box
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(display_name.unwrap_or(model_name).to_string()),
                            )
                            .when(display_name.is_some(), |heading| {
                                heading.child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(cx.theme().muted)
                                        .child(model_name.to_string()),
                                )
                            })
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(cx.theme().muted)
                                    .child(provider_name),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new(ElementId::Name(
                                    format!("llm-select-model-{model_id}").into(),
                                ))
                                .when(selected, |button| button.primary())
                                .when(!selected, |button| button.outline())
                                .small()
                                .label(if selected {
                                    "Selected"
                                } else {
                                    "Use for this feature"
                                })
                                .on_click(cx.listener(
                                    move |shell, _, _, cx| {
                                        shell
                                            .settings_modal
                                            .select_llm_model(model_id_owned.clone(), cx);
                                    },
                                )),
                            )
                            .child(
                                Button::new(ElementId::Name(
                                    format!("llm-edit-model-{model_id}").into(),
                                ))
                                .ghost()
                                .small()
                                .label(if is_editing_model {
                                    "Cancel edit"
                                } else {
                                    "Edit metadata"
                                })
                                .on_click(cx.listener(
                                    move |shell, _, window, cx| {
                                        if shell.settings_modal.llm_editing_model.as_deref()
                                            == Some(edit_model_id.as_str())
                                        {
                                            shell.settings_modal.llm_editing_model = None;
                                        } else {
                                            shell.settings_modal.begin_llm_model_metadata_edit(
                                                edit_model_id.clone(),
                                            );
                                            let metadata = shell
                                                .settings_modal
                                                .llm_model_metadata_draft
                                                .clone();
                                            for (key, state) in &metadata_states {
                                                let value = metadata.get(key).map_or_else(
                                                    String::new,
                                                    |value| {
                                                        value
                                                            .as_str()
                                                            .map(str::to_string)
                                                            .unwrap_or_else(|| {
                                                                if value.is_number() {
                                                                    value.to_string()
                                                                } else {
                                                                    String::new()
                                                                }
                                                            })
                                                    },
                                                );
                                                state.update(cx, |state, cx| {
                                                    state.set_value(value, window, cx)
                                                });
                                            }
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                Button::new(ElementId::Name(
                                    format!("llm-test-model-{model_id}").into(),
                                ))
                                .outline()
                                .small()
                                .disabled(test_disabled)
                                .label(test_label)
                                .on_click(cx.listener(
                                    move |shell, _, _, cx| {
                                        shell.settings_modal.test_llm_model(
                                            test_provider_id.clone(),
                                            Some(test_model_name.clone()),
                                            cx,
                                        );
                                    },
                                )),
                            ),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
                        capability_labels.into_iter().map(|label| {
                            div()
                                .px_2()
                                .py(px(2.0))
                                .rounded_sm()
                                .bg(cx.theme().accent)
                                .text_size(px(10.0))
                                .child(label.to_string())
                        }),
                    ))
                    .child(div().flex().flex_wrap().gap_x_3().gap_y_1().children(
                        metadata_lines.into_iter().map(|line| {
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted)
                                .child(line)
                        }),
                    ))
                    .when_some(test_latency, |card, latency| {
                        card.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().success)
                                .child(format!("Connected in {latency} ms")),
                        )
                    })
                    .when_some(test_error, |card, message| {
                        card.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().danger)
                                .child(message),
                        )
                    })
                    .when(is_editing_model, |row| {
                        let fields = [
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
                        row.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_3()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .children(fields.into_iter().filter_map(|(key, label)| {
                                    metadata_field_states.get(key).map(|state| {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(cx.theme().muted)
                                                    .child(label),
                                            )
                                            .child(Input::new(state).small())
                                    })
                                }))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Input modalities"),
                                )
                                .children(
                                    ["text", "image", "audio", "video", "pdf"].into_iter().map(
                                        |modality| {
                                            let checked = metadata_draft
                                                .get("inputModalities")
                                                .and_then(Value::as_array)
                                                .is_some_and(|items| {
                                                    items
                                                        .iter()
                                                        .any(|item| item.as_str() == Some(modality))
                                                });
                                            Checkbox::new(ElementId::Name(
                                                format!("llm-input-{model_id}-{modality}").into(),
                                            ))
                                            .checked(checked)
                                            .label(modality)
                                            .on_click(
                                                cx.listener(move |shell, checked: &bool, _, cx| {
                                                    shell
                                                        .settings_modal
                                                        .set_llm_model_metadata_modality(
                                                            "inputModalities",
                                                            modality,
                                                            *checked,
                                                        );
                                                    cx.notify();
                                                }),
                                            )
                                        },
                                    ),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Output modalities"),
                                )
                                .children(
                                    ["text", "image", "audio", "video", "pdf"].into_iter().map(
                                        |modality| {
                                            let checked = metadata_draft
                                                .get("outputModalities")
                                                .and_then(Value::as_array)
                                                .is_some_and(|items| {
                                                    items
                                                        .iter()
                                                        .any(|item| item.as_str() == Some(modality))
                                                });
                                            Checkbox::new(ElementId::Name(
                                                format!("llm-output-{model_id}-{modality}").into(),
                                            ))
                                            .checked(checked)
                                            .label(modality)
                                            .on_click(
                                                cx.listener(move |shell, checked: &bool, _, cx| {
                                                    shell
                                                        .settings_modal
                                                        .set_llm_model_metadata_modality(
                                                            "outputModalities",
                                                            modality,
                                                            *checked,
                                                        );
                                                    cx.notify();
                                                }),
                                            )
                                        },
                                    ),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Capabilities"),
                                )
                                .children(
                                    [
                                        ("supportsMultimodal", "Multimodal"),
                                        ("supportsTools", "Tools"),
                                        ("supportsReasoning", "Reasoning"),
                                        ("supportsStructuredOutput", "Structured output"),
                                        ("supportsPromptCaching", "Prompt caching"),
                                    ]
                                    .into_iter()
                                    .map(|(key, label)| {
                                        let checked = metadata_draft
                                            .get(key)
                                            .and_then(Value::as_bool)
                                            .unwrap_or(false);
                                        Checkbox::new(ElementId::Name(
                                            format!("llm-capability-{model_id}-{key}").into(),
                                        ))
                                        .checked(checked)
                                        .label(label)
                                        .on_click(
                                            cx.listener(move |shell, checked: &bool, _, cx| {
                                                shell
                                                    .settings_modal
                                                    .set_llm_model_metadata_flag(key, *checked);
                                                cx.notify();
                                            }),
                                        )
                                    }),
                                )
                                .when(metadata_error.is_some(), |editor| {
                                    editor.child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(cx.theme().danger)
                                            .child(metadata_error.unwrap_or_default().to_string()),
                                    )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new(format!("llm-save-metadata-{model_id}"))
                                                .primary()
                                                .small()
                                                .label("Save metadata")
                                                .on_click(cx.listener(move |shell, _, _, cx| {
                                                    let values = metadata_states_for_save
                                                        .iter()
                                                        .map(|(key, state)| {
                                                            (
                                                                key.clone(),
                                                                state.read(cx).value().to_string(),
                                                            )
                                                        })
                                                        .collect::<std::collections::HashMap<_, _>>(
                                                        );
                                                    match super::apply_llm_model_metadata_inputs(
                                                        &shell
                                                            .settings_modal
                                                            .llm_model_metadata_draft,
                                                        &values,
                                                    ) {
                                                        Ok(metadata) => shell
                                                            .settings_modal
                                                            .save_llm_model_metadata(metadata, cx),
                                                        Err(error) => {
                                                            shell
                                                                .settings_modal
                                                                .llm_model_metadata_error =
                                                                Some(error)
                                                        }
                                                    }
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new(format!("llm-cancel-metadata-{model_id}"))
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                                .on_click(cx.listener(|shell, _, _, cx| {
                                                    shell.settings_modal.llm_editing_model = None;
                                                    shell.settings_modal.llm_model_metadata_error =
                                                        None;
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        )
                    })
                    .when(
                        model.get("source").and_then(Value::as_str) == Some("manual"),
                        |row| {
                            row.child(
                                Button::new(ElementId::Name(
                                    format!("llm-remove-model-{model_id}").into(),
                                ))
                                .ghost()
                                .small()
                                .label("Remove")
                                .on_click(cx.listener(
                                    move |shell, _, _, cx| {
                                        shell.settings_modal.remove_llm_model(&remove_model_id, cx);
                                    },
                                )),
                            )
                        },
                    )
            }))
            .into_any_element()
    };

    let mut feature_options = div().flex().flex_col().gap_3();
    if supports_temperature {
        let lower = ((temperature * 10.0 - 1.0).round() / 10.0).max(0.0);
        let upper = ((temperature * 10.0 + 1.0).round() / 10.0).min(2.0);
        feature_options = feature_options.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .p_3()
                .rounded_md()
                .bg(cx.theme().input)
                .child(
                    div().flex().flex_col().gap_1().child("Temperature").child(
                        div()
                            .text_size(px(11.0))
                            .text_color(cx.theme().muted)
                            .child("Controls response randomness (0.0–2.0)."),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("llm-temperature-down")
                                .outline()
                                .small()
                                .label("−")
                                .disabled(lower == temperature)
                                .on_click(cx.listener(move |shell, _, _, cx| {
                                    shell.settings_modal.set_llm_feature_option(
                                        active_feature,
                                        "temperature",
                                        serde_json::json!(lower),
                                        cx,
                                    );
                                })),
                        )
                        .child(format!("{temperature:.1}"))
                        .child(
                            Button::new("llm-temperature-up")
                                .outline()
                                .small()
                                .label("+")
                                .disabled(upper == temperature)
                                .on_click(cx.listener(move |shell, _, _, cx| {
                                    shell.settings_modal.set_llm_feature_option(
                                        active_feature,
                                        "temperature",
                                        serde_json::json!(upper),
                                        cx,
                                    );
                                })),
                        ),
                ),
        );
    }
    if supports_reasoning {
        let mut reasoning = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .bg(cx.theme().input)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child("Reasoning mode")
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(cx.theme().muted)
                                    .child("Enable deeper reasoning for this feature."),
                            ),
                    )
                    .child(
                        Button::new("llm-reasoning-toggle")
                            .when(reasoning_enabled, |button| button.primary())
                            .when(!reasoning_enabled, |button| button.outline())
                            .small()
                            .label(if reasoning_enabled {
                                "Enabled"
                            } else {
                                "Disabled"
                            })
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.settings_modal.set_llm_feature_option(
                                    active_feature,
                                    "reasoningEnabled",
                                    serde_json::json!(!reasoning_enabled),
                                    cx,
                                );
                            })),
                    ),
            );
        if reasoning_enabled {
            if reasoning_mode == Some("budget") {
                let budget = selections
                    .get(&reasoning_budget_key)
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        selected_entry
                            .and_then(|entry| {
                                entry.pointer("/metadata/reasoningMode/defaultBudget")
                            })
                            .and_then(Value::as_u64)
                    })
                    .unwrap_or(8192);
                let lower = budget.saturating_sub(1024).max(1);
                let upper = budget.saturating_add(1024);
                reasoning = reasoning.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child("Thinking token budget")
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("llm-budget-down")
                                        .outline()
                                        .small()
                                        .label("−")
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            shell.settings_modal.set_llm_feature_option(
                                                active_feature,
                                                "reasoningBudget",
                                                serde_json::json!(lower),
                                                cx,
                                            );
                                        })),
                                )
                                .child(format!("{budget}"))
                                .child(
                                    Button::new("llm-budget-up")
                                        .outline()
                                        .small()
                                        .label("+")
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            shell.settings_modal.set_llm_feature_option(
                                                active_feature,
                                                "reasoningBudget",
                                                serde_json::json!(upper),
                                                cx,
                                            );
                                        })),
                                ),
                        ),
                );
            } else if matches!(reasoning_mode, Some("effort" | "hybrid")) {
                let levels = selected_entry
                    .and_then(|entry| entry.pointer("/metadata/supportedThinkingLevels"))
                    .and_then(Value::as_array)
                    .map(|levels| {
                        levels
                            .iter()
                            .filter_map(Value::as_str)
                            .filter(|level| {
                                matches!(
                                    *level,
                                    "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
                                )
                            })
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .filter(|levels| !levels.is_empty())
                    .unwrap_or_else(|| vec!["low".into(), "medium".into(), "high".into()]);
                reasoning =
                    reasoning.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child("Reasoning effort")
                            .child(div().flex().flex_wrap().gap_2().children(
                                levels.into_iter().map(|level| {
                                    let selected = level == reasoning_level;
                                    let option = level.clone();
                                    Button::new(ElementId::Name(
                                        format!("llm-reasoning-level-{level}").into(),
                                    ))
                                    .when(selected, |button| button.primary())
                                    .when(!selected, |button| button.outline())
                                    .small()
                                    .label(level)
                                    .on_click(cx.listener(
                                        move |shell, _, _, cx| {
                                            shell.settings_modal.set_llm_feature_option(
                                                active_feature,
                                                "reasoningLevel",
                                                serde_json::json!(option),
                                                cx,
                                            );
                                        },
                                    ))
                                }),
                            )),
                    );
            } else if reasoning_mode == Some("none") {
                reasoning = reasoning.child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("This model uses built-in reasoning; no additional settings are available."));
            }
        }
        feature_options = feature_options.child(reasoning);
    }

    let timeout = config
        .get("llmRequestTimeoutSeconds")
        .and_then(Value::as_i64)
        .unwrap_or(180);
    let active_host_disabled = active_provider == "google_translate_free";
    let add_model_state = model_draft_state.clone();
    let discovery_provider_matches = discovery_provider == Some(active_provider.as_str());
    let discovery_provider_id = active_provider.clone();
    let test_provider_id = active_provider.clone();
    let _metadata_draft = metadata_draft;
    let discovery_rows = if discovery_provider_matches {
        discovered_models
            .iter()
            .map(|summary| {
                let model_id = summary.model.clone();
                let model_provider = active_provider.clone();
                let already_added = models.is_some_and(|models| {
                    models.values().any(|entry| {
                        entry.get("provider").and_then(Value::as_str)
                            == Some(model_provider.as_str())
                            && entry.get("model").and_then(Value::as_str) == Some(model_id.as_str())
                    })
                });
                let summary_for_add = summary.clone();
                let provider_for_add = active_provider.clone();
                let mut metadata = Vec::new();
                if let Some(context) = summary.context_window {
                    metadata.push(format!("{context} context"));
                }
                if let Some(output) = summary.max_output_tokens {
                    metadata.push(format!("{output} max output"));
                }
                if !summary.input_modalities.is_empty() {
                    metadata.push(format!(
                        "Input: {}",
                        summary
                            .input_modalities
                            .iter()
                            .map(|modality| format!("{modality:?}").to_lowercase())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ));
                }
                if !summary.output_modalities.is_empty() {
                    metadata.push(format!(
                        "Output: {}",
                        summary
                            .output_modalities
                            .iter()
                            .map(|modality| format!("{modality:?}").to_lowercase())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ));
                }
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                summary
                                    .display_name
                                    .clone()
                                    .filter(|name| !name.trim().is_empty())
                                    .unwrap_or_else(|| summary.model.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(cx.theme().muted)
                                    .child(if metadata.is_empty() {
                                        summary.model.clone()
                                    } else {
                                        format!("{} · {}", summary.model, metadata.join(" · "))
                                    }),
                            ),
                    )
                    .child(
                        Button::new(ElementId::Name(
                            format!("llm-discovered-add-{}", llm_slug(&summary.model)).into(),
                        ))
                        .when(already_added, |button| button.outline())
                        .when(!already_added, |button| button.primary())
                        .small()
                        .label(if already_added { "Added" } else { "Add" })
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.settings_modal.add_discovered_llm_model(
                                &provider_for_add,
                                summary_for_add.clone(),
                                cx,
                            );
                        })),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut body = div()
        .id("settings-llm-service-panel")
        .flex()
        .flex_col()
        .gap_5()
        .size_full()
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .pb_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::BOLD)
                        .child("LLM Service"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(cx.theme().muted)
                        .child("Configure providers and models used for polishing, translation, and transcript summaries."),
                ),
        );

    body = body.child(section(
        "Feature Models",
        "Choose a dedicated model for each transcript operation.",
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().gap_2().children(feature_tabs))
            .child(model_list)
            .child(feature_options)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(model_draft_state).small()))
                    .child(
                        Button::new("llm-add-model")
                            .primary()
                            .small()
                            .disabled(model_draft.trim().is_empty())
                            .label("Add model")
                            .on_click(cx.listener(move |shell, _, window, cx| {
                                shell.settings_modal.add_llm_model(cx);
                                add_model_state.update(cx, |state, cx| {
                                    state.set_value(String::new(), window, cx);
                                });
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element(),
        cx,
    ));

    body = body.child(section(
        "Providers",
        "Select a provider and configure its endpoint and credentials.",
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().flex_wrap().gap_2().children(provider_buttons))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().group_box)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(provider_label(&active_provider, custom_providers)),
                    )
                    .when(!active_host_disabled, |card| {
                        card.child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("API host"))
                            .child(Input::new(api_host_state).small())
                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("API key"))
                            .child(Input::new(api_key_state).small())
                    })
                    .when(active_host_disabled, |card| {
                        card.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted)
                                .child("Google Translate Free uses its built-in endpoint and does not require credentials."),
                        )
                    })
                    .when(active_custom_provider && !editing_active_provider, |card| {
                        let provider_id = active_provider.clone();
                        let provider_name = custom_providers
                            .and_then(|providers| providers.get(&active_provider))
                            .and_then(|provider| provider.get("name"))
                            .and_then(Value::as_str)
                            .unwrap_or("Custom Provider")
                            .to_string();
                        let provider_strategy = custom_providers
                            .and_then(|providers| providers.get(&active_provider))
                            .and_then(|provider| provider.get("strategy"))
                            .and_then(Value::as_str)
                            .unwrap_or("openai_compatible")
                            .to_string();
                        let name_state = custom_provider_name_state.clone();
                        let delete_provider_id = provider_id.clone();
                        let delete_provider_name = provider_name.clone();
                        card.child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new("llm-edit-custom-provider")
                                        .outline()
                                        .small()
                                        .label("Edit provider")
                                        .on_click(cx.listener(move |shell, _, window, cx| {
                                            shell.settings_modal.llm_editing_provider =
                                                Some(provider_id.clone());
                                            shell.settings_modal.llm_custom_provider_name =
                                                provider_name.clone();
                                            shell.settings_modal.set_llm_edit_provider_strategy(
                                                &provider_strategy,
                                            );
                                            name_state.update(cx, |state, cx| {
                                                state.set_value(provider_name.clone(), window, cx);
                                            });
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("llm-delete-custom-provider")
                                        .outline()
                                        .small()
                                        .label("Delete provider")
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            shell.settings_modal.llm_pending_delete_provider =
                                                Some(delete_provider_id.clone());
                                            shell.global_dialog.show(
                                                "Delete LLM Provider",
                                                format!("Delete {delete_provider_name} and its saved models?"),
                                                crate::views::dialogs::global_dialog::DialogKind::Confirm,
                                            );
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .when(active_custom_provider && editing_active_provider, |card| {
                        card.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_3()
                                .rounded_md()
                                .bg(cx.theme().input)
                                .child("Provider name")
                                .child(Input::new(custom_provider_name_state).small())
                                .child("API mode")
                                .child(
                                    div().flex().flex_wrap().gap_2().children([
                                        ("openai_compatible", "OpenAI Compatible"),
                                        ("openai_responses", "OpenAI Responses"),
                                        ("anthropic", "Anthropic"),
                                        ("gemini", "Gemini"),
                                    ].into_iter().map(|(strategy, label)| {
                                        Button::new(ElementId::Name(
                                            format!("llm-edit-strategy-{strategy}").into(),
                                        ))
                                        .when(edit_provider_strategy == strategy, |button| button.primary())
                                        .when(edit_provider_strategy != strategy, |button| button.outline())
                                        .small()
                                        .label(label)
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            shell.settings_modal.set_llm_edit_provider_strategy(strategy);
                                            cx.notify();
                                        }))
                                    })),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            Button::new("llm-save-provider-edit")
                                                .primary()
                                                .small()
                                                .disabled(custom_provider_name.trim().is_empty())
                                                .label("Save")
                                                .on_click(cx.listener(|shell, _, window, cx| {
                                                    shell.settings_modal.save_llm_custom_provider_edit(cx);
                                                    shell.llm_custom_provider_name_state.update(cx, |state, cx| {
                                                        state.set_value(String::new(), window, cx);
                                                    });
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("llm-cancel-provider-edit")
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                                .on_click(cx.listener(|shell, _, window, cx| {
                                                    shell.settings_modal.llm_editing_provider = None;
                                                    shell.settings_modal.llm_custom_provider_name.clear();
                                                    shell.llm_custom_provider_name_state.update(cx, |state, cx| {
                                                        state.set_value(String::new(), window, cx);
                                                    });
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Button::new("llm-test-connection")
                            .outline()
                            .small()
                            .disabled(matches!(
                                provider_test_state,
                                Some(OnlineProviderTestState::Loading)
                            ))
                            .label(match provider_test_state {
                                Some(OnlineProviderTestState::Loading) => "Testing connection…",
                                Some(OnlineProviderTestState::Success { .. }) => "Connection successful",
                                Some(OnlineProviderTestState::Error { .. }) => "Test connection again",
                                None => "Test connection",
                            })
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell.settings_modal.test_llm_provider(test_provider_id.clone(), cx);
                            })),
                    )
                    .when(
                        matches!(provider_test_state, Some(OnlineProviderTestState::Success { .. })),
                        |content| {
                            let latency = match provider_test_state {
                                Some(OnlineProviderTestState::Success { latency_ms }) => *latency_ms,
                                _ => 0,
                            };
                            content.child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().success)
                                    .child(format!("Connected in {latency} ms")),
                            )
                        },
                    )
                    .when(
                        matches!(provider_test_state, Some(OnlineProviderTestState::Error { .. })),
                        |content| {
                            let message = match provider_test_state {
                                Some(OnlineProviderTestState::Error { message }) => message.clone(),
                                _ => String::new(),
                            };
                            content.child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().danger)
                                    .child(message),
                            )
                        },
                    )
                    .child(
                        Button::new("llm-discover-models")
                            .outline()
                            .small()
                            .disabled(discovery_loading || active_host_disabled)
                            .label(if discovery_loading { "Discovering models…" } else { "Discover models" })
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                shell
                                    .settings_modal
                                    .discover_llm_models(discovery_provider_id.clone(), cx);
                            })),
                    )
                    .when(discovery_provider_matches && discovery_loading, |content| {
                        content.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted)
                                .child("Loading models from this provider…"),
                        )
                    })
                    .when(discovery_provider_matches && discovery_error.is_some(), |content| {
                        content.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().danger)
                                .child(discovery_error.unwrap_or_default().to_string()),
                        )
                    })
                    .when(!discovery_rows.is_empty(), |content| {
                        content.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .children(discovery_rows),
                        )
                    })
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().input)
                    .child("Request Timeout")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("llm-timeout-down")
                                    .outline()
                                    .small()
                                    .label("−")
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.settings_modal.adjust_llm_timeout(-10, cx);
                                    })),
                            )
                            .child(format!("{timeout} s"))
                            .child(
                                Button::new("llm-timeout-up")
                                    .outline()
                                    .small()
                                    .label("+")
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.settings_modal.adjust_llm_timeout(10, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element(),
        cx,
    ));

    body = body.child(
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().group_box)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("Custom Provider"))
                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("Add an OpenAI-compatible, Responses, Anthropic, or Gemini endpoint.")),
                    )
                    .child(
                        Button::new("llm-toggle-custom-provider")
                            .outline()
                            .small()
                            .label(if show_custom_provider_form { "Cancel" } else { "Add custom provider" })
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.settings_modal.llm_show_custom_provider_form =
                                    !shell.settings_modal.llm_show_custom_provider_form;
                                cx.notify();
                            })),
                    ),
            )
            .when(show_custom_provider_form, |card| {
                card.child(Input::new(custom_provider_name_state).small())
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children([
                                ("openai_compatible", "OpenAI Compatible"),
                                ("openai_responses", "OpenAI Responses"),
                                ("anthropic", "Anthropic"),
                                ("gemini", "Gemini"),
                            ].into_iter().map(|(strategy, label)| {
                                Button::new(ElementId::Name(format!("llm-custom-strategy-{strategy}").into()))
                                    .when(custom_provider_strategy == strategy, |button| button.primary())
                                    .when(custom_provider_strategy != strategy, |button| button.outline())
                                    .small()
                                    .label(label)
                                    .on_click(cx.listener(move |shell, _, _, cx| {
                                        shell.settings_modal.set_llm_custom_provider_strategy(strategy);
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(
                        Button::new("llm-create-custom-provider")
                            .primary()
                            .small()
                            .label("Create provider")
                            .disabled(custom_provider_name.trim().is_empty())
                            .on_click(cx.listener(|shell, _, window, cx| {
                                shell.settings_modal.add_llm_custom_provider(cx);
                                shell.llm_custom_provider_name_state.update(cx, |state, cx| {
                                    state.set_value(String::new(), window, cx);
                                });
                                let (host, key) = shell.settings_modal.active_llm_provider_values();
                                shell.llm_provider_api_host_state.update(cx, |state, cx| {
                                    state.set_value(host, window, cx);
                                });
                                shell.llm_provider_api_key_state.update(cx, |state, cx| {
                                    state.set_value(key, window, cx);
                                });
                                cx.notify();
                            })),
                    )
            }),
    );

    body.into_any_element()
}
