//! Complete Main Application Shell for Sona Desktop.
//! Provides full 1:1 parity with the React frontend, powered by pure Rust and GPUI Kit.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::create::{BatchImportPanel, LiveRecordPanel, OptionsCapsule};
use super::dialogs::{
    DiagnosticsModal, ErrorDialog, ExportModal, GlobalDialog, NotificationCenter, OnboardingWizard,
    ProjectDialogs, RecoveryModal,
};
use super::projects::{ProjectsRail, ProjectsResults, ProjectsToolbar, SelectionBar};
use super::settings::SettingsModal;
use super::tools::{
    ApiServerToolPanel, McpToolPanel, MediaServerToolPanel, SubtitlesToolPanel, ToolKind,
    ToolsSidebar, VoiceTypingToolPanel,
};
use super::transcript::TranscriptWorkbench;
use crate::app::GlobalDesktopServices;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AppMode {
    Create,
    Projects,
    Tools,
    Workbench,
}

pub struct MainShell {
    pub mode: AppMode,
    pub status_text: String,
    pub project_count: usize,
    pub loading: bool,

    // Sub-views
    pub live_record: LiveRecordPanel,
    pub batch_import: BatchImportPanel,
    pub options_capsule: OptionsCapsule,

    pub projects_rail: ProjectsRail,
    pub projects_toolbar: ProjectsToolbar,
    pub projects_results: ProjectsResults,
    pub selection_bar: SelectionBar,

    pub tools_sidebar: ToolsSidebar,
    pub subtitles_tool: SubtitlesToolPanel,
    pub voice_typing_tool: VoiceTypingToolPanel,
    pub api_server_tool: ApiServerToolPanel,
    pub mcp_tool: McpToolPanel,
    pub media_server_tool: MediaServerToolPanel,

    pub workbench: TranscriptWorkbench,

    // Modals & Overlays
    pub settings_modal: SettingsModal,
    pub diagnostics_modal: DiagnosticsModal,
    pub recovery_modal: RecoveryModal,
    pub notification_center: NotificationCenter,
    pub global_dialog: GlobalDialog,
    pub export_modal: ExportModal,
    pub onboarding: OnboardingWizard,
    pub project_dialogs: ProjectDialogs,
    pub error_dialog: ErrorDialog,
}

impl MainShell {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = cx.global::<GlobalDesktopServices>().0.clone();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let projects = services.projects.list().await.unwrap_or_default();
            let count = projects.len();

            this.update(cx, |view, cx| {
                view.project_count = count;
                view.loading = false;
                view.status_text =
                    format!("Decoupled core connected: SQLite WAL active, {count} projects loaded");
                cx.notify();
            })
            .ok();
        })
        .detach();

        Self {
            mode: AppMode::Create,
            status_text: "Connecting to decoupled backend core...".to_string(),
            project_count: 0,
            loading: true,

            live_record: LiveRecordPanel::new(),
            batch_import: BatchImportPanel::new(),
            options_capsule: OptionsCapsule::new(),

            projects_rail: ProjectsRail::new(),
            projects_toolbar: ProjectsToolbar::new(),
            projects_results: ProjectsResults::new(),
            selection_bar: SelectionBar::new(),

            tools_sidebar: ToolsSidebar::new(),
            subtitles_tool: SubtitlesToolPanel::new(),
            voice_typing_tool: VoiceTypingToolPanel::new(),
            api_server_tool: ApiServerToolPanel::new(),
            mcp_tool: McpToolPanel::new(),
            media_server_tool: MediaServerToolPanel::new(),

            workbench: TranscriptWorkbench::new(),

            settings_modal: SettingsModal::new(),
            diagnostics_modal: DiagnosticsModal::new(),
            recovery_modal: RecoveryModal::new(),
            notification_center: NotificationCenter::new(),
            global_dialog: GlobalDialog::new(),
            export_modal: ExportModal::new(),
            onboarding: OnboardingWizard::new(),
            project_dialogs: ProjectDialogs::new(),
            error_dialog: ErrorDialog::new(),
        }
    }

    pub fn set_mode(&mut self, mode: AppMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }
}

