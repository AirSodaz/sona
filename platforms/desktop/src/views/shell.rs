//! Complete Main Application Shell for Sona Desktop.
//! Provides strict 1:1 parity with the React frontend, powered by pure Rust and GPUI Kit.
//!
//! Milestone 2 Features:
//! - 56px 3-column header grid (1fr auto 1fr) with 24px padding (Gap G01, G05)
//! - ProjectSelectorDropdown with swatch, pipeline Zap, and counts (Gap G02)
//! - 3-Mode TabNavigation with Lucide SVG icons, sliding pill, and Ctrl+Tab cycling (Gap G03)
//! - SyncHeaderPill with 6 lifecycle states and sync actions (Gap G04)
//! - NotificationCenter with live task ledger grouping and red badge (Gap G04)
//! - Strict 350px left panel layout and sliding Transcript Workbench host (Gap G01, G05)

use std::collections::HashMap;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{StatefulInteractiveElement as _, *};
use sona_core::project::ProjectRecord;
use sona_core::sync::{SyncLifecycleState, SyncStatusSnapshot};

use super::create::{CreateWorkspacePanel, OptionsCapsule, TranscriptionOptionsModel};
use super::dialogs::{
    DiagnosticsModal, ErrorDialog, ExportModal, GlobalDialog, NotificationCenter, OnboardingWizard,
    ProjectDialogs, RecoveryModal,
};
use super::projects::{ProjectsRail, ProjectsResults, ProjectsToolbar, SelectionBar};
use super::settings::{SettingsModal, SettingsTab};
use super::tools::{
    ApiServerToolPanel, McpToolPanel, MediaServerToolPanel, SubtitlesToolPanel, ToolKind,
    ToolsSidebar, VoiceTypingToolPanel,
};
use super::transcript::TranscriptWorkbench;
use crate::app::GlobalDesktopServices;
use crate::theme::tokens::{
    COLOR_PROJECT_AMBER, HEADER_HEIGHT, PANEL_LEFT_WIDTH, PANEL_RIGHT_MIN_WIDTH, RADIUS_MD,
    RADIUS_SM, project_color_from_index,
};

#[inline]
fn icon(name: LucideIcon) -> Icon {
    Icon::default().path(name.path())
}

/// 3 Core Application Modes matching React desktop shell.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Create,
    Projects,
    Tools,
}

/// Project selector dropdown state and popover controller.
pub struct ProjectSelectorDropdown {
    pub is_open: bool,
    pub query: String,
    pub active_project_id: Option<String>,
    pub projects: Vec<ProjectRecord>,
    pub count_inbox: usize,
    pub project_counts: HashMap<String, usize>,
    pub is_recording: bool,
}

impl ProjectSelectorDropdown {
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            active_project_id: None,
            projects: Vec::new(),
            count_inbox: 0,
            project_counts: HashMap::new(),
            is_recording: false,
        }
    }

    pub fn load_data(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let projects = services.projects.list().await.unwrap_or_default();
            let history_items = services
                .history
                .query_db(|repo| repo.list_items(sona_core::history::HistoryListOptions::default()))
                .await
                .unwrap_or_default();

            let mut inbox_count = 0;
            let mut counts = HashMap::new();
            for item in history_items {
                if item.deleted_at.is_none() {
                    match &item.project_id {
                        Some(pid) if !pid.is_empty() => {
                            *counts.entry(pid.clone()).or_insert(0) += 1;
                        }
                        _ => inbox_count += 1,
                    }
                }
            }

            this.update(cx, |view, cx| {
                view.project_selector.projects = projects;
                view.project_selector.count_inbox = inbox_count;
                view.project_selector.project_counts = counts;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn select_project(&mut self, project_id: Option<String>, cx: &mut Context<MainShell>) {
        self.active_project_id = project_id;
        self.is_open = false;
        self.query.clear();
        cx.notify();
    }

    pub fn toggle(&mut self, cx: &mut Context<MainShell>) {
        if self.is_recording {
            return;
        }
        self.is_open = !self.is_open;
        if self.is_open {
            self.load_data(cx);
        }
        cx.notify();
    }

    pub fn active_project(&self) -> Option<&ProjectRecord> {
        self.active_project_id
            .as_ref()
            .and_then(|id| self.projects.iter().find(|p| &p.id == id))
    }

    pub fn filtered_projects(&self) -> Vec<&ProjectRecord> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.projects.iter().collect()
        } else {
            self.projects
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&q))
                .collect()
        }
    }
}

