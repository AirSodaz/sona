#![allow(dead_code)]
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};

use super::super::SettingsModal;
use super::tester::section;
use super::words::parse_dictionary_entries;

#[derive(Clone, Copy)]
pub enum EditorKind {
    Polish,
    Summary,
}

pub fn extract_hotword_weight(text: &str) -> (String, Option<f64>) {
    let Some((word, weight)) = text.rsplit_once(':') else {
        return (text.trim().to_string(), None);
    };
    let Ok(weight) = weight.trim().parse::<f64>() else {
        return (text.trim().to_string(), None);
    };
    if word.trim().is_empty() {
        (text.trim().to_string(), None)
    } else {
        (word.trim().to_string(), Some(weight))
    }
}

pub fn legacy_dictionary_sets(content: &str) -> (Value, Value) {
    let entries = parse_dictionary_entries(content);
    let global_entries = entries
        .iter()
        .filter(|entry| entry.scope_id == "global")
        .collect::<Vec<_>>();
    let hotword_rules = global_entries
        .iter()
        .filter(|entry| !entry.raw.contains("->") && !entry.raw.contains("=>"))
        .enumerate()
        .map(|(index, entry)| json!({ "id": format!("hw_{index}"), "text": entry.raw }))
        .collect::<Vec<_>>();
    let replacement_rules = global_entries
        .iter()
        .filter_map(|entry| {
            let (from, to) = entry
                .raw
                .split_once("->")
                .or_else(|| entry.raw.split_once("=>"))?;
            let from = from.trim();
            let to = to.trim();
            (!from.is_empty() && !to.is_empty()).then_some((from, to))
        })
        .enumerate()
        .map(|(index, (from, to))| json!({ "id": format!("rep_{index}"), "from": from, "to": to }))
        .collect::<Vec<_>>();
    (
        json!([{
            "id": "unified-hotwords",
            "name": "Unified Hotwords",
            "enabled": true,
            "rules": hotword_rules
        }]),
        json!([{
            "id": "unified-replacements",
            "name": "Unified Replacements",
            "enabled": true,
            "ignoreCase": false,
            "rules": replacement_rules
        }]),
    )
}

pub fn polish_preset_options(config: &Value) -> Vec<(String, String)> {
    let mut options = vec![
        ("clean".to_string(), "Clean Spoken".to_string()),
        ("verbatim".to_string(), "Verbatim".to_string()),
        ("formal".to_string(), "Formal Written".to_string()),
    ];
    if let Some(presets) = config.get("polishCustomPresets").and_then(Value::as_array) {
        options.extend(presets.iter().filter_map(|preset| {
            let id = preset.get("id")?.as_str()?.trim();
            let name = preset.get("name")?.as_str()?.trim();
            (!id.is_empty() && !name.is_empty()).then(|| (id.to_string(), name.to_string()))
        }));
    }
    options
}

pub fn summary_template_options(config: &Value) -> Vec<(String, String)> {
    let mut options = vec![
        ("general".to_string(), "General".to_string()),
        ("meeting".to_string(), "Meeting".to_string()),
        ("lecture".to_string(), "Lecture".to_string()),
    ];
    if let Some(templates) = config
        .get("summaryCustomTemplates")
        .and_then(Value::as_array)
    {
        options.extend(templates.iter().filter_map(|template| {
            let id = template.get("id")?.as_str()?.trim();
            let name = template.get("name")?.as_str()?.trim();
            (!id.is_empty() && !name.is_empty()).then(|| (id.to_string(), name.to_string()))
        }));
    }
    options
}

pub fn selected_feature_model_name(config: &Value, feature: &str) -> Option<String> {
    let model_id = config
        .get("llmSettings")?
        .get("selections")?
        .get(format!("{feature}ModelId"))?
        .as_str()?;
    let model = config.get("llmSettings")?.get("models")?.get(model_id)?;
    model
        .pointer("/metadata/displayName")
        .and_then(Value::as_str)
        .or_else(|| model.get("model").and_then(Value::as_str))
        .filter(|name| !name.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub fn preset_detail(
    title: &'static str,
    details: &'static str,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .px_4()
        .py_3()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_sm()
        .bg(cx.theme().background)
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_sm()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().group_box)
                .text_size(px(12.0))
                .text_color(cx.theme().foreground)
                .whitespace_normal()
                .child(details),
        )
}

