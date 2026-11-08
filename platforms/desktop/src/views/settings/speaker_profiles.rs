use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};
use sona_core::transcription::speaker::{SpeakerProfile, SpeakerProfileSample};
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) fn speaker_profiles_panel(
    config: &Value,
    name_state: &Entity<InputState>,
    search_state: &Entity<InputState>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let profiles = normalized_profiles(config);
    let projects = cx.entity().read(cx).project_selector.projects.clone();
    let name = name_state.read(cx).value().trim().to_string();
    let query = search_state.read(cx).value().trim().to_lowercase();
    let scope_filter = cx
        .entity()
        .read(cx)
        .settings_modal
        .speaker_scope_filter
        .clone();
    let selected_scope = cx
        .entity()
        .read(cx)
        .settings_modal
        .speaker_new_scope
        .clone();
    let expanded = cx
        .entity()
        .read(cx)
        .settings_modal
        .speaker_expanded_ids
        .clone();
    let edit_scope = cx
        .entity()
        .read(cx)
        .settings_modal
        .speaker_edit_scope
        .clone();
    let selected_project_ids = cx
        .entity()
        .read(cx)
        .settings_modal
        .speaker_scope_project_ids
        .clone();
    let import_error = cx
        .entity()
        .read(cx)
        .settings_modal
        .speaker_import_error
        .clone();
    let mut filter_items = vec![("all".to_string(), "All Scopes".to_string(), profiles.len())];
    filter_items.push((
        "global".to_string(),
        "Global".to_string(),
        profiles
            .iter()
            .filter(|profile| profile.scope != "project")
            .count(),
    ));
    filter_items.extend(projects.iter().map(|project| {
        (
            project.id.clone(),
            project.name.clone(),
            profiles
                .iter()
                .filter(|profile| {
                    profile.scope == "project" && profile.project_ids.contains(&project.id)
                })
                .count(),
        )
    }));
    let visible_profiles = profiles
        .iter()
        .filter(|profile| {
            let in_scope = scope_filter == "all"
                || (scope_filter == "global" && profile.scope != "project")
                || (profile.scope == "project" && profile.project_ids.contains(&scope_filter));
            in_scope && (query.is_empty() || profile.name.to_lowercase().contains(&query))
        })
        .cloned()
        .collect::<Vec<_>>();

    div().flex().flex_col().gap_3()
        .child(section_header("Speaker Profiles", "Build a global library of known speakers from local reference audio files. Projects can then choose which profiles are active.", cx))
        .when(import_error.is_some(), |this| this.child(div().px_3().py_2().rounded_md().bg(cx.theme().danger.opacity(0.10)).text_size(px(12.0)).text_color(cx.theme().danger).child(import_error.clone().unwrap_or_default())))
        .child(div().flex().items_center().justify_between().gap_3().flex_wrap()
            .child(div().flex().gap_1().flex_wrap().children(filter_items.into_iter().map(|(id, label, count)| {
                let selected = scope_filter == id;
                Button::new(ElementId::Name(format!("speaker-scope-filter-{id}").into()))
                    .when(selected, |button| button.primary()).when(!selected, |button| button.outline())
                    .small().label(format!("{label} ({count})"))
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        shell.settings_modal.speaker_scope_filter = id.clone();
                        shell.settings_modal.speaker_new_scope = if id == "all" { "global".to_string() } else { id.clone() };
                        cx.notify();
                    }))
            })))
            .child(div().w(px(220.0)).child(Input::new(search_state).aria_label("Search speakers"))))
        .child(div().flex().items_end().gap_3().p_3().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().background)
            .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                .child(label("Profile Name", cx))
                .child(Input::new(name_state).aria_label("Profile Name")))
            .child(div().w(px(200.0)).flex().flex_col().gap_1()
                .child(label("Target Scope", cx))
                .child(Button::new("speaker-new-scope").outline().small().label(scope_label(&selected_scope, &projects))
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.settings_modal.speaker_new_scope_open = !shell.settings_modal.speaker_new_scope_open;
                        cx.notify();
                    }))))
            .child(Button::new("speaker-add-profile").primary().small().label("Add Profile").disabled(name.is_empty())
                .on_click(cx.listener(move |shell, _, window, cx| {
                    let name = shell.speaker_profile_name_state.read(cx).value().trim().to_string();
                    if name.is_empty() { return; }
                    let scope = shell.settings_modal.speaker_new_scope.clone();
                    let profile = SpeakerProfile {
                        id: Uuid::new_v4().to_string(), name: name.clone(), enabled: true,
                        scope: if scope == "global" { "global" } else { "project" }.to_string(),
                        project_ids: if scope == "global" { Vec::new() } else { vec![scope] },
                        samples: Vec::new(),
                    };
                    let mut profiles = normalized_profiles(&shell.settings_modal.model_config);
                    let id = profile.id.clone();
                    profiles.push(profile);
                    shell.settings_modal.persist_model_setting("speakerProfiles".into(), serde_json::to_value(profiles).unwrap_or_else(|_| json!([])), cx);
                    let profile_name_state = cx.new(|cx| InputState::new(window, cx).default_value(name));
                    let profile_name_id = id.clone();
                    let subscription = cx.subscribe(&profile_name_state, move |shell, state, event: &gpui_kit::component::input::InputEvent, cx| {
                        if !matches!(event, gpui_kit::component::input::InputEvent::Change) { return; }
                        let name = state.read(cx).value().to_string();
                        let mut profiles = normalized_profiles(&shell.settings_modal.model_config);
                        if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == profile_name_id) { profile.name = name; }
                        shell.settings_modal.persist_model_setting("speakerProfiles".into(), serde_json::to_value(profiles).unwrap_or_else(|_| json!([])), cx);
                    });
                    shell.speaker_profile_name_states.insert(id.clone(), (profile_name_state, subscription));
                    shell.speaker_profile_name_state.update(cx, |state, cx| state.set_value("", window, cx));
                    shell.settings_modal.speaker_expanded_ids.insert(id);
                    cx.notify();
                }))))
        .when(cx.entity().read(cx).settings_modal.speaker_new_scope_open, |this| {
            let options = std::iter::once(("global".to_string(), "Global".to_string())).chain(projects.iter().map(|project| (project.id.clone(), project.name.clone())).collect::<Vec<_>>());
            this.child(div().flex().flex_wrap().gap_1().children(options.map(|(id, label)| {
                let selected = selected_scope == id;
                Button::new(ElementId::Name(format!("speaker-new-scope-option-{id}").into()))
                    .when(selected, |button| button.primary()).when(!selected, |button| button.ghost())
                    .small().label(label)
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        shell.settings_modal.speaker_new_scope = id.clone();
                        shell.settings_modal.speaker_new_scope_open = false;
                        cx.notify();
                    }))
            })))
        })
        .when(visible_profiles.is_empty(), |this| this.child(div().p_6().text_center().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("No speaker profiles found.")))
        .children(visible_profiles.into_iter().map(|profile| {
            let profile_id = profile.id.clone();
            let expand_id = profile_id.clone();
            let scope_id = profile_id.clone();
            let scope_profile_ids = profile.project_ids.clone();
            let scope_profile_scope = profile.scope.clone();
            let enabled_id = profile_id.clone();
            let delete_id = profile_id.clone();
            let delete_label = profile.name.clone();
            let name_state = cx.entity().read(cx).speaker_profile_name_states.get(&profile.id).map(|(state, _)| state.clone());
            let name_field = if let Some(name_state) = name_state {
                Input::new(&name_state).aria_label("Speaker profile name").into_any_element()
            } else {
                div().flex_1().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child(profile.name.clone()).into_any_element()
            };
            let is_expanded = expanded.contains(&profile_id);
            let readiness = readiness(&profile.samples);
            let projects_for_profile = projects.clone();
            div().id(ElementId::Name(format!("speaker-profile-{}", profile.id).into()))
                .flex().flex_col().gap_2().p_3().rounded_md().border_1().border_color(cx.theme().border)
                .when(!profile.enabled, |this| this.bg(cx.theme().muted.opacity(0.08)))
                .child(div().flex().items_center().gap_2()
                .child(Button::new(ElementId::Name(format!("speaker-expand-{}", profile.id).into())).ghost().small()
                        .icon(icon(if is_expanded { LucideIcon::ChevronDown } else { LucideIcon::ChevronRight }))
                        .tooltip(if is_expanded { "Collapse profile" } else { "Expand profile" })
                        .accessibility_label(if is_expanded { "Collapse speaker profile" } else { "Expand speaker profile" })
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if !shell.settings_modal.speaker_expanded_ids.insert(expand_id.clone()) { shell.settings_modal.speaker_expanded_ids.remove(&expand_id); }
                            cx.notify();
                        })))
                    .child(div().flex_1().min_w_0().child(name_field))
                    .child(Button::new(ElementId::Name(format!("speaker-profile-scope-{}", profile.id).into())).outline().small()
                        .label(scope_label_profile(&profile, &projects_for_profile))
                        .tooltip("Click to edit target scope")
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            shell.settings_modal.speaker_edit_scope = Some(scope_id.clone());
                            shell.settings_modal.speaker_scope_project_ids = scope_profile_ids.clone();
                            shell.settings_modal.speaker_scope_modal_type = scope_profile_scope.clone();
                            cx.notify();
                        })))
                    .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child(format!("{} samples", profile.samples.len())))
                    .child(div().flex_1().min_w_0().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child(readiness.2))
                    .child(Switch::new(ElementId::Name(format!("speaker-profile-enabled-{}", profile.id).into())).checked(profile.enabled).accessibility_label("Enable speaker profile")
                        .on_change(cx.listener(move |shell, enabled, _, cx| {
                            let mut profiles = normalized_profiles(&shell.settings_modal.model_config);
                            if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == enabled_id) { profile.enabled = *enabled; }
                            shell.settings_modal.persist_model_setting("speakerProfiles".into(), serde_json::to_value(profiles).unwrap_or_else(|_| json!([])), cx);
                        })))
                        .child(Button::new(ElementId::Name(format!("speaker-profile-delete-{}", profile.id).into())).ghost().small().icon(icon(LucideIcon::Trash)).tooltip(format!("Delete {}", delete_label)).accessibility_label(format!("Delete {}", delete_label))
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            let mut profiles = normalized_profiles(&shell.settings_modal.model_config);
                            if let Some(profile) = profiles.iter().find(|item| item.id == delete_id).cloned() {
                                let paths = profile.samples.into_iter().map(|sample| sample.file_path).collect::<Vec<_>>();
                                cx.spawn(async move |_this, _cx| { for path in paths { let _ = tokio::fs::remove_file(path).await; } }).detach();
                            }
                            profiles.retain(|item| item.id != delete_id);
                            shell.speaker_profile_name_states.remove(&delete_id);
                            shell.settings_modal.speaker_expanded_ids.remove(&delete_id);
                            shell.settings_modal.persist_model_setting("speakerProfiles".into(), serde_json::to_value(profiles).unwrap_or_else(|_| json!([])), cx);
                            shell.settings_modal.remove_automation_profile_dependency("speakerProfile", delete_id.clone(), cx);
                        }))))
                .when(is_expanded, |this| {
                    let id = profile.id.clone();
                    this.child(div().flex().items_center().justify_between().gap_3().pl(px(36.0))
                        .child(div().flex_1().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child("Import local reference clips. Audio is normalized to 16k mono WAV and stored under app-managed data."))
                        .child(div().text_size(px(11.0)).text_color(cx.theme().muted_foreground).child(readiness.1))
                        .child(Button::new(ElementId::Name(format!("speaker-import-{}", id).into())).outline().small().label("Import Samples")
                            .on_click(cx.listener(move |_shell, _, _, cx| {
                                let id = id.clone();
                                cx.spawn(async move |this, cx: &mut AsyncApp| {
                                    let selected = rfd::AsyncFileDialog::new().add_filter("Audio Files", &["wav", "mp3", "m4a", "aac", "ogg", "flac", "mp4", "webm"]).pick_files().await;
                                    let Some(files) = selected else { return; };
                                    let provider = crate::platform::system::paths::DesktopPathProvider::from_default();
                                    let mut imported = Vec::new();
                                    let mut import_error = None;
                                    for file in files {
                                        let source = file.path().to_string_lossy().to_string();
                                        let source_name = file.file_name();
                                        match crate::platform::models::speaker_processing::import_speaker_profile_sample(&provider, id.clone(), source, Some(source_name)).await { Ok(sample) => imported.push(sample), Err(error) => import_error = Some(error) }
                                    }
                                    this.update(cx, |shell, cx| {
                                        shell.settings_modal.speaker_import_error = import_error.map(|_| "Failed to import one or more speaker reference samples.".to_string());
                                        if imported.is_empty() { cx.notify(); return; }
                                        let mut profiles = normalized_profiles(&shell.settings_modal.model_config);
                                        if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == id) {
                                            for sample in imported { if !profile.samples.iter().any(|saved| saved.id == sample.id) { profile.samples.push(sample); } }
                                        }
                                        shell.settings_modal.speaker_expanded_ids.insert(id.clone());
                                        shell.settings_modal.persist_model_setting("speakerProfiles".into(), serde_json::to_value(profiles).unwrap_or_else(|_| json!([])), cx);
                                    }).ok();
                                }).detach();
                            })))
                        .children(profile.samples.iter().map(|sample| sample_row(sample, &profile.id, cx))))
                })
        }))
        .when(edit_scope.is_some(), |this| {
            let profile_id = edit_scope.clone().unwrap_or_default();
            this.child(div().flex().flex_col().gap_2().p_3().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Edit target scope"))
                .child(div().flex().gap_1().children([("global", "Global"), ("project", "Specific Projects")].into_iter().map(|(scope, label)| {
                    let selected = cx.entity().read(cx).settings_modal.speaker_scope_modal_type == scope;
                    Button::new(ElementId::Name(format!("speaker-scope-modal-{scope}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(label)
                        .on_click(cx.listener(move |_shell, _, _, cx| { _shell.settings_modal.speaker_scope_modal_type = scope.to_string(); cx.notify(); }))
                })))
                .when(cx.entity().read(cx).settings_modal.speaker_scope_modal_type == "project", |this| {
                    let this = this.when(projects.is_empty(), |this| this.child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("No projects created yet.")));
                    this.children(projects.iter().map(|project| {
                        let id = project.id.clone();
                        let selected = selected_project_ids.contains(&id);
                        Button::new(ElementId::Name(format!("speaker-scope-project-{id}").into())).when(selected, |button| button.primary()).when(!selected, |button| button.outline()).small().label(project.name.clone())
                            .on_click(cx.listener(move |shell, _, _, cx| {
                                let ids = &mut shell.settings_modal.speaker_scope_project_ids;
                                if ids.contains(&id) { ids.retain(|item| item != &id); } else { ids.push(id.clone()); }
                                cx.notify();
                            }))
                    }))
                })
                .child(div().flex().justify_end().gap_2()
                    .child(Button::new("speaker-scope-cancel").ghost().small().label("Cancel").on_click(cx.listener(|shell, _, _, cx| { shell.settings_modal.speaker_edit_scope = None; cx.notify(); })))
                    .child(Button::new("speaker-scope-save").primary().small().label("Save").on_click(cx.listener(move |shell, _, _, cx| {
                        let scope = shell.settings_modal.speaker_scope_modal_type.clone();
                        let project_ids = shell.settings_modal.speaker_scope_project_ids.clone();
                        let mut profiles = normalized_profiles(&shell.settings_modal.model_config);
                        if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == profile_id) {
                            profile.scope = scope.clone(); profile.project_ids = if scope == "project" { project_ids } else { Vec::new() };
                        }
                        shell.settings_modal.speaker_edit_scope = None;
                        shell.settings_modal.persist_model_setting("speakerProfiles".into(), serde_json::to_value(profiles).unwrap_or_else(|_| json!([])), cx);
                    }))))
            )
        })
        .into_any_element()
}

