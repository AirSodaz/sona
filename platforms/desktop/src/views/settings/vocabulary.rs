use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, StatefulInteractiveElement as _, *};
use serde_json::{Value, json};

pub(super) fn vocabulary_panel(
    config: &Value,
    active_tab: &str,
    active_prompt_tab: &str,
    polish_edit_id: Option<&str>,
    summary_edit_id: Option<&str>,
    dictionary_mode: &str,
    dictionary_scope: &str,
    dictionary_textarea: &Entity<TextareaState>,
    dictionary_quick_add: &Entity<InputState>,
    dictionary_search: &Entity<InputState>,
    dictionary_test: &Entity<InputState>,
    vocabulary_name_state: &Entity<InputState>,
    vocabulary_body_state: &Entity<TextareaState>,
    translation_language_select: &Entity<SelectState<Vec<&'static str>>>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let auto_polish = config
        .get("autoPolish")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let auto_translate = config
        .get("autoTranslate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let auto_summary = config
        .get("autoSummary")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let dictionary_content = effective_dictionary_content(config);
    let dictionary_entries = parse_dictionary_entries(&dictionary_content);
    let projects = cx.entity().read(cx).project_selector.projects.clone();
    let project_refs = projects
        .iter()
        .map(|project| (project.id.clone(), project.name.clone()))
        .collect::<Vec<_>>();
    let diagnostics = validate_dictionary_content(&dictionary_content, &project_refs);
    let mut scopes = vec![(
        "global".to_string(),
        "Global".to_string(),
        dictionary_entries
            .iter()
            .filter(|entry| entry.scope_id == "global")
            .count(),
    )];
    scopes.push((
        "voice-typing".to_string(),
        "Voice Typing".to_string(),
        dictionary_entries
            .iter()
            .filter(|entry| entry.scope_id == "voice-typing")
            .count(),
    ));
    for project in &projects {
        let count = dictionary_entries
            .iter()
            .filter(|entry| entry.scope_id == project.id)
            .count();
        if !scopes.iter().any(|(id, _, _)| id == &project.id) {
            scopes.push((project.id.clone(), project.name.clone(), count));
        }
    }
    for entry in &dictionary_entries {
        if !scopes.iter().any(|(id, _, _)| id == &entry.scope_id) {
            let count = dictionary_entries
                .iter()
                .filter(|candidate| candidate.scope_id == entry.scope_id)
                .count();
            scopes.push((entry.scope_id.clone(), entry.scope_name.clone(), count));
        }
    }
    let search_query = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_search_query
        .clone();
    let editing_line = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_edit_line;
    let add_scope = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_add_scope
        .clone();
    let scope_menu_open = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_scope_menu_open;
    let add_scope_name = scopes
        .iter()
        .find(|(id, _, _)| id == &add_scope)
        .map(|(_, name, _)| name.clone())
        .unwrap_or_else(|| add_scope.clone());
    let tutorial_open = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_tutorial_open;
    let dialog_open = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_dialog_open;
    let dialog_type = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_dialog_type
        .clone();
    let (word_state, weight_state, from_state, to_state) = {
        let shell = cx.entity().read(cx);
        (
            shell.dictionary_word_state.clone(),
            shell.dictionary_weight_state.clone(),
            shell.dictionary_from_state.clone(),
            shell.dictionary_to_state.clone(),
        )
    };
    let dialog_word = word_state.read(cx).value().trim().to_string();
    let dialog_from = from_state.read(cx).value().trim().to_string();
    let dialog_to = to_state.read(cx).value().trim().to_string();
    let dialog_can_save = if dialog_type == "hotword" {
        !dialog_word.is_empty()
    } else {
        !dialog_from.is_empty() && !dialog_to.is_empty()
    };
    let quick_add_value = dictionary_quick_add.read(cx).value().to_string();
    let inline_hotword_state = cx.entity().read(cx).dictionary_inline_hotword_state.clone();
    let visible_entries = dictionary_entries
        .iter()
        .filter(|entry| dictionary_scope == "all" || entry.scope_id == dictionary_scope)
        .filter(|entry| {
            search_query.trim().is_empty()
                || entry
                    .label
                    .to_lowercase()
                    .contains(&search_query.trim().to_lowercase())
        })
        .cloned()
        .collect::<Vec<_>>();
    let hotword_entries = visible_entries
        .iter()
        .filter(|entry| !entry.raw.contains("->") && !entry.raw.contains("=>"))
        .cloned()
        .collect::<Vec<_>>();
    let replacement_entries = visible_entries
        .iter()
        .filter(|entry| entry.raw.contains("->") || entry.raw.contains("=>"))
        .cloned()
        .collect::<Vec<_>>();
    let test_input = cx
        .entity()
        .read(cx)
        .settings_modal
        .vocabulary_dictionary_test_input
        .clone();
    let test_output = visible_entries
        .iter()
        .filter_map(|entry| {
            entry
                .raw
                .split_once("->")
                .or_else(|| entry.raw.split_once("=>"))
                .map(|(from, to)| (from.trim(), to.trim()))
        })
        .fold(test_input.clone(), |text, (from, to)| {
            if from.is_empty() {
                text
            } else {
                text.replace(from, to)
            }
        });
    let dictionary_line_count = dictionary_content.lines().count().max(1);
    let global_term_count = dictionary_entries
        .iter()
        .filter(|entry| entry.scope_id == "global")
        .count();
    let project_scope_count = parse_dictionary_scopes(&dictionary_content).len();
    let cursor_position = dictionary_textarea.read(cx).cursor_position();
    let editor_cursor_line = cursor_position.line as usize;
    let project_suggestions = dictionary_scope_suggestions(
        &dictionary_content,
        editor_cursor_line,
        cursor_position.character as usize,
        &project_refs,
    );

    div()
        .id("settings-vocabulary-panel")
        .flex().flex_col().size_full().min_h_0().overflow_y_scroll()
        .px(px(32.0)).py(px(24.0))
        .child(
            div().w_full().max_w(px(900.0)).mx_auto().flex().flex_col().gap_4()
                .child(
                    div().flex().flex_col().gap_1().pb_4().border_b_1().border_color(cx.theme().border)
                        .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("Vocabulary & Language"))
                        .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground)
                            .child("Manage custom vocabulary, hotwords, text replacements, context presets, and summary templates.")),
                )
                .child(
                    div().flex().gap_2().children([
                        ("recognition", "Unified Dictionary", "Global vocabulary, text replacements, and project terms"),
                        ("prompts", "AI Prompts & Templates", "Polish keywords, context presets, and summary templates"),
                        ("speakers", "Speaker Profiles", "Local voice sample profiles and references"),
                    ].into_iter().map(|(tab, label, description)| {
                        let selected = active_tab == tab;
                        div().flex_1().min_w_0().flex().flex_col().gap_1().p_3().rounded_md().border_1()
                            .id(ElementId::Name(format!("settings-vocabulary-category-{tab}").into()))
                            .border_color(if selected { cx.theme().accent } else { cx.theme().border })
                            .bg(if selected { cx.theme().accent.opacity(0.10) } else { cx.theme().group_box })
                            .cursor_pointer().hover(|style| style.border_color(cx.theme().accent))
                            .on_click(cx.listener(move |shell, _, window, cx| {
                                shell.settings_modal.vocabulary_tab = tab.to_string();
                                shell.settings_modal.vocabulary_polish_edit_id = None;
                                shell.settings_modal.vocabulary_summary_edit_id = None;
                                shell.vocabulary_name_state.update(cx, |state, cx| state.set_value("", window, cx));
                                shell.vocabulary_body_state.update(cx, |state, cx| state.set_value("", window, cx));
                                cx.notify();
                            }))
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child(label))
                            .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child(description))
                    })),
                )
                .when(active_tab == "recognition", |panel| {
                    panel.child(
                        Button::new("dictionary-tutorial-toggle")
                            .ghost()
                            .small()
                            .label(if tutorial_open { "Hide Syntax Guide & Examples" } else { "Show Syntax Guide & Examples" })
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.settings_modal.vocabulary_dictionary_tutorial_open =
                                    !shell.settings_modal.vocabulary_dictionary_tutorial_open;
                                cx.notify();
                            })),
                    )
                })
                .when(active_tab == "recognition", |panel| {
                    panel.child(
                        section("Unified Dictionary", "Global vocabulary, text replacements, and project-scoped terms.", cx)
                            .child(div().flex().items_center().justify_between().gap_3()
                                .child(div().flex().items_center().gap_1().children([("visual", "Visual"), ("code", "Code")].into_iter().map(|(mode, label)| {
                                    let selected = dictionary_mode == mode;
                                    Button::new(ElementId::Name(format!("dictionary-mode-{mode}").into()))
                                        .when(selected, |button| button.primary())
                                        .when(!selected, |button| button.ghost())
                                        .small().label(label)
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            shell.settings_modal.vocabulary_dictionary_mode = mode.to_string();
                                            cx.notify();
                                        }))
                                })))
                                .child(div().flex().items_center().gap_1()
                                    .when(dictionary_mode == "visual", |actions| actions.child(
                                        Button::new("dictionary-new-entry")
                                            .primary().small().label("New Entry")
                                            .on_click(cx.listener(|shell, _, window, cx| {
                                                shell.settings_modal.vocabulary_dictionary_edit_line = None;
                                                shell.settings_modal.vocabulary_dictionary_dialog_open = true;
                                                shell.settings_modal.vocabulary_dictionary_dialog_type = "hotword".to_string();
                                                if shell.settings_modal.vocabulary_dictionary_scope != "all" {
                                                    shell.settings_modal.vocabulary_dictionary_add_scope = shell.settings_modal.vocabulary_dictionary_scope.clone();
                                                } else {
                                                    shell.settings_modal.vocabulary_dictionary_add_scope = "global".to_string();
                                                }
                                                shell.dictionary_word_state.update(cx, |state, cx| { state.set_value("", window, cx); state.focus(window, cx); });
                                                shell.dictionary_weight_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                shell.dictionary_from_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                shell.dictionary_to_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                cx.notify();
                                            })),
                                    ))
                                    .child(Button::new("dictionary-format").outline().small().label("Format")
                                        .tooltip("Format indentation, canonicalize arrows, and sync project links")
                                        .on_click(cx.listener(|shell, _, window, cx| {
                                            let source = shell.dictionary_textarea_state.read(cx).value().to_string();
                                            let projects = shell.project_selector.projects.iter().map(|project| (project.id.clone(), project.name.clone())).collect::<Vec<_>>();
                                            let formatted = format_dictionary_content(&source, &projects);
                                            let (hotwords, replacements) = legacy_dictionary_sets(&formatted);
                                            shell.dictionary_textarea_state.update(cx, |state, cx| state.set_value(formatted.clone(), window, cx));
                                            shell.settings_modal.persist_model_settings(vec![
                                                ("dictionaryContent".into(), Value::String(formatted)),
                                                ("hotwordSets".into(), hotwords),
                                                ("textReplacementSets".into(), replacements),
                                            ], cx);
                                        })))
                                    .child(Button::new("dictionary-copy").ghost().small().label("Copy")
                                        .tooltip("Copy dictionary YAML to clipboard")
                                        .on_click(cx.listener(|shell, _, _, cx| {
                                            let source = shell.dictionary_textarea_state.read(cx).value().to_string();
                                            cx.write_to_clipboard(ClipboardItem::new_string(source));
                                        })))
                                    .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground)
                                        .child(format!("{} terms", dictionary_entries.len())))))
                            .child(div().flex().items_center().gap_2().py_2().flex_wrap()
                                .child(div().flex().items_center().gap_1()
                                    .child(div().text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(cx.theme().muted_foreground).child("Target:"))
                                    .child(Button::new("dictionary-add-scope")
                                        .outline().small().w(px(150.0)).label(add_scope_name.clone())
                                        .tooltip("Choose the scope for the new dictionary entry")
                                        .on_click(cx.listener(|shell, _, _, cx| {
                                            shell.settings_modal.vocabulary_dictionary_scope_menu_open =
                                                !shell.settings_modal.vocabulary_dictionary_scope_menu_open;
                                            cx.notify();
                                        }))))
                                .child(div().flex_1().min_w(px(180.0))
                                    .on_key_down(cx.listener(|shell, event: &KeyDownEvent, window, cx| {
                                        if event.keystroke.key.eq_ignore_ascii_case("enter") {
                                            let raw = shell.dictionary_quick_add_state.read(cx).value().to_string();
                                            shell.settings_modal.vocabulary_dictionary_edit_line = None;
                                            submit_dictionary_entry(shell, &raw, window, cx);
                                            window.prevent_default();
                                        }
                                    }))
                                    .child(Input::new(dictionary_quick_add).w_full().h(px(36.0)).aria_label("Add dictionary term")))
                                .child(Button::new("dictionary-quick-add").primary().small().label("Add")
                                    .disabled(quick_add_value.trim().is_empty())
                                    .on_click(cx.listener(|shell, _, window, cx| {
                                        let raw = shell.dictionary_quick_add_state.read(cx).value().trim().to_string();
                                        shell.settings_modal.vocabulary_dictionary_edit_line = None;
                                        submit_dictionary_entry(shell, &raw, window, cx);
                                    }))))
                            .when(scope_menu_open, |row| row.child(
                                div().flex().flex_wrap().gap_1().p_2().rounded_sm().border_1().border_color(cx.theme().border).bg(cx.theme().background)
                                    .children(scopes.iter().map(|(scope_id, label, count)| {
                                        let selected = add_scope == *scope_id;
                                        let selected_scope_id = scope_id.clone();
                                        Button::new(ElementId::Name(format!("dictionary-add-scope-{scope_id}").into()))
                                            .when(selected, |button| button.primary())
                                            .when(!selected, |button| button.ghost())
                                            .small().label(format!("{label} ({count})"))
                                            .on_click(cx.listener(move |shell, _, _, cx| {
                                                shell.settings_modal.vocabulary_dictionary_add_scope = selected_scope_id.clone();
                                                shell.settings_modal.vocabulary_dictionary_scope_menu_open = false;
                                                cx.notify();
                                            }))
                                    }))
                            ))
                            .when(dictionary_mode == "code", |section| {
                                let textarea = dictionary_textarea.clone();
                                section
                                    .child(div().flex().items_center().gap_2().px_3().py_2()
                                        .bg(cx.theme().secondary.opacity(0.3))
                                        .border_b_1().border_color(cx.theme().border)
                                        .child(code_stat("Total terms:", dictionary_entries.len(), cx))
                                        .child(code_stat("Global:", global_term_count, cx))
                                        .child(code_stat("Projects:", project_scope_count, cx)))
                                    .child(div().relative().flex().items_stretch().h(px(300.0)).max_h(px(480.0)).overflow_hidden()
                                        .child(div().flex().flex_col().items_end().gap_1().px_2().py_2().bg(cx.theme().group_box).text_size(px(11.0)).text_color(cx.theme().muted_foreground)
                                            .children((1..=dictionary_line_count).map(|line| div().h(px(20.0)).child(line.to_string()))))
                                        .child(div().flex_1().h_full()
                                            .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, window, cx| {
                                                if event.keystroke.key.eq_ignore_ascii_case("tab") {
                                                    let selected = textarea.read(cx).selected_range();
                                                    let mut content = textarea.read(cx).value().to_string();
                                                    content.replace_range(selected.clone(), "  ");
                                                    let cursor = selected.start + 2;
                                                    textarea.update(cx, |state, cx| {
                                                        state.set_value(content.clone(), window, cx);
                                                        state.set_selected_range(cursor..cursor, cx);
                                                        state.focus(window, cx);
                                                    });
                                                    let (hotwords, replacements) = legacy_dictionary_sets(&content);
                                                    shell.settings_modal.persist_model_settings(vec![
                                                        ("dictionaryContent".into(), Value::String(content)),
                                                        ("hotwordSets".into(), hotwords),
                                                        ("textReplacementSets".into(), replacements),
                                                    ], cx);
                                                    window.prevent_default();
                                                }
                                            }))
                                            .child(Textarea::new(dictionary_textarea).w_full().h_full().aria_label("Unified dictionary YAML"))))
                                    .when(!project_suggestions.is_empty(), |editor| editor.child(
                                        div().absolute().top(px(20.0 * (editor_cursor_line as f32 + 1.0))).left(px(60.0))
                                            .min_w(px(220.0)).overflow_hidden().rounded_md().border_1().border_color(cx.theme().border)
                                            .bg(cx.theme().background).shadow_lg()
                                            .child(div().px_2().py_1().bg(cx.theme().group_box).text_size(px(11.0)).font_weight(FontWeight::SEMIBOLD).text_color(cx.theme().muted_foreground).child("Link Project"))
                                            .children(project_suggestions.iter().map(|(project_id, project_name)| {
                                                let project_id = project_id.clone();
                                                let project_name = project_name.clone();
                                                let line = editor_cursor_line;
                                                Button::new(ElementId::Name(format!("dictionary-suggest-{project_id}").into()))
                                                    .ghost().w_full().small().label(format!("{project_name} (id:{project_id})"))
                                                    .on_click(cx.listener(move |shell, _, window, cx| {
                                                        let source = shell.dictionary_textarea_state.read(cx).value().to_string();
                                                        let updated = select_dictionary_scope_suggestion(&source, line, &project_id, &project_name);
                                                        shell.dictionary_textarea_state.update(cx, |state, cx| state.set_value(updated.clone(), window, cx));
                                                        let (hotwords, replacements) = legacy_dictionary_sets(&updated);
                                                        shell.settings_modal.persist_model_settings(vec![
                                                            ("dictionaryContent".into(), Value::String(updated)),
                                                            ("hotwordSets".into(), hotwords),
                                                            ("textReplacementSets".into(), replacements),
                                                        ], cx);
                                                    }))
                                            }))
                                    ))
                                    .child(div().flex().flex_col().gap_1().pt_2()
                                        .child(div().text_size(px(12.0)).font_weight(FontWeight::SEMIBOLD).child(if diagnostics.is_empty() { "No syntax or link issues" } else { "Syntax & Link Issues" }))
                                        .children(diagnostics.iter().map(|(line, severity, message)| {
                                            let line = *line;
                                            div().flex().items_center().gap_2().px_2().py_1().rounded_sm().bg(cx.theme().group_box)
                                                .id(ElementId::Name(format!("dictionary-diagnostic-{line}").into()))
                                                .cursor_pointer().hover(|style| style.bg(cx.theme().secondary.opacity(0.4)))
                                                .tooltip(|window, cx| Tooltip::new("Click to jump to line").build(window, cx))
                                                .on_click(cx.listener(move |shell, _, window, cx| {
                                                    let content = shell.dictionary_textarea_state.read(cx).value().to_string();
                                                    let range = dictionary_line_range(&content, line);
                                                    shell.dictionary_textarea_state.update(cx, |state, cx| {
                                                        state.focus(window, cx);
                                                        state.set_selected_range(range, cx);
                                                    });
                                                }))
                                                .child(div().text_size(px(11.0)).font_weight(FontWeight::MEDIUM).text_color(if *severity == "error" { cx.theme().danger } else { cx.theme().warning }).child(format!("Line {line} · {severity}")))
                                                .child(div().text_size(px(12.0)).text_color(cx.theme().foreground).child(message.clone()))
                                        })))
                            })
                            .when(dictionary_mode == "visual", |section| {
                    let inline_scope_id = if dictionary_scope == "all" {
                        "global".to_string()
                    } else {
                        dictionary_scope.to_string()
                    };
                                section
                                    .child(div().flex().items_center().justify_between().gap_3()
                                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child(format!("{} visible", visible_entries.len())))
                                        .child(div().flex().items_center().gap_1()
                                            .child(Input::new(dictionary_search).w(px(220.0)).h(px(30.0)).aria_label("Search dictionary"))
                                            .when(!search_query.is_empty(), |row| row.child(Button::new("dictionary-search-clear").ghost().small().label("×")
                                                .tooltip("Clear search")
                                                .on_click(cx.listener(|shell, _, window, cx| {
                                                    shell.dictionary_search_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                    shell.settings_modal.vocabulary_dictionary_search_query.clear();
                                                    cx.notify();
                                                }))))))
                                    .child(div().flex().gap_1().children(std::iter::once(("all".to_string(), "All scopes".to_string(), dictionary_entries.len())).chain(scopes.iter().cloned()).map(|(scope_id, label, count)| {
                                        let selected = dictionary_scope == scope_id;
                                        Button::new(ElementId::Name(format!("dictionary-scope-{scope_id}").into()))
                                            .when(selected, |button| button.primary())
                                            .when(!selected, |button| button.ghost())
                                            .small().label(format!("{label} ({count})"))
                                            .on_click(cx.listener(move |shell, _, _, cx| {
                                                shell.settings_modal.vocabulary_dictionary_scope = scope_id.clone();
                                                cx.notify();
                                            }))
                                    })))
                                    .child(div().flex().flex_col().gap_4()
                                        .child(div().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                                            .child(div().flex().items_center().justify_between().px_4().pt_3().pb_2()
                                                .child(div().flex().flex_col().gap_1()
                                                    .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Hotwords & Vocabulary"))
                                                    .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Bias speech recognition accuracy for specific names, jargon, or keywords.")))
                                                    .child(div().text_size(px(10.0)).text_color(cx.theme().muted_foreground).child(hotword_entries.len().to_string())))
                                            .child(div().flex().flex_wrap().items_center().gap_2().px_4().pb_4()
                                                .children(hotword_entries.iter().map(|entry| dictionary_hotword_chip(entry, &dictionary_scope, editing_line, cx)))
                                                .child(div().w(px(200.0))
                                        .on_key_down(cx.listener(move |shell, event: &KeyDownEvent, window, cx| {
                                            if event.keystroke.key.eq_ignore_ascii_case("enter") {
                                                let raw = shell.dictionary_inline_hotword_state.read(cx).value().trim().to_string();
                                                submit_dictionary_entry_for_scope(shell, &raw, &inline_scope_id, window, cx);
                                                shell.dictionary_inline_hotword_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                window.prevent_default();
                                            }
                                        }))
                                        .child(Input::new(&inline_hotword_state).w_full().h(px(30.0)).aria_label("Add hotword")))))
                                        .child(div().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                                            .child(div().flex().items_center().justify_between().px_4().pt_3().pb_2()
                                                .child(div().flex().flex_col().gap_1()
                                                    .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Text Replacements & Corrections"))
                                                    .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Automatically correct misrecognitions and expand snippets upon transcription.")))
                                                .child(div().text_size(px(10.0)).text_color(cx.theme().muted_foreground).child(replacement_entries.len().to_string())))
                                            .child(div().flex().flex_col().gap_1().px_4().pb_3()
                                                .when(replacement_entries.is_empty(), |body| body.child(
                                                    div().w_full().p_4().text_center().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("No replacement rules in current scope")
                                                ))
                                                .children(replacement_entries.iter().map(|entry| dictionary_replacement_row(entry, editing_line, cx))))
                                            .child(div().mx_4().my_2().flex().flex_col().gap_2().p_3().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().background)
                                        .child(div().text_size(px(12.0)).font_weight(FontWeight::SEMIBOLD).child("Test Replacement Rules"))
                                        .child(Input::new(dictionary_test).w_full().h(px(30.0)).aria_label("Test replacement rules"))
                                        .when(!test_input.is_empty(), |panel| panel.child(div().flex().items_center().gap_2().text_size(px(12.0))
                                            .child(div().text_color(cx.theme().muted_foreground).child("Result:"))
                                            .child(div().text_color(cx.theme().foreground).child(test_output)))))))
                            }),
                    )
                })
                .when(active_tab == "recognition" && tutorial_open, |panel| {
                    panel.child(
                        section("Syntax Guide & Examples", "Dictionary terms, replacements, and project-specific scopes.", cx)
                            .child(tutorial_block(
                                "1. Terms & Hotwords",
                                "Add custom vocabulary or names to bias speech recognition accuracy.",
                                "- Sona\n- DeepSeek\n- Sherpa-onnx :2.0",
                                Some("Tip: :2.0 weight suffix is supported by sherpa-onnx transducer models."),
                                cx,
                            ))
                            .child(tutorial_block(
                                "2. Replacements & Corrections",
                                "Use -> or => to automatically correct misrecognitions and expand snippets.",
                                "- misheard phrase -> Correct phrase\n- btw => by the way",
                                None,
                                cx,
                            ))
                            .child(tutorial_block(
                                "3. Project-Specific Scoping",
                                "Define terms under projects: with (id:project_id) to apply them only to a project.",
                                "projects:\n  Voice Typing (id:voice-typing):\n    - Dictation Hotword\n  Project Name (id:proj-1):\n    - Sprint Planning\n    - PRD -> Product Doc",
                                None,
                                cx,
                            ))
                            .child(tutorial_block(
                                "4. Tips & Shortcuts",
                                "Tab inserts two-space indentation. Format realigns spacing and synchronizes renamed project links.",
                                "Diagnostics entries identify the source line and severity.",
                                Some("Select an issue in the diagnostics panel to jump directly to its line."),
                                cx,
                            )),
                    )
                })
                .when(active_tab == "prompts", |panel| {
                    panel
                        .child(div().flex().items_center().justify_between().gap_2()
                            .child(div().flex().items_center().gap_1().children([
                            ("polish", "Polish"), ("translation", "Translation"), ("summary", "Summary")
                        ].into_iter().map(|(tab, label)| {
                            let selected = active_prompt_tab == tab;
                            Button::new(ElementId::Name(format!("settings-vocabulary-prompt-{tab}").into()))
                                .when(selected, |button| button.primary()).when(!selected, |button| button.ghost())
                                .small().label(label)
                                .on_click(cx.listener(move |shell, _, window, cx| {
                                    shell.settings_modal.vocabulary_prompt_tab = tab.to_string();
                                    shell.settings_modal.vocabulary_polish_edit_id = None;
                                    shell.settings_modal.vocabulary_summary_edit_id = None;
                                    shell.vocabulary_name_state.update(cx, |state, cx| state.set_value("", window, cx));
                                    shell.vocabulary_body_state.update(cx, |state, cx| state.set_value("", window, cx));
                                    cx.notify();
                                }))
                        })))
                            .child({
                                let configured = sona_core::automation::is_feature_llm_config_complete(config, active_prompt_tab);
                                let model_name = selected_feature_model_name(config, active_prompt_tab);
                                let feature = active_prompt_tab.to_string();
                                let label = if configured { "LLM Service: Configured" } else { "LLM Service: Not Configured" };
                                Button::new("settings-vocabulary-llm-status")
                                    .when(configured, |button| button.outline())
                                    .when(!configured, |button| button.ghost())
                                    .small()
                                    .label(match model_name { Some(name) if configured => format!("{label} · {name}"), _ => label.to_string() })
                                    .tooltip(if configured { "Model configured for this feature. Open LLM Service settings." } else { "LLM service is not configured for this feature. Open LLM Service settings." })
                                    .on_click(cx.listener(move |shell, _, _, cx| {
                                        shell.settings_modal.llm_active_feature = feature.clone();
                                        shell.settings_modal.set_tab(crate::views::settings::SettingsTab::LlmService, cx);
                                    }))
                            }))
                        .when(active_prompt_tab == "polish", |panel| {
                            panel.child(
                                section("Polish Automation & Presets", "Configure automatic polishing and the default context preset.", cx)
                                    .child(toggle_row("Auto Polish", "Automatically polish text after transcription or batch import.", auto_polish, "settings-vocab-auto-polish", "autoPolish", cx))
                                    .when(auto_polish, |panel| panel.child(
                                        div().flex().items_center().justify_between().gap_3()
                                            .child(div().flex().flex_col().gap_1()
                                                .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Live Recording Polish Frequency"))
                                                .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Number of segments accumulated before background polish.")))
                                            .child(div().flex().gap_1().children([3_u64, 5, 10, 15].into_iter().map(|frequency| {
                                                let selected = config.get("autoPolishFrequency").and_then(Value::as_u64).unwrap_or(5) == frequency;
                                                Button::new(ElementId::Name(format!("settings-vocab-polish-frequency-{frequency}").into()))
                                                    .when(selected, |button| button.primary()).when(!selected, |button| button.outline())
                                                    .small().label(format!("Every {frequency}"))
                                                    .on_click(cx.listener(move |shell, _, _, cx| {
                                                        shell.settings_modal.persist_model_setting("autoPolishFrequency".into(), json!(frequency), cx);
                                                    }))
                                            })))
                                    ))
                                    .child(choice_row(
                                        "Default Polish Preset",
                                        config.get("polishPresetId").and_then(Value::as_str).unwrap_or("clean"),
                                        "polishPresetId",
                                        polish_preset_options(config),
                                        cx,
                                    )),
                            )
                            .child(section("Built-in Presets", "These presets are provided by Sona and can be viewed here, but not edited.", cx)
                                .children([
                                    ("Clean Spoken", "Removes filler words, stutters, and verbal clutter while preserving original phrasing, tone, and facts."),
                                    ("Verbatim", "Restores all audio details, preserving every filler word, stutter, and original word order. Only corrects ASR errors and punctuation."),
                                    ("Formal Written", "Restructures fragmented speech into grammatically standard, well-organized written text. Refines syntax without fabricating facts."),
                                ].into_iter().map(|(name, context)| preset_detail(name, context, cx))))
                            .child(custom_records_section(
                                config,
                                "Custom Presets",
                                "Create reusable name + context pairs and switch between them from text polish or project settings.",
                                "polishCustomPresets",
                                "context",
                                "polishPresetId",
                                "clean",
                                "polishPreset",
                                EditorKind::Polish,
                                polish_edit_id,
                                vocabulary_name_state,
                                vocabulary_body_state,
                                cx,
                            ))
                        })
                        .when(active_prompt_tab == "translation", |panel| {
                            panel.child(section("Translation Automation", "Configure automatic translation and the default target language for transcripts.", cx)
                                .child(toggle_row("Auto Translate", "Automatically translate a transcript after transcription completes.", auto_translate, "settings-vocab-auto-translate", "autoTranslate", cx))
                                .child(div().flex().items_center().justify_between().gap_3()
                                    .child(div().flex().flex_col().gap_1()
                                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Default Target Language"))
                                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Default target language used when initiating transcript translation.")))
                                    .child(Select::new(translation_language_select).w(px(280.0)).menu_width(px(280.0)))))
                        })
                        .when(active_prompt_tab == "summary", |panel| {
                            panel.child(section("Summary Automation & Templates", "Configure automatic summarization and the default summary template.", cx)
                                .child(toggle_row("Auto Summarize", "Automatically generate a summary after transcription and polishing.", auto_summary, "settings-vocab-auto-summary", "autoSummary", cx))
                                .child(choice_row(
                                    "Default Summary Template",
                                    config.get("summaryTemplateId").and_then(Value::as_str).unwrap_or("general"),
                                    "summaryTemplateId",
                                    summary_template_options(config),
                                    cx,
                                )))
                                .child(section("Built-in Summary Templates", "These templates are provided by Sona and can be viewed here, but not edited.", cx)
                                    .children([
                                        ("General", "1. A short overview paragraph.\n2. A concise list of key points.\n3. Follow-up items or next steps only if they are supported by the transcript."),
                                        ("Meeting", "1. Meeting overview.\n2. Decisions made.\n3. Action items with owners when the transcript names them.\n4. Open questions, blockers, or risks."),
                                        ("Lecture", "1. Lecture overview.\n2. Core concepts or arguments.\n3. Important examples, evidence, or explanations.\n4. Review points or next steps for study."),
                                    ].into_iter().map(|(name, instructions)| preset_detail(name, instructions, cx))))
                                .child(custom_records_section(
                                    config,
                                    "Custom Summary Templates",
                                    "Create reusable summary template names and instructions for different workflows.",
                                    "summaryCustomTemplates",
                                    "instructions",
                                    "summaryTemplateId",
                                    "general",
                                    "summaryTemplate",
                                    EditorKind::Summary,
                                    summary_edit_id,
                                    vocabulary_name_state,
                                    vocabulary_body_state,
                                    cx,
                                ))
                        })
                })
                .when(active_tab == "speakers", |panel| {
                    let (name_state, search_state) = {
                        let shell = cx.entity().read(cx);
                        (
                            shell.speaker_profile_name_state.clone(),
                            shell.speaker_profile_search_state.clone(),
                        )
                    };
                    panel.child(super::speaker_profiles::speaker_profiles_panel(
                        config,
                        &name_state,
                        &search_state,
                        cx,
                    ))
                }),
        )
        .when(dialog_open, |panel| {
            panel.child(
                div()
                    .id("dictionary-entry-overlay")
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(hsla(0.0, 0.0, 0.0, 0.55))
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.settings_modal.vocabulary_dictionary_dialog_open = false;
                        shell.settings_modal.vocabulary_dictionary_edit_line = None;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id("dictionary-entry-dialog")
                            .flex()
                            .flex_col()
                            .w(px(480.0))
                            .max_h(px(620.0))
                            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            .on_key_down(cx.listener(|shell, event: &KeyDownEvent, window, cx| {
                                if event.keystroke.key.eq_ignore_ascii_case("enter") {
                                    let input_focused = shell.dictionary_word_state.read(cx).focus_handle(cx).is_focused(window)
                                        || shell.dictionary_weight_state.read(cx).focus_handle(cx).is_focused(window)
                                        || shell.dictionary_from_state.read(cx).focus_handle(cx).is_focused(window)
                                        || shell.dictionary_to_state.read(cx).focus_handle(cx).is_focused(window);
                                    if input_focused {
                                        submit_dictionary_dialog_entry(shell, window, cx);
                                        window.prevent_default();
                                        cx.stop_propagation();
                                    }
                                }
                            }))
                            .overflow_y_scrollbar()
                            .gap_4()
                            .p_5()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .shadow_xl()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(div().text_size(px(17.0)).font_weight(FontWeight::SEMIBOLD).child(if editing_line.is_some() { "Edit Vocabulary Entry" } else { "New Vocabulary Entry" }))
                                    .child(Button::new("dictionary-dialog-close").ghost().small().label("×")
                                        .tooltip("Close")
                                        .on_click(cx.listener(|shell, _, _, cx| {
                                            shell.settings_modal.vocabulary_dictionary_dialog_open = false;
                                            shell.settings_modal.vocabulary_dictionary_edit_line = None;
                                            cx.notify();
                                        }))),
                            )
                            .child(
                                div().flex().flex_col().gap_2()
                                    .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Entry Type"))
                                    .child(div().flex().gap_2()
                                        .child(Button::new("dictionary-dialog-hotword")
                                            .when(dialog_type == "hotword", |button| button.primary())
                                            .when(dialog_type != "hotword", |button| button.outline())
                                            .small().label("Hotword / Term")
                                            .on_click(cx.listener(|shell, _, window, cx| {
                                                shell.settings_modal.vocabulary_dictionary_dialog_type = "hotword".to_string();
                                                shell.dictionary_word_state.update(cx, |state, cx| state.focus(window, cx));
                                                cx.notify();
                                            })))
                                        .child(Button::new("dictionary-dialog-replacement")
                                            .when(dialog_type == "replacement", |button| button.primary())
                                            .when(dialog_type != "replacement", |button| button.outline())
                                            .small().label("Replacement Rule")
                                            .on_click(cx.listener(|shell, _, window, cx| {
                                                shell.settings_modal.vocabulary_dictionary_dialog_type = "replacement".to_string();
                                                shell.dictionary_from_state.update(cx, |state, cx| state.focus(window, cx));
                                                cx.notify();
                                            })))),
                            )
                            .child(
                                div().flex().flex_col().gap_2()
                                    .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Target Scope"))
                                    .child(div().flex().flex_wrap().gap_1().max_h(px(112.0)).overflow_y_scrollbar()
                                        .children(scopes.iter().map(|(scope_id, label, _)| {
                                            let selected = add_scope == *scope_id;
                                            let selected_scope_id = scope_id.clone();
                                            Button::new(ElementId::Name(format!("dictionary-dialog-scope-{scope_id}").into()))
                                                .when(selected, |button| button.primary())
                                                .when(!selected, |button| button.outline())
                                                .small().label(label.clone())
                                                .on_click(cx.listener(move |shell, _, _, cx| {
                                                    shell.settings_modal.vocabulary_dictionary_add_scope = selected_scope_id.clone();
                                                    cx.notify();
                                                }))
                                        }))),
                            )
                            .when(dialog_type == "hotword", |dialog| {
                                dialog
                                    .child(div().flex().flex_col().gap_2()
                                        .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Term / Hotword"))
                                        .child(Input::new(&word_state).w_full().h(px(36.0)).aria_label("Term or hotword")))
                                    .child(div().flex().flex_col().gap_2()
                                        .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Weight Boost (Optional)"))
                                        .child(Input::new(&weight_state).w_full().h(px(36.0)).aria_label("Weight boost"))
                                        .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child("Weight suffixes are supported by sherpa-onnx transducer models.")))
                            })
                            .when(dialog_type == "replacement", |dialog| {
                                dialog
                                    .child(div().flex().flex_col().gap_2()
                                        .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Original Text (From)"))
                                        .child(Input::new(&from_state).w_full().h(px(36.0)).aria_label("Original text")))
                                    .child(div().flex().flex_col().gap_2()
                                        .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Target Replacement (To)"))
                                        .child(Input::new(&to_state).w_full().h(px(36.0)).aria_label("Target replacement")))
                            })
                            .child(
                                div().flex().justify_end().gap_2().pt_2().border_t_1().border_color(cx.theme().border)
                                    .child(Button::new("dictionary-dialog-cancel").ghost().small().label("Cancel")
                                        .on_click(cx.listener(|shell, _, _, cx| {
                                            shell.settings_modal.vocabulary_dictionary_dialog_open = false;
                                            shell.settings_modal.vocabulary_dictionary_edit_line = None;
                                            cx.notify();
                                        })))
                                    .child(Button::new("dictionary-dialog-save").primary().small().label("Save Entry")
                                        .disabled(!dialog_can_save)
                                        .on_click(cx.listener(|shell, _, window, cx| submit_dictionary_dialog_entry(shell, window, cx)))),
                            ),
                    ),
            )
        })
        .into_any_element()
}

