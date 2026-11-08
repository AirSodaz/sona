use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use gpui_kit::*;
use serde_json::{Value, json};
use sona_core::runtime::app_update::{AppUpdateCheckResult, UpdateChannel};
use tokio::sync::{Notify, watch};

use super::SettingsModal;
use super::types::AboutUpdateStatus;

#[derive(Clone, Debug, PartialEq)]
pub struct AboutSettingsTab {
    pub about_update_status: AboutUpdateStatus,
    pub about_update_result: Option<AppUpdateCheckResult>,
    pub about_update_progress: f32,
    pub about_installed_app: Option<PathBuf>,
}

impl Default for AboutSettingsTab {
    fn default() -> Self {
        Self {
            about_update_status: AboutUpdateStatus::Idle,
            about_update_result: None,
            about_update_progress: 0.0,
            about_installed_app: None,
        }
    }
}

impl AboutSettingsTab {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check_about_updates(&mut self, cx: &mut Context<SettingsModal>) {
        if matches!(
            self.about_update_status,
            AboutUpdateStatus::Checking
                | AboutUpdateStatus::Downloading
                | AboutUpdateStatus::Installing
        ) {
            return;
        }
        let channel = match env!("SONA_BUILD_CHANNEL") {
            "stable" => UpdateChannel::Stable,
            "nightly" => UpdateChannel::Nightly,
            _ => return,
        };
        self.about_update_status = AboutUpdateStatus::Checking;
        self.about_update_result = None;
        self.about_update_progress = 0.0;
        self.about_installed_app = None;
        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let config = services
                .config
                .load()
                .ok()
                .flatten()
                .unwrap_or_else(|| json!({}));
            let mirror = sona_model_downloads::parse_download_mirror(
                config
                    .get("modelDownloadMirror")
                    .and_then(Value::as_str)
                    .unwrap_or("auto"),
            );
            let result = sona_model_downloads::check_app_update(
                services.downloads.client(),
                None,
                channel,
                mirror,
            )
            .await
            .map_err(|error| error.to_string());
            this.update(cx, |modal, cx| {
                match result {
                    Ok(result) => {
                        modal.about.about_update_status = if result.update_available {
                            AboutUpdateStatus::Available
                        } else {
                            AboutUpdateStatus::UpToDate
                        };
                        modal.about.about_update_result = Some(result);
                    }
                    Err(error) => {
                        modal.about.about_update_status = AboutUpdateStatus::Error(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn install_about_update(&mut self, cx: &mut Context<SettingsModal>) {
        if self.about_update_status != AboutUpdateStatus::Available {
            return;
        }
        let Some(update) = self.about_update_result.clone() else {
            self.about_update_status = AboutUpdateStatus::Error(
                "The update information is no longer available. Check again.".into(),
            );
            cx.notify();
            return;
        };
        let context = match cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .sqlite
            .current_context()
        {
            Ok(context) => context,
            Err(error) => {
                self.about_update_status = AboutUpdateStatus::Error(error);
                cx.notify();
                return;
            }
        };
        let config = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .config
            .clone();
        let client = cx
            .global::<crate::app::GlobalDesktopServices>()
            .0
            .downloads
            .client()
            .clone();
        let update_dir = context.app_data_dir().join("updates");
        let version = safe_update_version(&update.latest_version);
        let package_path = update_dir.join(format!("sona-{version}{}", update_package_extension()));
        let running_exe = std::env::current_exe().unwrap_or_default();
        let update = update.clone();
        self.about_update_status = AboutUpdateStatus::Downloading;
        self.about_update_progress = 0.0;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let config = config.load().ok().flatten().unwrap_or_else(|| json!({}));
            let mirror = sona_model_downloads::parse_download_mirror(
                config
                    .get("modelDownloadMirror")
                    .and_then(Value::as_str)
                    .unwrap_or("auto"),
            );
            let cancel = Arc::new(Notify::new());
            let (progress_tx, mut progress_rx) = watch::channel((0_u64, 0_u64));
            let progress_callback: Box<dyn FnMut(u64, u64) + Send> =
                Box::new(move |current, total| {
                    progress_tx.send_replace((current, total));
                });
            let download = sona_model_downloads::download_update_package(
                &client,
                &update.asset_url,
                &package_path,
                &update.signature,
                None,
                mirror,
                cancel,
                Some(progress_callback),
            );
            tokio::pin!(download);
            let download_result = loop {
                tokio::select! {
                    result = &mut download => break result.map_err(|error| error.to_string()),
                    changed = progress_rx.changed() => {
                        if changed.is_ok() {
                            let (current, total) = *progress_rx.borrow_and_update();
                            let progress = if total == 0 {
                                0.0
                            } else {
                                (current as f64 / total as f64 * 100.0) as f32
                            };
                            this.update(cx, |modal, cx| {
                                modal.about.about_update_progress = progress.clamp(0.0, 100.0);
                                cx.notify();
                            }).ok();
                        }
                    }
                }
            };
            if let Err(error) = download_result {
                this.update(cx, |modal, cx| {
                    modal.about.about_update_status = AboutUpdateStatus::Error(error);
                    cx.notify();
                })
                .ok();
                return;
            }
            this.update(cx, |modal, cx| {
                modal.about.about_update_status = AboutUpdateStatus::Installing;
                cx.notify();
            })
            .ok();
            let package_path_for_install = package_path.clone();
            let install_result = tokio::task::spawn_blocking(move || {
                sona_model_downloads::install_update_package(
                    &package_path_for_install,
                    &running_exe,
                    false,
                )
                .map_err(|error| error.to_string())
            })
            .await;
            this.update(cx, |modal, cx| {
                match install_result {
                    Ok(Ok(installed_path)) => {
                        modal.about.about_update_status = AboutUpdateStatus::Downloaded;
                        modal.about.about_installed_app = Some(installed_path);
                    }
                    Ok(Err(error)) => {
                        modal.about.about_update_status = AboutUpdateStatus::Error(error)
                    }
                    Err(error) => {
                        modal.about.about_update_status =
                            AboutUpdateStatus::Error(error.to_string())
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn relaunch_about_update(&mut self, cx: &mut Context<SettingsModal>) {
        let Some(path) = self.about_installed_app.clone() else {
            self.about_update_status =
                AboutUpdateStatus::Error("Installed application path is missing.".into());
            cx.notify();
            return;
        };
        match spawn_updated_app(&path) {
            Ok(()) => cx.quit(),
            Err(error) => {
                self.about_update_status = AboutUpdateStatus::Error(error);
                cx.notify();
            }
        }
    }

    pub fn render(&mut self, cx: &mut Context<SettingsModal>) -> impl IntoElement {
        let status = &self.about_update_status;
        let update = self.about_update_result.as_ref();
        let progress = self.about_update_progress;

        div()
            .id("settings-about-panel")
            .flex().flex_col().size_full().min_h_0().overflow_y_scroll()
            .px(px(32.0)).py(px(24.0))
            .child(
                div().w_full().max_w(px(900.0)).mx_auto().flex().flex_col().gap_6()
                    .child(
                        div().flex().items_center().justify_between().pb_4().border_b_1().border_color(cx.theme().border)
                            .child(
                                div().flex().flex_col().gap_1()
                                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("About Sona"))
                                    .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Speech recognition and audio transcription workspace.")),
                            )
                            .child(
                                Button::new("settings-about-check-updates")
                                    .outline()
                                    .small()
                                    .label("Check for Updates")
                                    .disabled(matches!(status, AboutUpdateStatus::Checking | AboutUpdateStatus::Downloading | AboutUpdateStatus::Installing))
                                    .on_click(cx.listener(|modal, _, _, cx| {
                                        modal.about.check_about_updates(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div().flex().flex_col().gap_3().p_4().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                            .child(
                                div().flex().items_center().justify_between()
                                    .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Version"))
                                    .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(env!("CARGO_PKG_VERSION"))),
                            )
                            .child(
                                div().flex().items_center().justify_between()
                                    .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Release Channel"))
                                    .child(div().text_size(px(13.0)).child(env!("SONA_BUILD_CHANNEL"))),
                            )
                            .child(
                                div().flex().items_center().justify_between()
                                    .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Framework"))
                                    .child(div().text_size(px(13.0)).child("GPUI Kit 0.9.0")),
                            ),
                    )
                    .child(match status {
                        AboutUpdateStatus::Idle => div().into_any_element(),
                        AboutUpdateStatus::Checking => {
                            div().flex().items_center().gap_2().p_4().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                                .child(Spinner::new())
                                .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Checking for updates…"))
                                .into_any_element()
                        }
                        AboutUpdateStatus::UpToDate => {
                            div().p_4().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                                .child(div().text_size(px(13.0)).text_color(cx.theme().success).child("Sona is up to date."))
                                .into_any_element()
                        }
                        AboutUpdateStatus::Available => {
                            let version = update.map(|u| u.latest_version.as_str()).unwrap_or("newer");
                            div().flex().items_center().justify_between().p_4().rounded_md().border_1().border_color(cx.theme().primary).bg(cx.theme().primary.opacity(0.06))
                                .child(
                                    div().flex().flex_col().gap_1()
                                        .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child(format!("Update Available: v{version}")))
                                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("A new version of Sona is ready to download.")),
                                )
                                .child(
                                    Button::new("settings-about-install-update")
                                        .primary()
                                        .small()
                                        .label("Download & Install")
                                        .on_click(cx.listener(|modal, _, _, cx| {
                                            modal.about.install_about_update(cx);
                                        })),
                                )
                                .into_any_element()
                        }
                        AboutUpdateStatus::Downloading => {
                            div().flex().flex_col().gap_2().p_4().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                                .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(format!("Downloading update… {:.0}%", progress)))
                                .child(
                                    div().h(px(4.0)).w_full().rounded_full().bg(cx.theme().muted)
                                        .child(div().h_full().w(px(progress.clamp(0.0, 100.0) * 8.0)).rounded_full().bg(cx.theme().primary)),
                                )
                                .into_any_element()
                        }
                        AboutUpdateStatus::Installing => {
                            div().flex().items_center().gap_2().p_4().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box)
                                .child(Spinner::new())
                                .child(div().text_size(px(13.0)).child("Installing update…"))
                                .into_any_element()
                        }
                        AboutUpdateStatus::Downloaded => {
                            div().flex().items_center().justify_between().p_4().rounded_md().border_1().border_color(cx.theme().success).bg(cx.theme().success.opacity(0.06))
                                .child(
                                    div().flex().flex_col().gap_1()
                                        .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child("Update Ready"))
                                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Restart Sona to complete the update.")),
                                )
                                .child(
                                    Button::new("settings-about-relaunch")
                                        .primary()
                                        .small()
                                        .label("Restart Sona")
                                        .on_click(cx.listener(|modal, _, _, cx| {
                                            modal.about.relaunch_about_update(cx);
                                        })),
                                )
                                .into_any_element()
                        }
                        AboutUpdateStatus::Error(err) => {
                            div().p_4().rounded_md().border_1().border_color(cx.theme().danger).bg(cx.theme().danger.opacity(0.06))
                                .child(div().text_size(px(13.0)).text_color(cx.theme().danger).child(format!("Update failed: {err}")))
                                .into_any_element()
                        }
                    }),
            )
            .into_any_element()
    }
}

fn safe_update_version(version: &str) -> String {
    let sanitized = version
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "latest".to_string()
    } else {
        sanitized
    }
}

fn update_package_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        ".zip"
    }
    #[cfg(target_os = "macos")]
    {
        ".dmg"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        ".tar.gz"
    }
}

fn spawn_updated_app(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(path).spawn();

    #[cfg(not(target_os = "macos"))]
    let result = Command::new(path).spawn();

    result
        .map(|_| ())
        .map_err(|error| format!("Failed to relaunch the updated application: {error}"))
}
