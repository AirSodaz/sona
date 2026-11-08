use gpui_kit::component::IndexPath;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::{SettingsModal, SettingsModalEvent};

pub struct GeneralSettingsTab {
    pub selected_theme: String,
    pub selected_lang: String,
    pub selected_font: String,
    pub autostart: bool,
    pub minimize_to_tray: bool,
    pub silent_start: bool,
    pub auto_check_updates: bool,
    pub language_select: Entity<SelectState<Vec<&'static str>>>,
    pub font_select: Entity<SelectState<Vec<&'static str>>>,
    pub log_level_select: Entity<SelectState<Vec<&'static str>>>,
    _language_subscription: Subscription,
    _font_subscription: Subscription,
    _log_level_subscription: Subscription,
}

impl GeneralSettingsTab {
    pub fn new(
        initial_config: &Value,
        window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> Self {
        let log_levels = ["trace", "debug", "info", "warn", "error"];
        let log_level = initial_config
            .get("logLevel")
            .and_then(Value::as_str)
            .unwrap_or("info");
        let log_level_index = log_levels
            .iter()
            .position(|level| *level == log_level)
            .unwrap_or(2);
        let log_level_select = cx.new(|cx| {
            SelectState::new(
                log_levels.to_vec(),
                Some(IndexPath::new(log_level_index)),
                window,
                cx,
            )
        });
        let _log_level_subscription = cx.subscribe(
            &log_level_select,
            move |modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(level)) = event {
                    log::set_max_level(match *level {
                        "trace" => log::LevelFilter::Trace,
                        "debug" => log::LevelFilter::Debug,
                        "warn" => log::LevelFilter::Warn,
                        "error" => log::LevelFilter::Error,
                        _ => log::LevelFilter::Info,
                    });
                    modal.persist_general_setting(
                        "logLevel",
                        Value::String((*level).to_string()),
                        cx,
                    );
                }
            },
        );