fn dictionary_edit_button(
    entry: &DictionaryEntry,
    editing_line: Option<usize>,
    cx: &mut Context<MainShell>,
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
    .on_click(cx.listener(move |shell, _, window, cx| {
        shell.settings_modal.vocabulary_dictionary_edit_line = Some(line);
        shell.settings_modal.vocabulary_dictionary_add_scope = scope_id.clone();
        let is_replacement = raw.contains("->") || raw.contains("=>");
        shell.settings_modal.vocabulary_dictionary_dialog_open = true;
        shell.settings_modal.vocabulary_dictionary_dialog_type = if is_replacement {
            "replacement"
        } else {
            "hotword"
        }
        .to_string();
        if let Some((from, to)) = raw.split_once("->").or_else(|| raw.split_once("=>")) {
            shell.dictionary_from_state.update(cx, |state, cx| {
                state.set_value(from.trim(), window, cx);
                state.focus(window, cx);
            });
            shell
                .dictionary_to_state
                .update(cx, |state, cx| state.set_value(to.trim(), window, cx));
            shell
                .dictionary_word_state
                .update(cx, |state, cx| state.set_value("", window, cx));
            shell
                .dictionary_weight_state
                .update(cx, |state, cx| state.set_value("", window, cx));
        } else {
            let (word, weight) = extract_hotword_weight(&raw);
            shell.dictionary_word_state.update(cx, |state, cx| {
                state.set_value(word, window, cx);
                state.focus(window, cx);
            });
            shell.dictionary_weight_state.update(cx, |state, cx| {
                state.set_value(
                    weight
                        .map(|weight| format!("{weight:.1}"))
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            shell
                .dictionary_from_state
                .update(cx, |state, cx| state.set_value("", window, cx));
            shell
                .dictionary_to_state
                .update(cx, |state, cx| state.set_value("", window, cx));
        }
        cx.notify();
    }))
}

fn dictionary_delete_button(entry: &DictionaryEntry, cx: &mut Context<MainShell>) -> Button {
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
    .on_click(cx.listener(move |shell, _, window, cx| {
        let current = shell.dictionary_textarea_state.read(cx).value().to_string();
        let updated = remove_dictionary_line(&current, line);
        let (hotwords, replacements) = legacy_dictionary_sets(&updated);
        shell
            .dictionary_textarea_state
            .update(cx, |state, cx| state.set_value(updated.clone(), window, cx));
        shell.settings_modal.persist_model_settings(
            vec![
                ("dictionaryContent".into(), Value::String(updated)),
                ("hotwordSets".into(), hotwords),
                ("textReplacementSets".into(), replacements),
            ],
            cx,
        );
        if shell.settings_modal.vocabulary_dictionary_edit_line == Some(line) {
            shell.settings_modal.vocabulary_dictionary_edit_line = None;
            shell.settings_modal.vocabulary_dictionary_dialog_open = false;
        }
        cx.notify();
    }))
}

fn dictionary_hotword_chip(
    entry: &DictionaryEntry,
    dictionary_scope: &str,
    editing_line: Option<usize>,
    cx: &mut Context<MainShell>,
) -> gpui_kit::Div {
    let (word, weight) = extract_hotword_weight(&entry.raw);
    div()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_full()
        .border_1()
        .border_color(cx.theme().border.opacity(0.7))
        .bg(cx.theme().secondary.opacity(0.35))
        .child(div().text_size(px(12.0)).child(word))
        .when_some(weight, |chip, weight| {
            chip.child(
                div()
                    .text_size(px(10.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("x{weight:.1}")),
            )
        })
        .when(dictionary_scope == "all", |chip| {
            chip.child(
                div()
                    .text_size(px(10.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(entry.scope_name.clone()),
            )
        })
        .child(dictionary_edit_button(entry, editing_line, cx))
        .child(dictionary_delete_button(entry, cx))
}

fn dictionary_replacement_row(
    entry: &DictionaryEntry,
    editing_line: Option<usize>,
    cx: &mut Context<MainShell>,
) -> gpui_kit::Div {
    let (from, to) = entry
        .raw
        .split_once("->")
        .or_else(|| entry.raw.split_once("=>"))
        .unwrap_or((entry.raw.as_str(), ""));
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(div().text_size(px(12.0)).child(from.trim().to_string()))
                .child(LucideIcon::ArrowRight)
                .child(div().text_size(px(12.0)).child(to.trim().to_string())),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(entry.scope_name.clone()),
                )
                .child(dictionary_edit_button(entry, editing_line, cx))
                .child(dictionary_delete_button(entry, cx)),
        )
}

fn code_stat(label: &'static str, value: usize, cx: &Context<MainShell>) -> gpui_kit::Div {
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

fn dictionary_line_range(content: &str, line_number: usize) -> std::ops::Range<usize> {
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

fn dictionary_scope_suggestions(
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

fn select_dictionary_scope_suggestion(
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

fn section(
    title: &'static str,
    description: &'static str,
    cx: &Context<MainShell>,
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

fn tutorial_block(
    title: &'static str,
    description: &'static str,
    example: &'static str,
    hint: Option<&'static str>,
    cx: &Context<MainShell>,
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

pub(crate) fn effective_dictionary_content(config: &Value) -> String {
    if let Some(content) = config
        .get("dictionaryContent")
        .and_then(Value::as_str)
        .filter(|content| !content.trim().is_empty())
    {
        return content.to_string();
    }

    let mut lines = vec!["# Global Vocabulary & Replacements".to_string()];
    let mut hotwords = std::collections::HashSet::new();
    if let Some(sets) = config.get("hotwordSets").and_then(Value::as_array) {
        for rule in sets
            .iter()
            .filter_map(|set| set.get("rules").and_then(Value::as_array))
            .flatten()
        {
            if let Some(text) = rule.get("text").and_then(Value::as_str) {
                let text = text.trim();
                if !text.is_empty() && hotwords.insert(text.to_string()) {
                    lines.push(format!("- {text}"));
                }
            }
        }
    }

    if let Some(sets) = config.get("textReplacementSets").and_then(Value::as_array) {
        for rule in sets
            .iter()
            .filter_map(|set| set.get("rules").and_then(Value::as_array))
            .flatten()
        {
            let from = rule
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            let to = rule
                .get("to")
                .and_then(Value::as_str)
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct DictionaryEntry {
    line: usize,
    scope_id: String,
    scope_name: String,
    label: String,
    raw: String,
}

fn parse_dictionary_entries(content: &str) -> Vec<DictionaryEntry> {
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

fn extract_hotword_weight(text: &str) -> (String, Option<f64>) {
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

fn format_dictionary_content(content: &str, known_projects: &[(String, String)]) -> String {
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

fn format_dictionary_term(raw: &str) -> String {
    raw.split_once("->")
        .or_else(|| raw.split_once("=>"))
        .map(|(from, to)| format!("{} -> {}", from.trim(), to.trim()))
        .unwrap_or_else(|| raw.trim().to_string())
}

fn parse_dictionary_scopes(content: &str) -> Vec<(String, String)> {
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

fn validate_dictionary_content(
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

pub(crate) fn legacy_dictionary_sets(content: &str) -> (Value, Value) {
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

fn parse_dictionary_scope(header: &str) -> Option<(String, String)> {
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

fn remove_dictionary_line(content: &str, line: usize) -> String {
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

fn add_global_dictionary_entry(content: &str, value: &str) -> String {
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

fn add_dictionary_entry_to_scope(
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

pub(crate) fn submit_dictionary_entry(
    shell: &mut MainShell,
    value: &str,
    window: &mut Window,
    cx: &mut Context<MainShell>,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let current = shell.dictionary_textarea_state.read(cx).value().to_string();
    let target_scope_id = shell.settings_modal.vocabulary_dictionary_add_scope.clone();
    let target_scope_name = if target_scope_id == "voice-typing" {
        "Voice Typing".to_string()
    } else {
        shell
            .project_selector
            .projects
            .iter()
            .find(|project| project.id == target_scope_id)
            .map(|project| project.name.clone())
            .unwrap_or_else(|| target_scope_id.clone())
    };
    let updated = match shell.settings_modal.vocabulary_dictionary_edit_line {
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
    shell
        .dictionary_textarea_state
        .update(cx, |state, cx| state.set_value(updated.clone(), window, cx));
    shell.settings_modal.persist_model_settings(
        vec![
            ("dictionaryContent".into(), Value::String(updated)),
            ("hotwordSets".into(), hotwords),
            ("textReplacementSets".into(), replacements),
        ],
        cx,
    );
    shell
        .dictionary_quick_add_state
        .update(cx, |state, cx| state.set_value("", window, cx));
    shell.settings_modal.vocabulary_dictionary_edit_line = None;
    shell.settings_modal.vocabulary_dictionary_dialog_open = false;
    cx.notify();
}

fn submit_dictionary_entry_for_scope(
    shell: &mut MainShell,
    value: &str,
    scope_id: &str,
    window: &mut Window,
    cx: &mut Context<MainShell>,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let scope_name = if scope_id == "voice-typing" {
        "Voice Typing".to_string()
    } else {
        shell
            .project_selector
            .projects
            .iter()
            .find(|project| project.id == scope_id)
            .map(|project| project.name.clone())
            .unwrap_or_else(|| scope_id.to_string())
    };
    let current = shell.dictionary_textarea_state.read(cx).value().to_string();
    let updated = add_dictionary_entry_to_scope(&current, value, scope_id, &scope_name);
    let (hotwords, replacements) = legacy_dictionary_sets(&updated);
    shell
        .dictionary_textarea_state
        .update(cx, |state, cx| state.set_value(updated.clone(), window, cx));
    shell.settings_modal.persist_model_settings(
        vec![
            ("dictionaryContent".into(), Value::String(updated)),
            ("hotwordSets".into(), hotwords),
            ("textReplacementSets".into(), replacements),
        ],
        cx,
    );
    cx.notify();
}

fn submit_dictionary_dialog_entry(
    shell: &mut MainShell,
    window: &mut Window,
    cx: &mut Context<MainShell>,
) {
    let value = if shell.settings_modal.vocabulary_dictionary_dialog_type == "hotword" {
        let word = shell
            .dictionary_word_state
            .read(cx)
            .value()
            .trim()
            .to_string();
        if word.is_empty() {
            return;
        }
        let weight = shell
            .dictionary_weight_state
            .read(cx)
            .value()
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|weight| *weight > 0.0);
        weight.map_or(word.clone(), |weight| format!("{word} :{weight:.1}"))
    } else {
        let from = shell
            .dictionary_from_state
            .read(cx)
            .value()
            .trim()
            .to_string();
        let to = shell
            .dictionary_to_state
            .read(cx)
            .value()
            .trim()
            .to_string();
        if from.is_empty() || to.is_empty() {
            return;
        }
        format!("{from} -> {to}")
    };
    shell
        .dictionary_quick_add_state
        .update(cx, |state, cx| state.set_value(value.clone(), window, cx));
    submit_dictionary_entry(shell, &value, window, cx);
    shell
        .dictionary_word_state
        .update(cx, |state, cx| state.set_value("", window, cx));
    shell
        .dictionary_weight_state
        .update(cx, |state, cx| state.set_value("", window, cx));
    shell
        .dictionary_from_state
        .update(cx, |state, cx| state.set_value("", window, cx));
    shell
        .dictionary_to_state
        .update(cx, |state, cx| state.set_value("", window, cx));
}

fn update_dictionary_line(content: &str, line: usize, value: &str) -> String {
    if line == 0 || value.trim().is_empty() {
        return content.to_string();
    }
    let mut lines = content
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let Some(source) = lines.get(line - 1) else {
        return content.to_string();
    };
    let indentation = source
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect::<String>();
    lines[line - 1] = format!("{indentation}- {}", value.trim().trim_start_matches("- "));
    lines.join("\n")
}

fn toggle_row(
    title: &'static str,
    hint: &'static str,
    enabled: bool,
    id: &'static str,
    key: &'static str,
    cx: &Context<MainShell>,
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
                .on_change(cx.listener(move |shell, value, _, cx| {
                    shell.settings_modal.persist_model_setting(
                        key.to_string(),
                        Value::Bool(*value),
                        cx,
                    );
                })),
        )
}

fn choice_row(
    title: &'static str,
    selected: &str,
    key: &'static str,
    options: Vec<(String, String)>,
    cx: &Context<MainShell>,
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
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        shell.settings_modal.persist_model_setting(
                            key.to_string(),
                            Value::String(id.clone()),
                            cx,
                        );
                    }))
                })),
        )
}

#[derive(Clone, Copy)]
enum EditorKind {
    Polish,
    Summary,
}

fn custom_records_section(
    config: &Value,
    title: &'static str,
    description: &'static str,
    records_key: &'static str,
    body_key: &'static str,
    default_key: &'static str,
    fallback_id: &'static str,
    dependency_kind: &'static str,
    kind: EditorKind,
    editing_id: Option<&str>,
    name_state: &Entity<InputState>,
    body_state: &Entity<TextareaState>,
    cx: &mut Context<MainShell>,
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
                            .on_click(cx.listener(move |shell, _, window, cx| {
                                let name = shell
                                    .vocabulary_name_state
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .to_string();
                                let body = shell
                                    .vocabulary_body_state
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .to_string();
                                if name.is_empty() || body.is_empty() {
                                    return;
                                }
                                let current_edit_id = match kind {
                                    EditorKind::Polish => {
                                        shell.settings_modal.vocabulary_polish_edit_id.clone()
                                    }
                                    EditorKind::Summary => {
                                        shell.settings_modal.vocabulary_summary_edit_id.clone()
                                    }
                                };
                                let mut next_records = shell
                                    .settings_modal
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
                                shell.settings_modal.persist_model_setting(
                                    records_key.to_string(),
                                    Value::Array(next_records),
                                    cx,
                                );
                                shell.settings_modal.vocabulary_polish_edit_id = None;
                                shell.settings_modal.vocabulary_summary_edit_id = None;
                                shell
                                    .vocabulary_name_state
                                    .update(cx, |state, cx| state.set_value("", window, cx));
                                shell
                                    .vocabulary_body_state
                                    .update(cx, |state, cx| state.set_value("", window, cx));
                            })),
                    ),
                ),
        )
        .when(records.is_empty(), |section| {
            section.child(
                div()
                    .px_4()
                    .py_3()
                    .bg(cx.theme().background)
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(match kind {
                        EditorKind::Polish => "No custom presets yet.",
                        EditorKind::Summary => "No custom summary templates yet.",
                    }),
            )
        })
        .children(records.into_iter().map(|(id, name, body)| {
            let active_edit = editing_id.as_deref() == Some(id.as_str());
            let edit_id = id.clone();
            let edit_name = name.clone();
            let edit_body = body.clone();
            let delete_id = id.clone();
            let name_state = name_state.clone();
            let body_state = body_state.clone();
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_3()
                .bg(cx.theme().background)
                .border_t_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .min_w_0()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("settings-vocabulary-edit-{records_key}-{id}")
                                            .into(),
                                    ))
                                    .outline()
                                    .small()
                                    .label(if active_edit { "Editing" } else { "Edit" })
                                    .on_click(cx.listener(
                                        move |shell, _, window, cx| {
                                            match kind {
                                                EditorKind::Polish => {
                                                    shell.settings_modal.vocabulary_polish_edit_id =
                                                        Some(edit_id.clone())
                                                }
                                                EditorKind::Summary => {
                                                    shell
                                                        .settings_modal
                                                        .vocabulary_summary_edit_id =
                                                        Some(edit_id.clone())
                                                }
                                            }
                                            name_state.update(cx, |state, cx| {
                                                state.set_value(edit_name.clone(), window, cx)
                                            });
                                            body_state.update(cx, |state, cx| {
                                                state.set_value(edit_body.clone(), window, cx)
                                            });
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    Button::new(ElementId::Name(
                                        format!("settings-vocabulary-delete-{records_key}-{id}")
                                            .into(),
                                    ))
                                    .danger()
                                    .small()
                                    .label("Delete")
                                    .on_click(cx.listener(
                                        move |shell, _, window, cx| {
                                            let mut next_records = shell
                                                .settings_modal
                                                .model_config
                                                .get(records_key)
                                                .and_then(Value::as_array)
                                                .cloned()
                                                .unwrap_or_default();
                                            next_records.retain(|record| {
                                                record.get("id").and_then(Value::as_str)
                                                    != Some(delete_id.as_str())
                                            });
                                            let mut settings = vec![(
                                                records_key.to_string(),
                                                Value::Array(next_records),
                                            )];
                                            if shell
                                                .settings_modal
                                                .model_config
                                                .get(default_key)
                                                .and_then(Value::as_str)
                                                == Some(delete_id.as_str())
                                            {
                                                settings.push((
                                                    default_key.to_string(),
                                                    Value::String(fallback_id.to_string()),
                                                ));
                                            }
                                            shell
                                                .settings_modal
                                                .persist_model_settings(settings, cx);
                                            let is_current_editor = match kind {
                                                EditorKind::Polish => {
                                                    shell
                                                        .settings_modal
                                                        .vocabulary_polish_edit_id
                                                        .as_deref()
                                                        == Some(delete_id.as_str())
                                                }
                                                EditorKind::Summary => {
                                                    shell
                                                        .settings_modal
                                                        .vocabulary_summary_edit_id
                                                        .as_deref()
                                                        == Some(delete_id.as_str())
                                                }
                                            };
                                            if is_current_editor {
                                                shell.settings_modal.vocabulary_polish_edit_id =
                                                    None;
                                                shell.settings_modal.vocabulary_summary_edit_id =
                                                    None;
                                                shell
                                                    .vocabulary_name_state
                                                    .update(cx, |state, cx| {
                                                        state.set_value("", window, cx)
                                                    });
                                                shell
                                                    .vocabulary_body_state
                                                    .update(cx, |state, cx| {
                                                        state.set_value("", window, cx)
                                                    });
                                            }
                                            shell
                                                .settings_modal
                                                .remove_automation_profile_dependency(
                                                    dependency_kind,
                                                    delete_id.clone(),
                                                    cx,
                                                );
                                            cx.notify();
                                        },
                                    )),
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

fn polish_preset_options(config: &Value) -> Vec<(String, String)> {
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

fn summary_template_options(config: &Value) -> Vec<(String, String)> {
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

fn selected_feature_model_name(config: &Value, feature: &str) -> Option<String> {
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

fn preset_detail(
    title: &'static str,
    details: &'static str,
    cx: &Context<MainShell>,
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

#[cfg(test)]
mod tests {
    use super::{
        add_dictionary_entry_to_scope, add_global_dictionary_entry, dictionary_line_range,
        dictionary_scope_suggestions, effective_dictionary_content, extract_hotword_weight,
        format_dictionary_content, legacy_dictionary_sets, parse_dictionary_entries,
        polish_preset_options, remove_dictionary_line, select_dictionary_scope_suggestion,
        summary_template_options, update_dictionary_line, validate_dictionary_content,
    };
    use serde_json::json;

    #[test]
    fn uses_unified_content_before_legacy_dictionary_sets() {
        let config = json!({
            "dictionaryContent": "- Explicit\n",
            "hotwordSets": [{"rules": [{"text": "Legacy"}]}]
        });

        assert_eq!(effective_dictionary_content(&config), "- Explicit\n");
    }

    #[test]
    fn serializes_legacy_dictionary_sets_when_unified_content_is_empty() {
        let config = json!({
            "dictionaryContent": "  ",
            "hotwordSets": [{"rules": [{"text": "Sona"}, {"text": " Sona "}]}],
            "textReplacementSets": [{"rules": [{"from": "sona ai", "to": "Sona"}]}]
        });

        assert_eq!(
            effective_dictionary_content(&config),
            "# Global Vocabulary & Replacements\n- Sona\n- sona ai -> Sona"
        );
    }

    #[test]
    fn parses_global_hotwords_replacements_and_scoped_entries() {
        let entries = parse_dictionary_entries(
            "# comment\n- Sona :2.0\n- Sona AI -> Sona\nprojects:\n  Design (id:project-1):\n    - GPUI\n",
        );

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].scope_id, "global");
        assert_eq!(entries[0].label, "Sona :2.0");
        assert_eq!(entries[1].label, "Sona AI -> Sona");
        assert_eq!(entries[2].scope_id, "project-1");
        assert_eq!(entries[2].scope_name, "Design");
    }

    #[test]
    fn legacy_dictionary_sync_keeps_global_terms_and_excludes_project_terms() {
        let (hotwords, replacements) = legacy_dictionary_sets(
            "- Sona :2.0\n- Sona AI => Sona\nprojects:\n  Design (id:project-1):\n    - GPUI\n",
        );
        assert_eq!(hotwords[0]["rules"][0]["text"], "Sona :2.0");
        assert_eq!(hotwords[0]["rules"].as_array().unwrap().len(), 1);
        assert_eq!(replacements[0]["rules"][0]["from"], "Sona AI");
        assert_eq!(replacements[0]["rules"][0]["to"], "Sona");
    }

    #[test]
    fn removes_only_the_selected_source_line() {
        let content = "# keep\n- first\n- second\nprojects:\n";
        assert_eq!(
            remove_dictionary_line(content, 3),
            "# keep\n- first\nprojects:\n"
        );
    }

    #[test]
    fn quick_add_inserts_global_term_and_avoids_duplicates() {
        let content = "- Sona";
        assert_eq!(
            add_global_dictionary_entry(content, "Kubernetes"),
            "- Sona\n- Kubernetes"
        );
        assert_eq!(add_global_dictionary_entry(content, "Sona"), content);
    }

    #[test]
    fn quick_add_targets_existing_and_new_project_scopes() {
        let content = "- Global\nprojects:\n  Design (id:project-1):\n    - Existing";
        let updated = add_dictionary_entry_to_scope(content, "New Term", "project-1", "Design");
        assert!(updated.contains("  Design (id:project-1):\n    - New Term\n    - Existing"));
        assert_eq!(
            add_dictionary_entry_to_scope(&updated, "new term", "project-1", "Design"),
            updated
        );

        let added = add_dictionary_entry_to_scope(&updated, "Scoped", "project-2", "Research");
        assert!(added.ends_with("  Research (id:project-2):\n    - Scoped"));
    }

    #[test]
    fn editing_dictionary_line_preserves_scope_indentation() {
        let content = "projects:\n  Design (id:project-1):\n    - GPUI";
        assert_eq!(
            update_dictionary_line(content, 3, "Sona -> Sona AI"),
            "projects:\n  Design (id:project-1):\n    - Sona -> Sona AI"
        );
    }

    #[test]
    fn extracts_hotword_weight_without_losing_colons_in_the_term() {
        assert_eq!(
            extract_hotword_weight("Namespace::Type :2.5"),
            ("Namespace::Type".to_string(), Some(2.5))
        );
        assert_eq!(
            extract_hotword_weight("Term :invalid"),
            ("Term :invalid".to_string(), None)
        );
        assert_eq!(extract_hotword_weight(" :2.0"), (":2.0".to_string(), None));
    }

    #[test]
    fn moving_an_edited_entry_replaces_its_old_scope_and_prevents_duplicates() {
        let original = "- Global\nprojects:\n  Design (id:project-1):\n    - GPUI\n";
        let moved = remove_dictionary_line(original, 4);
        let moved =
            add_dictionary_entry_to_scope(&moved, "Sona -> Sona AI", "project-2", "Research");

        assert!(!moved.contains("    - GPUI"));
        assert!(moved.contains("  Research (id:project-2):\n    - Sona -> Sona AI"));
        assert_eq!(
            add_dictionary_entry_to_scope(&moved, "sona -> sona ai", "project-2", "Research"),
            moved
        );
    }

    #[test]
    fn formatting_canonicalizes_arrows_and_renamed_project_links() {
        let content =
            "-  misheard => corrected  \nprojects:\n  Old Name (id:project-1):\n    -  GPUI  ";
        assert_eq!(
            format_dictionary_content(content, &[("project-1".into(), "Renamed".into())]),
            "# Global Vocabulary & Replacements\n- misheard -> corrected\n\n# Project Specific Terms & Replacements\nprojects:\n  Renamed (id:project-1):\n    - GPUI"
        );
    }

    #[test]
    fn formatting_preserves_empty_scopes_and_links_names_to_projects() {
        assert_eq!(
            format_dictionary_content(
                "projects:\n  Design:",
                &[("project-1".into(), "Design".into())],
            ),
            "# Global Vocabulary & Replacements\n# - ExampleTerm\n# - MisrecognizedWord -> CorrectWord\n\n# Project Specific Terms & Replacements\nprojects:\n  Design (id:project-1):\n    # - Term"
        );
    }

    #[test]
    fn validates_empty_terms_replacement_sides_and_unknown_project_links() {
        let issues = validate_dictionary_content(
            "- \n- -> target\nprojects:\n  Missing (id:missing):\n    - valid",
            &[],
        );
        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].1, "error");
        assert_eq!(issues[1].1, "error");
        assert_eq!(issues[2].1, "warning");
    }

    #[test]
    fn editor_line_range_selects_the_requested_utf8_line() {
        let content = "- Global\nprojects:\n  設計 (id:design):\n    - Sona\n";
        let range = dictionary_line_range(content, 3);
        assert_eq!(&content[range], "  設計 (id:design):");
    }

    #[test]
    fn project_suggestions_follow_the_cursor_prefix_inside_projects_only() {
        let projects = vec![("proj-1".to_string(), "Design Tools".to_string())];
        let content = "- Global\nprojects:\n  des and ignored suffix";
        assert_eq!(
            dictionary_scope_suggestions(content, 2, 5, &projects),
            vec![("proj-1".to_string(), "Design Tools".to_string())]
        );
        assert!(dictionary_scope_suggestions("- Global\n  des", 1, 5, &projects).is_empty());
        assert!(dictionary_scope_suggestions("projects:\n  - des", 1, 6, &projects).is_empty());
    }

    #[test]
    fn selecting_a_project_suggestion_adds_a_scoped_term_line() {
        let content = "- Global\nprojects:\n  Design\n  Next Scope";
        assert_eq!(
            select_dictionary_scope_suggestion(content, 2, "proj-1", "Design"),
            "- Global\nprojects:\n  Design (id:proj-1):\n    - \n  Next Scope"
        );
    }

    #[test]
    fn default_options_include_builtin_and_valid_custom_entries() {
        let config = json!({
            "polishCustomPresets": [
                {"id": "work", "name": "Work", "context": "Keep terminology"},
                {"id": "", "name": "Ignored", "context": "Invalid id"},
                {"id": "missing-name", "context": "Invalid name"}
            ],
            "summaryCustomTemplates": [
                {"id": "brief", "name": "Brief", "instructions": "Summarize briefly"},
                {"id": "", "name": "Ignored", "instructions": "Invalid id"}
            ]
        });

        let polish = polish_preset_options(&config);
        assert_eq!(polish.len(), 4);
        assert_eq!(polish[0], ("clean".to_string(), "Clean Spoken".to_string()));
        assert_eq!(polish[3], ("work".to_string(), "Work".to_string()));

        let summary = summary_template_options(&config);
        assert_eq!(summary.len(), 4);
        assert_eq!(summary[0], ("general".to_string(), "General".to_string()));
        assert_eq!(summary[3], ("brief".to_string(), "Brief".to_string()));
    }
}
