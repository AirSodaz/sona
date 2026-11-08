//! Emby and Jellyfin subtitle assistant.

use crate::app::GlobalDesktopServices;
use crate::integrations::media_server::{
    MediaServerInfo, MediaServerLibrary, MediaServerLoginPayload, MediaServerMediaItem,
    MediaServerProfile, MediaServerSubtitleMode, MediaServerType,
};
use crate::views::create::batch_import::MediaServerTaskContext;
use crate::views::create::workspace::CreateWorkspaceTab;
use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::collections::{BTreeMap, HashSet};

const TARGET_LANGUAGES: [(&str, &str); 5] = [
    ("zh-CN", "简体中文"),
    ("zh-TW", "繁體中文"),
    ("en", "English"),
    ("ja", "日本語"),
    ("ko", "한국어"),
];

pub struct MediaServerToolPanel {
    profile_name: Entity<InputState>,
    server_url: Entity<InputState>,
    api_key: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    search: Entity<InputState>,
    search_query: String,
    _search_subscription: Subscription,
    server_type: MediaServerType,
    allow_invalid_certs: bool,
    auto_connect_attempted: bool,
    is_connecting: bool,
    connection_error: Option<String>,
    server_info: Option<MediaServerInfo>,
    active_profile_id: Option<String>,
    libraries: Vec<MediaServerLibrary>,
    selected_library_id: Option<String>,
    target_language: String,
    subtitle_mode: MediaServerSubtitleMode,
    overwrite_existing_external: bool,
    processing_settings_loaded: bool,
    submitted_notice: Option<String>,
    only_missing_target: bool,
    selected_item_ids: HashSet<String>,
    collapsed_seasons: HashSet<String>,
    audio_track_map: std::collections::HashMap<String, i32>,
    is_loading_items: bool,
    items: Vec<MediaServerMediaItem>,
    item_fetch_generation: u64,
}

