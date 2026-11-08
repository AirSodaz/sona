//! Project creation, editing, and deletion modals.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::base::Disableable as _;
use gpui_kit::base::FocusableExt as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{RADIUS_LG, RADIUS_MD};
use sona_core::project::{
    ProjectCreateInput, ProjectPipelineConfig, ProjectRecord, ProjectUpdateInput,
};

pub struct ProjectDialogs {
    pub is_create_open: bool,
    pub is_delete_open: bool,
    pub is_settings_open: bool,

    // Create State
    pub project_name_state: Entity<InputState>,
    pub is_submitting: bool,

    // Delete State
    pub delete_project_id: Option<String>,
    pub delete_project_name: String,
    pub delete_item_count: usize,
    pub delete_cascade_to_inbox: bool,

    // Settings State
    pub settings_project_id: Option<String>,
    pub settings_name_state: Entity<InputState>,
    pub settings_description_state: Entity<InputState>,
    pub settings_color: String,
    pub settings_pipeline_enabled: bool,
    pub settings_auto_polish: bool,
    pub settings_auto_translate: bool,
    pub settings_auto_summary: bool,
    pub settings_auto_export: bool,
    pub settings_target_language: String,
    pub settings_export_format: String,
    pub settings_export_directory_state: Entity<InputState>,
}

impl ProjectDialogs {
    pub fn new(window: &mut Window, cx: &mut Context<super::super::shell::MainShell>) -> Self {
        Self {
            is_create_open: false,
            is_delete_open: false,
            is_settings_open: false,
            project_name_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Project name")),
            is_submitting: false,
            delete_project_id: None,
            delete_project_name: String::new(),
            delete_item_count: 0,
            delete_cascade_to_inbox: true,
            settings_project_id: None,
            settings_name_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Project name")),
            settings_description_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Short description")),
            settings_color: "#64748b".to_string(),
            settings_pipeline_enabled: false,
            settings_auto_polish: false,
            settings_auto_translate: false,
            settings_auto_summary: false,
            settings_auto_export: false,
            settings_target_language: "en".to_string(),
            settings_export_format: "txt".to_string(),
            settings_export_directory_state: cx
                .new(|cx| InputState::new(window, cx).placeholder("Export directory")),
        }
    }

    pub fn open_create(
        &mut self,
        window: &mut Window,
        cx: &mut Context<super::super::shell::MainShell>,
    ) {
        self.is_create_open = true;
        self.is_delete_open = false;
        self.is_settings_open = false;
        self.project_name_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
    }

    pub fn open_delete(
        &mut self,
        project_id: String,
        project_name: String,
        item_count: usize,
        cx: &mut Context<super::super::shell::MainShell>,
    ) {
        self.delete_project_id = Some(project_id);
        self.delete_project_name = project_name;
        self.delete_item_count = item_count;
        self.delete_cascade_to_inbox = true;
        self.is_delete_open = true;
        self.is_create_open = false;
        self.is_settings_open = false;
        cx.notify();
    }

    pub fn open_settings(
        &mut self,
        project: &ProjectRecord,
        window: &mut Window,
        cx: &mut Context<super::super::shell::MainShell>,
    ) {
        self.settings_project_id = Some(project.id.clone());
        self.settings_color = project
            .color
            .clone()
            .unwrap_or_else(|| "#64748b".to_string());
        let pipeline = project.pipeline.clone().unwrap_or_default();
        self.settings_pipeline_enabled = pipeline.enabled;
        self.settings_auto_polish = pipeline.auto_polish;
        self.settings_auto_translate = pipeline.auto_translate;
        self.settings_auto_summary = pipeline.auto_summary;
        self.settings_auto_export = pipeline.auto_export;
        self.settings_target_language =
            pipeline.target_language.unwrap_or_else(|| "en".to_string());
        self.settings_export_format = pipeline.export_format.unwrap_or_else(|| "txt".to_string());
        let export_dir = pipeline.export_directory.unwrap_or_default();
        self.settings_export_directory_state
            .update(cx, |state, cx| {
                state.set_value(export_dir, window, cx);
            });
        self.settings_name_state.update(cx, |state, cx| {
            state.set_value(project.name.clone(), window, cx);
            state.focus(window, cx);
        });
        self.settings_description_state.update(cx, |state, cx| {
            state.set_value(project.description.clone(), window, cx);
        });
        self.is_settings_open = true;
        self.is_create_open = false;
        self.is_delete_open = false;
        cx.notify();
    }

    pub fn close(&mut self) {
        self.is_create_open = false;
        self.is_delete_open = false;
        self.is_settings_open = false;
    }