pub fn custom_records_section(
    config: &Value,
    title: &'static str,
    description: &'static str,
    records_key: &'static str,
    body_key: &'static str,
    default_key: &'static str,
    fallback_id: &'static str,
    _dependency_kind: &'static str,
    kind: EditorKind,
    editing_id: Option<&str>,
    name_state: &Entity<InputState>,
    body_state: &Entity<TextareaState>,
    cx: &mut Context<SettingsModal>,
) -> gpui_kit::Div {
    let records = config
        .get(records_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|record| {
            Some((
                record.get("id")?.as_str()?.to_string(),
                record.get("name")?.as_str()?.to_string(),
                record.get(body_key)?.as_str()?.to_string(),
            ))
        })
        .collect::<Vec<_>>();
    let editing_id = editing_id.map(str::to_string);
    let can_save = !name_state.read(cx).value().trim().is_empty()
        && !body_state.read(cx).value().trim().is_empty();
    let submit_label = match (kind, editing_id.is_some()) {
        (EditorKind::Polish, false) => "Add Preset",
        (EditorKind::Polish, true) => "Save Preset",
        (EditorKind::Summary, false) => "Add Template",
        (EditorKind::Summary, true) => "Save Template",
    };

    section(title, description, cx)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .px_4()
                .py_3()
                .bg(cx.theme().background)
                .child(Input::new(name_state).aria_label("Name"))
                .child(
                    Textarea::new(body_state)
                        .h(px(96.0))
                        .aria_label(match kind {
                            EditorKind::Polish => "Preset context",
                            EditorKind::Summary => "Template instructions",
                        }),
                )
                .child(
                    div().flex().justify_end().child(
                        Button::new(format!("settings-vocabulary-save-{records_key}"))
                            .primary()
                            .small()
                            .label(submit_label)
                            .disabled(!can_save)
                            .on_click(cx.listener(move |modal, _, window, cx| {
                                let name = modal
                                    .vocabulary
                                    .vocabulary_name_state
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .to_string();
                                let body = modal
                                    .vocabulary
                                    .vocabulary_body_state
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .to_string();
                                if name.is_empty() || body.is_empty() {
                                    return;
                                }
                                let current_edit_id = match kind {
                                    EditorKind::Polish => modal.vocabulary.polish_edit_id.clone(),
                                    EditorKind::Summary => modal.vocabulary.summary_edit_id.clone(),
                                };
                                let mut next_records = modal
                                    .model_config
                                    .get(records_key)
                                    .and_then(Value::as_array)
                                    .cloned()
                                    .unwrap_or_default();
                                if let Some(edit_id) = current_edit_id {
                                    if let Some(record) = next_records.iter_mut().find(|record| {
                                        record.get("id").and_then(Value::as_str)
                                            == Some(edit_id.as_str())
                                    }) {
                                        record["name"] = Value::String(name);
                                        record[body_key] = Value::String(body);
                                    }
                                } else {
                                    let mut record = json!({
                                        "id": uuid::Uuid::new_v4().to_string(),
                                        "name": name,
                                    });
                                    record[body_key] = Value::String(body);
                                    next_records.push(record);
                                }
                                modal.persist_model_setting(
                                    records_key.to_string(),
                                    Value::Array(next_records),
                                    cx,
                                );
                                match kind {
                                    EditorKind::Polish => {
                                        modal.vocabulary.polish_edit_id = None;
                                    }
                                    EditorKind::Summary => {
                                        modal.vocabulary.summary_edit_id = None;
                                    }
                                }
                                modal
                                    .vocabulary
                                    .vocabulary_name_state
                                    .update(cx, |state, cx| {
                                        state.set_value("", window, cx);
                                    });
                                modal
                                    .vocabulary
                                    .vocabulary_body_state
                                    .update(cx, |state, cx| {
                                        state.set_value("", window, cx);
                                    });
                                cx.notify();
                            })),
                    ),
                ),
        )
        .when(records.is_empty(), |section| {
            section.child(
                div()
                    .px_4()
                    .py_3()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(match kind {
                        EditorKind::Polish => "No custom polish presets saved.",
                        EditorKind::Summary => "No custom summary templates saved.",
                    }),
            )
        })
        .children(records.into_iter().map(|(id, name, body)| {
            let active_id = id.clone();
            let name_for_edit = name.clone();
            let body_for_edit = body.clone();
            let id_for_delete = id.clone();
            let name_for_delete = name.clone();
            let is_selected_default = config
                .get(default_key)
                .and_then(Value::as_str)
                .is_some_and(|current| current == id);

            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(name),
                                )
                                .when(is_selected_default, |row| {
                                    row.child(
                                        div()
                                            .px_2()
                                            .py(px(1.0))
                                            .rounded_full()
                                            .bg(cx.theme().primary.opacity(0.12))
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().primary)
                                            .child("Default"),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new(format!("settings-vocabulary-edit-{id}"))
                                        .ghost()
                                        .small()
                                        .icon(LucideIcon::Pencil)
                                        .tooltip("Edit")
                                        .accessibility_label(format!("Edit {name_for_edit}"))
                                        .on_click(cx.listener(move |modal, _, window, cx| {
                                            match kind {
                                                EditorKind::Polish => {
                                                    modal.vocabulary.polish_edit_id =
                                                        Some(active_id.clone());
                                                }
                                                EditorKind::Summary => {
                                                    modal.vocabulary.summary_edit_id =
                                                        Some(active_id.clone());
                                                }
                                            }
                                            let name = name_for_edit.clone();
                                            let body = body_for_edit.clone();
                                            modal.vocabulary.vocabulary_name_state.update(
                                                cx,
                                                |state, cx| {
                                                    state.set_value(name, window, cx);
                                                    state.focus(window, cx);
                                                },
                                            );
                                            modal.vocabulary.vocabulary_body_state.update(
                                                cx,
                                                |state, cx| {
                                                    state.set_value(body, window, cx);
                                                },
                                            );
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(format!("settings-vocabulary-delete-{id}"))
                                        .ghost()
                                        .small()
                                        .icon(LucideIcon::Trash)
                                        .tooltip("Delete")
                                        .accessibility_label(format!("Delete {name_for_delete}"))
                                        .on_click(cx.listener(move |modal, _, window, cx| {
                                            let mut next_records = modal
                                                .model_config
                                                .get(records_key)
                                                .and_then(Value::as_array)
                                                .cloned()
                                                .unwrap_or_default();
                                            next_records.retain(|record| {
                                                record.get("id").and_then(Value::as_str)
                                                    != Some(id_for_delete.as_str())
                                            });
                                            modal.persist_model_setting(
                                                records_key.to_string(),
                                                Value::Array(next_records),
                                                cx,
                                            );
                                            if modal
                                                .model_config
                                                .get(default_key)
                                                .and_then(Value::as_str)
                                                == Some(id_for_delete.as_str())
                                            {
                                                modal.persist_model_setting(
                                                    default_key.to_string(),
                                                    Value::String(fallback_id.to_string()),
                                                    cx,
                                                );
                                            }
                                            match kind {
                                                EditorKind::Polish => {
                                                    if modal.vocabulary.polish_edit_id.as_deref()
                                                        == Some(id_for_delete.as_str())
                                                    {
                                                        modal.vocabulary.polish_edit_id = None;
                                                        modal
                                                            .vocabulary
                                                            .vocabulary_name_state
                                                            .update(cx, |state, cx| {
                                                                state.set_value("", window, cx);
                                                            });
                                                        modal
                                                            .vocabulary
                                                            .vocabulary_body_state
                                                            .update(cx, |state, cx| {
                                                                state.set_value("", window, cx);
                                                            });
                                                    }
                                                }
                                                EditorKind::Summary => {
                                                    if modal.vocabulary.summary_edit_id.as_deref()
                                                        == Some(id_for_delete.as_str())
                                                    {
                                                        modal.vocabulary.summary_edit_id = None;
                                                        modal
                                                            .vocabulary
                                                            .vocabulary_name_state
                                                            .update(cx, |state, cx| {
                                                                state.set_value("", window, cx);
                                                            });
                                                        modal
                                                            .vocabulary
                                                            .vocabulary_body_state
                                                            .update(cx, |state, cx| {
                                                                state.set_value("", window, cx);
                                                            });
                                                    }
                                                }
                                            }
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                )
        }))
}
