//! Live subtitle settings backed by the persisted desktop configuration.

use crate::app::GlobalDesktopServices;
use crate::views::shell::MainShell;
use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Icon;
use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

fn setting_bool(services: &crate::services::DesktopServices, key: &str, default: bool) -> bool {
    services
        .config
        .get_setting(key)
        .ok()
        .flatten()
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

pub struct SubtitlesToolPanel;

impl SubtitlesToolPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&mut self, cx: &mut Context<MainShell>) -> impl IntoElement {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let start_on_launch = setting_bool(&services, "startOnLaunch", false);
        let always_on_top = setting_bool(&services, "alwaysOnTop", true);
        let click_through_locked = setting_bool(&services, "lockWindow", false);
        let translation_enabled = setting_bool(&services, "realtimeTranslationEnabled", false);

        div()
            .id("tools-panel-subtitles")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(48.0))
            .py(px(32.0))
            .child(
                div().w_full().max_w(px(860.0)).mx_auto().flex().flex_col().gap(px(28.0))
                    .child(
                        div().flex().items_start().gap_4().pb_6().border_b_1().border_color(cx.theme().border)
                            .child(div().flex().size(px(48.0)).items_center().justify_center().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().group_box).text_color(cx.theme().primary).child(Icon::new(LucideIcon::Captions).size(px(24.0))))
                            .child(div().flex().flex_col().gap_1()
                                .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("Live Subtitles"))
                                .child(div().text_size(px(14.0)).text_color(cx.theme().muted_foreground).child("Configure the floating caption window and live translation."))),
                    )
                    .child(section("Window Behavior", "Control how the caption window behaves.", cx)
                        .child(toggle_row("Start on Launch", "Open the caption window when Sona starts.", "startOnLaunch", start_on_launch, cx))
                        .child(toggle_row("Always on Top", "Keep captions above other windows.", "alwaysOnTop", always_on_top, cx))
                        .child(toggle_row("Lock Click-Through", "Prevent accidental movement and clicks on the caption window.", "lockWindow", click_through_locked, cx))
                    )
                    .child(section("Real-time Translation", "Translate recognized speech in the caption window.", cx)
                        .child(toggle_row("Enable Translation", "Show translated captions alongside the original speech.", "realtimeTranslationEnabled", translation_enabled, cx))
                        .when(translation_enabled, |panel| panel.child(div().flex().items_start().gap_3().p_3().rounded_md().bg(cx.theme().input)
                            .child(Icon::new(LucideIcon::Info).size(px(15.0)).text_color(cx.theme().muted_foreground))
                            .child(div().text_size(px(12.0)).text_color(cx.theme().muted_foreground).child("Choose source and target languages, display priority, and caption appearance in Settings > Subtitles & Voice Typing."))))
                    )
                    .child(div().flex().items_center().gap_2().text_size(px(12.0)).text_color(cx.theme().muted_foreground)
                        .child(Icon::new(LucideIcon::Settings2).size(px(14.0)))
                        .child("Window appearance and language controls are available in Settings."))
            )
    }
}

impl Default for SubtitlesToolPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn section(
    title: &'static str,
    description: &'static str,
    cx: &Context<MainShell>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(description),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .border_t_1()
                .border_color(cx.theme().border),
        )
}

fn toggle_row(
    title: &'static str,
    hint: &'static str,
    key: &'static str,
    checked: bool,
    cx: &Context<MainShell>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_size(px(14.0)).child(title))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                ),
        )
        .child(
            Switch::new(ElementId::Name(format!("subtitle-setting-{key}").into()))
                .checked(checked)
                .accessibility_label(title)
                .on_change(cx.listener(move |_shell, value, _, cx| {
                    let config = cx.global::<GlobalDesktopServices>().0.config.clone();
                    if let Err(error) = config.set_setting(key, &Value::Bool(*value)) {
                        log::error!("Failed to update subtitle setting {key}: {error}");
                    }
                    cx.notify();
                })),
        )
}