impl MediaServerToolPanel {
    pub fn new(window: &mut Window, cx: &mut Context<MainShell>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search movies and TV shows..."));
        let search_subscription = cx.subscribe(&search, |shell, state, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let query = state.read(cx).value().trim().to_string();
                let panel = &mut shell.media_server_tool;
                panel.search_query = query.clone();
                panel.selected_item_ids.clear();
                panel.item_fetch_generation += 1;
                let generation = panel.item_fetch_generation;
                let profile_id = panel.active_profile_id.clone();
                let library_id = panel.selected_library_id.clone();
                panel.is_loading_items = profile_id.is_some();
                cx.notify();
                if let Some(profile_id) = profile_id {
                    let service = cx.global::<GlobalDesktopServices>().0.media_server.clone();
                    cx.spawn(async move |this, cx| {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        let result = service
                            .items(
                                Some(&profile_id),
                                library_id.as_deref(),
                                (!query.is_empty()).then_some(query.as_str()),
                                0,
                                300,
                            )
                            .await;
                        this.update(cx, |shell, cx| {
                            let panel = &mut shell.media_server_tool;
                            if panel.item_fetch_generation != generation {
                                return;
                            }
                            panel.is_loading_items = false;
                            match result {
                                Ok(result) => {
                                    panel.items = result.items;
                                    panel.connection_error = None;
                                }
                                Err(error) => panel.connection_error = Some(error),
                            }
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                }
            }
        });
        Self {
            profile_name: cx.new(|cx| InputState::new(window, cx).placeholder("Home media server")),
            server_url: cx
                .new(|cx| InputState::new(window, cx).placeholder("http://localhost:8096")),
            api_key: cx.new(|cx| InputState::new(window, cx).placeholder("Server API key")),
            username: cx.new(|cx| InputState::new(window, cx).placeholder("Admin username")),
            password: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Admin password")
                    .masked(true)
            }),
            search,
            search_query: String::new(),
            _search_subscription: search_subscription,
            server_type: MediaServerType::Emby,
            allow_invalid_certs: false,
            auto_connect_attempted: false,
            is_connecting: false,
            connection_error: None,
            server_info: None,
            active_profile_id: None,
            libraries: Vec::new(),
            selected_library_id: None,
            target_language: "zh-CN".to_string(),
            subtitle_mode: MediaServerSubtitleMode::Bilingual,
            overwrite_existing_external: true,
            processing_settings_loaded: false,
            submitted_notice: None,
            only_missing_target: false,
            selected_item_ids: HashSet::new(),
            collapsed_seasons: HashSet::new(),
            audio_track_map: std::collections::HashMap::new(),
            is_loading_items: false,
            items: Vec::new(),
            item_fetch_generation: 0,
        }
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        if !self.processing_settings_loaded {
            self.processing_settings_loaded = true;
            if let Ok(Some(config)) = services.config.load()
                && let Some(settings) = config.get("embyProcessingSettings")
            {
                if let Some(mode) = settings
                    .get("defaultSubtitleMode")
                    .and_then(|value| value.as_str())
                {
                    self.subtitle_mode = match mode {
                        "pure_translation" => MediaServerSubtitleMode::PureTranslation,
                        "separate_tracks" => MediaServerSubtitleMode::SeparateTracks,
                        _ => MediaServerSubtitleMode::Bilingual,
                    };
                }
                if let Some(overwrite) = settings
                    .get("overwriteExistingExternal")
                    .and_then(|value| value.as_bool())
                {
                    self.overwrite_existing_external = overwrite;
                }
            }
        }
        let profiles = services.media_server.list_profiles().unwrap_or_default();

        if !self.auto_connect_attempted {
            self.auto_connect_attempted = true;
            let active_id = services.media_server.active_profile_id();
            if let Some(profile) = profiles
                .iter()
                .find(|profile| Some(&profile.id) == active_id.as_ref())
                .cloned()
            {
                self.is_connecting = true;
                self.active_profile_id = Some(profile.id.clone());
                let service = services.media_server.clone();
                cx.spawn(async move |this, cx| {
                    let result = async {
                        let info = service.test_connection(profile.clone()).await?;
                        let libraries = service.libraries(Some(&profile.id)).await?;
                        Ok::<_, String>((info, libraries))
                    }
                    .await;
                    this.update(cx, |shell, cx| {
                        let panel = &mut shell.media_server_tool;
                        panel.is_connecting = false;
                        match result {
                            Ok((info, libraries)) => {
                                panel.server_info = Some(info);
                                panel.libraries = libraries;
                                panel.selected_library_id =
                                    panel.libraries.first().map(|item| item.id.clone());
                            }
                            Err(error) => {
                                panel.server_info = None;
                                panel.connection_error = Some(error);
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }

        let connected = self.server_info.is_some();
        let status = if self.is_connecting {
            "Connecting...".to_string()
        } else if let Some(info) = &self.server_info {
            if info.server_name.is_empty() {
                "Connected".to_string()
            } else {
                info.server_name.clone()
            }
        } else if profiles.is_empty() {
            "Not Connected".to_string()
        } else {
            "Disconnected".to_string()
        };
        let status_color = if self.is_connecting {
            cx.theme().warning
        } else if connected {
            cx.theme().success
        } else {
            cx.theme().muted_foreground
        };

        let mut header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .pb_6()
            .border_b_1()
            .border_color(cx.theme().border);
        header = header
            .child(div().flex().items_start().gap_4()
                .child(div().flex().size(px(48.0)).items_center().justify_center().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box).text_color(cx.theme().primary)
                    .child(Icon::new(LucideIcon::Tv).size(px(24.0))))
                .child(div().flex().flex_col().gap_1()
                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("Media Server Subtitle Assistant"))
                    .child(div().text_size(px(14.0)).text_color(cx.theme().muted_foreground).child("Detect missing subtitles in Emby and Jellyfin with local transcription."))))
            .child(div().flex().items_center().gap_3()
                .child(div().text_size(px(13.0)).text_color(status_color).child(status)));

        if connected {
            header = header.child(
                Button::new("media-server-refresh")
                    .ghost()
                    .small()
                    .icon(LucideIcon::RefreshCw)
                    .tooltip("Refresh media library")
                    .on_click(cx.listener(|shell, _, _, cx| {
                        let Some(profile_id) = shell.media_server_tool.active_profile_id.clone()
                        else {
                            return;
                        };
                        let library_id = shell.media_server_tool.selected_library_id.clone();
                        shell.media_server_tool.item_fetch_generation += 1;
                        let generation = shell.media_server_tool.item_fetch_generation;
                        let service = cx.global::<GlobalDesktopServices>().0.media_server.clone();
                        shell.media_server_tool.is_loading_items = true;
                        cx.spawn(async move |this, cx| {
                            let result = service
                                .items(Some(&profile_id), library_id.as_deref(), None, 0, 300)
                                .await;
                            this.update(cx, |shell, cx| {
                                if shell.media_server_tool.item_fetch_generation != generation {
                                    return;
                                }
                                shell.media_server_tool.is_loading_items = false;
                                match result {
                                    Ok(result) => {
                                        shell.media_server_tool.items = result.items;
                                        shell.media_server_tool.connection_error = None;
                                    }
                                    Err(error) => {
                                        shell.media_server_tool.connection_error = Some(error)
                                    }
                                }
                                cx.notify();
                            })
                            .ok();
                        })
                        .detach();
                    })),
            );
        }

        let mut root = div()
            .id("tools-panel-media-server")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(48.0))
            .py(px(32.0));
        let mut content = div()
            .w_full()
            .max_w(px(920.0))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .child(header);

        if !profiles.is_empty() {
            let mut saved = div().flex().flex_col().gap_2().child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Saved Servers"),
            );
            for profile in &profiles {
                let profile_id = profile.id.clone();
                let active = self.active_profile_id.as_deref() == Some(profile.id.as_str());
                let server_type = if profile.server_type == MediaServerType::Emby {
                    "Emby"
                } else {
                    "Jellyfin"
                };
                let row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(LucideIcon::Server)
                                    .size(px(15.0))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{} · {server_type}", profile.name)),
                            ),
                    )
                    .child(
                        Button::new(ElementId::Name(
                            format!("media-server-profile-{}", profile.id).into(),
                        ))
                        .ghost()
                        .small()
                        .label(if active { "Active" } else { "Connect" })
                        .disabled(active || self.is_connecting)
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            let service =
                                cx.global::<GlobalDesktopServices>().0.media_server.clone();
                            let profile_id = profile_id.clone();
                            shell.media_server_tool.is_connecting = true;
                            shell.media_server_tool.connection_error = None;
                            cx.spawn(async move |this, cx| {
                                let result = async {
                                    service.set_active_profile(&profile_id)?;
                                    let profile = service
                                        .list_profiles()?
                                        .into_iter()
                                        .find(|item| item.id == profile_id)
                                        .ok_or_else(|| {
                                            "Saved media server profile was not found".to_string()
                                        })?;
                                    let info = service.test_connection(profile.clone()).await?;
                                    let libraries = service.libraries(Some(&profile_id)).await?;
                                    Ok::<_, String>((profile, info, libraries))
                                }
                                .await;
                                this.update(cx, |shell, cx| {
                                    let panel = &mut shell.media_server_tool;
                                    panel.is_connecting = false;
                                    match result {
                                        Ok((profile, info, libraries)) => {
                                            panel.active_profile_id = Some(profile.id);
                                            panel.server_info = Some(info);
                                            panel.libraries = libraries;
                                            panel.selected_library_id =
                                                panel.libraries.first().map(|item| item.id.clone());
                                            panel.items.clear();
                                            panel.selected_item_ids.clear();
                                            panel.audio_track_map.clear();
                                        }
                                        Err(error) => {
                                            panel.server_info = None;
                                            panel.connection_error = Some(error);
                                        }
                                    }
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        })),
                    );
                saved = saved.child(row);
            }
            content = content.child(saved);
        }

        if !connected {
            let server_type = self.server_type;
            let mut form = div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Connect a Server"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("media-server-type-emby")
                                .small()
                                .label("Emby")
                                .disabled(server_type == MediaServerType::Emby)
                                .on_click(cx.listener(|shell, _, _, cx| {
                                    shell.media_server_tool.server_type = MediaServerType::Emby;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("media-server-type-jellyfin")
                                .small()
                                .label("Jellyfin")
                                .disabled(server_type == MediaServerType::Jellyfin)
                                .on_click(cx.listener(|shell, _, _, cx| {
                                    shell.media_server_tool.server_type = MediaServerType::Jellyfin;
                                    cx.notify();
                                })),
                        ),
                );
            let mut fields = div()
                .w_full()
                .max_w(px(480.0))
                .flex()
                .flex_col()
                .gap_3()
                .child(Input::new(&self.profile_name).small())
                .child(Input::new(&self.server_url).small())
                .child(Input::new(&self.username).small())
                .child(Input::new(&self.password).small())
                .child(Input::new(&self.api_key).small());
            fields = fields.child(
                Button::new("media-server-invalid-certs")
                    .small()
                    .label(if self.allow_invalid_certs {
                        "Allow invalid SSL certificates: On"
                    } else {
                        "Allow invalid SSL certificates: Off"
                    })
                    .on_click(cx.listener(|shell, _, _, cx| {
                        shell.media_server_tool.allow_invalid_certs =
                            !shell.media_server_tool.allow_invalid_certs;
                        cx.notify();
                    })),
            );
            fields = fields.child(
                Button::new("media-server-login-connect")
                    .outline()
                    .label(if self.is_connecting {
                        "Connecting..."
                    } else {
                        "Login & Connect"
                    })
                    .disabled(self.is_connecting)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        let panel = &shell.media_server_tool;
                        let name = panel.profile_name.read(cx).value().trim().to_string();
                        let server_url = panel.server_url.read(cx).value().trim().to_string();
                        let username = panel.username.read(cx).value().trim().to_string();
                        let password = panel.password.read(cx).value().to_string();
                        if server_url.is_empty() || username.is_empty() {
                            shell.media_server_tool.connection_error =
                                Some("Server URL and username are required".to_string());
                            cx.notify();
                            return;
                        }
                        let server_type = panel.server_type;
                        let allow_invalid_certs = panel.allow_invalid_certs;
                        shell.media_server_tool.is_connecting = true;
                        shell.media_server_tool.connection_error = None;
                        let service = cx.global::<GlobalDesktopServices>().0.media_server.clone();
                        cx.spawn(async move |this, cx| {
                            let result = async {
                                let auth = service
                                    .login(MediaServerLoginPayload {
                                        server_url: server_url.clone(),
                                        server_type,
                                        username,
                                        password: (!password.is_empty()).then_some(password),
                                        allow_invalid_certs,
                                    })
                                    .await?;
                                let now = chrono::Utc::now().timestamp_millis();
                                let profile = MediaServerProfile {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    name: if name.is_empty() {
                                        server_url.clone()
                                    } else {
                                        name
                                    },
                                    server_type,
                                    server_url,
                                    api_key: auth.access_token,
                                    username: Some(auth.user_name),
                                    admin_user_id: Some(auth.user_id),
                                    allow_invalid_certs,
                                    created_at: now,
                                    updated_at: now,
                                };
                                let info = service.test_connection(profile.clone()).await?;
                                service.save_profile(profile.clone())?;
                                let libraries = service.libraries(Some(&profile.id)).await?;
                                Ok::<_, String>((profile, info, libraries))
                            }
                            .await;
                            this.update(cx, |shell, cx| {
                                let panel = &mut shell.media_server_tool;
                                panel.is_connecting = false;
                                match result {
                                    Ok((profile, info, libraries)) => {
                                        panel.active_profile_id = Some(profile.id);
                                        panel.server_info = Some(info);
                                        panel.libraries = libraries;
                                        panel.selected_library_id =
                                            panel.libraries.first().map(|item| item.id.clone());
                                        panel.items.clear();
                                        panel.selected_item_ids.clear();
                                        panel.audio_track_map.clear();
                                        panel.connection_error = None;
                                    }
                                    Err(error) => panel.connection_error = Some(error),
                                }
                                cx.notify();
                            })
                            .ok();
                        })
                        .detach();
                    })),
            );
            fields = fields.child(
                Button::new("media-server-connect-save")
                    .outline()
                    .label(if self.is_connecting {
                        "Connecting..."
                    } else {
                        "Test Connection & Save"
                    })
                    .disabled(self.is_connecting)
                    .on_click(cx.listener(|shell, _, _, cx| {
                        let name = shell
                            .media_server_tool
                            .profile_name
                            .read(cx)
                            .value()
                            .trim()
                            .to_string();
                        let server_url = shell
                            .media_server_tool
                            .server_url
                            .read(cx)
                            .value()
                            .trim()
                            .to_string();
                        let api_key = shell
                            .media_server_tool
                            .api_key
                            .read(cx)
                            .value()
                            .trim()
                            .to_string();
                        if server_url.is_empty() || api_key.is_empty() {
                            shell.media_server_tool.connection_error =
                                Some("Server URL and API key are required".to_string());
                            cx.notify();
                            return;
                        }
                        let now = chrono::Utc::now().timestamp_millis();
                        let profile = MediaServerProfile {
                            id: uuid::Uuid::new_v4().to_string(),
                            name: if name.is_empty() {
                                server_url.clone()
                            } else {
                                name
                            },
                            server_type: shell.media_server_tool.server_type,
                            server_url,
                            api_key,
                            username: None,
                            admin_user_id: None,
                            allow_invalid_certs: false,
                            created_at: now,
                            updated_at: now,
                        };
                        shell.media_server_tool.is_connecting = true;
                        shell.media_server_tool.connection_error = None;
                        let service = cx.global::<GlobalDesktopServices>().0.media_server.clone();
                        cx.spawn(async move |this, cx| {
                            let result = async {
                                let info = service.test_connection(profile.clone()).await?;
                                service.save_profile(profile.clone())?;
                                let libraries = service.libraries(Some(&profile.id)).await?;
                                Ok::<_, String>((profile, info, libraries))
                            }
                            .await;
                            this.update(cx, |shell, cx| {
                                let panel = &mut shell.media_server_tool;
                                panel.is_connecting = false;
                                match result {
                                    Ok((profile, info, libraries)) => {
                                        panel.active_profile_id = Some(profile.id);
                                        panel.server_info = Some(info);
                                        panel.libraries = libraries;
                                        panel.selected_library_id =
                                            panel.libraries.first().map(|item| item.id.clone());
                                        panel.connection_error = None;
                                    }
                                    Err(error) => panel.connection_error = Some(error),
                                }
                                cx.notify();
                            })
                            .ok();
                        })
                        .detach();
                    })),
            );
            form = form.child(fields);
            content = content.child(form);
        }

        if let Some(error) = self.connection_error.clone() {
            content = content.child(
                div()
                    .flex()
                    .items_start()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().warning)
                    .text_color(cx.theme().warning)
                    .child(Icon::new(LucideIcon::TriangleAlert).size(px(16.0)))
                    .child(div().text_size(px(13.0)).child(error)),
            );
        }

