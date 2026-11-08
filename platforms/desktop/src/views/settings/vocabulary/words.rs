#![allow(dead_code)]
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::super::SettingsModal;
use super::hotwords::{extract_hotword_weight, legacy_dictionary_sets};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryEntry {
    pub line: usize,
    pub scope_id: String,
    pub scope_name: String,
    pub label: String,
    pub raw: String,
}
pub fn effective_dictionary_content(config: &serde_json::Value) -> String {
    if let Some(content) = config
        .get("dictionaryContent")
        .and_then(serde_json::Value::as_str)
        .filter(|content| !content.trim().is_empty())
    {
        return content.to_string();
    }

    let mut lines = vec!["# Global Vocabulary & Replacements".to_string()];
    let mut hotwords = std::collections::HashSet::new();
    if let Some(sets) = config
        .get("hotwordSets")
        .and_then(serde_json::Value::as_array)
    {
        for rule in sets
            .iter()
            .filter_map(|set| set.get("rules").and_then(serde_json::Value::as_array))
            .flatten()
        {
            if let Some(text) = rule.get("text").and_then(serde_json::Value::as_str) {
                let text = text.trim();
                if !text.is_empty() && hotwords.insert(text.to_string()) {
                    lines.push(format!("- {text}"));
                }
            }
        }
    }

    if let Some(sets) = config
        .get("textReplacementSets")
        .and_then(serde_json::Value::as_array)
    {
        for rule in sets
            .iter()
            .filter_map(|set| set.get("rules").and_then(serde_json::Value::as_array))
            .flatten()
        {
            let from = rule
                .get("from")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .trim();
            let to = rule
                .get("to")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .trim();
            if !from.is_empty() && !to.is_empty() {
                lines.push(format!("- {from} -> {to}"));
            }
        }
    }

    if lines.len() == 1 {
        lines.push("# - Sona".to_string());
        lines.push("# - MisrecognizedWord -> CorrectWord".to_string());
    }
    lines.join("\n")
}

pub fn parse_dictionary_entries(content: &str) -> Vec<DictionaryEntry> {
    let mut entries = Vec::new();
    let mut in_projects = false;
    let mut current_scope = ("global".to_string(), "Global".to_string());

    for (line_index, raw_line) in content.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if matches!(trimmed, "projects" | "projects:") {
            in_projects = true;
            current_scope = ("global".to_string(), "Global".to_string());
            continue;
        }
        if in_projects
            && !trimmed.starts_with('-')
            && raw_line.starts_with(char::is_whitespace)
            && trimmed.ends_with(':')
        {
            if let Some(scope) = parse_dictionary_scope(trimmed) {
                current_scope = scope;
            }
            continue;
        }
        let Some(term) = trimmed
            .strip_prefix('-')
            .map(str::trim)
            .filter(|term| !term.is_empty())
        else {
            continue;
        };
        let label =
            if let Some((from, to)) = term.split_once("->").or_else(|| term.split_once("=>")) {
                format!("{} -> {}", from.trim(), to.trim())
            } else {
                term.to_string()
            };
        entries.push(DictionaryEntry {
            line: line_index + 1,
            scope_id: current_scope.0.clone(),
            scope_name: current_scope.1.clone(),
            label,
            raw: term.to_string(),
        });
    }
    entries
}

pub fn format_dictionary_content(content: &str, known_projects: &[(String, String)]) -> String {
    let entries = parse_dictionary_entries(content);
    let mut lines = vec!["# Global Vocabulary & Replacements".to_string()];
    let global = entries
        .iter()
        .filter(|entry| entry.scope_id == "global")
        .collect::<Vec<_>>();
    if global.is_empty() {
        lines.push("# - ExampleTerm".to_string());
        lines.push("# - MisrecognizedWord -> CorrectWord".to_string());
    } else {
        lines.extend(
            global
                .into_iter()
                .map(|entry| format!("- {}", format_dictionary_term(&entry.raw))),
        );
    }

    let scopes = parse_dictionary_scopes(content);
    if !scopes.is_empty() || !known_projects.is_empty() {
        lines.push(String::new());
        lines.push("# Project Specific Terms & Replacements".to_string());
        lines.push("projects:".to_string());
        for (parsed_id, parsed_name) in scopes {
            let project = known_projects.iter().find(|(known_id, known_name)| {
                known_id == &parsed_id || known_name.eq_ignore_ascii_case(&parsed_name)
            });
            let is_voice_typing =
                parsed_id == "voice-typing" || parsed_name.eq_ignore_ascii_case("voice typing");
            let (id, name) = if is_voice_typing {
                ("voice-typing", "Voice Typing")
            } else if let Some((id, name)) = project {
                (id.as_str(), name.as_str())
            } else {
                (parsed_id.as_str(), parsed_name.as_str())
            };
            lines.push(format!("  {name} (id:{id}):"));
            let project_entries = entries
                .iter()
                .filter(|entry| entry.scope_id == parsed_id)
                .collect::<Vec<_>>();
            if project_entries.is_empty() {
                lines.push("    # - Term".to_string());
            }
            for entry in project_entries {
                lines.push(format!("    - {}", format_dictionary_term(&entry.raw)));
            }
        }
    }
    lines.join("\n")
}