/// Dynamic cloud sync status pill and popover controller.
pub struct SyncHeaderPill {
    pub is_open: bool,
    pub is_action_busy: bool,
    pub status: Option<SyncStatusSnapshot>,
}

impl SyncHeaderPill {
    pub fn new() -> Self {
        Self {
            is_open: false,
            is_action_busy: false,
            status: None,
        }
    }

    pub fn refresh_status(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let status = services.sync.get_status().await.ok();
            this.update(cx, |view, cx| {
                view.sync_pill.status = status;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn run_sync_now(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        self.is_action_busy = true;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let _ = services.sync.run_now().await;
            let status = services.sync.get_status().await.ok();
            this.update(cx, |view, cx| {
                view.sync_pill.status = status;
                view.sync_pill.is_action_busy = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_pause(&mut self, cx: &mut Context<MainShell>) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let is_paused = self
            .status
            .as_ref()
            .map_or(false, |s| s.state == SyncLifecycleState::Paused);
        self.is_action_busy = true;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let _ = services.sync.set_paused(!is_paused).await;
            let status = services.sync.get_status().await.ok();
            this.update(cx, |view, cx| {
                view.sync_pill.status = status;
                view.sync_pill.is_action_busy = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle(&mut self, cx: &mut Context<MainShell>) {
        self.is_open = !self.is_open;
        if self.is_open {
            self.refresh_status(cx);
        }
        cx.notify();
    }
}

/// Root Application Shell view.
pub struct MainShell {
    pub mode: AppMode,
    pub status_text: String,
    pub project_count: usize,
    pub loading: bool,
    pub has_active_transcript: bool,

    // Header Components
    pub project_selector: ProjectSelectorDropdown,
    pub sync_pill: SyncHeaderPill,
    pub notification_center: Entity<NotificationCenter>,

    // Sub-views
    pub options_model: Entity<TranscriptionOptionsModel>,
    pub options_capsule: OptionsCapsule,
    pub create_workspace: Entity<CreateWorkspacePanel>,

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
    pub global_dialog: GlobalDialog,
    pub export_modal: ExportModal,
    pub onboarding: OnboardingWizard,
    pub project_dialogs: ProjectDialogs,
    pub error_dialog: ErrorDialog,
}

impl MainShell {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let notification_center = cx.new(|_| NotificationCenter::new());

        let options_model = cx.new(|_| TranscriptionOptionsModel::new());
        let options_capsule = OptionsCapsule::new(options_model.clone());
        let create_workspace = cx.new(|cx| CreateWorkspacePanel::new(options_model.clone(), cx));

        let mut shell = Self {
            mode: AppMode::Create,
            status_text: "Connecting to decoupled backend core...".to_string(),
            project_count: 0,
            loading: true,
            has_active_transcript: false,

            project_selector: ProjectSelectorDropdown::new(),
            sync_pill: SyncHeaderPill::new(),
            notification_center,

            options_model,
            options_capsule,
            create_workspace,

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
            global_dialog: GlobalDialog::new(),
            export_modal: ExportModal::new(),
            onboarding: OnboardingWizard::new(),
            project_dialogs: ProjectDialogs::new(),
            error_dialog: ErrorDialog::new(),
        };

        shell.project_selector.load_data(cx);
        shell.sync_pill.refresh_status(cx);
        shell
            .notification_center
            .update(cx, |nc, cx| nc.load_tasks(cx));

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

        shell
    }

    pub fn set_mode(&mut self, mode: AppMode, cx: &mut Context<Self>) {
        if self.mode == mode {
            // Re-click: scroll active container to top
            return;
        }
        self.mode = mode;
        cx.notify();
    }

    pub fn cycle_mode(&mut self, reverse: bool, cx: &mut Context<Self>) {
        let modes = [AppMode::Create, AppMode::Projects, AppMode::Tools];
        let current_idx = match self.mode {
            AppMode::Create => 0,
            AppMode::Projects => 1,
            AppMode::Tools => 2,
        };
        let next_idx = if reverse {
            (current_idx + modes.len() - 1) % modes.len()
        } else {
            (current_idx + 1) % modes.len()
        };
        self.set_mode(modes[next_idx], cx);
    }

    pub fn should_show_transcript_host(&self) -> bool {
        match self.mode {
            AppMode::Create => true,
            AppMode::Projects => self.has_active_transcript,
            AppMode::Tools => false,
        }
    }

    fn render_project_selector(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_proj = self.project_selector.active_project().cloned();
        let display_title = active_proj
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Inbox".to_string());
        let has_pipeline = active_proj
            .as_ref()
            .and_then(|p| p.pipeline.as_ref())
            .map_or(false, |p| p.enabled);

        let filtered: Vec<ProjectRecord> = self
            .project_selector
            .filtered_projects()
            .into_iter()
            .cloned()
            .collect();
        let is_inbox_active = self.project_selector.active_project_id.is_none();
        let is_open = self.project_selector.is_open;
        let is_rec = self.project_selector.is_recording;
        let count_inbox = self.project_selector.count_inbox;
        let project_counts = self.project_selector.project_counts.clone();
        let active_pid = self.project_selector.active_project_id.clone();
        let query = self.project_selector.query.clone();

        div()
            .relative()
            .child(
                // Trigger button (height 30px, max-width 190px, padding 4px 10px, gap 7px)
                div()
                    .id("project-selector-trigger")
                    .flex()
                    .items_center()
                    .h(px(30.0))
                    .max_w(px(190.0))
                    .px(px(10.0))
                    .py(px(4.0))
                    .gap(px(7.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(RADIUS_SM)
                    .bg(cx.theme().background)
                    .text_size(px(13.0))
                    .text_color(cx.theme().foreground)
                    .cursor_pointer()
                    .when(is_rec, |this| this.opacity(0.55).cursor_not_allowed())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.project_selector.toggle(cx);
                    }))
                    // Swatch or Inbox Icon
                    .child(if let Some(proj) = active_proj {
                        let swatch_color = project_color_from_index(proj.sort_order);
                        div()
                            .size(px(16.0))
                            .rounded(px(4.0))
                            .bg(swatch_color.opacity(0.14))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(px(8.0)).rounded_full().bg(swatch_color))
                            .into_any_element()
                    } else {
                        div()
                            .size(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(LucideIcon::Inbox).size(px(14.0)))
                            .into_any_element()
                    })
                    // Project Title
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(display_title),
                    )
                    // Zap Icon if automation enabled
                    .when(has_pipeline, |this| {
                        this.child(
                            icon(LucideIcon::Zap)
                                .size(px(12.0))
                                .text_color(COLOR_PROJECT_AMBER),
                        )
                    })
                    // Chevron Down Icon
                    .child(
                        icon(LucideIcon::ChevronDown)
                            .size(px(14.0))
                            .text_color(cx.theme().muted),
                    ),
            )
            // Popover panel (width 280px, padding 8px, border-radius 8px, shadow-lg)
            .when(is_open, |this| {
                this.child(
                    div()
                        .id("project-selector-popover")
                        .absolute()
                        .top(px(36.0))
                        .left_0()
                        .w(px(280.0))
                        .p_2()
                        .rounded(RADIUS_MD)
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap_1()
                        // Search bar input
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1p5()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    icon(LucideIcon::Search)
                                        .size(px(14.0))
                                        .text_color(cx.theme().muted),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(cx.theme().muted)
                                        .child(if query.is_empty() {
                                            "Search projects...".to_string()
                                        } else {
                                            query
                                        }),
                                ),
                        )
                        // Inbox item row
                        .child(
                            div()
                                .id("proj-row-inbox")
                                .flex()
                                .items_center()
                                .gap_2()
                                .h(px(32.0))
                                .px_2()
                                .rounded(RADIUS_SM)
                                .bg(if is_inbox_active {
                                    cx.theme().secondary
                                } else {
                                    gpui_kit::transparent_black()
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.project_selector.select_project(None, cx);
                                }))
                                .child(icon(LucideIcon::Inbox).size(px(14.0)))
                                .child(div().flex_1().text_size(px(13.0)).child("Inbox"))
                                .when(is_inbox_active, |this| {
                                    this.child(icon(LucideIcon::Check).size(px(14.0)))
                                })
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted)
                                        .child(count_inbox.to_string()),
                                ),
                        )
                        // User project rows
                        .child(
                            div()
                                .id("proj-scroll-list")
                                .flex()
                                .flex_col()
                                .max_h(px(220.0))
                                .overflow_y_scroll()
                                .children(filtered.into_iter().enumerate().map(|(i, proj)| {
                                    let is_active = active_pid.as_deref() == Some(&proj.id);
                                    let count = project_counts.get(&proj.id).copied().unwrap_or(0);
                                    let p_color = project_color_from_index(proj.sort_order);
                                    let p_has_zap =
                                        proj.pipeline.as_ref().map_or(false, |p| p.enabled);
                                    let pid = proj.id.clone();

                                    div()
                                        .id(ElementId::from(format!("proj-row-{}", i)))
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .h(px(32.0))
                                        .px_2()
                                        .rounded(RADIUS_SM)
                                        .bg(if is_active {
                                            cx.theme().secondary
                                        } else {
                                            gpui_kit::transparent_black()
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.project_selector
                                                .select_project(Some(pid.clone()), cx);
                                        }))
                                        .child(div().size(px(14.0)).rounded_full().bg(p_color))
                                        .child(
                                            div()
                                                .flex_1()
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .text_size(px(13.0))
                                                .child(proj.name.clone()),
                                        )
                                        .when(p_has_zap, |this| {
                                            this.child(
                                                icon(LucideIcon::Zap)
                                                    .size(px(12.0))
                                                    .text_color(COLOR_PROJECT_AMBER),
                                            )
                                        })
                                        .when(is_active, |this| {
                                            this.child(icon(LucideIcon::Check).size(px(14.0)))
                                        })
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(cx.theme().muted)
                                                .child(count.to_string()),
                                        )
                                })),
                        )
                        // Footer buttons: New Project & Open Project Center
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pt_1p5()
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .child(
                                    Button::new("btn-new-project-dropdown")
                                        .ghost()
                                        .xsmall()
                                        .label("+ New Project")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.project_selector.is_open = false;
                                            this.project_dialogs.open_create();
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("btn-open-project-center-dropdown")
                                        .ghost()
                                        .xsmall()
                                        .label("Project Center")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.project_selector.is_open = false;
                                            this.set_mode(AppMode::Projects, cx);
                                        })),
                                ),
                        ),
                )
            })
    }

    fn render_tab_navigation(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let modes = [
            (AppMode::Create, LucideIcon::SquarePen, "Create"),
            (AppMode::Projects, LucideIcon::Book, "Projects"),
            (AppMode::Tools, LucideIcon::Wrench, "Tools"),
        ];

        let active_index = match self.mode {
            AppMode::Create => 0,
            AppMode::Projects => 1,
            AppMode::Tools => 2,
        };

        div()
            .id("tab-navigation")
            .relative()
            .flex()
            .items_center()
            .p(px(4.0))
            .rounded(RADIUS_MD)
            .bg(cx.theme().input)
            .gap(px(4.0))
            // Sliding Pill Background Indicator (width 120px, height 32px, left animated)
            .child(
                div()
                    .id("tab-slider")
                    .absolute()
                    .top(px(4.0))
                    .left(px(4.0 + (active_index as f32) * (120.0 + 4.0)))
                    .w(px(120.0))
                    .h(px(32.0))
                    .rounded(RADIUS_SM)
                    .bg(cx.theme().background)
                    .shadow_sm(),
            )
            // 3 Mode Tab Buttons
            .children(
                modes
                    .into_iter()
                    .enumerate()
                    .map(|(_i, (mode, icon_asset, label))| {
                        let is_active = self.mode == mode;

                        div()
                            .id(ElementId::from(format!("tab-btn-{:?}", mode)))
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(8.0))
                            .h(px(32.0))
                            .w(px(120.0))
                            .px(px(12.0))
                            .rounded(RADIUS_SM)
                            .cursor_pointer()
                            .text_size(px(14.0))
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
                            .child(icon(icon_asset).size(px(16.0)))
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_mode(mode, cx);
                            }))
                    }),
            )
    }

    fn render_sync_pill(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.sync_pill.is_open;
        let is_busy = self.sync_pill.is_action_busy;
        let lifecycle = self
            .sync_pill
            .status
            .as_ref()
            .map(|s| s.state)
            .unwrap_or(SyncLifecycleState::Disabled);
        let conflict_count = self
            .sync_pill
            .status
            .as_ref()
            .map(|s| s.conflict_count)
            .unwrap_or(0);

        div()
            .relative()
            .child(
                // Status Pill trigger button
                div()
                    .id("sync-header-pill-trigger")
                    .flex()
                    .items_center()
                    .h(px(30.0))
                    .px(px(10.0))
                    .py(px(4.0))
                    .gap(px(6.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_full()
                    .bg(cx.theme().background)
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sync_pill.toggle(cx);
                    }))
                    .child(if conflict_count > 0 {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                icon(LucideIcon::TriangleAlert)
                                    .size(px(14.0))
                                    .text_color(COLOR_PROJECT_AMBER),
                            )
                            .child(format!("{conflict_count} conflicts"))
                            .into_any_element()
                    } else {
                        match lifecycle {
                            SyncLifecycleState::Disabled => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::CloudOff)
                                        .size(px(14.0))
                                        .text_color(cx.theme().muted),
                                )
                                .child("Cloud Sync")
                                .into_any_element(),
                            SyncLifecycleState::Syncing => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::RefreshCw)
                                        .size(px(14.0))
                                        .text_color(cx.theme().primary),
                                )
                                .child("Syncing...")
                                .into_any_element(),
                            SyncLifecycleState::Paused => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::Pause)
                                        .size(px(14.0))
                                        .text_color(cx.theme().muted),
                                )
                                .child("Paused")
                                .into_any_element(),
                            SyncLifecycleState::Locked => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::Lock)
                                        .size(px(14.0))
                                        .text_color(cx.theme().muted),
                                )
                                .child("Locked")
                                .into_any_element(),
                            SyncLifecycleState::Error => div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    icon(LucideIcon::CircleAlert)
                                        .size(px(14.0))
                                        .text_color(cx.theme().danger),
                                )
                                .child("Sync error")
                                .into_any_element(),
                            _ => div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(div().size(px(6.0)).rounded_full().bg(cx.theme().success))
                                .child(icon(LucideIcon::Cloud).size(px(14.0)))
                                .child("Synced")
                                .into_any_element(),
                        }
                    }),
            )
            // Popover dialog (width 280px)
            .when(is_open, |this| {
                this.child(
                    div()
                        .id("sync-header-popover")
                        .absolute()
                        .top(px(36.0))
                        .right_0()
                        .w(px(280.0))
                        .p_3()
                        .rounded(RADIUS_MD)
                        .bg(cx.theme().background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap_3()
                        // Popover header
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
                                        .child(icon(LucideIcon::Cloud).size(px(16.0)))
                                        .child(
                                            div()
                                                .text_size(px(14.0))
                                                .font_weight(FontWeight::BOLD)
                                                .child("Cloud Sync"),
                                        ),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(cx.theme().secondary)
                                        .text_size(px(11.0))
                                        .child(format!("{:?}", lifecycle)),
                                ),
                        )
                        // Actions Toolbar
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("btn-sync-run-now")
                                        .primary()
                                        .small()
                                        .label("Sync now")
                                        .disabled(
                                            is_busy
                                                || lifecycle == SyncLifecycleState::Syncing
                                                || lifecycle == SyncLifecycleState::Paused,
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.sync_pill.run_sync_now(cx);
                                        })),
                                )
                                .child(
                                    Button::new("btn-sync-pause")
                                        .ghost()
                                        .small()
                                        .label(if lifecycle == SyncLifecycleState::Paused {
                                            "Resume"
                                        } else {
                                            "Pause"
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.sync_pill.toggle_pause(cx);
                                        })),
                                )
                                .child(
                                    Button::new("btn-sync-settings")
                                        .ghost()
                                        .small()
                                        .label("⚙")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.sync_pill.is_open = false;
                                            this.settings_modal.set_tab(SettingsTab::Sync);
                                            this.settings_modal.open();
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            })
    }
}