        if connected {
            let mut server_section = div().flex().flex_col().gap_3();
            if let Some(info) = &self.server_info {
                server_section = server_section.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(13.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            Icon::new(LucideIcon::CircleCheck)
                                .size(px(16.0))
                                .text_color(cx.theme().success),
                        )
                        .child(format!("{} · v{}", info.server_name, info.version)),
                );
            }

            let mut library_row = div().flex().items_center().gap_2();
            for library in &self.libraries {
                let library_id = library.id.clone();
                let profile_id = self.active_profile_id.clone();
                let selected = self.selected_library_id.as_deref() == Some(library.id.as_str());
                let button = Button::new(ElementId::Name(
                    format!("media-server-library-{}", library.id).into(),
                ))
                .outline()
                .small()
                .label(library.name.clone())
                .disabled(selected || self.is_loading_items)
                .on_click(cx.listener(move |shell, _, _, cx| {
                    let Some(profile_id) = profile_id.clone() else {
                        return;
                    };
                    shell.media_server_tool.selected_library_id = Some(library_id.clone());
                    shell.media_server_tool.item_fetch_generation += 1;
                    let generation = shell.media_server_tool.item_fetch_generation;
                    shell.media_server_tool.is_loading_items = true;
                    let service = cx.global::<GlobalDesktopServices>().0.media_server.clone();
                    let request_library_id = library_id.clone();
                    cx.spawn(async move |this, cx| {
                        let result = service
                            .items(Some(&profile_id), Some(&request_library_id), None, 0, 300)
                            .await;
                        this.update(cx, |shell, cx| {
                            if shell.media_server_tool.item_fetch_generation != generation {
                                return;
                            }
                            shell.media_server_tool.is_loading_items = false;
                            match result {
                                Ok(result) => {
                                    shell.media_server_tool.items = result.items;
                                    shell.media_server_tool.connection_error = None;
                                }
                                Err(error) => {
                                    shell.media_server_tool.connection_error = Some(error)
                                }
                            }
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                }));
                library_row = library_row.child(button);
            }
            server_section = server_section.child(library_row);

            let mut language_controls = div().flex().items_center().gap_1();
            for (code, label) in TARGET_LANGUAGES {
                let selected = self.target_language == code;
                language_controls = language_controls.child(
                    Button::new(ElementId::Name(
                        format!("media-server-language-{code}").into(),
                    ))
                    .small()
                    .label(label)
                    .disabled(selected)
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        shell.media_server_tool.target_language = code.to_string();
                        shell.media_server_tool.selected_item_ids.clear();
                        cx.notify();
                    })),
                );
            }
            let filtered_items: Vec<_> = self
                .items
                .iter()
                .filter(|item| {
                    let query_matches = self.search_query.trim().is_empty()
                        || item
                            .name
                            .to_lowercase()
                            .contains(&self.search_query.trim().to_lowercase())
                        || item.series_name.as_deref().is_some_and(|series| {
                            series
                                .to_lowercase()
                                .contains(&self.search_query.trim().to_lowercase())
                        });
                    query_matches
                        && (!self.only_missing_target
                            || !crate::integrations::media_server::common::has_target_subtitle(
                                &item.media_streams,
                                &self.target_language,
                            ))
                })
                .collect();
            let queued_item_ids = shell_queued_media_items(cx);
            let selectable_ids: Vec<_> = filtered_items
                .iter()
                .filter(|item| !queued_item_ids.contains(&item.id))
                .map(|item| item.id.clone())
                .collect();
            let selected_count = selectable_ids
                .iter()
                .filter(|id| self.selected_item_ids.contains(*id) && !queued_item_ids.contains(*id))
                .count();
            let total_selectable_count = selectable_ids.len();
            let all_selected = !selectable_ids.is_empty() && selected_count == selectable_ids.len();
            server_section = server_section.child(
                div().flex().flex_col().gap_2().child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().w(px(300.0)).child(Input::new(&self.search).small()))
                                .child(language_controls),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    Button::new("media-server-only-missing")
                                        .small()
                                        .label(if self.only_missing_target {
                                            "Showing missing"
                                        } else {
                                            "Only missing subtitles"
                                        })
                                        .on_click(cx.listener(|shell, _, _, cx| {
                                            let panel = &mut shell.media_server_tool;
                                            panel.only_missing_target = !panel.only_missing_target;
                                            panel.selected_item_ids.clear();
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("media-server-select-all")
                                        .small()
                                        .label(if all_selected {
                                            "Clear selection"
                                        } else {
                                            "Select all"
                                        })
                                        .disabled(selectable_ids.is_empty())
                                        .on_click(cx.listener(move |shell, _, _, cx| {
                                            let panel = &mut shell.media_server_tool;
                                            if all_selected {
                                                panel.selected_item_ids.clear();
                                            } else {
                                                panel
                                                    .selected_item_ids
                                                    .extend(selectable_ids.iter().cloned());
                                            }
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "Selected {selected_count} / {} items",
                                            total_selectable_count
                                        )),
                                ),
                        ),
                ),
            );

            let mut item_list = div()
                .flex()
                .flex_col()
                .min_h(px(120.0))
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .p_3();
            if self.is_loading_items {
                item_list = item_list.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Loading media library items..."),
                );
            } else if filtered_items.is_empty() {
                let empty_message = if self.selected_library_id.is_some() {
                    if self.items.is_empty() {
                        "No media items found."
                    } else {
                        "No media items found matching the current filter."
                    }
                } else {
                    "Select a library to browse its media."
                };
                item_list = item_list.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(empty_message),
                );
            } else {
                let mut standalone = Vec::new();
                let mut seasons = BTreeMap::<String, Vec<&MediaServerMediaItem>>::new();
                for item in filtered_items.iter().copied() {
                    if item.series_name.is_some()
                        || item
                            .item_type
                            .as_deref()
                            .is_some_and(|kind| kind.eq_ignore_ascii_case("episode"))
                    {
                        let season = item.season_name.clone().unwrap_or_else(|| {
                            format!("Season {}", item.season_index.unwrap_or(1))
                        });
                        let key = format!(
                            "{} - {season}",
                            item.series_name.as_deref().unwrap_or("Series")
                        );
                        seasons.entry(key).or_default().push(item);
                    } else {
                        standalone.push(item);
                    }
                }
                for items in seasons.values_mut() {
                    items.sort_by_key(|item| item.episode_index.unwrap_or_default());
                }
                for item in standalone {
                    item_list = item_list.child(media_item_row(
                        item,
                        &self.target_language,
                        self.selected_item_ids.contains(&item.id),
                        queued_item_ids.contains(&item.id),
                        self.audio_track_map.get(&item.id).copied(),
                        cx,
                    ));
                }
                for (season, items) in seasons {
                    let is_collapsed = self.collapsed_seasons.contains(&season);
                    let missing_ids: Vec<_> = items
                        .iter()
                        .filter(|item| {
                            !crate::integrations::media_server::common::has_target_subtitle(
                                &item.media_streams,
                                &self.target_language,
                            ) && !queued_item_ids.contains(&item.id)
                        })
                        .map(|item| item.id.clone())
                        .collect();
                    let missing_count = missing_ids.len();
                    let season_for_toggle = season.clone();
                    item_list = item_list.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                Button::new(ElementId::Name(
                                    format!("media-server-season-{season}").into(),
                                ))
                                .ghost()
                                .small()
                                .icon(if is_collapsed {
                                    LucideIcon::ChevronRight
                                } else {
                                    LucideIcon::ChevronDown
                                })
                                .label(format!(
                                    "{season} · {} episodes · {missing_count} missing",
                                    items.len()
                                ))
                                .on_click(cx.listener(
                                    move |shell, _, _, cx| {
                                        let collapsed =
                                            &mut shell.media_server_tool.collapsed_seasons;
                                        if !collapsed.remove(&season_for_toggle) {
                                            collapsed.insert(season_for_toggle.clone());
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                Button::new(ElementId::Name(
                                    format!("media-server-season-select-{season}").into(),
                                ))
                                .small()
                                .label("Select missing")
                                .disabled(missing_ids.is_empty())
                                .on_click(cx.listener(
                                    move |shell, _, _, cx| {
                                        shell
                                            .media_server_tool
                                            .selected_item_ids
                                            .extend(missing_ids.iter().cloned());
                                        cx.notify();
                                    },
                                )),
                            ),
                    );
                    if !is_collapsed {
                        for item in items {
                            item_list = item_list.child(media_item_row(
                                item,
                                &self.target_language,
                                self.selected_item_ids.contains(&item.id),
                                queued_item_ids.contains(&item.id),
                                self.audio_track_map.get(&item.id).copied(),
                                cx,
                            ));
                        }
                    }
                }
            }
            server_section = server_section.child(item_list);

            let mode_options = [
                (MediaServerSubtitleMode::Bilingual, "Bilingual Subtitles"),
                (MediaServerSubtitleMode::PureTranslation, "Translation Only"),
                (MediaServerSubtitleMode::SeparateTracks, "Separate Tracks"),
            ];
            let mut mode_picker = div().flex().items_center().gap_1();
            for (mode, label) in mode_options {
                let selected = self.subtitle_mode == mode;
                mode_picker = mode_picker.child(
                    Button::new(ElementId::Name(
                        format!("media-server-mode-{mode:?}").into(),
                    ))
                    .small()
                    .outline()
                    .label(label)
                    .when(selected, |button| button.primary())
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        shell.media_server_tool.subtitle_mode = mode;
                        persist_processing_settings(
                            cx,
                            mode,
                            shell.media_server_tool.overwrite_existing_external,
                        );
                        cx.notify();
                    })),
                );
            }
            let submit_items: Vec<_> = filtered_items
                .iter()
                .filter(|item| {
                    self.selected_item_ids.contains(&item.id) && !queued_item_ids.contains(&item.id)
                })
                .map(|item| (*item).clone())
                .collect();
            let submit_count = submit_items.len();
            let selected_profile_id = self.active_profile_id.clone().unwrap_or_default();
            let subtitle_mode = self.subtitle_mode;
            let overwrite = self.overwrite_existing_external;
            let submit_bar = div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .flex_wrap()
                .mt_3()
                .p_3()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .bg(cx.theme().secondary.opacity(0.16))
                .child(
                    div().flex().items_center().gap_3().flex_wrap()
                        .child(div().flex().items_center().gap_2()
                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Subtitle mode:"))
                            .child(mode_picker))
                        .child(Checkbox::new("media-server-overwrite-external")
                            .checked(self.overwrite_existing_external)
                            .label("Overwrite existing external subtitle of same language")
                            .on_click(cx.listener(|shell, checked: &bool, _, cx| {
                                shell.media_server_tool.overwrite_existing_external = *checked;
                                persist_processing_settings(
                                    cx,
                                    shell.media_server_tool.subtitle_mode,
                                    *checked,
                                );
                                cx.notify();
                            }))),
                )
                .child(
                    Button::new("media-server-submit")
                        .primary()
                        .icon(LucideIcon::Download)
                        .label(format!("Transcribe ({submit_count})"))
                        .disabled(submit_count == 0 || self.is_loading_items)
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            if submit_items.is_empty() {
                                return;
                            }
                            let contexts = submit_items.iter().map(|item| {
                                let audio = item.media_streams.iter()
                                    .filter(|stream| stream.stream_type.eq_ignore_ascii_case("audio"))
                                    .find(|stream| shell.media_server_tool.audio_track_map.get(&item.id) == Some(&stream.index))
                                    .or_else(|| item.media_streams.iter().find(|stream| stream.stream_type.eq_ignore_ascii_case("audio") && stream.is_default))
                                    .or_else(|| item.media_streams.iter().find(|stream| stream.stream_type.eq_ignore_ascii_case("audio")));
                                let audio_stream_index = audio.map(|stream| stream.index).unwrap_or(0);
                                let source_language = audio.and_then(|stream| stream.language.as_deref())
                                    .map(str::trim).filter(|language| !language.is_empty() && !language.eq_ignore_ascii_case("und"))
                                    .map(str::to_string);
                                let existing_external_subtitle_index = overwrite.then(|| item.media_streams.iter()
                                    .find(|stream| stream.stream_type.eq_ignore_ascii_case("subtitle")
                                        && stream.is_external
                                        && crate::integrations::media_server::common::has_target_subtitle(
                                            std::slice::from_ref(*stream),
                                            &shell.media_server_tool.target_language,
                                        ))
                                    .map(|stream| stream.index)).flatten();
                                let item_name = item.series_name.as_ref().map(|series| {
                                    let season = item.season_name.clone().unwrap_or_default();
                                    let episode = item.episode_index.map(|n| format!("E{n} - ")).unwrap_or_default();
                                    format!("{series} {season} {episode}{}", item.name).split_whitespace().collect::<Vec<_>>().join(" ")
                                }).unwrap_or_else(|| item.name.clone());
                                (item_name, MediaServerTaskContext {
                                    profile_id: selected_profile_id.clone(),
                                    item_id: item.id.clone(),
                                    audio_stream_index,
                                    subtitle_mode,
                                    target_language: shell.media_server_tool.target_language.clone(),
                                    source_language,
                                    existing_external_subtitle_index,
                                    container: item.container.clone(),
                                })
                            }).collect::<Vec<_>>();
                            let batch = shell.create_workspace.read(cx).batch_import.clone();
                            batch.update(cx, |batch, cx| {
                                batch.add_media_server_items(contexts, cx);
                            });
                            shell.media_server_tool.selected_item_ids.clear();
                            let notice = format!(
                                "Successfully added {} tasks to the transcription queue", submit_count
                            );
                            shell.media_server_tool.submitted_notice = Some(notice.clone());
                            cx.spawn(async move |this, cx| {
                                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                                this.update(cx, |shell, cx| {
                                    if shell.media_server_tool.submitted_notice.as_deref()
                                        == Some(notice.as_str())
                                    {
                                        shell.media_server_tool.submitted_notice = None;
                                        cx.notify();
                                    }
                                })
                                .ok();
                            })
                            .detach();
                            shell.create_workspace.update(cx, |workspace, cx| {
                                workspace.set_tab(CreateWorkspaceTab::BatchImport, cx);
                            });
                            shell.set_mode(crate::views::shell::AppMode::Create, cx);
                            cx.notify();
                        })),
                );
            server_section = server_section.child(submit_bar);
            if let Some(notice) = self.submitted_notice.as_deref() {
                server_section = server_section.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .pt_2()
                        .text_size(px(12.0))
                        .text_color(cx.theme().success)
                        .child(Icon::new(LucideIcon::CircleCheck).size(px(14.0)))
                        .child(notice.to_string()),
                );
            }
            content = content.child(server_section);
        }

        root = root.child(content);
        root
    }
}

