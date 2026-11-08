use gpui_kit::component::IndexPath;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

use super::SettingsModal;
use super::types::OnlineProviderTestState;
use crate::app::GlobalDesktopServices;

pub struct ModelsSettingsTab {
    pub model_search_state: Entity<InputState>,
    pub model_engine_select: Entity<SelectState<Vec<&'static str>>>,
    pub model_status_select: Entity<SelectState<Vec<&'static str>>>,
    pub model_label_select: Entity<SelectState<Vec<&'static str>>>,
    pub model_catalog: Option<Value>,
    pub model_catalog_loading: bool,
    pub model_catalog_error: Option<String>,
    pub model_download_ids: HashMap<String, String>,
    pub model_download_progress: HashMap<String, f32>,
    pub pending_model_delete: Option<(String, String, String)>,
    pub model_scenario_batch: bool,
    pub model_selector_open: Option<String>,
    pub model_engine_filter: String,
    pub model_status_filter: String,
    pub model_label_filter: String,
    pub model_search_query: String,
    pub expanded_online_provider: Option<String>,
    pub online_provider_api_key_revealed: HashSet<String>,
    pub online_provider_test_states: HashMap<String, OnlineProviderTestState>,
    pub online_provider_field_states: HashMap<String, Entity<InputState>>,
    pub cuda_addon_status: Option<sona_core::runtime::cuda_addon::CudaAddonInspection>,
    pub cuda_addon_loading: bool,
    pub cuda_addon_downloading: bool,
    pub cuda_addon_progress: f32,
    pub cuda_addon_error: Option<String>,
    _filter_subscriptions: [Subscription; 3],
    _search_subscription: Subscription,
}