        let app_language_options = [
            ("auto", "Automatic"),
            ("en", "English"),
            ("zh", "简体中文"),
            ("zh-TW", "繁體中文"),
            ("ja", "日本語"),
            ("ko", "한국어"),
        ];
        let app_language = initial_config
            .get("appLanguage")
            .and_then(Value::as_str)
            .filter(|value| app_language_options.iter().any(|(code, _)| code == value))
            .unwrap_or("auto");
        let app_language_index = app_language_options
            .iter()
            .position(|(code, _)| *code == app_language)
            .unwrap_or(0);
        let language_select = cx.new(|cx| {
            SelectState::new(
                app_language_options
                    .iter()
                    .map(|(_, label)| *label)
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(app_language_index)),
                window,
                cx,
            )
        });
        let _language_subscription = cx.subscribe(
            &language_select,
            move |modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some((language, _)) = app_language_options
                        .iter()
                        .find(|(_, option_label)| *option_label == *label)
                {
                    modal.general.selected_lang = (*language).to_string();
                    modal.persist_general_setting(
                        "appLanguage",
                        Value::String((*language).to_string()),
                        cx,
                    );
                }
            },
        );

        let font_options = [
            ("system", "System"),
            ("serif", "Serif"),
            ("sans", "Sans-serif"),
            ("mono", "Monospace"),
            ("arial", "Arial"),
            ("georgia", "Georgia"),
        ];
        let font = initial_config
            .get("font")
            .and_then(Value::as_str)
            .filter(|value| font_options.iter().any(|(code, _)| code == value))
            .unwrap_or("system");
        let font_index = font_options
            .iter()
            .position(|(code, _)| *code == font)
            .unwrap_or(0);
        let font_select = cx.new(|cx| {
            SelectState::new(
                font_options
                    .iter()
                    .map(|(_, label)| *label)
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(font_index)),
                window,
                cx,
            )
        });
        let _font_subscription = cx.subscribe(
            &font_select,
            move |modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some((font, _)) = font_options
                        .iter()
                        .find(|(_, option_label)| *option_label == *label)
                {
                    modal.general.selected_font = (*font).to_string();
                    modal.persist_general_setting("font", Value::String((*font).to_string()), cx);
                }
            },
        );

        let selected_theme = initial_config
            .get("theme")
            .and_then(Value::as_str)
            .unwrap_or("auto")
            .to_string();
        let autostart = initial_config
            .get("autoStart")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let minimize_to_tray = initial_config
            .get("minimizeToTrayOnExit")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let silent_start = initial_config
            .get("silentStart")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let auto_check_updates = initial_config
            .get("autoCheckUpdates")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        Self {
            selected_theme,
            selected_lang: app_language.to_string(),
            selected_font: font.to_string(),
            autostart,
            minimize_to_tray,
            silent_start,
            auto_check_updates,
            language_select,
            font_select,
            log_level_select,
            _language_subscription,
            _font_subscription,
            _log_level_subscription,
        }
    }

    pub fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<SettingsModal>,
    ) -> impl IntoElement {
        let selected_theme = self.selected_theme.clone();
        let autostart = self.autostart;
        let minimize_to_tray = self.minimize_to_tray;
        let silent_start = self.silent_start;
        let auto_check_updates = self.auto_check_updates;

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .pb_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("General"))
                    .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Configure application startup, updates, and environment diagnostics.")),
            )
            .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Appearance"))
            .child(
                div().flex().items_center().justify_between().gap_4().py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Language"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Choose the language used by Sona.")))
                    .child(Select::new(&self.language_select).w(px(200.0)).menu_width(px(200.0))),
            )
            .child(
                div().flex().flex_col().gap_2().py_2()
                    .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Theme"))
                    .child(div().flex().gap_2().children(["light", "dark", "auto"].into_iter().map(|theme| {
                        let selected = selected_theme == theme;
                        let label = match theme { "light" => "Light", "dark" => "Dark", _ => "Automatic" };
                        Button::new(format!("settings-theme-{theme}"))
                            .when(selected, |button| button.primary())
                            .when(!selected, |button| button.outline())
                            .small()
                            .label(label)
                            .on_click(cx.listener(move |modal, _, window, cx| {
                                modal.general.selected_theme = theme.to_string();
                                let mode = match theme {
                                    "light" => gpui_kit::component::theme::ThemeMode::Light,
                                    "dark" => gpui_kit::component::theme::ThemeMode::Dark,
                                    _ => gpui_kit::component::theme::ThemeMode::from(window.appearance()),
                                };
                                gpui_kit::component::theme::Theme::change(mode, None, cx);
                                modal.persist_general_setting(
                                    "theme",
                                    Value::String(theme.to_string()),
                                    cx,
                                );
                                cx.notify();
                            }))
                    }))),
            )
            .child(
                div().flex().items_center().justify_between().gap_4().py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Font"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Choose the font used by Sona.")))
                    .child(Select::new(&self.font_select).w(px(240.0)).menu_width(px(240.0))),
            )
            .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Startup and updates"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Log level"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Set the minimum level written to the application log.")))
                    .child(Select::new(&self.log_level_select).w(px(180.0)).menu_width(px(180.0))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Launch on startup"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Start Sona automatically when you sign in to Windows.")))
                    .child(
                        Switch::new("btn-toggle-autostart")
                            .checked(autostart)
                            .on_change(cx.listener(|modal, checked, _, cx| {
                                let value = *checked;
                                modal.general.autostart = value;
                                if !value {
                                    modal.general.silent_start = false;
                                }
                                modal.persist_preference("autoStart", value, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Minimize to tray on exit"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Keep Sona running in the system tray when the main window closes.")))
                    .child(
                        Switch::new("btn-toggle-tray")
                            .checked(minimize_to_tray)
                            .on_change(cx.listener(|modal, checked, _, cx| {
                                let value = *checked;
                                modal.general.minimize_to_tray = value;
                                if !value {
                                    modal.general.silent_start = false;
                                }
                                modal.persist_preference("minimizeToTrayOnExit", value, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Start minimized"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child(if !autostart || !minimize_to_tray { "Requires Launch on startup and Minimize to tray on exit to be enabled." } else { "Open Sona directly in the system tray at sign-in." })))
                    .child(
                        Switch::new("btn-toggle-silent-start")
                            .checked(autostart && minimize_to_tray && silent_start)
                            .disabled(!autostart || !minimize_to_tray)
                            .on_change(cx.listener(|modal, checked, _, cx| {
                                if !modal.general.autostart || !modal.general.minimize_to_tray {
                                    return;
                                }
                                let value = *checked;
                                modal.general.silent_start = value;
                                modal.persist_preference("silentStart", value, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Check for updates automatically"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Check for new Sona releases automatically.")))
                    .child(
                        Switch::new("btn-toggle-auto-check-updates")
                            .checked(auto_check_updates)
                            .on_change(cx.listener(|modal, checked, _, cx| {
                                let value = *checked;
                                modal.general.auto_check_updates = value;
                                modal.persist_preference("autoCheckUpdates", value, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(div().flex().flex_col().gap_1().pt_3().border_t_1().border_color(cx.theme().border)
                .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Model & Environment Diagnostics"))
                .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Inspect the local transcription path, runtime readiness, and packaged environment checks.")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .py_2()
                    .child(div().flex_1().flex().flex_col().gap_1()
                        .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child("Diagnostics"))
                        .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Review the current local setup and jump to the clearest fix when something is off.")))
                    .child(
                        Button::new("btn-open-diagnostics")
                            .outline()
                            .small()
                            .label("Open Diagnostics")
                            .on_click(cx.listener(|modal, _, _, cx| {
                                modal.close(cx);
                                cx.emit(SettingsModalEvent::OpenDiagnostics);
                            })),
                    ),
            )
    }
}