fn media_item_row(
    item: &MediaServerMediaItem,
    target_language: &str,
    selected: bool,
    queued: bool,
    selected_audio_index: Option<i32>,
    cx: &mut Context<MainShell>,
) -> gpui_kit::Div {
    let item_id = item.id.clone();
    let subtitle_status = if crate::integrations::media_server::common::has_target_subtitle(
        &item.media_streams,
        target_language,
    ) {
        "Has subtitles"
    } else {
        "Missing subtitles"
    };
    let item_name = if let Some(episode) = item.episode_index {
        format!("E{episode} - {}", item.name)
    } else {
        item.name.clone()
    };
    let duration = item.runtime_ticks.and_then(|ticks| {
        let seconds = ticks / 10_000_000;
        if seconds <= 0 {
            None
        } else {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            Some(if hours > 0 {
                format!("{hours}h {minutes}m")
            } else if minutes > 0 {
                format!("{minutes}m")
            } else {
                format!("{seconds}s")
            })
        }
    });
    let audio_streams: Vec<_> = item
        .media_streams
        .iter()
        .filter(|stream| stream.stream_type.eq_ignore_ascii_case("audio"))
        .collect();
    let selected_audio = selected_audio_index
        .and_then(|index| {
            audio_streams
                .iter()
                .find(|stream| stream.index == index)
                .copied()
        })
        .or_else(|| {
            audio_streams
                .iter()
                .find(|stream| stream.is_default)
                .copied()
        })
        .or_else(|| audio_streams.first().copied());
    let audio_label = selected_audio.map(|stream| {
        let mut parts = Vec::new();
        if let Some(title) = stream
            .display_title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
        {
            parts.push(title.trim().to_string());
        } else {
            if let Some(language) = &stream.language {
                parts.push(language.to_ascii_uppercase());
            }
            if let Some(codec) = &stream.codec {
                parts.push(codec.to_ascii_uppercase());
            }
        }
        if parts.is_empty() {
            parts.push(format!("Track #{}", stream.index));
        }
        if stream.is_default && !parts.join(" ").to_ascii_lowercase().contains("default") {
            parts.push("(Default)".to_string());
        }
        parts.join(" ")
    });
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new(ElementId::Name(
                        format!("media-server-item-select-{}", item.id).into(),
                    ))
                    .ghost()
                    .small()
                    .icon(if selected {
                        LucideIcon::CircleCheck
                    } else {
                        LucideIcon::Circle
                    })
                    .tooltip(if selected {
                        "Deselect item"
                    } else {
                        "Select item"
                    })
                    .disabled(queued)
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        let selected = &mut shell.media_server_tool.selected_item_ids;
                        if !selected.remove(&item_id) {
                            selected.insert(item_id.clone());
                        }
                        cx.notify();
                    })),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .child(item_name),
                        )
                        .when_some(duration, |this, duration| {
                            this.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(duration),
                            )
                        }),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(if subtitle_status == "Missing subtitles" {
                    cx.theme().warning
                } else {
                    cx.theme().muted_foreground
                })
                .child(if queued { "Queued" } else { subtitle_status }),
        )
        .when_some(audio_label, |row, label| {
            let item_id_for_audio = item.id.clone();
            let fallback_audio_index = selected_audio.map(|stream| stream.index);
            let stream_indices: Vec<_> = audio_streams.iter().map(|stream| stream.index).collect();
            row.child(
                Button::new(ElementId::Name(
                    format!("media-server-audio-{}", item.id).into(),
                ))
                .ghost()
                .small()
                .label(label)
                .tooltip("Change audio track")
                .on_click(cx.listener(move |shell, _, _, cx| {
                    if stream_indices.is_empty() {
                        return;
                    }
                    let next_index = {
                        let panel = &shell.media_server_tool;
                        let current = panel
                            .audio_track_map
                            .get(&item_id_for_audio)
                            .copied()
                            .or(fallback_audio_index);
                        let current_position = current.and_then(|index| {
                            stream_indices.iter().position(|value| *value == index)
                        });
                        stream_indices[current_position
                            .map_or(0, |position| (position + 1) % stream_indices.len())]
                    };
                    shell
                        .media_server_tool
                        .audio_track_map
                        .insert(item_id_for_audio.clone(), next_index);
                    cx.notify();
                })),
            )
        })
}