impl ModelsSettingsTab {
    pub fn new(window: &mut Window, cx: &mut Context<SettingsModal>) -> Self {
        let model_search_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search preset models..."));
        let _search_subscription = cx.subscribe(
            &model_search_state,
            |modal, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    modal.models.model_search_query = state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );

        let model_engine_select = cx.new(|cx| {
            SelectState::new(
                vec!["All Engines", "ONNX", "GGUF"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let model_status_select = cx.new(|cx| {
            SelectState::new(
                vec!["All Statuses", "Installed", "Not Installed", "Downloading"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let model_label_select = cx.new(|cx| {
            SelectState::new(
                vec!["All Tags", "Accurate", "Lite"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });

        let engine_sub = cx.subscribe(
            &model_engine_select,
            |modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    modal.models.model_engine_filter = match *value {
                        "ONNX" => "sherpa-onnx",
                        "GGUF" => "llama-cpp",
                        _ => "all",
                    }
                    .to_string();
                    cx.notify();
                }
            },
        );

        let status_sub = cx.subscribe(
            &model_status_select,
            |modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    modal.models.model_status_filter = match *value {
                        "Installed" => "installed",
                        "Not Installed" => "not-installed",
                        "Downloading" => "downloading",
                        _ => "all",
                    }
                    .to_string();
                    cx.notify();
                }
            },
        );

        let label_sub = cx.subscribe(
            &model_label_select,
            |modal, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    modal.models.model_label_filter = match *value {
                        "Accurate" => "accurate",
                        "Lite" => "lite",
                        _ => "all",
                    }
                    .to_string();
                    cx.notify();
                }
            },
        );

        let mut online_provider_field_states = HashMap::new();
        for provider in sona_core::ports::asr::online_asr_providers() {
            let provider_id = provider.id.clone();
            let mut fields = vec![("apiKey".to_string(), "password".to_string())];
            fields.extend(
                provider
                    .ui
                    .fields
                    .iter()
                    .filter(|field| field.name != "apiKey" && field.name != "model")
                    .map(|field| (field.name.clone(), field.field_type.clone())),
            );
            for (field_name, field_type) in fields {
                let key = format!("{provider_id}::{field_name}");
                let state = cx.new(|cx| {
                    let mut state = InputState::new(window, cx).placeholder(&field_name);
                    if field_type == "password" {
                        state = state.masked(true);
                    }
                    state
                });
                online_provider_field_states.insert(key, state);
            }
        }

        Self {
            model_search_state,
            model_engine_select,
            model_status_select,
            model_label_select,
            model_catalog: None,
            model_catalog_loading: false,
            model_catalog_error: None,
            model_download_ids: HashMap::new(),
            model_download_progress: HashMap::new(),
            pending_model_delete: None,
            model_scenario_batch: false,
            model_selector_open: None,
            model_engine_filter: "all".to_string(),
            model_status_filter: "all".to_string(),
            model_label_filter: "all".to_string(),
            model_search_query: String::new(),
            expanded_online_provider: None,
            online_provider_api_key_revealed: HashSet::new(),
            online_provider_test_states: HashMap::new(),
            online_provider_field_states,
            cuda_addon_status: None,
            cuda_addon_loading: false,
            cuda_addon_downloading: false,
            cuda_addon_progress: 0.0,
            cuda_addon_error: None,
            _filter_subscriptions: [engine_sub, status_sub, label_sub],
            _search_subscription,
        }
    }

    pub fn load_model_catalog(&mut self, cx: &mut Context<SettingsModal>) {
        if self.model_catalog_loading {
            return;
        }
        self.model_catalog_loading = true;
        self.model_catalog_error = None;
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result =
                crate::platform::agent_control::facade::models::get_model_catalog(&services).await;
            this.update(cx, |modal, cx| {
                modal.models.model_catalog_loading = false;
                match result {
                    Ok(snapshot) => {
                        modal.models.model_catalog = Some(snapshot);
                        modal.models.model_catalog_error = None;
                    }
                    Err(error) => modal.models.model_catalog_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn load_cuda_addon_status(&mut self, cx: &mut Context<SettingsModal>) {
        if self.cuda_addon_loading {
            return;
        }
        self.cuda_addon_loading = true;
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = services
                .sqlite
                .current_context()
                .map_err(|error| error.to_string())
                .and_then(|context| {
                    crate::platform::models::downloads::get_cuda_addon_status(
                        context.app_data_dir(),
                    )
                });
            this.update(cx, |modal, cx| {
                modal.models.cuda_addon_loading = false;
                match result {
                    Ok(status) => {
                        modal.models.cuda_addon_status = Some(status);
                        modal.models.cuda_addon_error = None;
                    }
                    Err(error) => modal.models.cuda_addon_error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn begin_cuda_addon_download(&mut self, cx: &mut Context<SettingsModal>) {
        if self.cuda_addon_downloading {
            return;
        }
        self.cuda_addon_downloading = true;
        self.cuda_addon_progress = 0.0;
        self.cuda_addon_error = None;
        let download_id = format!("settings-cuda-{}", uuid::Uuid::new_v4());
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let mut events = cx.global::<crate::app::GlobalEventBus>().0.subscribe();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let base_dir = services
                .sqlite
                .current_context()
                .map(|context| context.app_data_dir().join("models"))
                .map_err(|error| error.to_string());
            let result = if let Ok(base_dir) = base_dir {
                let config = services.config.load().ok().flatten().unwrap_or_else(|| json!({}));
                let mirror = config
                    .get("modelDownloadMirror")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let download = crate::platform::models::downloads::download_and_install_cuda_addon(
                    &base_dir,
                    Some(services.emitter.clone()),
                    &services.downloads,
                    download_id.clone(),
                    mirror,
                    None,
                    None,
                    None,
                );
                tokio::pin!(download);
                loop {
                    tokio::select! {
                        result = &mut download => break result,
                        event = events.recv() => {
                            let Ok(event) = event else { continue };
                            if event.name != "download-progress" { continue; }
                            let Some(values) = event.payload.as_array() else { continue };
                            if values.get(2).and_then(Value::as_str) != Some(download_id.as_str()) { continue; }
                            let downloaded = values.first().and_then(Value::as_f64).unwrap_or_default();
                            let total = values.get(1).and_then(Value::as_f64).unwrap_or_default();
                            let progress = if total > 0.0 { (downloaded / total * 100.0) as f32 } else { 0.0 };
                            this.update(cx, |modal, cx| {
                                modal.models.cuda_addon_progress = progress.clamp(0.0, 100.0);
                                cx.notify();
                            }).ok();
                        }
                    }
                }
            } else {
                Err(base_dir.unwrap_err())
            };
            this.update(cx, |modal, cx| {
                modal.models.cuda_addon_downloading = false;
                match result {
                    Ok(status) => {
                        modal.models.cuda_addon_status = Some(status);
                        modal.persist_model_setting(
                            "gpuAcceleration".to_string(),
                            Value::String("cuda".to_string()),
                            cx,
                        );
                    }
                    Err(error) => {
                        modal.models.cuda_addon_error = Some(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn restore_model_defaults(
        &mut self,
        modal: &mut SettingsModal,
        cx: &mut Context<SettingsModal>,
    ) {
        let Some(defaults) = self
            .model_catalog
            .as_ref()
            .and_then(|snapshot| snapshot.get("restoreDefaults"))
            .cloned()
        else {
            return;
        };
        if !modal.model_config.is_object() {
            modal.model_config = json!({});
        }
        for (config_key, default_key) in [
            ("streamingModelPath", "streamingModelPath"),
            ("batchModelPath", "batchModelPath"),
            ("liveVadModelPath", "vadModelPath"),
            ("batchVadModelPath", "vadModelPath"),
            ("livePunctuationModelPath", "punctuationModelPath"),
            ("batchPunctuationModelPath", "punctuationModelPath"),
            (
                "liveSpeakerSegmentationModelPath",
                "speakerSegmentationModelPath",
            ),
            (
                "batchSpeakerSegmentationModelPath",
                "speakerSegmentationModelPath",
            ),
            ("liveSpeakerEmbeddingModelPath", "speakerEmbeddingModelPath"),
            (
                "batchSpeakerEmbeddingModelPath",
                "speakerEmbeddingModelPath",
            ),
            ("liveAlignmentModelPath", "alignmentModelPath"),
            ("batchAlignmentModelPath", "alignmentModelPath"),
        ] {
            if let Some(value) = defaults.get(default_key) {
                modal.model_config[config_key] = value.clone();
            }
        }
        for key in ["enableITN", "batchVadEnabled", "maxConcurrent"] {
            if let Some(value) = defaults.get(key) {
                modal.model_config[key] = value.clone();
            }
        }
        if let Some(buffer) = defaults.get("vadBufferSize") {
            modal.model_config["liveVadBufferSize"] = buffer.clone();
            modal.model_config["batchVadBufferSize"] = buffer.clone();
        }
        let persisted = modal.model_config.clone();
        let config = cx.global::<GlobalDesktopServices>().0.config.clone();
        cx.spawn(async move |_this, _cx: &mut AsyncApp| {
            let _ = config.save(&persisted);
        })
        .detach();
        cx.notify();
    }

    pub fn download_model(&mut self, model_id: String, cx: &mut Context<SettingsModal>) {
        if self.model_download_ids.contains_key(&model_id) {
            return;
        }
        let download_id = format!("settings-{model_id}-{}", uuid::Uuid::new_v4());
        self.model_download_ids
            .insert(model_id.clone(), download_id.clone());
        self.model_download_progress.insert(model_id.clone(), 0.0);

        let services = cx.global::<GlobalDesktopServices>().0.clone();
        let mut events = cx.global::<crate::app::GlobalEventBus>().0.subscribe();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let config = services.config.load().ok().flatten().unwrap_or_else(|| json!({}));
            let mirror = config
                .get("modelDownloadMirror")
                .and_then(Value::as_str)
                .map(str::to_string);
            let models_dir = services
                .sqlite
                .current_context()
                .map(|context| context.app_data_dir().join("models"))
                .map_err(|error| error.to_string());
            let result = if let Ok(models_dir) = models_dir {
                let cancel_notify = std::sync::Arc::new(tokio::sync::Notify::new());
                let _download_guard = services
                    .downloads
                    .register_download(download_id.clone(), cancel_notify);
                let download = crate::platform::model_downloads::download_preset_model(
                    &models_dir,
                    Some(services.emitter.clone()),
                    &services.downloads,
                    model_id.clone(),
                    download_id.clone(),
                    mirror,
                );
                tokio::pin!(download);
                loop {
                    tokio::select! {
                        result = &mut download => break result,
                        event = events.recv() => {
                            let Ok(event) = event else { continue };
                            if event.name != "download-progress" { continue; }
                            let Some(values) = event.payload.as_array() else { continue };
                            if values.get(2).and_then(Value::as_str) != Some(download_id.as_str()) { continue; }
                            let downloaded = values.first().and_then(Value::as_f64).unwrap_or_default();
                            let total = values.get(1).and_then(Value::as_f64).unwrap_or_default();
                            let progress = if total > 0.0 { (downloaded / total * 100.0) as f32 } else { 0.0 };
                            this.update(cx, |modal, cx| {
                                modal.models.model_download_progress.insert(model_id.clone(), progress.clamp(0.0, 100.0));
                                cx.notify();
                            }).ok();
                        }
                    }
                }
            } else {
                Err(models_dir.unwrap_err())
            };

            let catalog = crate::platform::agent_control::facade::models::get_model_catalog(&services).await;
            this.update(cx, |modal, cx| {
                modal.models.model_download_ids.remove(&model_id);
                modal.models.model_download_progress.remove(&model_id);
                match catalog {
                    Ok(snapshot) => modal.models.model_catalog = Some(snapshot),
                    Err(error) => modal.models.model_catalog_error = Some(error),
                }
                if let Err(error) = result {
                    modal.models.model_catalog_error = Some(error);
                }
                cx.notify();
            }).ok();
        }).detach();
        cx.notify();
    }

    pub fn delete_model(
        &mut self,
        model_id: String,
        path: String,
        cx: &mut Context<SettingsModal>,
    ) {
        let services = cx.global::<GlobalDesktopServices>().0.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = crate::platform::agent_control::facade::models::delete_preset_model(
                &services, model_id,
            )
            .await;
            let catalog = if result.is_ok() {
                crate::platform::agent_control::facade::models::get_model_catalog(&services).await
            } else {
                result.as_ref().map(|_| Value::Null).map_err(Clone::clone)
            };
            this.update(cx, |modal, cx| {
                if let Err(error) = result {
                    modal.models.model_catalog_error = Some(error);
                } else if let Ok(snapshot) = catalog {
                    modal.models.model_catalog = Some(snapshot);
                    let config = services
                        .config
                        .load()
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| json!({}));
                    let mut config = config;
                    clear_model_path_references(&mut config, &path);
                    let _ = services.config.save(&config);
                } else if let Err(error) = catalog {
                    modal.models.model_catalog_error = Some(error);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn test_online_provider_model(
        &mut self,
        modal: &mut SettingsModal,
        provider_id: String,
        model_id: String,
        cx: &mut Context<SettingsModal>,
    ) {
        let state_key = format!("{provider_id}::{model_id}");
        let mut provider_config = modal
            .model_config
            .pointer(&format!("/asr/providers/online/{provider_id}"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !provider_config.is_object() {
            provider_config = json!({});
        }
        provider_config["model"] = Value::String(model_id.clone());
        self.online_provider_test_states
            .insert(state_key.clone(), OnlineProviderTestState::Loading);
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = sona_online_asr::test_online_asr_provider(&provider_id, &provider_config)
                .await
                .map_err(|error| error.to_string());
            let state = match result {
                Ok(latency_ms) => OnlineProviderTestState::Success { latency_ms },
                Err(message) => OnlineProviderTestState::Error { message },
            };
            let clear_after_delay = matches!(state, OnlineProviderTestState::Success { .. });
            this.update(cx, |modal, cx| {
                modal
                    .models
                    .online_provider_test_states
                    .insert(state_key.clone(), state.clone());
                cx.notify();
            })
            .ok();

            if clear_after_delay {
                tokio::time::sleep(std::time::Duration::from_millis(3500)).await;
                this.update(cx, |modal, cx| {
                    if matches!(
                        modal.models.online_provider_test_states.get(&state_key),
                        Some(OnlineProviderTestState::Success { .. })
                    ) {
                        modal.models.online_provider_test_states.remove(&state_key);
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn render(
        &mut self,
        model_config: &Value,
        cx: &mut Context<SettingsModal>,
    ) -> impl IntoElement {
        let model_catalog = self.model_catalog.as_ref();
        let loading = self.model_catalog_loading;
        let error = self.model_catalog_error.clone();
        let model_scenario_batch = self.model_scenario_batch;

        let asr_selected = model_config
            .pointer(if model_scenario_batch {
                "/asr/selections/batch/modelId"
            } else {
                "/asr/selections/live/modelId"
            })
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();

        let asr_options = vec![("default".to_string(), "Default Model".to_string())];
        let mut gpu_options = vec![
            ("auto".to_string(), "Automatic".to_string()),
            ("cpu".to_string(), "CPU only".to_string()),
        ];
        if !cfg!(target_os = "macos") {
            let cuda_label = if self
                .cuda_addon_status
                .as_ref()
                .is_some_and(|status| status.is_installed)
            {
                "NVIDIA CUDA (Installed)"
            } else {
                "NVIDIA CUDA (Download required)"
            };
            gpu_options.push(("cuda".to_string(), cuda_label.to_string()));
        }

        let _model_search_query = self.model_search_query.clone();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Model Hub"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted)
                            .child(
                                "Select recognition models and configure transcription behavior.",
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Model Selection"),
                    )
                    .child(
                        div().flex().gap_2().children([
                            Button::new("settings-model-scenario-live")
                                .when(!model_scenario_batch, |b| b.primary())
                                .when(model_scenario_batch, |b| b.outline())
                                .small()
                                .label("Real-time Live")
                                .on_click(cx.listener(|modal, _, _, cx| {
                                    modal.models.model_scenario_batch = false;
                                    cx.notify();
                                })),
                            Button::new("settings-model-scenario-batch")
                                .when(model_scenario_batch, |b| b.primary())
                                .when(!model_scenario_batch, |b| b.outline())
                                .small()
                                .label("Batch Import")
                                .on_click(cx.listener(|modal, _, _, cx| {
                                    modal.models.model_scenario_batch = true;
                                    cx.notify();
                                })),
                        ]),
                    )
                    .child(model_selector_row(
                        "Recognition Model",
                        "asr",
                        &asr_selected,
                        &asr_options,
                        self.model_selector_open.as_deref() == Some("asr"),
                        loading,
                        cx,
                    ))
                    .child(model_selector_row(
                        "GPU Acceleration",
                        "gpu-acceleration",
                        model_config
                            .get("gpuAcceleration")
                            .and_then(Value::as_str)
                            .unwrap_or("auto"),
                        &gpu_options,
                        self.model_selector_open.as_deref() == Some("gpu-acceleration"),
                        self.cuda_addon_downloading,
                        cx,
                    ))
                    .when(self.cuda_addon_loading, |panel| {
                        panel.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted)
                                .child("Checking CUDA runtime..."),
                        )
                    })
                    .when(self.cuda_addon_status.is_some(), |panel| {
                        panel.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted)
                                .child(
                                    if self
                                        .cuda_addon_status
                                        .as_ref()
                                        .is_some_and(|status| status.is_installed)
                                    {
                                        "CUDA runtime ready"
                                    } else {
                                        "CUDA runtime not installed"
                                    },
                                ),
                        )
                    })
                    .when(self.cuda_addon_downloading, |panel| {
                        panel.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .w(px(220.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted)
                                        .child(format!(
                                            "Downloading CUDA runtime ({:.0}%)...",
                                            self.cuda_addon_progress
                                        )),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(4.0))
                                        .rounded_sm()
                                        .bg(cx.theme().border)
                                        .child(
                                            div()
                                                .h(px(4.0))
                                                .w(px(220.0
                                                    * self.cuda_addon_progress.clamp(0.0, 100.0)
                                                    / 100.0))
                                                .rounded_sm()
                                                .bg(cx.theme().accent),
                                        ),
                                ),
                        )
                    })
                    .when(self.cuda_addon_error.is_some(), |panel| {
                        panel.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().danger)
                                .child(format!(
                                    "CUDA installation failed: {}",
                                    self.cuda_addon_error.as_deref().unwrap_or_default()
                                )),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Local Model Management"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted)
                            .child("Download and manage local recognition models."),
                    ),
            )
            .when(model_catalog.is_some(), |body| {
                body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Select::new(&self.model_engine_select)
                                .w(px(130.0))
                                .menu_width(px(170.0)),
                        )
                        .child(
                            Select::new(&self.model_status_select)
                                .w(px(130.0))
                                .menu_width(px(190.0)),
                        )
                        .child(
                            Select::new(&self.model_label_select)
                                .w(px(120.0))
                                .menu_width(px(150.0)),
                        )
                        .child(div().flex_1().child(Input::new(&self.model_search_state))),
                )
            })
            .when(loading && model_catalog.is_none(), |body| {
                body.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted)
                        .child("Checking local models..."),
                )
            })
            .when_some(error, |body, err| {
                body.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted)
                        .child(format!(
                            "Could not check local models: {err}. Download status may be stale."
                        )),
                )
            })
            .child(
                div()
                    .pt_3()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Online Model Management"),
            )
    }
}

pub fn clear_model_path_references(value: &mut Value, deleted_path: &str) {
    match value {
        Value::String(path) if path == deleted_path => *path = String::new(),
        Value::Array(items) => {
            for item in items {
                clear_model_path_references(item, deleted_path);
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                clear_model_path_references(field, deleted_path);
            }
        }
        _ => {}
    }
}

pub fn set_json_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }
    if !root.is_object() {
        *root = json!({});
    }
    let mut current = root;
    for key in &path[..path.len() - 1] {
        if !current.get(*key).is_some_and(Value::is_object) {
            current[*key] = json!({});
        }
        current = &mut current[*key];
    }
    current[path[path.len() - 1]] = value;
}

pub fn model_selector_row(
    title: &'static str,
    field: &'static str,
    selected: &str,
    options: &[(String, String)],
    _is_open: bool,
    loading: bool,
    cx: &mut Context<SettingsModal>,
) -> impl IntoElement {
    let current_label = options
        .iter()
        .find(|(val, _)| val == selected)
        .map(|(_, label)| label.as_str())
        .unwrap_or(selected);

    div()
        .flex()
        .items_center()
        .justify_between()
        .py_1()
        .child(div().text_size(px(13.0)).child(title))
        .child(
            div().flex().items_center().gap_2().child(
                Button::new(ElementId::Name(format!("model-select-{field}").into()))
                    .outline()
                    .small()
                    .label(current_label.to_string())
                    .disabled(loading)
                    .on_click(cx.listener(move |modal, _, _, cx| {
                        if modal.models.model_selector_open.as_deref() == Some(field) {
                            modal.models.model_selector_open = None;
                        } else {
                            modal.models.model_selector_open = Some(field.to_string());
                        }
                        cx.notify();
                    })),
            ),
        )
}

pub fn update_online_provider_model_config(
    config: &mut Value,
    provider_id: &str,
    model_id: &str,
    added: bool,
    is_default: bool,
) {
    let Some(provider) = sona_core::ports::asr::find_online_asr_provider(provider_id) else {
        return;
    };
    let provider_config = config
        .pointer("/asr/providers/online")
        .and_then(|providers| providers.get(provider_id));
    let mut ids = online_provider_added_model_ids(provider, provider_config);
    if added {
        ids.insert(model_id.to_string());
    } else {
        ids.remove(model_id);
    }
    let next_added = ids.clone();
    set_json_path(
        config,
        &["asr", "providers", "online", provider_id, "addedModels"],
        Value::Array(ids.into_iter().map(Value::String).collect()),
    );
    if !added
        && let Some(selections) = config
            .pointer_mut("/asr/selections")
            .and_then(Value::as_object_mut)
    {
        for slot in ["live", "caption", "voiceTyping", "batch"] {
            let Some(selection) = selections.get_mut(slot).and_then(Value::as_object_mut) else {
                continue;
            };
            if selection.get("engine").and_then(Value::as_str) != Some("online")
                || selection.get("providerId").and_then(Value::as_str) != Some(provider_id)
            {
                continue;
            }
            let selected_model = selection.get("modelId").and_then(Value::as_str);
            if selected_model == Some(model_id)
                || (selected_model.is_none() && is_default)
                || next_added.is_empty()
            {
                let mode = selection.get("mode").cloned().unwrap_or(Value::Null);
                *selection = serde_json::Map::from_iter([
                    ("engine".to_string(), Value::String("local".to_string())),
                    ("mode".to_string(), mode),
                    ("modelId".to_string(), Value::Null),
                    ("modelPath".to_string(), Value::String(String::new())),
                ]);
            }
        }
    }
}

pub fn online_provider_model_options(
    model_config: &Value,
    batch: bool,
    selected_id: Option<&str>,
) -> Vec<(String, String)> {
    let mode = if batch { "batch" } else { "streaming" };
    let mut options = Vec::new();
    for provider in sona_core::ports::asr::online_asr_providers() {
        if (batch && !provider.batch.local_file_mode.supported)
            || (!batch && provider.streaming.supported == Some(false))
        {
            continue;
        }
        let provider_config = model_config
            .pointer("/asr/providers/online")
            .and_then(|providers| providers.get(&provider.id));
        let has_api_key = provider_config
            .and_then(|config| config.get("apiKey"))
            .and_then(Value::as_str)
            .is_some_and(|key| !key.trim().is_empty());
        let selected_provider = selected_id.is_some_and(|selected| {
            selected == provider.id || selected.starts_with(&format!("{}::", provider.id))
        });
        if !has_api_key && !selected_provider {
            continue;
        }

        let added_models = online_provider_added_model_ids(provider, provider_config);
        let selected_model =
            selected_id.and_then(|selected| selected.strip_prefix(&format!("{}::", provider.id)));
        let active_models = provider
            .models
            .iter()
            .filter(|model| model.modes.iter().any(|model_mode| model_mode == mode))
            .filter(|model| {
                added_models.contains(model.id.as_str())
                    || selected_model == Some(model.id.as_str())
                    || (selected_id == Some(provider.id.as_str()) && model.is_default == Some(true))
            })
            .collect::<Vec<_>>();
        let provider_name = online_provider_default_name(&provider.id);
        if active_models.is_empty() {
            if selected_provider {
                options.push((provider.id.clone(), provider_name.to_string()));
            }
            continue;
        }
        for model in active_models.iter() {
            let explicit_id = format!("{}::{}", provider.id, model.id);
            let value = if active_models.len() == 1 && selected_id != Some(explicit_id.as_str()) {
                provider.id.clone()
            } else {
                explicit_id
            };
            options.push((value, format!("{} · {}", model.name, provider_name)));
        }
    }
    options
}

pub fn online_provider_added_model_ids(
    provider: &sona_core::ports::asr::OnlineAsrProvider,
    config: Option<&Value>,
) -> HashSet<String> {
    if let Some(models) = config
        .and_then(|config| config.get("addedModels"))
        .and_then(Value::as_array)
    {
        return models
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    let has_api_key = config
        .and_then(|config| config.get("apiKey"))
        .and_then(Value::as_str)
        .is_some_and(|key| !key.trim().is_empty());
    if !has_api_key {
        return HashSet::new();
    }
    let defaults = provider
        .models
        .iter()
        .filter(|model| model.is_default == Some(true))
        .map(|model| model.id.clone())
        .collect::<Vec<_>>();
    if !defaults.is_empty() {
        return defaults.into_iter().collect();
    }
    provider
        .models
        .first()
        .map(|model| [model.id.clone()].into_iter().collect())
        .unwrap_or_default()
}

pub fn online_provider_default_name(provider_id: &str) -> &'static str {
    match provider_id {
        "volcengine-doubao" => "Volcengine",
        "groq-whisper" => "Groq",
        "mistral-voxtral" => "Mistral",
        "openai-whisper" => "OpenAI",
        "deepgram" => "Deepgram",
        "assemblyai" => "AssemblyAI",
        "elevenlabs" => "ElevenLabs",
        "azure-speech" => "Azure Speech",
        "google-gemini" => "Google Gemini",
        "xai" => "xAI Audio",
        "gladia" => "Gladia",
        "revai" => "Rev.ai",
        "fal-whisper" => "FAL AI Whisper",
        _ => "Online Provider",
    }
}

pub fn online_provider_logo_path(provider_id: &str, provider_name: &str) -> Option<&'static str> {
    let token = format!("{provider_id} {provider_name}").to_lowercase();
    let path = if token.contains("volcengine") || token.contains("doubao") {
        "models/volcengine.png"
    } else if token.contains("groq") {
        "models/groq.png"
    } else if token.contains("mistral") || token.contains("voxtral") {
        "models/mistral.png"
    } else if token.contains("openai") {
        "models/openai.png"
    } else if token.contains("deepgram") {
        "models/deepgram.png"
    } else if token.contains("assemblyai") || token.contains("assembly") {
        "models/assemblyai.png"
    } else if token.contains("elevenlabs") {
        "models/elevenlabs.png"
    } else if token.contains("azure") {
        "providers/azure.svg"
    } else if token.contains("gemini") {
        "providers/gemini.svg"
    } else if token.contains("fal-whisper") || token.contains(" fal") {
        "providers/fal.svg"
    } else if token.contains("gladia") {
        "providers/gladia.svg"
    } else if token.contains("revai") || token.contains("rev.ai") {
        "providers/revai.svg"
    } else if token.contains("xai") || token.contains("x.ai") {
        "providers/xai.svg"
    } else if token.contains("r2t2") {
        "models/r2t2.png"
    } else if token.contains("qwen")
        || token.contains("sense-voice")
        || token.contains("sensevoice")
    {
        "models/sensevoice.png"
    } else if token.contains("fire-red") || token.contains("firered") {
        "models/firered.png"
    } else if token.contains("whisper") {
        "models/whisper.png"
    } else if token.contains("omnilingual") {
        "models/meta.png"
    } else if token.contains("paraformer") {
        "models/paraformer.png"
    } else if token.contains("funasr") {
        "models/funasr.png"
    } else if token.contains("zipformer") || token.contains("icefall") || token.contains("k2") {
        "models/zipformer.png"
    } else if token.contains("parakeet") || token.contains("nemo") || token.contains("canary") {
        "models/nvidia.png"
    } else if token.contains("dolphin") {
        "models/dolphin.png"
    } else if token.contains("moonshine") {
        "models/moonshine.png"
    } else {
        return None;
    };
    Some(path)
}

pub fn online_provider_languages_tooltip(languages: &[String]) -> String {
    const MAX_TOOLTIP_LANGUAGES: usize = 24;
    let sample = languages
        .iter()
        .take(MAX_TOOLTIP_LANGUAGES)
        .map(|code| online_language_display_name(code))
        .collect::<Vec<_>>()
        .join(", ");
    if languages.len() > MAX_TOOLTIP_LANGUAGES {
        format!("{sample} ... ({} languages in total)", languages.len())
    } else {
        sample
    }
}

pub fn online_language_display_name(code: &str) -> &str {
    match code {
        "af" => "Afrikaans",
        "am" => "Amharic",
        "ar" => "Arabic",
        "as" => "Assamese",
        "az" => "Azerbaijani",
        "ba" => "Bashkir",
        "be" => "Belarusian",
        "bg" => "Bulgarian",
        "bn" => "Bengali",
        "bo" => "Tibetan",
        "br" => "Breton",
        "bs" => "Bosnian",
        "ca" => "Catalan",
        "cs" => "Czech",
        "cy" => "Welsh",
        "da" => "Danish",
        "de" => "German",
        "el" => "Greek",
        "en" => "English",
        "es" => "Spanish",
        "et" => "Estonian",
        "eu" => "Basque",
        "fa" => "Persian",
        "fi" => "Finnish",
        "fil" => "Filipino",
        "fr" => "French",
        "gl" => "Galician",
        "gu" => "Gujarati",
        "ha" => "Hausa",
        "haw" => "Hawaiian",
        "he" => "Hebrew",
        "hi" => "Hindi",
        "hr" => "Croatian",
        "ht" => "Haitian Creole",
        "hu" => "Hungarian",
        "hy" => "Armenian",
        "id" => "Indonesian",
        "is" => "Icelandic",
        "it" => "Italian",
        "ja" => "Japanese",
        "jv" | "jw" => "Javanese",
        "ka" => "Georgian",
        "kk" => "Kazakh",
        "km" => "Khmer",
        "kn" => "Kannada",
        "ko" => "Korean",
        "la" => "Latin",
        "lb" => "Luxembourgish",
        "ln" => "Lingala",
        "lo" => "Lao",
        "lt" => "Lithuanian",
        "lv" => "Latvian",
        "mg" => "Malagasy",
        "mi" => "Maori",
        "mk" => "Macedonian",
        "ml" => "Malayalam",
        "mn" => "Mongolian",
        "mr" => "Marathi",
        "ms" => "Malay",
        "mt" => "Maltese",
        "my" => "Myanmar",
        "ne" => "Nepali",
        "nl" => "Dutch",
        "nn" => "Nynorsk",
        "no" => "Norwegian",
        "oc" => "Occitan",
        "pa" => "Punjabi",
        "pl" => "Polish",
        "ps" => "Pashto",
        "pt" => "Portuguese",
        "ro" => "Romanian",
        "ru" => "Russian",
        "sa" => "Sanskrit",
        "sd" => "Sindhi",
        "si" => "Sinhala",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "sn" => "Shona",
        "so" => "Somali",
        "sq" => "Albanian",
        "sr" => "Serbian",
        "su" => "Sundanese",
        "sv" => "Swedish",
        "sw" => "Swahili",
        "ta" => "Tamil",
        "te" => "Telugu",
        "tg" => "Tajik",
        "th" => "Thai",
        "tk" => "Turkmen",
        "tl" => "Tagalog",
        "tr" => "Turkish",
        "tt" => "Tatar",
        "uk" => "Ukrainian",
        "ur" => "Urdu",
        "uz" => "Uzbek",
        "vi" => "Vietnamese",
        "yi" => "Yiddish",
        "yo" => "Yoruba",
        "yue" => "Cantonese",
        "zh" => "Chinese",
        _ => code,
    }
}

pub fn online_provider_is_flash_batch_mode(
    provider: &sona_core::ports::asr::OnlineAsrProvider,
    config: Option<&Value>,
) -> bool {
    let endpoint = config
        .and_then(|config| config.get("batchEndpoint"))
        .and_then(Value::as_str)
        .or_else(|| {
            provider
                .defaults
                .get("batchEndpoint")
                .and_then(Value::as_str)
        })
        .unwrap_or(&provider.batch.local_file_mode.endpoint);
    let resource_id = config
        .and_then(|config| config.get("batchResourceId"))
        .and_then(Value::as_str)
        .or_else(|| {
            provider
                .defaults
                .get("batchResourceId")
                .and_then(Value::as_str)
        })
        .unwrap_or(&provider.batch.local_file_mode.resource_id);
    endpoint == provider.batch.local_file_mode.endpoint
        && resource_id == provider.batch.local_file_mode.resource_id
}
