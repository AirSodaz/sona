#![allow(dead_code)]
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::super::SettingsModal;
use super::words::parse_dictionary_scope;

pub fn validate_dictionary_content(
    content: &str,
    known_projects: &[(String, String)],
) -> Vec<(usize, &'static str, String)> {
    let mut issues = Vec::new();
    let mut in_projects = false;
    for (index, raw_line) in content.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if matches!(trimmed, "projects" | "projects:") {
            in_projects = true;
            continue;
        }
        if let Some(term) = trimmed.strip_prefix('-') {
            let term = term.trim();
            if term.is_empty() {
                issues.push((
                    line,
                    "error",
                    "Dictionary entries cannot be empty.".to_string(),
                ));
            } else if let Some((from, to)) = term.split_once("->").or_else(|| term.split_once("=>"))
                && (from.trim().is_empty() || to.trim().is_empty())
            {
                issues.push((
                    line,
                    "error",
                    "Replacement rules need text on both sides of the arrow.".to_string(),
                ));
            }
            continue;
        }
        if in_projects {
            if !trimmed.ends_with(':') {
                issues.push((
                    line,
                    "error",
                    "Project scopes must end with a colon.".to_string(),
                ));
                continue;
            }
            if let Some((id, name)) = parse_dictionary_scope(trimmed) {
                let is_voice_typing =
                    id == "voice-typing" || name.eq_ignore_ascii_case("voice typing");
                let known = is_voice_typing
                    || known_projects.iter().any(|(known_id, known_name)| {
                        known_id == &id || known_name.eq_ignore_ascii_case(&name)
                    });
                if !known {
                    issues.push((
                        line,
                        "warning",
                        format!("No project matches '{name}' (id:{id})."),
                    ));
                }
            } else {
                issues.push((line, "error", "Invalid project scope header.".to_string()));
            }
        } else {
            issues.push((
                line,
                "warning",
                "Expected a dictionary entry or a projects: section.".to_string(),
            ));
        }
    }
    issues
}

pub fn code_stat(label: &'static str, value: usize, cx: &Context<SettingsModal>) -> gpui_kit::Div {
    div()
        .px_2()
        .py_1()
        .rounded_full()
        .bg(cx.theme().secondary)
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(format!("{label} {value}"))
}

pub fn section(
    title: &'static str,
    description: &'static str,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}

pub fn tutorial_block(
    title: &'static str,
    description: &'static str,
    example: &'static str,
    hint: Option<&'static str>,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_sm()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
        .child(
            div()
                .w_full()
                .px_3()
                .py_2()
                .rounded_sm()
                .bg(cx.theme().group_box)
                .font_family("Consolas, monospace")
                .text_size(px(12.0))
                .child(example),
        )
        .when_some(hint, |block, hint| {
            block.child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(hint),
            )
        })
}

pub fn toggle_row(
    title: &'static str,
    hint: &'static str,
    enabled: bool,
    id: &'static str,
    key: &'static str,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
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
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                ),
        )
        .child(
            Switch::new(id)
                .checked(enabled)
                .accessibility_label(title)
                .on_change(cx.listener(move |modal, value, _, cx| {
                    modal.persist_model_setting(key.to_string(), Value::Bool(*value), cx);
                })),
        )
}

pub fn choice_row(
    title: &'static str,
    selected: &str,
    key: &'static str,
    options: Vec<(String, String)>,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(options.into_iter().map(|(id, label)| {
                    let is_selected = selected == id;
                    Button::new(ElementId::Name(
                        format!("settings-vocabulary-{key}-{id}").into(),
                    ))
                    .when(is_selected, |button| button.primary())
                    .when(!is_selected, |button| button.outline())
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |modal, _, _, cx| {
                        modal.persist_model_setting(key.to_string(), Value::String(id.clone()), cx);
                    }))
                })),
        )
}