pub fn format_dictionary_term(raw: &str) -> String {
    raw.split_once("->")
        .or_else(|| raw.split_once("=>"))
        .map(|(from, to)| format!("{} -> {}", from.trim(), to.trim()))
        .unwrap_or_else(|| raw.trim().to_string())
}

pub fn parse_dictionary_scopes(content: &str) -> Vec<(String, String)> {
    let mut scopes = Vec::new();
    let mut in_projects = false;
    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if matches!(trimmed, "projects" | "projects:") {
            in_projects = true;
            continue;
        }
        if !in_projects
            || trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('-')
            || !raw_line.starts_with(char::is_whitespace)
            || !trimmed.ends_with(':')
        {
            continue;
        }
        if let Some(scope) = parse_dictionary_scope(trimmed)
            && !scopes.iter().any(|(id, _)| id == &scope.0)
        {
            scopes.push(scope);
        }
    }
    scopes
}

pub fn parse_dictionary_scope(header: &str) -> Option<(String, String)> {
    let header = header.strip_suffix(':')?.trim();
    let (name, explicit_id) = if let Some(open) = header.rfind("(id:") {
        let name = header[..open].trim();
        let id = header[open + 4..].strip_suffix(')')?.trim();
        (name, Some(id))
    } else if let Some(open) = header.rfind("(id=") {
        let name = header[..open].trim();
        let id = header[open + 4..].strip_suffix(')')?.trim();
        (name, Some(id))
    } else {
        (header, None)
    };
    let name = name.trim_matches(['"', '\'']).trim();
    if name.is_empty() {
        return None;
    }
    let id = explicit_id
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            name.to_ascii_lowercase()
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
        });
    Some((id, name.to_string()))
}

pub fn remove_dictionary_line(content: &str, line: usize) -> String {
    if line == 0 {
        return content.to_string();
    }
    let mut lines = content
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if line <= lines.len() {
        lines.remove(line - 1);
    }
    lines.join("\n")
}

pub fn add_global_dictionary_entry(content: &str, value: &str) -> String {
    let value = value.trim().trim_start_matches("- ").trim();
    if value.is_empty() {
        return content.to_string();
    }

    let mut lines = content
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let insertion = lines
        .iter()
        .position(|line| matches!(line.trim(), "projects" | "projects:"))
        .unwrap_or(lines.len());
    let entry = format!("- {value}");
    if lines.iter().any(|line| line.trim() == entry) {
        return content.to_string();
    }
    lines.insert(insertion, entry);
    lines.join("\n")
}

pub fn add_dictionary_entry_to_scope(
    content: &str,
    value: &str,
    scope_id: &str,
    scope_name: &str,
) -> String {
    let value = value
        .trim()
        .trim_start_matches('-')
        .trim()
        .replace("=>", "->");
    if value.is_empty() {
        return content.to_string();
    }
    if scope_id == "global" {
        let candidate = value.to_lowercase();
        if parse_dictionary_entries(content).iter().any(|entry| {
            entry.scope_id == "global"
                && entry.raw.replace("=>", "->").trim().to_lowercase() == candidate
        }) {
            return content.to_string();
        }
        return add_global_dictionary_entry(content, &value);
    }

    let candidate = value.to_lowercase();
    if parse_dictionary_entries(content).iter().any(|entry| {
        entry.scope_id == scope_id
            && entry.raw.replace("=>", "->").trim().to_lowercase() == candidate
    }) {
        return content.to_string();
    }

    let essentially_empty = content.trim().is_empty()
        || content
            .lines()
            .all(|line| line.trim().is_empty() || line.trim().starts_with('#'));
    if essentially_empty {
        return format!(
            "# Global Vocabulary & Replacements\n\n# Project Specific Terms & Replacements\nprojects:\n  {scope_name} (id:{scope_id}):\n    - {value}\n"
        );
    }

    let mut lines = content.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let existing_header = lines.iter().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim();
        (line.starts_with(char::is_whitespace)
            && trimmed.ends_with(':')
            && parse_dictionary_scope(trimmed).is_some_and(|(id, _)| id == scope_id))
        .then_some(index)
    });
    if let Some(header_index) = existing_header {
        lines.insert(header_index + 1, format!("    - {value}"));
        return lines.join("\n");
    }

    let projects_index = lines
        .iter()
        .position(|line| matches!(line.trim(), "projects" | "projects:"));
    if projects_index.is_none() {
        lines.extend([
            String::new(),
            "# Project Specific Terms & Replacements".to_string(),
            "projects:".to_string(),
        ]);
    }
    lines.push(format!("  {scope_name} (id:{scope_id}):"));
    lines.push(format!("    - {value}"));
    lines.join("\n")
}

