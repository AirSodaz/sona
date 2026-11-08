//! Live diagnostics for audio devices, model files, local storage, and sync.

use std::path::PathBuf;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{StatefulInteractiveElement as _, *};
use serde_json::Value;
use sona_core::sync::SyncLifecycleState;

use super::super::shell::MainShell;

#[derive(Clone)]
struct DiagnosticCheck {
    name: &'static str,
    detail: String,
    ready: bool,
}

pub struct DiagnosticsModal {
    pub is_open: bool,
    is_loading: bool,
    error_message: Option<String>,
    checks: Vec<DiagnosticCheck>,
    log_dir: Option<PathBuf>,
}

impl DiagnosticsModal {
    pub fn new() -> Self {
        Self {
            is_open: false,
            is_loading: false,
            error_message: None,
            checks: Vec::new(),
            log_dir: None,
        }
    }

    pub fn open(&mut self, cx: &mut Context<MainShell>) {
        self.is_open = true;
        self.load(cx);
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn load(&mut self, cx: &mut Context<MainShell>) {
        self.is_loading = true;
        self.error_message = None;
        cx.notify();

        let services = cx.global::<crate::app::GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let config = services.config.load();
            let context = services.sqlite.current_context();
            let devices =
                tokio::task::spawn_blocking(crate::integrations::audio::get_microphone_devices)
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result);
            let sync_status = services.sync.get_status().await;

            let result = match (config, context) {
                (Ok(Some(config)), Ok(context)) => {
                    let microphone_check = match devices {
                        Ok(devices) if !devices.is_empty() => DiagnosticCheck {
                            name: "Microphone",
                            detail: format!("{} input device(s) available", devices.len()),
                            ready: true,
                        },
                        Ok(_) => DiagnosticCheck {
                            name: "Microphone",
                            detail: "No input devices were found".to_string(),
                            ready: false,
                        },
                        Err(error) => DiagnosticCheck {
                            name: "Microphone",
                            detail: error,
                            ready: false,
                        },
                    };

                    let model_paths = [
                        config.get("streamingModelPath").and_then(Value::as_str),
                        config.get("batchModelPath").and_then(Value::as_str),
                    ];
                    let configured_models = model_paths
                        .into_iter()
                        .flatten()
                        .map(str::trim)
                        .filter(|path| !path.is_empty())
                        .collect::<Vec<_>>();
                    let missing_models = configured_models
                        .iter()
                        .filter(|path| !std::path::Path::new(path).exists())
                        .count();
                    let models_check = if configured_models.is_empty() {
                        DiagnosticCheck {
                            name: "Transcription models",
                            detail: "No model paths are configured".to_string(),
                            ready: false,
                        }
                    } else if missing_models > 0 {
                        DiagnosticCheck {
                            name: "Transcription models",
                            detail: format!(
                                "{missing_models} configured model path(s) are missing"
                            ),
                            ready: false,
                        }
                    } else {
                        DiagnosticCheck {
                            name: "Transcription models",
                            detail: format!(
                                "{} configured model path(s) found",
                                configured_models.len()
                            ),
                            ready: true,
                        }
                    };

                    let data_dir = context.app_data_dir().to_path_buf();
                    let storage_check = DiagnosticCheck {
                        name: "Local database",
                        detail: if data_dir.join("sona.db").is_file() {
                            format!("Database found in {}", data_dir.display())
                        } else {
                            format!("Data directory is not available at {}", data_dir.display())
                        },
                        ready: data_dir.join("sona.db").is_file(),
                    };

                    let sync_check = match sync_status {
                        Ok(status) => DiagnosticCheck {
                            name: "Cloud sync",
                            detail: format!("Current state: {:?}", status.state),
                            ready: !matches!(
                                status.state,
                                SyncLifecycleState::Locked | SyncLifecycleState::Error
                            ),
                        },
                        Err(error) => DiagnosticCheck {
                            name: "Cloud sync",
                            detail: error,
                            ready: false,
                        },
                    };

                    Ok((
                        vec![microphone_check, models_check, storage_check, sync_check],
                        data_dir.join("logs"),
                    ))
                }
                (Err(error), _) | (_, Err(error)) => Err(error),
                (Ok(None), _) => Err("Application settings are unavailable".to_string()),
            };

            this.update(cx, |shell, cx| {
                shell.diagnostics_modal.is_loading = false;
                match result {
                    Ok((checks, log_dir)) => {
                        shell.diagnostics_modal.checks = checks;
                        shell.diagnostics_modal.log_dir = Some(log_dir);
                    }
                    Err(error) => shell.diagnostics_modal.error_message = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_logs(&mut self, cx: &mut Context<MainShell>) {
        let Some(path) = self.log_dir.clone() else {
            self.error_message =
                Some("Log directory is unavailable until diagnostics finish loading".into());
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(&path)?;
                crate::platform::opener::open_path(path)
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result.map_err(|error| error.to_string()));
            if let Err(error) = result {
                this.update(cx, |shell, cx| {
                    shell.diagnostics_modal.error_message = Some(error);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let checks = self.checks.clone();
        let is_loading = self.is_loading;
        let error_message = self.error_message.clone();

        div().when(self.is_open, |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .bg(hsla(0.0, 0.0, 0.0, 0.5))
                    .id("diagnostics-backdrop")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.diagnostics_modal.close();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(520.0))
                            .max_h(px(680.0))
                            .p_6()
                            .rounded_xl()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .shadow_xl()
                            .id("diagnostics-panel")
                            .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Icon::new(LucideIcon::Stethoscope).size(px(18.0)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(16.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .child("Model & Environment Diagnostics"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Button::new("btn-refresh-diagnostics")
                                                    .ghost()
                                                    .small()
                                                    .label(if is_loading {
                                                        "Scanning..."
                                                    } else {
                                                        "Refresh"
                                                    })
                                                    .disabled(is_loading)
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.diagnostics_modal.load(cx);
                                                    })),
                                            )
                                            .child(
                                                Button::new("btn-close-diagnostics")
                                                    .ghost()
                                                    .small()
                                                    .label("Close")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.diagnostics_modal.close();
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            .when_some(error_message, |this, error| {
                                this.child(
                                    div()
                                        .id("diagnostics-error")
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().danger.opacity(0.1))
                                        .text_color(cx.theme().danger)
                                        .child(error),
                                )
                            })
                            .when(is_loading && checks.is_empty(), |this| {
                                this.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .py_4()
                                        .text_color(cx.theme().muted)
                                        .child(Icon::new(LucideIcon::LoaderCircle).size(px(16.0)))
                                        .child(
                                            "Scanning local devices, models, storage, and sync...",
                                        ),
                                )
                            })
                            .child(div().flex().flex_col().gap_2().children(
                                checks.into_iter().map(|check| {
                                    div()
                                        .flex()
                                        .items_start()
                                        .justify_between()
                                        .gap_4()
                                        .p_3()
                                        .rounded_md()
                                        .bg(cx.theme().input)
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(check.name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(cx.theme().muted)
                                                        .child(check.detail),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(if check.ready {
                                                    cx.theme().success
                                                } else {
                                                    cx.theme().warning
                                                })
                                                .child(if check.ready { "Ready" } else { "Check" }),
                                        )
                                }),
                            ))
                            .child(
                                div().flex().justify_end().child(
                                    Button::new("btn-open-diagnostic-logs")
                                        .outline()
                                        .small()
                                        .label("Open Log Folder")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.diagnostics_modal.open_logs(cx);
                                        })),
                                ),
                            ),
                    ),
            )
        })
    }
}

impl Default for DiagnosticsModal {
    fn default() -> Self {
        Self::new()
    }
}