impl Render for MainShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modes = [
            (AppMode::Create, "Create"),
            (AppMode::Projects, "Projects"),
            (AppMode::Tools, "Tools"),
            (AppMode::Workbench, "Editor"),
        ];

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                // Top Application Header Bar
                div()
                    .flex()
                    .h(px(52.0))
                    .w_full()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().title_bar)
                    .child(
                        // App Brand & Core Link Status
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Sona"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(cx.theme().secondary)
                                    .text_size(px(11.0))
                                    .text_color(if self.loading {
                                        cx.theme().muted
                                    } else {
                                        cx.theme().primary
                                    })
                                    .child(if self.loading {
                                        "Connecting Core..."
                                    } else {
                                        "Pure Rust Core"
                                    }),
                            ),
                    )
                    .child(
                        // Mode Switcher Tabs
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .p_1()
                            .rounded_lg()
                            .bg(cx.theme().input)
                            .children(modes.into_iter().map(|(m, label)| {
                                let is_active = self.mode == m;
                                div()
                                    .id(ElementId::from(format!("nav-tab-{:?}", m)))
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(if is_active {
                                        cx.theme().background
                                    } else {
                                        gpui_kit::transparent_black()
                                    })
                                    .text_size(px(13.0))
                                    .font_weight(if is_active {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::MEDIUM
                                    })
                                    .text_color(if is_active {
                                        cx.theme().foreground
                                    } else {
                                        cx.theme().muted
                                    })
                                    .cursor_pointer()
                                    .child(label)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_mode(m, cx);
                                    }))
                            })),
                    )
                    .child(
                        // Right Actions: Notifications, Settings
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("btn-open-diagnostics")
                                    .ghost()
                                    .small()
                                    .label("Health")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.diagnostics_modal.toggle();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-open-notifications")
                                    .ghost()
                                    .small()
                                    .label("🔔")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.notification_center.toggle();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("btn-open-settings")
                                    .outline()
                                    .small()
                                    .label("⚙ Settings")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_modal.open();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                // Main Viewport depending on active AppMode
                div()
                    .flex()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .child(match self.mode {
                        AppMode::Create => div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .w_full()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .h_full()
                                            .border_r_1()
                                            .border_color(cx.theme().border)
                                            .child(self.live_record.render(cx)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .h_full()
                                            .child(self.batch_import.render(cx)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p_3()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background)
                                    .child(self.options_capsule.render(cx)),
                            )
                            .into_any_element(),
                        AppMode::Projects => div()
                            .flex()
                            .size_full()
                            .child(self.projects_rail.render(cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .h_full()
                                    .child(self.projects_toolbar.render(cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(self.projects_results.render(cx)),
                                    )
                                    .when(self.projects_toolbar.is_selection_active, |this| {
                                        this.child(self.selection_bar.render(cx))
                                    }),
                            )
                            .into_any_element(),
                        AppMode::Tools => {
                            let tool_panel: AnyElement = match self.tools_sidebar.active_tool {
                                ToolKind::Subtitles => {
                                    self.subtitles_tool.render(cx).into_any_element()
                                }
                                ToolKind::VoiceTyping => {
                                    self.voice_typing_tool.render(cx).into_any_element()
                                }
                                ToolKind::ApiServer => {
                                    self.api_server_tool.render(cx).into_any_element()
                                }
                                ToolKind::Mcp => self.mcp_tool.render(cx).into_any_element(),
                                ToolKind::MediaServer => {
                                    self.media_server_tool.render(cx).into_any_element()
                                }
                            };

                            div()
                                .flex()
                                .size_full()
                                .child(self.tools_sidebar.render(cx))
                                .child(div().flex_1().h_full().child(tool_panel))
                                .into_any_element()
                        }
                        AppMode::Workbench => self.workbench.render(cx).into_any_element(),
                    }),
            )
            .child(
                // Modals
                self.settings_modal.render(cx),
            )
            .child(self.diagnostics_modal.render(cx))
            .child(self.recovery_modal.render(cx))
            .child(self.notification_center.render(cx))
            .child(self.global_dialog.render(cx))
            .child(self.export_modal.render(cx))
            .child(self.onboarding.render(cx))
            .child(self.project_dialogs.render(cx))
            .child(self.error_dialog.render(cx))
    }
}
