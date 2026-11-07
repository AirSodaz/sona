use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::*;

use super::{AboutUpdateStatus, MainShell};
use sona_core::runtime::app_update::AppUpdateCheckResult;

pub(super) fn about_panel(
    status: &AboutUpdateStatus,
    update: Option<&AppUpdateCheckResult>,
    progress: f32,
    cx: &mut Context<MainShell>,
) -> AnyElement {
    let channel = env!("SONA_BUILD_CHANNEL");
    let channel_label = match channel {
        "nightly" => "Nightly",
        "ci" => "CI Build",
        "dev" => "Dev",
        _ => "Stable",
    };
    let version = env!("SONA_DISPLAY_VERSION");
    let updates_supported =
        env!("SONA_IS_UPDATE_SUPPORTED") == "true" && matches!(channel, "stable" | "nightly");
    let colors = (cx.theme().background, cx.theme().border, cx.theme().muted);

    let update_area = if !updates_supported {
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(cx.theme().input)
            .text_color(colors.2)
            .child(format!(
                "Auto-update is disabled for {channel_label} builds."
            ))
            .into_any_element()
    } else {
        match status {
            AboutUpdateStatus::Idle => div()
                .child(
                    Button::new("about-check-updates")
                        .primary()
                        .label("Check for updates")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.settings_modal.check_about_updates(cx);
                            cx.notify();
                        })),
                )
                .into_any_element(),
            AboutUpdateStatus::Checking => div()
                .flex()
                .items_center()
                .gap_2()
                .text_color(colors.2)
                .child(Spinner::new())
                .child("Checking for updates...")
                .into_any_element(),
            AboutUpdateStatus::UpToDate => div()
                .flex()
                .items_center()
                .gap_2()
                .text_color(cx.theme().success)
                .child("Sona is up to date.")
                .into_any_element(),
            AboutUpdateStatus::Available => {
                let latest = update
                    .map(|update| update.latest_version.clone())
                    .unwrap_or_else(|| "unknown".into());
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(format!("Version {latest} is available."))
                    .child(
                        Button::new("about-install-update")
                            .primary()
                            .small()
                            .label("Install update")
                            .on_click(cx.listener(|shell, _, _, cx| {
                                shell.settings_modal.install_about_update(cx);
                            })),
                    )
                    .into_any_element()
            }
            AboutUpdateStatus::Downloading | AboutUpdateStatus::Installing => {
                let (label, progress) = if *status == AboutUpdateStatus::Downloading {
                    ("Downloading update...", progress)
                } else {
                    ("Installing update...", 100.0)
                };
                let fill_width = px(220.0 * progress.clamp(0.0, 100.0) / 100.0);
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(colors.2)
                            .child(label)
                            .child(format!("{progress:.0}%")),
                    )
                    .child(
                        div().h(px(6.0)).w_full().rounded_full().bg(colors.1).child(
                            div()
                                .h_full()
                                .w(fill_width)
                                .rounded_full()
                                .bg(cx.theme().accent),
                        ),
                    )
                    .into_any_element()
            }
            AboutUpdateStatus::Downloaded => div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .text_color(cx.theme().success)
                .child("Update installed. Restart Sona to finish.")
                .child(
                    Button::new("about-relaunch-update")
                        .primary()
                        .small()
                        .label("Relaunch now")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.settings_modal.relaunch_about_update(cx);
                        })),
                )
                .into_any_element(),
            AboutUpdateStatus::Error(_) => div()
                .child(
                    Button::new("about-retry-updates")
                        .primary()
                        .label("Check for updates")
                        .on_click(cx.listener(|shell, _, _, cx| {
                            shell.settings_modal.check_about_updates(cx);
                            cx.notify();
                        })),
                )
                .into_any_element(),
        }
    };

    div()
        .id("settings-about-panel")
        .flex()
        .flex_col()
        .items_center()
        .w_full()
        .max_w(px(480.0))
        .gap_5()
        .size_full()
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .py_5()
                .child(
                    div()
                        .flex()
                        .size_16()
                        .items_center()
                        .justify_center()
                        .rounded_xl()
                        .bg(colors.0)
                        .border_1()
                        .border_color(colors.1)
                        .child(Icon::new(IconName::AudioLines).size_8()),
                )
                .child(
                    div()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::BOLD)
                        .child("Sona"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(12.0))
                        .text_color(colors.2)
                        .child(format!("v{version}"))
                        .child("/")
                        .child(channel_label),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(colors.2)
                        .child("A privacy-first, offline transcript editor."),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .child(
                    Button::new("about-homepage")
                        .outline()
                        .label("Source code · github.com/AirSodaz/sona")
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.spawn(async move |_, _| {
                                if let Err(error) = crate::platform::opener::open_url_async(
                                    "https://github.com/AirSodaz/sona".into(),
                                )
                                .await
                                {
                                    log::error!("Failed to open project homepage: {error}");
                                }
                            })
                            .detach();
                        })),
                )
                .child(
                    Button::new("about-open-logs")
                        .outline()
                        .label("Open log folder")
                        .on_click(cx.listener(|_, _, _, cx| {
                            let context = cx
                                .global::<crate::app::GlobalDesktopServices>()
                                .0
                                .sqlite
                                .current_context();
                            match context {
                                Ok(context) => {
                                    let log_dir = context.app_data_dir().join("logs");
                                    cx.spawn(async move |_, _| {
                                        if let Err(error) =
                                            crate::platform::system::status::open_log_folder(
                                                &log_dir,
                                            )
                                        {
                                            log::error!("Failed to open log folder: {error}");
                                        }
                                    })
                                    .detach();
                                }
                                Err(error) => log::error!("Failed to resolve log folder: {error}"),
                            }
                        })),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_3()
                        .rounded_lg()
                        .border_1()
                        .border_color(colors.1)
                        .bg(colors.0)
                        .child(Icon::new(IconName::Heart).size_5())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Built with open source software"),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(colors.2)
                                        .child("Powered by the open source community"),
                                ),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(colors.1)
                        .bg(colors.0)
                        .child(div().w_full().child(update_area)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(colors.2)
                        .child("MIT License · Built with open source software"),
                ),
        )
        .into_any_element()
}