pub fn update_dictionary_line(content: &str, line: usize, value: &str) -> String {
    if line == 0 || value.trim().is_empty() {
        return content.to_string();
    }
    let mut lines = content
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if line <= lines.len() {
        let current = &lines[line - 1];
        let indent = current
            .chars()
            .take_while(|character| character.is_whitespace())
            .collect::<String>();
        let value = value.trim().trim_start_matches("- ").trim();
        lines[line - 1] = format!("{indent}- {value}");
    }
    lines.join("\n")
}

pub fn dictionary_line_range(content: &str, line_number: usize) -> std::ops::Range<usize> {
    let start = content
        .split_inclusive('\n')
        .take(line_number.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let length = content
        .lines()
        .nth(line_number.saturating_sub(1))
        .map(str::len)
        .unwrap_or_default();
    start..start + length
}

pub fn dictionary_scope_suggestions(
    content: &str,
    cursor_line: usize,
    cursor_column: usize,
    known_projects: &[(String, String)],
) -> Vec<(String, String)> {
    let lines = content.split('\n').collect::<Vec<_>>();
    let Some(current_line) = lines.get(cursor_line).copied() else {
        return Vec::new();
    };
    let Some(projects_line) = lines
        .iter()
        .position(|line| matches!(line.trim(), "projects" | "projects:"))
    else {
        return Vec::new();
    };
    let prefix = current_line
        .char_indices()
        .take_while(|(byte_index, character)| {
            let utf16_width =
                current_line[..*byte_index].encode_utf16().count() + character.len_utf16();
            utf16_width <= cursor_column
        })
        .map(|(_, character)| character)
        .collect::<String>();
    let query = prefix.trim();
    if cursor_line <= projects_line
        || query.starts_with('-')
        || query.is_empty()
        || !query.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character.is_whitespace()
                || character == '_'
                || ('\u{4e00}'..='\u{9fa5}').contains(&character)
        })
    {
        return Vec::new();
    }
    let query = query.to_lowercase();
    std::iter::once(("voice-typing".to_string(), "Voice Typing".to_string()))
        .chain(known_projects.iter().cloned())
        .filter(|(id, name)| {
            name.to_lowercase().contains(&query) || id.to_lowercase().contains(&query)
        })
        .collect()
}

pub fn select_dictionary_scope_suggestion(
    content: &str,
    cursor_line: usize,
    project_id: &str,
    project_name: &str,
) -> String {
    let mut lines = content
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if cursor_line >= lines.len() {
        return content.to_string();
    }
    lines[cursor_line] = format!("  {project_name} (id:{project_id}):");
    if lines
        .get(cursor_line + 1)
        .is_some_and(|line| !line.trim().starts_with('-'))
    {
        lines.insert(cursor_line + 1, "    - ".to_string());
    } else if cursor_line + 1 == lines.len() {
        lines.push("    - ".to_string());
    }
    lines.join("\n")
}

