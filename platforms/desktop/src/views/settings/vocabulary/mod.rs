pub mod hotwords;
pub mod tester;
pub mod words;

pub use hotwords::*;
pub use tester::*;
pub use words::*;

use gpui_kit::component::IndexPath;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, StatefulInteractiveElement as _, *};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use super::SettingsModal;
use super::languages::LANGUAGE_OPTIONS;

pub struct VocabularySettingsTab {
    pub dictionary_textarea: Entity<TextareaState>,
    pub dictionary_quick_add: Entity<InputState>,
    pub dictionary_search: Entity<InputState>,
    pub dictionary_test: Entity<InputState>,
    pub word_state: Entity<InputState>,
    pub weight_state: Entity<InputState>,
    pub from_state: Entity<InputState>,
    pub to_state: Entity<InputState>,
    pub inline_hotword_state: Entity<InputState>,
    pub vocabulary_name_state: Entity<InputState>,
    pub vocabulary_body_state: Entity<TextareaState>,
    pub speaker_profile_name_state: Entity<InputState>,
    pub speaker_profile_search_state: Entity<InputState>,
    pub translation_language_select: Entity<SelectState<Vec<&'static str>>>,
    pub speaker_profile_name_states: HashMap<String, (Entity<InputState>, Subscription)>,
    pub active_tab: String,
    pub active_prompt_tab: String,
    pub auto_translate: bool,
    pub dictionary_mode: String,
    pub dictionary_scope: String,
    pub search_query: String,
    pub test_input: String,
    pub edit_line: Option<usize>,
    pub tutorial_open: bool,
    pub add_scope: String,
    pub scope_menu_open: bool,
    pub dialog_open: bool,
    pub dialog_type: String,
    pub polish_edit_id: Option<String>,
    pub summary_edit_id: Option<String>,
    pub speaker_scope_filter: String,
    pub speaker_new_scope: String,
    pub speaker_new_scope_open: bool,
    pub speaker_expanded_ids: HashSet<String>,
    pub speaker_edit_scope: Option<String>,
    pub speaker_scope_modal_type: String,
    pub speaker_scope_project_ids: Vec<String>,
    pub speaker_import_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl VocabularySettingsTab {
    pub fn new(
        initial_config: &Value,
        window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> Self {
        let initial_content = effective_dictionary_content(initial_config);
        let dictionary_textarea =
            cx.new(|cx| TextareaState::new(window, cx).default_value(initial_content));
        let dictionary_quick_add =
            cx.new(|cx| InputState::new(window, cx).placeholder("Add term (e.g. Sona, or A -> B)"));
        let dictionary_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search dictionary..."));
        let dictionary_test =
            cx.new(|cx| InputState::new(window, cx).placeholder("Test input text..."));
        let word_state = cx.new(|cx| InputState::new(window, cx).placeholder("Word"));
        let weight_state = cx.new(|cx| InputState::new(window, cx).placeholder("Weight"));
        let from_state = cx.new(|cx| InputState::new(window, cx).placeholder("From"));
        let to_state = cx.new(|cx| InputState::new(window, cx).placeholder("To"));
        let inline_hotword_state = cx.new(|cx| InputState::new(window, cx));
        let vocabulary_name_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Preset name"));
        let vocabulary_body_state = cx.new(|cx| TextareaState::new(window, cx));
        let speaker_profile_name_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Speaker profile name"));
        let speaker_profile_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search speakers..."));
        let mut subscriptions = Vec::new();
        let translation_language = initial_config
            .get("translationLanguage")
            .and_then(Value::as_str)
            .unwrap_or("zh");
        let translation_language_index = LANGUAGE_OPTIONS
            .iter()
            .position(|(code, _)| *code == translation_language)
            .unwrap_or(0);
        let translation_language_select = cx.new(|cx| {
            SelectState::new(
                LANGUAGE_OPTIONS.iter().map(|(_, label)| *label).collect(),
                Some(IndexPath::new(translation_language_index)),
                window,
                cx,
            )
        });

        subscriptions.push(cx.subscribe(
            &translation_language_select,
            |_modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some((code, _)) = LANGUAGE_OPTIONS
                        .iter()
                        .find(|(_, option_label)| *option_label == *label)
                {
                    let code = (*code).to_string();
                    let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
                    cx.spawn(async move |_this, _cx: &mut AsyncApp| {
                        let _ = services
                            .config
                            .set_setting("translationLanguage", &Value::String(code));
                    })
                    .detach();
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe(
            &dictionary_search,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    modal.vocabulary.search_query = state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe(
            &dictionary_test,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    modal.vocabulary.test_input = state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        ));

        subscriptions.push(cx.subscribe(
            &dictionary_textarea,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let content = state.read(cx).value().to_string();
                    let (hotwords, replacements) = legacy_dictionary_sets(&content);
                    modal.persist_model_settings(
                        vec![
                            ("dictionaryContent".to_string(), Value::String(content)),
                            ("hotwordSets".to_string(), hotwords),
                            ("textReplacementSets".to_string(), replacements),
                        ],
                        cx,
                    );
                }
            },
        ));

        Self {
            dictionary_textarea,
            dictionary_quick_add,
            dictionary_search,
            dictionary_test,
            word_state,
            weight_state,
            from_state,
            to_state,
            inline_hotword_state,
            vocabulary_name_state,
            vocabulary_body_state,
            speaker_profile_name_state,
            speaker_profile_search_state,
            translation_language_select,
            speaker_profile_name_states: HashMap::new(),
            active_tab: "recognition".to_string(),
            active_prompt_tab: "polish".to_string(),
            auto_translate: initial_config
                .get("autoTranslate")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            dictionary_mode: "visual".to_string(),
            dictionary_scope: "all".to_string(),
            search_query: String::new(),
            test_input: String::new(),
            edit_line: None,
            tutorial_open: false,
            add_scope: "global".to_string(),
            scope_menu_open: false,
            dialog_open: false,
            dialog_type: "hotword".to_string(),
            polish_edit_id: None,
            summary_edit_id: None,
            speaker_scope_filter: "all".to_string(),
            speaker_new_scope: "global".to_string(),
            speaker_new_scope_open: false,
            speaker_expanded_ids: HashSet::new(),
            speaker_edit_scope: None,
            speaker_scope_modal_type: "global".to_string(),
            speaker_scope_project_ids: Vec::new(),
            speaker_import_error: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn render(&mut self, config: &Value, cx: &mut Context<SettingsModal>) -> impl IntoElement {
        let dictionary_content = self.dictionary_textarea.read(cx).value().to_string();
        let dictionary_entries = parse_dictionary_entries(&dictionary_content);
        let project_refs: Vec<(String, String)> = Vec::new();
        let diagnostics = validate_dictionary_content(&dictionary_content, &project_refs);

        let mut scopes = vec![
            (
                "global".to_string(),
                "Global".to_string(),
                dictionary_entries
                    .iter()
                    .filter(|e| e.scope_id == "global")
                    .count(),
            ),
            (
                "voice-typing".to_string(),
                "Voice Typing".to_string(),
                dictionary_entries
                    .iter()
                    .filter(|e| e.scope_id == "voice-typing")
                    .count(),
            ),
        ];
        for entry in &dictionary_entries {
            if !scopes.iter().any(|(id, _, _)| id == &entry.scope_id) {
                let count = dictionary_entries
                    .iter()
                    .filter(|c| c.scope_id == entry.scope_id)
                    .count();
                scopes.push((entry.scope_id.clone(), entry.scope_name.clone(), count));
            }
        }

        let search_query = self.search_query.clone();
        let editing_line = self.edit_line;
        let add_scope = self.add_scope.clone();
        let scope_menu_open = self.scope_menu_open;
        let add_scope_name = scopes
            .iter()
            .find(|(id, _, _)| id == &add_scope)
            .map(|(_, name, _)| name.clone())
            .unwrap_or_else(|| add_scope.clone());
        let tutorial_open = self.tutorial_open;
        let dialog_open = self.dialog_open;
        let dialog_type = self.dialog_type.clone();
        let dialog_word = self.word_state.read(cx).value().trim().to_string();
        let dialog_from = self.from_state.read(cx).value().trim().to_string();
        let dialog_to = self.to_state.read(cx).value().trim().to_string();
        let dialog_can_save = if dialog_type == "hotword" {
            !dialog_word.is_empty()
        } else {
            !dialog_from.is_empty() && !dialog_to.is_empty()
        };
        let quick_add_value = self.dictionary_quick_add.read(cx).value().to_string();
        let visible_entries = dictionary_entries
            .iter()
            .filter(|entry| {
                self.dictionary_scope == "all" || entry.scope_id == self.dictionary_scope
            })
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
        let test_input = self.test_input.clone();
        let _test_output = visible_entries
            .iter()
            .filter_map(|entry| {
                entry
                    .raw
                    .split_once("->")
                    .or_else(|| entry.raw.split_once("=>"))
                    .map(|(f, t)| (f.trim(), t.trim()))
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
            .filter(|e| e.scope_id == "global")
            .count();
        let project_scope_count = parse_dictionary_scopes(&dictionary_content).len();
        let active_tab = self.active_tab.clone();
        let active_prompt_tab = self.active_prompt_tab.clone();
        let auto_translate = self.auto_translate;
        let dictionary_mode = self.dictionary_mode.clone();
        let dictionary_scope = self.dictionary_scope.clone();

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
                                .on_click(cx.listener(move |modal, _, window, cx| {
                                    modal.vocabulary.active_tab = tab.to_string();
                                    modal.vocabulary.polish_edit_id = None;
                                    modal.vocabulary.summary_edit_id = None;
                                    modal.vocabulary.vocabulary_name_state.update(cx, |state, cx| state.set_value("", window, cx));
                                    modal.vocabulary.vocabulary_body_state.update(cx, |state, cx| state.set_value("", window, cx));
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
                                .on_click(cx.listener(|modal, _, _, cx| {
                                    modal.vocabulary.tutorial_open = !modal.vocabulary.tutorial_open;
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
                                            .on_click(cx.listener(move |modal, _, _, cx| {
                                                modal.vocabulary.dictionary_mode = mode.to_string();
                                                cx.notify();
                                            }))
                                    })))
                                    .child(div().flex().items_center().gap_1()
                                        .when(dictionary_mode == "visual", |actions| actions.child(
                                            Button::new("dictionary-new-entry")
                                                .primary().small().label("New Entry")
                                                .on_click(cx.listener(|modal, _, window, cx| {
                                                    modal.vocabulary.edit_line = None;
                                                    modal.vocabulary.dialog_open = true;
                                                    modal.vocabulary.dialog_type = "hotword".to_string();
                                                    if modal.vocabulary.dictionary_scope != "all" {
                                                        modal.vocabulary.add_scope = modal.vocabulary.dictionary_scope.clone();
                                                    } else {
                                                        modal.vocabulary.add_scope = "global".to_string();
                                                    }
                                                    modal.vocabulary.word_state.update(cx, |state, cx| { state.set_value("", window, cx); state.focus(window, cx); });
                                                    modal.vocabulary.weight_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                    modal.vocabulary.from_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                    modal.vocabulary.to_state.update(cx, |state, cx| state.set_value("", window, cx));
                                                    cx.notify();
                                                })),
                                        ))
                                        .child(Button::new("dictionary-format").outline().small().label("Format")
                                            .tooltip("Format indentation, canonicalize arrows, and sync project links")
                                            .on_click(cx.listener(|modal, _, window, cx| {
                                                let source = modal.vocabulary.dictionary_textarea.read(cx).value().to_string();
                                                let formatted = format_dictionary_content(&source, &[]);
                                                let (hotwords, replacements) = legacy_dictionary_sets(&formatted);
                                                modal.vocabulary.dictionary_textarea.update(cx, |state, cx| state.set_value(formatted.clone(), window, cx));
                                                modal.persist_model_settings(vec![
                                                    ("dictionaryContent".into(), Value::String(formatted)),
                                                    ("hotwordSets".into(), hotwords),
                                                    ("textReplacementSets".into(), replacements),
                                                ], cx);
                                            })))
                                        .child(Button::new("dictionary-copy").ghost().small().label("Copy")
                                            .tooltip("Copy dictionary YAML to clipboard")
                                            .on_click(cx.listener(|modal, _, _, cx| {
                                                let source = modal.vocabulary.dictionary_textarea.read(cx).value().to_string();
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
                                            .on_click(cx.listener(|modal, _, _, cx| {
                                                modal.vocabulary.scope_menu_open = !modal.vocabulary.scope_menu_open;
                                                cx.notify();
                                            }))))
                                    .child(div().flex_1().min_w(px(180.0))
                                        .on_key_down(cx.listener(|modal, event: &KeyDownEvent, window, cx| {
                                            if event.keystroke.key.eq_ignore_ascii_case("enter") {
                                                let raw = modal.vocabulary.dictionary_quick_add.read(cx).value().to_string();
                                                modal.vocabulary.edit_line = None;
                                                modal.submit_vocabulary_entry(&raw, window, cx);
                                                window.prevent_default();
                                            }
                                        }))
                                        .child(Input::new(&self.dictionary_quick_add).w_full().h(px(36.0)).aria_label("Add dictionary term")))
                                    .child(Button::new("dictionary-quick-add").primary().small().label("Add")
                                        .disabled(quick_add_value.trim().is_empty())
                                        .on_click(cx.listener(|modal, _, window, cx| {
                                            let raw = modal.vocabulary.dictionary_quick_add.read(cx).value().trim().to_string();
                                            modal.vocabulary.edit_line = None;
                                            modal.submit_vocabulary_entry(&raw, window, cx);
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
                                                .on_click(cx.listener(move |modal, _, _, cx| {
                                                    modal.vocabulary.add_scope = selected_scope_id.clone();
                                                    modal.vocabulary.scope_menu_open = false;
                                                    cx.notify();
                                                }))
                                        }))
                                ))
                                .when(dictionary_mode == "code", |section| {
                                    let textarea = self.dictionary_textarea.clone();
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
                                                .on_key_down(cx.listener(move |modal, event: &KeyDownEvent, window, cx| {
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
                                                        modal.persist_model_settings(vec![
                                                            ("dictionaryContent".into(), Value::String(content)),
                                                            ("hotwordSets".into(), hotwords),
                                                            ("textReplacementSets".into(), replacements),
                                                        ], cx);
                                                    }
                                                }))
                                                .child(Textarea::new(&self.dictionary_textarea).w_full().h_full().aria_label("Unified dictionary YAML"))))
                                        .child(div().flex().flex_col().gap_1().pt_2()
                                            .children(diagnostics.iter().map(|(line, severity, message)| {
                                                div().flex().items_center().gap_2().px_2().py_1().rounded_sm()
                                                    .bg(if *severity == "error" { cx.theme().danger.opacity(0.1) } else { cx.theme().warning.opacity(0.1) })
                                                    .child(div().text_size(px(11.0)).font_weight(FontWeight::MEDIUM).text_color(if *severity == "error" { cx.theme().danger } else { cx.theme().warning }).child(format!("Line {line} · {severity}")))
                                                    .child(div().text_size(px(12.0)).text_color(cx.theme().foreground).child(message.clone()))
                                            })))
                                })
                                .when(dictionary_mode == "visual", |section| {
                                    section
                                        .child(div().flex().items_center().justify_between().gap_3().py_2()
                                            .child(div().flex().items_center().gap_1().flex_wrap()
                                                .children(scopes.iter().map(|(scope_id, label, count)| {
                                                    let selected = dictionary_scope == *scope_id;
                                                    let selected_scope_id = scope_id.clone();
                                                    Button::new(ElementId::Name(format!("dictionary-scope-{scope_id}").into()))
                                                        .when(selected, |button| button.primary())
                                                        .when(!selected, |button| button.ghost())
                                                        .small().label(format!("{label} ({count})"))
                                                        .on_click(cx.listener(move |modal, _, _, cx| {
                                                            modal.vocabulary.dictionary_scope = selected_scope_id.clone();
                                                            cx.notify();
                                                        }))
                                                })))
                                            .child(div().w(px(200.0)).child(Input::new(&self.dictionary_search))))
                                        .child(div().flex().flex_col().gap_3()
                                            .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Hotwords"))
                                            .child(div().flex().flex_wrap().gap_2()
                                                .children(hotword_entries.iter().map(|entry| {
                                                    dictionary_hotword_chip(entry, &dictionary_scope, editing_line, cx)
                                                })))
                                            .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Replacements"))
                                            .child(div().flex().flex_col().gap_2()
                                                .children(replacement_entries.iter().map(|entry| {
                                                    dictionary_replacement_row(entry, editing_line, cx)
                                                }))))
                                }),
                        )
                    })
                    .when(active_tab == "recognition" && tutorial_open, |panel| {
                        panel.child(
                            section("Syntax Guide", "Quick syntax reference and common patterns for your unified dictionary.", cx)
                                .child(div().grid().grid_cols(2).gap_3()
                                    .child(tutorial_block("Single Word / Phrase", "Add words directly or assign recognition weights.", "- Sona\n- Sona :2.0", Some("Weights above 1.0 boost recognition preference."), cx))
                                    .child(tutorial_block("Text Replacement", "Define substitutions to correct common recognition errors.", "- sona ai -> Sona\n- misheard => corrected", Some("Supports both -> and => arrows."), cx))
                                    .child(tutorial_block("Project-Scoped Terms", "Scope hotwords and replacements to specific projects.", "projects:\n  Research (id:proj-1):\n    - SpecialTerm", Some("Terms apply when working within that project."), cx))
                                    .child(tutorial_block("Voice Typing Scope", "Scope terms specifically to the Voice Typing tool.", "projects:\n  Voice Typing (id:voice-typing):\n    - Punctuation -> .", Some("Applies only during Voice Typing sessions."), cx))),
                        )
                    })
                    .when(active_tab == "prompts", |panel| {
                        panel.child(
                            div().flex().flex_col().gap_4()
                                .child(div().flex().gap_2().children([
                                    ("polish", "Polish"),
                                    ("translation", "Translation"),
                                    ("summary", "Summary"),
                                ].into_iter().map(|(tab, label)| {
                                    let selected = active_prompt_tab == tab;
                                    Button::new(ElementId::Name(format!("settings-vocabulary-prompt-{tab}").into()))
                                        .when(selected, |button| button.primary())
                                        .when(!selected, |button| button.outline())
                                        .small().label(label)
                                        .on_click(cx.listener(move |modal, _, _, cx| {
                                            modal.vocabulary.active_prompt_tab = tab.to_string();
                                            cx.notify();
                                        }))
                                })))
                                .when(active_prompt_tab == "polish", |content| content
                                    .child(toggle_row("Auto Polish", "Automatically polish transcripts with LLM when completed.", config.get("autoPolish").and_then(Value::as_bool).unwrap_or(false), "toggle-auto-polish", "autoPolish", cx))
                                    .child(
                                    custom_records_section(
                                        config,
                                        "Polish Presets",
                                        "Custom presets for polishing styles and vocabulary context.",
                                        "polishCustomPresets",
                                        "context",
                                        "polishPreset",
                                        "clean",
                                        "polish",
                                        EditorKind::Polish,
                                        self.polish_edit_id.as_deref(),
                                        &self.vocabulary_name_state,
                                        &self.vocabulary_body_state,
                                        cx,
                                    ),
                                ))
                                .when(active_prompt_tab == "translation", |content| content
                                    .child(section("Translation Automation", "Configure automatic translation and the default target language for transcripts.", cx)
                                        .child(div().flex().items_center().justify_between().gap_4().py_2()
                                            .child(div().flex_1().flex().flex_col().gap_1()
                                                .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Auto Translate"))
                                                .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Automatically translate transcripts after transcription completes.")))
                                            .child(Switch::new("settings-translation-auto")
                                                .checked(auto_translate)
                                                .on_change(cx.listener(|modal, checked, _, cx| {
                                                    let value = *checked;
                                                    modal.vocabulary.auto_translate = value;
                                                    modal.persist_preference("autoTranslate", value, cx);
                                                    cx.notify();
                                                }))))
                                        .child(div().flex().items_center().justify_between().gap_4().py_2()
                                            .child(div().flex_1().flex().flex_col().gap_1()
                                                .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Default Target Language"))
                                                .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Default language used when starting transcript translation.")))
                                            .child(Select::new(&self.translation_language_select).w(px(280.0)).max_w_full()))))
                                .when(active_prompt_tab == "summary", |content| content
                                    .child(toggle_row("Auto Summary", "Automatically generate summary after transcription.", config.get("autoSummary").and_then(Value::as_bool).unwrap_or(false), "toggle-auto-summary", "autoSummary", cx))
                                    .child(custom_records_section(
                                        config,
                                        "Summary Templates",
                                        "Custom templates and instructions for transcript summaries.",
                                        "summaryCustomTemplates",
                                        "instructions",
                                        "summaryTemplate",
                                        "general",
                                        "summary",
                                        EditorKind::Summary,
                                        self.summary_edit_id.as_deref(),
                                        &self.vocabulary_name_state,
                                        &self.vocabulary_body_state,
                                        cx,
                                    )))
                        )
                    })
                    .when(active_tab == "speakers", |panel| {
                        panel.child(
                            section("Speaker Profiles", "Manage speaker voice profiles and recognition models.", cx)
                                .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Speaker diarization and profile assignment are configured here.")),
                        )
                    }),
            )
            .when(dialog_open, |panel| {
                panel.child(
                    div()
                        .id("dictionary-entry-modal-backdrop")
                        .absolute().inset_0().flex().items_center().justify_center()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .on_click(cx.listener(|modal, _, _, cx| {
                            modal.vocabulary.dialog_open = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .id("dictionary-modal-card")
                                .w(px(400.0)).p_6().rounded_lg().bg(cx.theme().background).border_1().border_color(cx.theme().border).shadow_2xl()
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(div().text_size(px(16.0)).font_weight(FontWeight::BOLD).child(if dialog_type == "hotword" { "Add / Edit Hotword" } else { "Add / Edit Replacement" }))
                                .child(
                                    div().flex().flex_col().gap_3().py_4()
                                        .when(dialog_type == "hotword", |form| {
                                            form.child(Input::new(&self.word_state))
                                                .child(Input::new(&self.weight_state))
                                        })
                                        .when(dialog_type != "hotword", |form| {
                                            form.child(Input::new(&self.from_state))
                                                .child(Input::new(&self.to_state))
                                        }),
                                )
                                .child(
                                    div().flex().justify_end().gap_2()
                                        .child(Button::new("btn-cancel-dict-dialog").ghost().small().label("Cancel")
                                            .on_click(cx.listener(|modal, _, _, cx| {
                                                modal.vocabulary.dialog_open = false;
                                                cx.notify();
                                            })))
                                        .child(Button::new("btn-save-dict-dialog").primary().small().label("Save")
                                            .disabled(!dialog_can_save)
                                            .on_click(cx.listener(|modal, _, window, cx| {
                                                modal.submit_vocabulary_dialog_entry(window, cx);
                                            }))),
                                ),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::hotwords::*;
    use super::tester::*;
    use super::words::*;
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