fn section_header(
    title: &'static str,
    description: &'static str,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
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

fn label(text: &'static str, cx: &Context<MainShell>) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

fn normalized_profiles(config: &Value) -> Vec<SpeakerProfile> {
    config
        .get("speakerProfiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| serde_json::from_value(value.clone()).ok())
        .collect()
}

fn scope_label(scope: &str, projects: &[sona_core::project::ProjectRecord]) -> String {
    if scope == "global" {
        "Global".to_string()
    } else {
        projects
            .iter()
            .find(|project| project.id == scope)
            .map(|project| project.name.clone())
            .unwrap_or_else(|| "Project".to_string())
    }
}

fn scope_label_profile(
    profile: &SpeakerProfile,
    projects: &[sona_core::project::ProjectRecord],
) -> String {
    if profile.scope != "project" {
        return "Global".to_string();
    }
    match profile.project_ids.as_slice() {
        [] => "No Projects".to_string(),
        [id] => projects
            .iter()
            .find(|project| &project.id == id)
            .map(|project| project.name.clone())
            .unwrap_or_else(|| id.clone()),
        ids => format!("{} Projects", ids.len()),
    }
}

fn readiness(samples: &[SpeakerProfileSample]) -> (usize, String, String) {
    let usable = samples
        .iter()
        .filter(|sample| sample.duration_seconds >= 4.0)
        .collect::<Vec<_>>();
    let duration = usable
        .iter()
        .map(|sample| sample.duration_seconds)
        .sum::<f32>();
    let state = if usable.len() >= 2 && duration >= 20.0 {
        "Ready for automatic matching"
    } else if !usable.is_empty() && duration >= 8.0 {
        "Can appear as a suggestion, but needs more usable samples before automatic matching."
    } else {
        "Needs more usable samples before automatic matching."
    };
    (
        usable.len(),
        format!(
            "{} usable samples · {}",
            usable.len(),
            format_duration(duration)
        ),
        state.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::{normalized_profiles, readiness};
    use serde_json::json;
    use sona_core::transcription::speaker::{SpeakerProfile, SpeakerProfileSample};

    fn sample(id: &str, seconds: f32) -> SpeakerProfileSample {
        SpeakerProfileSample {
            id: id.to_string(),
            file_path: format!("{id}.wav"),
            source_name: format!("{id}.wav"),
            duration_seconds: seconds,
        }
    }

    #[test]
    fn readiness_uses_only_usable_samples_and_matches_react_thresholds() {
        assert_eq!(readiness(&[sample("short", 3.9)]).0, 0);
        assert_eq!(
            readiness(&[sample("one", 8.0)]).2,
            "Can appear as a suggestion, but needs more usable samples before automatic matching."
        );
        assert_eq!(
            readiness(&[sample("one", 20.0)]).2,
            "Can appear as a suggestion, but needs more usable samples before automatic matching."
        );
        assert_eq!(
            readiness(&[sample("one", 10.0), sample("two", 10.0)]).2,
            "Ready for automatic matching"
        );
    }

    #[test]
    fn profile_json_round_trips_scope_and_samples() {
        let profile = SpeakerProfile {
            id: "profile-1".to_string(),
            name: "Alice".to_string(),
            enabled: true,
            scope: "project".to_string(),
            project_ids: vec!["project-1".to_string()],
            samples: vec![sample("sample-1", 12.5)],
        };
        let config = json!({ "speakerProfiles": [profile] });
        let restored = normalized_profiles(&config);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].scope, "project");
        assert_eq!(restored[0].project_ids, vec!["project-1"]);
        assert_eq!(restored[0].samples[0].duration_seconds, 12.5);
    }
}

fn format_duration(seconds: f32) -> String {
    if seconds >= 60.0 {
        format!("{}m {:.1}s", (seconds / 60.0).floor(), seconds % 60.0)
    } else {
        format!("{seconds:.1}s")
    }
}

fn icon(name: LucideIcon) -> Icon {
    Icon::new(name)
}

fn sample_row(
    sample: &SpeakerProfileSample,
    profile_id: &str,
    cx: &Context<MainShell>,
) -> impl IntoElement {
    let sample_id = sample.id.clone();
    let file_path = sample.file_path.clone();
    let profile_id = profile_id.to_string();
    div()
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(cx.theme().background)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(sample.source_name.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(format_duration(sample.duration_seconds)),
                ),
        )
        .child(
            Button::new(ElementId::Name(
                format!("speaker-sample-delete-{profile_id}-{sample_id}").into(),
            ))
            .ghost()
            .small()
            .icon(icon(LucideIcon::Trash))
            .tooltip("Delete sample")
            .accessibility_label("Delete sample")
            .on_click(cx.listener(move |shell, _, _, cx| {
                let mut profiles = normalized_profiles(&shell.settings_modal.model_config);
                if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == profile_id)
                {
                    profile.samples.retain(|sample| sample.id != sample_id);
                }
                let path = PathBuf::from(file_path.clone());
                cx.spawn(async move |_this, _cx| {
                    let _ = tokio::fs::remove_file(path).await;
                })
                .detach();
                shell.settings_modal.persist_model_setting(
                    "speakerProfiles".into(),
                    serde_json::to_value(profiles).unwrap_or_else(|_| json!([])),
                    cx,
                );
            })),
        )
}