pub fn dictionary_edit_button(
    entry: &DictionaryEntry,
    editing_line: Option<usize>,
    cx: &Context<SettingsModal>,
) -> Button {
    let line = entry.line;
    let raw = entry.raw.clone();
    let scope_id = entry.scope_id.clone();
    Button::new(ElementId::Name(
        format!("dictionary-edit-{}-{line}", entry.scope_id).into(),
    ))
    .ghost()
    .small()
    .icon(LucideIcon::Pencil)
    .when(editing_line == Some(line), |button| button.primary())
    .tooltip("Edit entry")
    .accessibility_label(format!("Edit {}", entry.label))
    .on_click(cx.listener(move |modal, _, window, cx| {
        modal.vocabulary.edit_line = Some(line);
        modal.vocabulary.add_scope = scope_id.clone();
        let is_replacement = raw.contains("->") || raw.contains("=>");
        modal.vocabulary.dialog_open = true;
        modal.vocabulary.dialog_type = if is_replacement {
            "replacement"
        } else {
            "hotword"
        }
        .to_string();
        if let Some((from, to)) = raw.split_once("->").or_else(|| raw.split_once("=>")) {
            modal.vocabulary.from_state.update(cx, |state, cx| {
                state.set_value(from.trim(), window, cx);
                state.focus(window, cx);
            });
            modal
                .vocabulary
                .to_state
                .update(cx, |state, cx| state.set_value(to.trim(), window, cx));
            modal
                .vocabulary
                .word_state
                .update(cx, |state, cx| state.set_value("", window, cx));
            modal
                .vocabulary
                .weight_state
                .update(cx, |state, cx| state.set_value("", window, cx));
        } else {
            let (word, weight) = extract_hotword_weight(&raw);
            modal.vocabulary.word_state.update(cx, |state, cx| {
                state.set_value(word, window, cx);
                state.focus(window, cx);
            });
            modal.vocabulary.weight_state.update(cx, |state, cx| {
                state.set_value(
                    weight
                        .map(|weight| format!("{weight:.1}"))
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            modal
                .vocabulary
                .from_state
                .update(cx, |state, cx| state.set_value("", window, cx));
            modal
                .vocabulary
                .to_state
                .update(cx, |state, cx| state.set_value("", window, cx));
        }
        cx.notify();
    }))
}

pub fn dictionary_delete_button(entry: &DictionaryEntry, cx: &Context<SettingsModal>) -> Button {
    let line = entry.line;
    let label = entry.label.clone();
    Button::new(ElementId::Name(
        format!("dictionary-remove-{}-{line}", entry.scope_id).into(),
    ))
    .ghost()
    .small()
    .icon(LucideIcon::Trash)
    .tooltip("Delete entry")
    .accessibility_label(format!("Delete {label}"))
    .on_click(cx.listener(move |modal, _, window, cx| {
        let current = modal
            .vocabulary
            .dictionary_textarea
            .read(cx)
            .value()
            .to_string();
        let updated = remove_dictionary_line(&current, line);
        let (hotwords, replacements) = legacy_dictionary_sets(&updated);
        modal
            .vocabulary
            .dictionary_textarea
            .update(cx, |state, cx| state.set_value(updated.clone(), window, cx));
        modal.persist_model_settings(
            vec![
                (
                    "dictionaryContent".into(),
                    serde_json::Value::String(updated),
                ),
                ("hotwordSets".into(), hotwords),
                ("textReplacementSets".into(), replacements),
            ],
            cx,
        );
        if modal.vocabulary.edit_line == Some(line) {
            modal.vocabulary.edit_line = None;
            modal.vocabulary.dialog_open = false;
        }
        cx.notify();
    }))
}

pub fn dictionary_hotword_chip(
    entry: &DictionaryEntry,
    dictionary_scope: &str,
    editing_line: Option<usize>,
    cx: &Context<SettingsModal>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_1p5()
        .rounded_md()
        .bg(if editing_line == Some(entry.line) {
            cx.theme().accent
        } else {
            cx.theme().input
        })
        .border_1()
        .border_color(if editing_line == Some(entry.line) {
            cx.theme().primary
        } else {
            cx.theme().border
        })
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().foreground)
                .child(entry.label.clone()),
        )
        .when(dictionary_scope == "all", |row| {
            row.child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(entry.scope_name.clone()),
            )
        })
        .child(dictionary_edit_button(entry, editing_line, cx))
        .child(dictionary_delete_button(entry, cx))
}

pub fn dictionary_replacement_row(
    entry: &DictionaryEntry,
    editing_line: Option<usize>,
    cx: &Context<SettingsModal>,
) -> impl IntoElement {
    let (from, to) = entry
        .raw
        .split_once("->")
        .or_else(|| entry.raw.split_once("=>"))
        .map(|(from, to)| (from.trim(), to.trim()))
        .unwrap_or((entry.raw.as_str(), ""));

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .p_3()
        .rounded_md()
        .bg(if editing_line == Some(entry.line) {
            cx.theme().accent
        } else {
            cx.theme().input
        })
        .border_1()
        .border_color(if editing_line == Some(entry.line) {
            cx.theme().primary
        } else {
            cx.theme().border
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .child(from.to_string()),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("→"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .child(to.to_string()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .when(entry.scope_id != "global", |actions| {
                    actions.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(entry.scope_name.clone()),
                    )
                })
                .child(dictionary_edit_button(entry, editing_line, cx))
                .child(dictionary_delete_button(entry, cx)),
        )
}
