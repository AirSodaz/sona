use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::automation::repository::AutomationRuleRecord;

pub(super) fn automation_panel(
    rules: &[AutomationRuleRecord],
    loading: bool,
    error: Option<&str>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
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
                            .p_3()
                            .text_size(px(13.0))
                            .text_color(cx.theme().muted_foreground)
                            .child("Loading automation rules…"),
                    )
                })
                .when_some(error, |panel, error| {
                    panel.child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().danger.opacity(0.08))
                            .text_color(cx.theme().danger)
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(format!("Could not load automation rules: {error}"))
                            .child(
                                Button::new("settings-automation-retry")
                                    .outline()
                                    .small()
                                    .label("Retry")
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.settings_modal.load_automation_state(cx);
                                    })),
                            ),
                    )
                })
                .when(!loading && error.is_none() && rules.is_empty(), |panel| {
                    panel.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .p_8()
                            .text_color(cx.theme().muted_foreground)
                            .child(Icon::new(LucideIcon::FolderSync).size(px(24.0)))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("No file rules yet"),
                            )
                            .child("Create a folder automation rule to watch for new recordings."),
                    )
                })
                .children(rules.into_iter().map(|rule| {
                    let rule_id = rule.id.clone();
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .p_4()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().group_box)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .min_w_0()
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
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "{} · {}",
                                            if rule.stage_config.auto_polish {
                                                "Polish"
                                            } else {
                                                "No polish"
                                            },
                                            if rule.stage_config.auto_translate {
                                                "Translate"
                                            } else {
                                                "No translation"
                                            },
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(if rule.enabled {
                                            cx.theme().success
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .child(if rule.enabled { "Enabled" } else { "Paused" }),
                                )
                                .child(
                                    Switch::new(ElementId::Name(
                                        format!("automation-rule-{}", rule.id).into(),
                                    ))
                                    .checked(rule.enabled)
                                    .disabled(loading)
                                    .accessibility_label(format!("Enable {}", rule.name))
                                    .on_change(cx.listener(move |shell, value, _, cx| {
                                        shell.settings_modal.set_automation_rule_enabled(
                                            rule_id.clone(),
                                            *value,
                                            cx,
                                        );
                                    })),
                                ),
                        )
                })),
        )
        .into_any_element()
}
