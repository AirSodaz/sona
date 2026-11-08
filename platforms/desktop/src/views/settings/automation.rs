use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::Button;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;
use sona_core::automation::repository::AutomationRepositoryState;

use super::SettingsModal;

#[derive(Default)]
pub struct AutomationSettingsTab {
    pub automation_state: Option<AutomationRepositoryState>,
    pub automation_loading: bool,
    pub automation_error: Option<String>,
}

impl AutomationSettingsTab {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_automation_state(&mut self, cx: &mut Context<SettingsModal>) {
        self.automation_loading = true;
        self.automation_error = None;
        let sqlite = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sqlite
            .clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result =
                crate::platform::repositories::automation::load_repository_state(&sqlite).await;
            this.update(cx, |modal, cx| {
                modal.automation.automation_loading = false;
                match result {
                    Ok(state) => modal.automation.automation_state = Some(state),
                    Err(error) => modal.automation.automation_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn set_automation_rule_enabled(
        &mut self,
        rule_id: String,
        enabled: bool,
        cx: &mut Context<SettingsModal>,
    ) {
        let Some(state) = self.automation_state.clone() else {
            return;
        };
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        let converted = state
            .rules
            .into_iter()
            .map(|rule| {
                let mut value = serde_json::to_value(rule).map_err(|error| error.to_string())?;
                if value.get("id").and_then(Value::as_str) == Some(rule_id.as_str()) {
                    value["enabled"] = Value::Bool(enabled);
                }
                serde_json::from_value(value).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<sona_core::automation::repository::AutomationRuleInput>, String>>(
            );
        let Ok(rules) = converted else {
            return;
        };
        self.automation_loading = true;
        self.automation_error = None;
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = async {
                crate::platform::repositories::automation::persist_rules(&services.sqlite, rules)
                    .await?;
                crate::platform::repositories::automation::load_repository_state(&services.sqlite)
                    .await
            }
            .await;
            this.update(cx, |modal, cx| {
                modal.automation.automation_loading = false;
                match result {
                    Ok(state) => modal.automation.automation_state = Some(state),
                    Err(error) => modal.automation.automation_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn remove_automation_profile_dependency(
        &mut self,
        dependency: &'static str,
        dependency_id: String,
        cx: &mut Context<SettingsModal>,
    ) {
        let sqlite = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sqlite
            .clone();
        self.automation_loading = true;
        self.automation_error = None;
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = async {
                let state =
                    crate::platform::repositories::automation::load_repository_state(&sqlite)
                        .await?;
                let profiles = state
                    .profiles
                    .into_iter()
                    .map(|profile| {
                        let mut input: sona_core::automation::repository::AutomationProfileInput =
                            serde_json::from_value(
                                serde_json::to_value(profile).map_err(|error| error.to_string())?,
                            )
                            .map_err(|error| error.to_string())?;
                        let value = match dependency {
                            "polishPreset" if input.polish_preset_id == dependency_id => {
                                input.polish_preset_id =
                                    sona_core::config::DEFAULT_POLISH_PRESET_ID.to_string();
                                true
                            }
                            "summaryTemplate" if input.summary_template_id == dependency_id => {
                                input.summary_template_id =
                                    sona_core::config::DEFAULT_SUMMARY_TEMPLATE_ID.to_string();
                                true
                            }
                            "speakerProfile"
                                if input.enabled_speaker_profile_ids.contains(&dependency_id) =>
                            {
                                input
                                    .enabled_speaker_profile_ids
                                    .retain(|profile_id| profile_id != &dependency_id);
                                true
                            }
                            _ => false,
                        };
                        Ok::<_, String>((input, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let changed = profiles.iter().any(|(_, changed)| *changed);
                if changed {
                    crate::platform::repositories::automation::persist_profiles(
                        &sqlite,
                        profiles.into_iter().map(|(profile, _)| profile).collect(),
                    )
                    .await?;
                }
                crate::platform::repositories::automation::load_repository_state(&sqlite).await
            }
            .await;
            this.update(cx, |modal, cx| {
                modal.automation.automation_loading = false;
                match result {
                    Ok(state) => modal.automation.automation_state = Some(state),
                    Err(error) => modal.automation.automation_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn render(&mut self, cx: &mut Context<SettingsModal>) -> impl IntoElement {
        let loading = self.automation_loading;
        let error = self.automation_error.as_deref();
        let rules = self
            .automation_state
            .as_ref()
            .map(|s| s.rules.as_slice())
            .unwrap_or_default();
        let rules = rules
            .iter()
            .filter(|rule| rule.kind == "file")
            .collect::<Vec<_>>();

        div()
            .id("settings-automation-panel")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(32.0))
            .py(px(24.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(900.0))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_3()
                            .pb_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(Icon::new(LucideIcon::Workflow).size(px(22.0)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(20.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Automation"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                "Monitor folders and process new files automatically.",
                                            ),
                                    ),
                            ),
                    )
                    .when(loading, |panel| {
                        panel.child(
                            div()
                                .p_4()
                                .rounded_md()
                                .bg(cx.theme().group_box)
                                .text_color(cx.theme().muted_foreground)
                                .child("Loading automation rules…"),
                        )
                    })
                    .when_some(error, |panel, error| {
                        panel.child(
                            div()
                                .p_4()
                                .rounded_md()
                                .bg(cx.theme().danger.opacity(0.12))
                                .text_color(cx.theme().danger)
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(div().child(format!("Failed to load automation rules: {error}")))
                                .child(
                                    Button::new("retry-load-automation")
                                        .outline()
                                        .small()
                                        .label("Retry")
                                        .on_click(cx.listener(|modal, _, _, cx| {
                                            modal.automation.load_automation_state(cx);
                                        })),
                                ),
                        )
                    })
                    .when(!loading && error.is_none() && rules.is_empty(), |panel| {
                        panel.child(
                            div()
                                .p_6()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().group_box)
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("No automation rules configured"),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Automation rules let Sona monitor directories and transcribe audio files in the background."),
                                ),
                        )
                    })
                    .children(rules.into_iter().map(|rule| {
                        let rule_id = rule.id.clone();
                        let is_enabled = rule.enabled;
                        div()
                            .p_4()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(rule.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(cx.theme().muted_foreground)
                                        .child(format!("Watch: {}", rule.watch_directory)),
                                    ),
                            )
                            .child(
                                Switch::new(format!("rule-toggle-{}", rule.id))
                                    .checked(is_enabled)
                                    .on_change(cx.listener(move |modal, value, _, cx| {
                                        modal.automation.set_automation_rule_enabled(
                                            rule_id.clone(),
                                            *value,
                                            cx,
                                        );
                                    })),
                            )
                    })),
            )
            .into_any_element()
    }
}