fn shell_queued_media_items(cx: &Context<MainShell>) -> HashSet<String> {
    let shell = cx.entity().read(cx);
    let profile_id = shell.media_server_tool.active_profile_id.as_deref();
    shell
        .create_workspace
        .read(cx)
        .batch_import
        .read(cx)
        .queue
        .iter()
        .filter(|item| {
            matches!(
                item.status,
                crate::views::create::BatchItemStatus::Pending
                    | crate::views::create::BatchItemStatus::Processing
            )
        })
        .filter_map(|item| item.media_server_context.as_ref())
        .filter(|context| profile_id.is_none_or(|id| context.profile_id == id))
        .map(|context| context.item_id.clone())
        .collect()
}

fn persist_processing_settings(
    cx: &Context<MainShell>,
    mode: MediaServerSubtitleMode,
    overwrite: bool,
) {
    let config_service = cx.global::<GlobalDesktopServices>().0.config.clone();
    let Ok(mut config) = config_service.load() else {
        return;
    };
    let mut config = config
        .take()
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    let Some(object) = config.as_object_mut() else {
        return;
    };
    let mut settings = object
        .get("embyProcessingSettings")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mode_value = match mode {
        MediaServerSubtitleMode::Bilingual => "bilingual",
        MediaServerSubtitleMode::PureTranslation => "pure_translation",
        MediaServerSubtitleMode::SeparateTracks => "separate_tracks",
    };
    settings.insert(
        "defaultSubtitleMode".to_string(),
        serde_json::Value::String(mode_value.to_string()),
    );
    settings.insert(
        "overwriteExistingExternal".to_string(),
        serde_json::Value::Bool(overwrite),
    );
    object.insert(
        "embyProcessingSettings".to_string(),
        serde_json::Value::Object(settings),
    );
    if let Err(error) = config_service.save(&config) {
        log::warn!("Failed to save media server processing settings: {error}");
    }
}