impl Render for MainShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_mode = self.mode;
        let show_transcript_host = self.should_show_transcript_host();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                // ==========================================
                // Top Application Header Bar (56px 3-column 1fr auto 1fr grid)
                // ==========================================
                div()
                    .id("app-header")
                    .flex()
                    .items_center()
                    .h(HEADER_HEIGHT)
                    .w_full()
                    .px(px(24.0)) // padding: 8px 24px
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().title_bar)
                    // Column 1: Left Logo & ProjectSelectorDropdown (1fr)
                    .child(
                        div()
                            .id("app-logo")
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_start()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .size(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon(LucideIcon::SquarePen).size(px(20.0))),
                            )
                            .child(self.render_project_selector(cx)),
                    )
                    // Column 2: Center 3-Mode TabNavigation (auto)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(self.render_tab_navigation(cx)),
                    )
                    // Column 3: Right Header Actions: SyncPill, NotificationCenter, Settings (1fr)
                    .child(
                        div()
                            .id("header-actions")
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.0))
                            .child(self.render_sync_pill(cx))
                            .child(self.notification_center.clone())
                            .child(
                                // Icon-only Settings button (36x36px)
                                div()
                                    .id("btn-open-settings")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(px(36.0))
                                    .rounded(RADIUS_MD)
                                    .cursor_pointer()
                                    .child(icon(LucideIcon::Settings).size(px(18.0)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_modal.open();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                // ==========================================
                // Main Viewport Container (.app-main)
                // ==========================================
                div()
                    .id("app-main")
                    .relative()
                    .flex()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .child(match current_mode {
                        AppMode::Create => div()
                            .id("workspace-mode-shell")
                            .flex()
                            .flex_1()
                            .size_full()
                            .child(
                                // Two-panel workspace container
                                div()
                                    .id("panel-container")
                                    .flex()
                                    .flex_1()
                                    .size_full()
                                    .overflow_hidden()
                                    // Left Panel (strict 350px width)
                                    .child(
                                        div()
                                            .id("panel-left")
                                            .flex_none()
                                            .w(PANEL_LEFT_WIDTH)
                                            .min_w(PANEL_LEFT_WIDTH)
                                            .max_w(PANEL_LEFT_WIDTH)
                                            .h_full()
                                            .flex()
                                            .flex_col()
                                            .bg(cx.theme().background)
                                            .child(
                                                // .panel-content (holding CreateWorkspacePanel)
                                                div()
                                                    .id("panel-content")
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .child(self.create_workspace.clone()),
                                            )
                                            .child(
                                                // .panel-footer (holding TranscriptionOptions capsule)
                                                div()
                                                    .id("panel-footer")
                                                    .relative()
                                                    .flex_none()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .px(px(20.0))
                                                    .pt(px(16.0))
                                                    .pb(px(20.0))
                                                    .child(
                                                        self.options_capsule.render(
                                                            self.create_workspace
                                                                .read(cx)
                                                                .live_record
                                                                .read(cx)
                                                                .is_recording,
                                                            cx,
                                                        ),
                                                    ),
                                            ),
                                    )
                                    // Right Panel (.panel-right: flex 1, min-width 500px, rounded-tl 8px)
                                    .child(
                                        div()
                                            .id("panel-right")
                                            .flex_1()
                                            .min_w(PANEL_RIGHT_MIN_WIDTH)
                                            .h_full()
                                            .bg(cx.theme().background)
                                            .border_t_1()
                                            .border_l_1()
                                            .border_color(cx.theme().border)
                                            .rounded_tl(RADIUS_MD)
                                            .overflow_hidden()
                                            .child(self.workbench.render(cx)),
                                    ),
                            )
                            .into_any_element(),
                        AppMode::Projects => div()
                            .id("projects-mode-shell")
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
                                .id("tools-mode-shell")
                                .flex()
                                .size_full()
                                .child(self.tools_sidebar.render(cx))
                                .child(div().flex_1().h_full().child(tool_panel))
                                .into_any_element()
                        }
                    })
                    // Persistent Transcript Host overlay (sliding over inset: 0 0 0 350px)
                    .when(
                        show_transcript_host && current_mode == AppMode::Projects,
                        |this| {
                            this.child(
                                div()
                                    .id("persistent-transcript-host")
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .right_0()
                                    .left(PANEL_LEFT_WIDTH)
                                    .border_t_1()
                                    .border_l_1()
                                    .border_color(cx.theme().border)
                                    .rounded_tl(RADIUS_MD)
                                    .bg(cx.theme().background)
                                    .overflow_hidden()
                                    .child(self.workbench.render(cx)),
                            )
                        },
                    ),
            )
            // Modals & Dialogs
            .child(self.settings_modal.render(cx))
            .child(self.diagnostics_modal.render(cx))
            .child(self.recovery_modal.render(cx))
            .child(self.global_dialog.render(cx))
            .child(self.export_modal.render(cx))
            .child(self.onboarding.render(cx))
            .child(self.project_dialogs.render(cx))
            .child(self.error_dialog.render(cx))
    }
}