    pub fn render(&mut self, cx: &mut Context<super::super::shell::MainShell>) -> impl IntoElement {
        if !self.is_create_open && !self.is_settings_open && !self.is_delete_open {
            return div();
        }
        div()
            .when(self.is_create_open, |this| {
                this.child(
                    div()
                        .id("modal-backdrop-create-project")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.project_dialogs.close();
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(420.0))
                                .p_6()
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_2xl()
                                .gap_4()
                                .id("create-project-card")
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .text_size(px(16.0))
                                        .font_weight(FontWeight::BOLD)
                                        .child("Create New Workspace"),
                                )
                                .child(Input::new(&self.project_name_state).focus_ring(true))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("btn-cancel-proj")
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.project_dialogs.close();
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-submit-proj")
                                                .primary()
                                                .small()
                                                .label(if self.is_submitting {
                                                    "Creating…"
                                                } else {
                                                    "Create Tag"
                                                })
                                                .disabled(self.is_submitting)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let name = this
                                                        .project_dialogs
                                                        .project_name_state
                                                        .read(cx)
                                                        .value()
                                                        .trim()
                                                        .to_string();
                                                    if !name.is_empty() {
                                                        let projects = cx
                                                            .global::<GlobalDesktopServices>()
                                                            .0
                                                            .projects
                                                            .clone();
                                                        this.project_dialogs.is_submitting = true;
                                                        cx.spawn(async move |shell, cx| {
                                                            let result = projects
                                                                .create(ProjectCreateInput {
                                                                    name,
                                                                    ..Default::default()
                                                                })
                                                                .await;
                                                            shell
                                                                .update(cx, |shell, cx| {
                                                                    shell
                                                                        .project_dialogs
                                                                        .is_submitting = false;
                                                                    if result.is_ok() {
                                                                        shell.project_dialogs.close();
                                                                        shell
                                                                            .project_selector
                                                                            .load_data(cx);
                                                                    }
                                                                    cx.notify();
                                                                })
                                                                .ok();
                                                        })
                                                        .detach();
                                                    }
                                                })),
                                        ),
                                ),
                        ),
                )
            })
            .when(self.is_settings_open, |this| {
                let project_id_for_delete = self.settings_project_id.clone();
                let project_name_for_delete = self.settings_name_state.read(cx).value().to_string();
                let is_submitting = self.is_submitting;

                this.child(
                    div()
                        .id("modal-backdrop-settings-project")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.project_dialogs.close();
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(520.0))
                                .p_6()
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_2xl()
                                .gap_4()
                                .id("settings-project-card")
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_size(px(18.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().foreground)
                                                .child("Project Settings"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .text_color(cx.theme().muted)
                                                .child("Configure project details and its deterministic processing pipeline."),
                                        ),
                                )
                                // Basic Info: Name & Description
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().foreground)
                                                .child("Project name"),
                                        )
                                        .child(Input::new(&self.settings_name_state).focus_ring(true)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(cx.theme().foreground)
                                                .child("Description"),
                                        )
                                        .child(Input::new(&self.settings_description_state).focus_ring(true)),
                                )
                                // Pipeline Configuration Section
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .p_3()
                                        .rounded(RADIUS_MD)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().secondary)
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
                                                            Icon::new(LucideIcon::Zap)
                                                                .size(px(15.0))
                                                                .text_color(cx.theme().primary),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(13.0))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(cx.theme().foreground)
                                                                .child("Enable Project Pipeline"),
                                                        ),
                                                )
                                                .child(
                                                    Switch::new("switch-project-pipeline")
                                                        .checked(self.settings_pipeline_enabled)
                                                        .on_click(cx.listener(|this, checked, _, cx| {
                                                            this.project_dialogs.settings_pipeline_enabled = *checked;
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(cx.theme().muted)
                                                .child("Customize auto-polish, translation language, and summary template for transcripts in this project."),
                                        )
                                        .when(self.settings_pipeline_enabled, |pipeline| {
                                            pipeline
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .pt_2()
                                                        .border_t_1()
                                                        .border_color(cx.theme().border)
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(div().text_size(px(13.0)).child("Auto Polish"))
                                                                .child(
                                                                    Switch::new("switch-auto-polish")
                                                                        .checked(self.settings_auto_polish)
                                                                        .on_click(cx.listener(|this, checked, _, cx| {
                                                                            this.project_dialogs.settings_auto_polish = *checked;
                                                                            cx.notify();
                                                                        })),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(div().text_size(px(13.0)).child("Auto Translate"))
                                                                .child(
                                                                    Switch::new("switch-auto-translate")
                                                                        .checked(self.settings_auto_translate)
                                                                        .on_click(cx.listener(|this, checked, _, cx| {
                                                                            this.project_dialogs.settings_auto_translate = *checked;
                                                                            cx.notify();
                                                                        })),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(div().text_size(px(13.0)).child("Auto Summary"))
                                                                .child(
                                                                    Switch::new("switch-auto-summary")
                                                                        .checked(self.settings_auto_summary)
                                                                        .on_click(cx.listener(|this, checked, _, cx| {
                                                                            this.project_dialogs.settings_auto_summary = *checked;
                                                                            cx.notify();
                                                                        })),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(div().text_size(px(13.0)).child("Auto Export"))
                                                                .child(
                                                                    Switch::new("switch-auto-export")
                                                                        .checked(self.settings_auto_export)
                                                                        .on_click(cx.listener(|this, checked, _, cx| {
                                                                            this.project_dialogs.settings_auto_export = *checked;
                                                                            cx.notify();
                                                                        })),
                                                                ),
                                                        ),
                                                )
                                        }),
                                )
                                // Footer
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .pt_2()
                                        .child(
                                            Button::new("btn-delete-project-from-settings")
                                                .danger()
                                                .outline()
                                                .small()
                                                .label("Delete Project")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if let Some(pid) = project_id_for_delete.clone() {
                                                        this.project_dialogs.close();
                                                        this.project_dialogs.open_delete(pid, project_name_for_delete.clone(), 0, cx);
                                                    }
                                                })),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    Button::new("btn-cancel-settings-proj")
                                                        .ghost()
                                                        .small()
                                                        .label("Cancel")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.project_dialogs.close();
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    Button::new("btn-save-settings-proj")
                                                        .primary()
                                                        .small()
                                                        .label(if is_submitting { "Saving…" } else { "Save" })
                                                        .disabled(is_submitting)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            let Some(project_id) = this.project_dialogs.settings_project_id.clone() else {
                                                                return;
                                                            };
                                                            let name = this.project_dialogs.settings_name_state.read(cx).value().trim().to_string();
                                                            if name.is_empty() {
                                                                return;
                                                            }
                                                            let description = this.project_dialogs.settings_description_state.read(cx).value().trim().to_string();
                                                            let color = this.project_dialogs.settings_color.clone();
                                                            let pipeline = ProjectPipelineConfig {
                                                                enabled: this.project_dialogs.settings_pipeline_enabled,
                                                                auto_polish: this.project_dialogs.settings_auto_polish,
                                                                auto_translate: this.project_dialogs.settings_auto_translate,
                                                                auto_summary: this.project_dialogs.settings_auto_summary,
                                                                auto_export: this.project_dialogs.settings_auto_export,
                                                                target_language: Some(this.project_dialogs.settings_target_language.clone()),
                                                                export_format: Some(this.project_dialogs.settings_export_format.clone()),
                                                                export_directory: None,
                                                                ..Default::default()
                                                            };
                                                            let input = ProjectUpdateInput {
                                                                name: Some(name),
                                                                description: Some(description),
                                                                color: Some(color),
                                                                pipeline: Some(pipeline),
                                                                ..Default::default()
                                                            };
                                                            this.project_dialogs.is_submitting = true;
                                                            let projects = cx.global::<GlobalDesktopServices>().0.projects.clone();
                                                            cx.spawn(async move |shell, cx| {
                                                                let result = projects.update(project_id, input).await;
                                                                shell.update(cx, |shell, cx| {
                                                                    shell.project_dialogs.is_submitting = false;
                                                                    if result.is_ok() {
                                                                        shell.project_dialogs.close();
                                                                        shell.project_selector.load_data(cx);
                                                                        shell.projects_results.load_data(cx);
                                                                    }
                                                                    cx.notify();
                                                                }).ok();
                                                            }).detach();
                                                        })),
                                                ),
                                        ),
                                ),
                        ),
                )
            })
            .when(self.is_delete_open, |this| {
                let project_name = self.delete_project_name.clone();
                let item_count = self.delete_item_count;
                let cascade_to_inbox = self.delete_cascade_to_inbox;
                let is_submitting = self.is_submitting;

                this.child(
                    div()
                        .id("modal-backdrop-delete-project")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(hsla(0.0, 0.0, 0.0, 0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.project_dialogs.close();
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(440.0))
                                .p_6()
                                .rounded(RADIUS_LG)
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_2xl()
                                .gap_4()
                                .id("delete-project-card")
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Icon::new(LucideIcon::TriangleAlert)
                                                .size(px(20.0))
                                                .text_color(cx.theme().danger),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(16.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(cx.theme().danger)
                                                .child("Delete Workspace"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .line_height(relative(1.4))
                                        .text_color(cx.theme().foreground)
                                        .child(format!(
                                            "Are you sure you want to delete \"{}\"?",
                                            project_name
                                        )),
                                )
                                .when(item_count > 0, |card| {
                                    card.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .p_3()
                                            .rounded(RADIUS_MD)
                                            .bg(cx.theme().secondary)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(cx.theme().muted)
                                                    .child(format!(
                                                        "This workspace contains {} items. Choose how to handle them:",
                                                        item_count
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_start()
                                                    .gap_2()
                                                    .id("del-opt-inbox")
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.project_dialogs.delete_cascade_to_inbox = true;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        div()
                                                            .size(px(14.0))
                                                            .mt(px(2.0))
                                                            .rounded_full()
                                                            .border_2()
                                                            .border_color(if cascade_to_inbox {
                                                                cx.theme().primary
                                                            } else {
                                                                cx.theme().muted
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .when(cascade_to_inbox, |bullet| {
                                                                bullet.child(
                                                                    div()
                                                                        .size(px(6.0))
                                                                        .rounded_full()
                                                                        .bg(cx.theme().primary),
                                                                )
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_size(px(13.0))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .child("Move to Inbox (Recommended)"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .text_color(cx.theme().muted)
                                                                    .child("Keep all transcripts; reset their workspace to Inbox."),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_start()
                                                    .gap_2()
                                                    .id("del-opt-trash")
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.project_dialogs.delete_cascade_to_inbox = false;
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        div()
                                                            .size(px(14.0))
                                                            .mt(px(2.0))
                                                            .rounded_full()
                                                            .border_2()
                                                            .border_color(if !cascade_to_inbox {
                                                                cx.theme().danger
                                                            } else {
                                                                cx.theme().muted
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .when(!cascade_to_inbox, |bullet| {
                                                                bullet.child(
                                                                    div()
                                                                        .size(px(6.0))
                                                                        .rounded_full()
                                                                        .bg(cx.theme().danger),
                                                                )
                                                            }),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .text_size(px(13.0))
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(cx.theme().danger)
                                                                    .child("Move all items to Trash"),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .text_color(cx.theme().muted)
                                                                    .child("Soft-delete all associated items and send them to Trash."),
                                                            ),
                                                    ),
                                            ),
                                    )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            Button::new("btn-cancel-del-proj")
                                                .ghost()
                                                .small()
                                                .label("Cancel")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.project_dialogs.close();
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            Button::new("btn-confirm-del-proj")
                                                .danger()
                                                .small()
                                                .label(if is_submitting {
                                                    "Deleting…"
                                                } else {
                                                    "Delete"
                                                })
                                                .disabled(is_submitting)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let Some(project_id) =
                                                        this.project_dialogs.delete_project_id.clone()
                                                    else {
                                                        return;
                                                    };
                                                    let cascade_action = if this
                                                        .project_dialogs
                                                        .delete_cascade_to_inbox
                                                    {
                                                        "moveToInbox"
                                                    } else {
                                                        "deleteItems"
                                                    };
                                                    this.project_dialogs.is_submitting = true;
                                                    let projects = cx
                                                        .global::<GlobalDesktopServices>()
                                                        .0
                                                        .projects
                                                        .clone();
                                                    cx.spawn(async move |shell, cx| {
                                                        let result = projects
                                                            .delete_with_cascade(
                                                                project_id.clone(),
                                                                cascade_action.to_string(),
                                                            )
                                                            .await;
                                                        shell
                                                            .update(cx, |shell, cx| {
                                                                shell
                                                                    .project_dialogs
                                                                    .is_submitting = false;
                                                                if result.is_ok() {
                                                                    shell.project_dialogs.close();
                                                                    if shell
                                                                        .project_selector
                                                                        .active_project_id
                                                                        .as_deref()
                                                                        == Some(&project_id)
                                                                    {
                                                                        shell
                                                                            .project_selector
                                                                            .active_project_id =
                                                                            None;
                                                                    }
                                                                    shell
                                                                        .project_selector
                                                                        .load_data(cx);
                                                                    shell
                                                                        .projects_results
                                                                        .load_data(cx);
                                                                }
                                                                cx.notify();
                                                            })
                                                            .ok();
                                                    })
                                                    .detach();
                                                })),
                                        ),
                                ),
                        ),
                )
            })
    }
}
