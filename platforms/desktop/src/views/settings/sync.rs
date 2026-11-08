use crate::views::shell::MainShell;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sona_core::sync::{
    SyncConflictResolution, SyncConflictSummary, SyncLifecycleState, SyncStatusSnapshot,
};

use super::SyncAction;

pub(super) fn sync_panel(
    enabled: bool,
    status: Option<&SyncStatusSnapshot>,
    conflicts: &[SyncConflictSummary],
    loading: bool,
    busy: bool,
    error: Option<&str>,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let status_state = status.map(|status| status.state);
    div()
        .id("settings-sync-panel")
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
                        .pb_4()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_size(px(20.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Cloud Sync"),
                        )
                        .child(
                            div()
                                .mt_1()
                                .text_size(px(13.0))
                                .text_color(cx.theme().muted_foreground)
                                .child("End-to-end encrypted synchronization across devices."),
                        ),
                )
                .child(
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
                                .child(div().font_weight(FontWeight::MEDIUM).child("Enable Cloud Sync"))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Show sync status and enable encrypted remote synchronization."),
                                ),
                        )
                        .child(
                            Switch::new("enable-cloud-sync-switch")
                                .checked(enabled)
                                .disabled(busy)
                                .accessibility_label("Enable Cloud Sync")
                                .on_change(cx.listener(|shell, checked, _, cx| {
                                    let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
                                    match services.config.set_setting("enableCloudSync", &serde_json::Value::Bool(*checked)) {
                                        Ok(()) => {
                                            shell.settings_modal.sync_status = None;
                                            shell.settings_modal.load_sync_state(cx);
                                        }
                                        Err(error) => shell.settings_modal.sync_error = Some(error),
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .when(!enabled, |panel| {
                    panel.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .p_4()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().input)
                            .text_color(cx.theme().muted_foreground)
                            .child("Cloud synchronization is turned off."),
                    )
                })
                .when(enabled && loading, |panel| {
                    panel.child(
                        div()
                            .p_4()
                            .text_color(cx.theme().muted_foreground)
                            .child("Loading sync status…"),
                    )
                })
                .when_some(error, |panel, error| {
                    panel.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().danger.opacity(0.08))
                            .text_color(cx.theme().danger)
                            .child(format!("Sync operation failed: {error}"))
                            .child(
                                Button::new("settings-sync-retry")
                                    .outline()
                                    .small()
                                    .label("Retry")
                                    .on_click(cx.listener(|shell, _, _, cx| {
                                        shell.settings_modal.sync_status = None;
                                        shell.settings_modal.load_sync_state(cx);
                                    })),
                            ),
                    )
                })
                .when(enabled && !loading && error.is_none() && status.is_none(), |panel| {
                    panel.child(
                        div()
                            .p_4()
                            .text_color(cx.theme().muted_foreground)
                            .child("Sync status is not available yet."),
                    )
                })
                .when_some(status, |panel, status| {
                    let state_label = match status.state {
                        SyncLifecycleState::Disabled => "Not connected",
                        SyncLifecycleState::Locked => "Locked",
                        SyncLifecycleState::Idle => "Connected",
                        SyncLifecycleState::Syncing => "Syncing",
                        SyncLifecycleState::Paused => "Paused",
                        SyncLifecycleState::Error => "Needs attention",
                    };
                    panel.child(
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
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .child(div().font_weight(FontWeight::MEDIUM).child("Sync status"))
                                    .child(div().text_color(cx.theme().primary).child(state_label)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_4()
                                    .text_size(px(12.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("Provider: {}", status.provider_id.as_deref().unwrap_or("Not configured")))
                                    .child(format!("Vault: {}", status.vault_id.as_deref().unwrap_or("Not configured")))
                                    .child(format!("Pending: {}", status.pending_operation_count))
                                    .child(format!("Conflicts: {}", status.conflict_count)),
                            )
                            .when_some(status.last_error.as_ref(), |body, issue| {
                                body.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().danger)
                                        .child(issue.message.clone()),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("settings-sync-run-now")
                                            .primary()
                                            .small()
                                            .label("Sync now")
                                            .disabled(busy || status.state != SyncLifecycleState::Idle)
                                            .on_click(cx.listener(|shell, _, _, cx| shell.settings_modal.run_sync_action(SyncAction::Run, cx))),
                                    )
                                    .when(status.state == SyncLifecycleState::Idle || status.state == SyncLifecycleState::Paused, |actions| {
                                        let paused = status.state == SyncLifecycleState::Idle;
                                        actions.child(
                                            Button::new("settings-sync-toggle-pause")
                                                .outline()
                                                .small()
                                                .label(if paused { "Pause sync" } else { "Resume sync" })
                                                .disabled(busy)
                                                .on_click(cx.listener(move |shell, _, _, cx| shell.settings_modal.run_sync_action(SyncAction::Pause(paused), cx))),
                                        )
                                    })
                                    .when(status.state == SyncLifecycleState::Idle || status.state == SyncLifecycleState::Paused, |actions| {
                                        actions.child(
                                            Button::new("settings-sync-lock")
                                                .ghost()
                                                .small()
                                                .label("Lock vault")
                                                .disabled(busy)
                                                .on_click(cx.listener(|shell, _, _, cx| shell.settings_modal.run_sync_action(SyncAction::Lock, cx))),
                                        )
                                    }),
                            ),
                    )
                })
                .when(enabled && !loading && status_state == Some(SyncLifecycleState::Disabled), |panel| {
                    panel.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .text_color(cx.theme().muted_foreground)
                            .child("Connect a storage provider to create or join a sync vault."),
                    )
                })
                .when(enabled && !conflicts.is_empty(), |panel| {
                    panel.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().font_weight(FontWeight::MEDIUM).child("Conflicts"))
                            .children(conflicts.iter().map(|conflict| {
                                let conflict_id = conflict.conflict_id.clone();
                                let entity = format!("{:?}", conflict.entity);
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .p_3()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .child(div().text_size(px(12.0)).child(format!("{} · {:?}", entity, conflict.kind)))
                                    .child(
                                        div()
                                            .flex()
                                            .gap_1()
                                            .child(Button::new(ElementId::Name(format!("sync-conflict-local-{}", conflict.conflict_id).into())).ghost().small().label("Keep current").disabled(busy).on_click(cx.listener({ let conflict_id = conflict_id.clone(); move |shell, _, _, cx| shell.settings_modal.run_sync_action(SyncAction::Resolve(conflict_id.clone(), SyncConflictResolution::KeepCurrent), cx) })))
                                            .child(Button::new(ElementId::Name(format!("sync-conflict-remote-{}", conflict.conflict_id).into())).ghost().small().label("Use remote").disabled(busy).on_click(cx.listener({ let conflict_id = conflict_id.clone(); move |shell, _, _, cx| shell.settings_modal.run_sync_action(SyncAction::Resolve(conflict_id.clone(), SyncConflictResolution::UseConflicting), cx) })))
                                            .child(Button::new(ElementId::Name(format!("sync-conflict-both-{}", conflict.conflict_id).into())).ghost().small().label("Keep both").disabled(busy).on_click(cx.listener(move |shell, _, _, cx| shell.settings_modal.run_sync_action(SyncAction::Resolve(conflict_id.clone(), SyncConflictResolution::KeepBoth), cx)))),
                                    )
                            })),
                    )
                }),
        )
        .into_any_element()
}
