use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{ElementId, *};
use serde_json::{Value, json};

use super::SettingsModal;
use crate::app::GlobalDesktopServices;

pub struct MicrophoneSettingsTab {
    pub selected_mic: String,
    pub selected_system_audio: String,
    pub microphone_devices: Vec<String>,
    pub system_audio_devices: Vec<String>,
    pub audio_devices_loading: bool,
    pub audio_devices_error: Option<String>,
    pub microphone_menu_open: bool,
    pub system_audio_menu_open: bool,
    pub microphone_boost: f64,
    pub keep_microphone_active: bool,
    pub mute_during_recording: bool,
    pub ffmpeg_enabled: bool,
    pub ffmpeg_path: String,
    pub microphone_peak: f32,
    pub system_audio_peak: f32,
    pub peak_meter_task: Option<Task<()>>,
    pub audio_preview_started: bool,
}

impl MicrophoneSettingsTab {
    pub fn new(initial_config: &Value) -> Self {
        let selected_mic = initial_config
            .get("microphoneId")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();
        let selected_system_audio = initial_config
            .get("systemAudioDeviceId")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();
        let microphone_boost = initial_config
            .get("microphoneBoost")
            .and_then(Value::as_f64)
            .unwrap_or(1.0);
        let keep_microphone_active = initial_config
            .get("keepMicrophoneActive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mute_during_recording = initial_config
            .get("muteDuringRecording")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let ffmpeg_enabled = initial_config
            .get("ffmpegEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let ffmpeg_path = initial_config
            .get("ffmpegPath")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        Self {
            selected_mic,
            selected_system_audio,
            microphone_devices: Vec::new(),
            system_audio_devices: Vec::new(),
            audio_devices_loading: false,
            audio_devices_error: None,
            microphone_menu_open: false,
            system_audio_menu_open: false,
            microphone_boost,
            keep_microphone_active,
            mute_during_recording,
            ffmpeg_enabled,
            ffmpeg_path,
            microphone_peak: 0.0,
            system_audio_peak: 0.0,
            peak_meter_task: None,
            audio_preview_started: false,
        }
    }

    pub fn on_activate(&mut self, cx: &mut Context<SettingsModal>) {
        if self.peak_meter_task.is_some() {
            return;
        }
        let mut events = cx.global::<crate::app::GlobalEventBus>().0.subscribe();
        self.peak_meter_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            while let Ok(event) = events.recv().await {
                let peak = event.decode::<i16>().ok();
                if event.name == crate::integrations::audio::MICROPHONE_PEAK_EVENT {
                    if let Some(peak) = peak {
                        let _ = this.update(cx, |modal, cx| {
                            modal.microphone.microphone_peak =
                                ((peak as f32).abs() / 32767.0).clamp(0.0, 1.0);
                            cx.notify();
                        });
                    }
                } else if event.name == crate::integrations::audio::SYSTEM_PEAK_EVENT
                    && let Some(peak) = peak
                {
                    let _ = this.update(cx, |modal, cx| {
                        modal.microphone.system_audio_peak =
                            ((peak as f32).abs() / 32767.0).clamp(0.0, 1.0);
                        cx.notify();
                    });
                }
            }
        }));
    }

    pub fn on_deactivate(&mut self, _cx: &mut Context<SettingsModal>) {
        self.peak_meter_task = None;
        self.microphone_peak = 0.0;
        self.system_audio_peak = 0.0;
    }

    pub fn load_devices(&mut self, cx: &mut Context<SettingsModal>) {
        if self.audio_devices_loading {
            return;
        }
        self.audio_devices_loading = true;
        self.audio_devices_error = None;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = tokio::task::spawn_blocking(|| {
                let microphones = crate::integrations::audio::get_microphone_devices()?;
                let system_devices = crate::integrations::audio::get_system_audio_devices()?;
                Ok::<_, String>((microphones, system_devices))
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result);

            this.update(cx, |modal, cx| {
                modal.microphone.audio_devices_loading = false;
                match result {
                    Ok((microphones, system_devices)) => {
                        modal.microphone.microphone_devices =
                            microphones.into_iter().map(|device| device.name).collect();
                        modal.microphone.system_audio_devices = system_devices
                            .into_iter()
                            .map(|device| device.name)
                            .collect();
                    }
                    Err(error) => modal.microphone.audio_devices_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn browse_ffmpeg(&mut self, cx: &mut Context<SettingsModal>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let picked = tokio::task::spawn_blocking(|| {
                rfd::FileDialog::new()
                    .add_filter("FFmpeg executable", &["exe", ""])
                    .pick_file()
            })
            .await
            .ok()
            .flatten();

            if let Some(path) = picked {
                let path_str = path.to_string_lossy().to_string();
                this.update(cx, |modal, cx| {
                    modal.microphone.ffmpeg_path = path_str.clone();
                    modal.persist_audio_value("ffmpegPath", Value::String(path_str), cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn reset_ffmpeg_path(&mut self, cx: &mut Context<SettingsModal>) {
        self.ffmpeg_path.clear();
        self.persist_audio_value("ffmpegPath", Value::String(String::new()), cx);
        cx.notify();
    }

    pub fn persist_audio_value(
        &mut self,
        key: &str,
        value: Value,
        cx: &mut Context<SettingsModal>,
    ) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let key_owned = key.to_string();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = services.config.set_setting(&key_owned, &value);
        })
        .detach();
    }

    pub fn render(&mut self, cx: &mut Context<SettingsModal>) -> impl IntoElement {
        let selected_mic = self.selected_mic.clone();
        let selected_system_audio = self.selected_system_audio.clone();
        let mic_menu_open = self.microphone_menu_open;
        let sys_menu_open = self.system_audio_menu_open;
        let mic_devices = self.microphone_devices.clone();
        let sys_devices = self.system_audio_devices.clone();
        let mic_boost = self.microphone_boost;
        let keep_mic_active = self.keep_microphone_active;
        let mute_recording = self.mute_during_recording;
        let ffmpeg_enabled = self.ffmpeg_enabled;
        let ffmpeg_path = self.ffmpeg_path.clone();
        let mic_peak = self.microphone_peak;
        let sys_peak = self.system_audio_peak;
        let loading = self.audio_devices_loading;
        let error = self.audio_devices_error.clone();

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
                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("Audio Devices"))
                    .child(div().text_size(px(13.0)).text_color(cx.theme().muted_foreground).child("Configure microphone, desktop audio capture, and decoding options.")),
            )
            .child(audio_section_title("Microphone", "Configure recording device and levels.", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        audio_row("Input device", "Choose the microphone used for recording.", cx)
                            .child(
                                div().flex().flex_col().gap_1().w(px(280.0))
                                    .child(
                                        Button::new("settings-mic-device")
                                            .outline().small()
                                            .label(if selected_mic == "default" { "System default" } else { selected_mic.as_str() })
                                            .on_click(cx.listener(|modal, _, _, cx| {
                                                modal.microphone.microphone_menu_open = !modal.microphone.microphone_menu_open;
                                                cx.notify();
                                            })),
                                    )
                                    .when(mic_menu_open, |menu| menu.children(
                                        std::iter::once("default".to_string())
                                            .chain(mic_devices)
                                            .map(|device| {
                                                let selected = device.clone();
                                                let label = if device == "default" { "System default".to_string() } else { device };
                                                div().child(
                                                    Button::new(ElementId::Name(format!("mic-device-{selected}").into()))
                                                        .ghost().small().label(label)
                                                        .on_click(cx.listener(move |modal, _, _, cx| {
                                                            modal.microphone.selected_mic = selected.clone();
                                                            modal.microphone.microphone_menu_open = false;
                                                            modal.microphone.persist_audio_value("microphoneId", Value::String(selected.clone()), cx);
                                                            cx.notify();
                                                        })),
                                                )
                                            })
                                    ))
                                    .child(audio_meter(mic_peak, cx)),
                            ),
                    )
                    .child(
                        audio_row("Microphone boost", "Input gain multiplier from 1.0x to 5.0x.", cx)
                            .child(
                                div().flex().items_center().gap_2()
                                    .child(
                                        Button::new("settings-mic-boost-down").outline().small().label("-")
                                            .on_click(cx.listener(|modal, _, _, cx| {
                                                let value = (modal.microphone.microphone_boost - 0.1).max(1.0);
                                                modal.microphone.microphone_boost = value;
                                                modal.microphone.persist_audio_value("microphoneBoost", json!(value), cx);
                                                cx.notify();
                                            })),
                                    )
                                    .child(div().w(px(48.0)).text_center().child(format!("{:.1}x", mic_boost)))
                                    .child(
                                        Button::new("settings-mic-boost-up").outline().small().label("+")
                                            .on_click(cx.listener(|modal, _, _, cx| {
                                                let value = (modal.microphone.microphone_boost + 0.1).min(5.0);
                                                modal.microphone.microphone_boost = value;
                                                modal.microphone.persist_audio_value("microphoneBoost", json!(value), cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(audio_toggle_row(
                        "Keep microphone active",
                        "Keep the microphone ready for voice typing.",
                        keep_mic_active,
                        "settings-keep-mic-active",
                        cx,
                        |modal, val, cx| {
                            modal.microphone.keep_microphone_active = val;
                            modal.microphone.persist_audio_value("keepMicrophoneActive", json!(val), cx);
                        },
                    )),
            )
            .child(audio_section_title("System audio", "Choose desktop audio capture behavior.", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        audio_row("Output device", "Choose the output device to capture.", cx)
                            .child(
                                div().flex().flex_col().gap_1().w(px(280.0))
                                    .child(
                                        Button::new("settings-system-device").outline().small()
                                            .label(if selected_system_audio == "default" { "System default" } else { selected_system_audio.as_str() })
                                            .on_click(cx.listener(|modal, _, _, cx| {
                                                modal.microphone.system_audio_menu_open = !modal.microphone.system_audio_menu_open;
                                                cx.notify();
                                            })),
                                    )
                                    .when(sys_menu_open, |menu| menu.children(
                                        std::iter::once("default".to_string())
                                            .chain(sys_devices)
                                            .map(|device| {
                                                let selected = device.clone();
                                                let label = if device == "default" { "System default".to_string() } else { device };
                                                div().child(
                                                    Button::new(ElementId::Name(format!("system-device-{selected}").into()))
                                                        .ghost().small().label(label)
                                                        .on_click(cx.listener(move |modal, _, _, cx| {
                                                            modal.microphone.selected_system_audio = selected.clone();
                                                            modal.microphone.system_audio_menu_open = false;
                                                            modal.microphone.persist_audio_value("systemAudioDeviceId", Value::String(selected.clone()), cx);
                                                            cx.notify();
                                                        })),
                                                )
                                            })
                                    ))
                                    .child(audio_meter(sys_peak, cx)),
                            ),
                    )
                    .child(audio_toggle_row(
                        "Mute during recording",
                        "Mute system output while recording.",
                        mute_recording,
                        "settings-mute-recording",
                        cx,
                        |modal, val, cx| {
                            modal.microphone.mute_during_recording = val;
                            modal.microphone.persist_audio_value("muteDuringRecording", json!(val), cx);
                        },
                    )),
            )
            .when(loading, |this| this.child(div().text_size(px(12.0)).text_color(cx.theme().muted).child("Loading audio devices...")))
            .when_some(error, |this, err| this.child(div().text_size(px(12.0)).text_color(cx.theme().muted).child(err)))
            .child(audio_section_title("Audio decoding", "Configure FFmpeg for imported audio and video files.", cx))
            .child(audio_toggle_row(
                "Enable FFmpeg decoder",
                "Use FFmpeg when decoding imported media.",
                ffmpeg_enabled,
                "settings-ffmpeg-enabled",
                cx,
                |modal, val, cx| {
                    modal.microphone.ffmpeg_enabled = val;
                    modal.microphone.persist_audio_value("ffmpegEnabled", json!(val), cx);
                },
            ))
            .when(ffmpeg_enabled, |this| this.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(div().flex_1().text_size(px(12.0)).text_color(cx.theme().muted)
                        .child(if ffmpeg_path.trim().is_empty() { "Using system PATH or detected FFmpeg".to_string() } else { ffmpeg_path.clone() }))
                    .child(Button::new("settings-ffmpeg-browse").outline().small().label("Change Path...")
                        .on_click(cx.listener(|modal, _, _, cx| modal.microphone.browse_ffmpeg(cx))))
                    .when(!ffmpeg_path.trim().is_empty(), |row| row.child(
                        Button::new("settings-ffmpeg-reset").ghost().small().label("Restore Default")
                            .on_click(cx.listener(|modal, _, _, cx| modal.microphone.reset_ffmpeg_path(cx)))
                    )),
            ))
    }
}

pub(crate) fn audio_meter(level: f32, cx: &Context<SettingsModal>) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .h(px(8.0))
        .w_full()
        .rounded_full()
        .bg(cx.theme().secondary)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(level))
                .bg(if level > 0.85 {
                    cx.theme().danger
                } else if level > 0.6 {
                    cx.theme().warning
                } else {
                    cx.theme().primary
                })
                .rounded_full(),
        )
}

fn audio_section_title(
    title: &'static str,
    description: &'static str,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .pt(px(8.0))
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}

fn audio_row(
    title: &'static str,
    hint: &'static str,
    cx: &Context<SettingsModal>,
) -> gpui_kit::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_2()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                ),
        )
}

fn audio_toggle_row(
    title: &'static str,
    hint: &'static str,
    enabled: bool,
    id: &'static str,
    cx: &Context<SettingsModal>,
    on_toggle: impl Fn(&mut SettingsModal, bool, &mut Context<SettingsModal>) + 'static + Send + Sync,
) -> gpui_kit::Div {
    audio_row(title, hint, cx).child(Switch::new(id).checked(enabled).on_change(cx.listener(
        move |modal, checked, _, cx| {
            on_toggle(modal, *checked, cx);
            cx.notify();
        },
    )))
}
