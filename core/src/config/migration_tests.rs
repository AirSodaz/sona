use super::*;
use crate::config::migrate_app_config;

#[test]
fn config_core_migrates_legacy_config_to_current_shape() {
    let legacy = json!({
        "configVersion": 1,
        "modelPath": "models/base",
        "recognitionModelPath": "models/recognition",
        "llmModel": "gpt-4.1-mini",
        "llmServiceType": "openai",
        "logLevel": "verbose",
        "microphoneBoost": 0.0,
        "captionBackgroundOpacity": 0.0,
        "summaryTemplate": "invalid-old-template",
        "polishScenario": "custom",
        "polishContext": "Use concise academic Chinese.",
        "polishKeywords": "Sona\nASR",
        "textReplacements": [
            { "id": "r1", "from": "foo", "to": "bar", "enabled": false }
        ],
        "hotwords": ["Sona", "Tauri"],
        "speakerProfiles": [
            {
                "id": " speaker-1 ",
                "name": " Alice ",
                "samples": [
                    {
                        "id": " sample-1 ",
                        "filePath": " C:/audio.wav ",
                        "sourceName": "",
                        "durationSeconds": -4
                    }
                ]
            },
            { "name": "Missing id" }
        ]
    });

    let result = migrate_app_config(None, Some(legacy), "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(result.config["configVersion"], 7);
    assert_eq!(result.config["streamingModelPath"], "models/recognition");
    assert_eq!(result.config["batchModelPath"], "models/recognition");
    assert_eq!(
        result.config["asr"]["selections"]["live"],
        json!({
            "engine": "local-sherpa",
            "mode": "streaming",
            "modelId": null,
            "modelPath": "models/recognition"
        })
    );
    assert_eq!(
        result.config["asr"]["selections"]["batch"],
        json!({
            "engine": "local-sherpa",
            "mode": "batch",
            "modelId": null,
            "modelPath": "models/recognition"
        })
    );
    assert_eq!(result.config["logLevel"], "info");
    assert_eq!(result.config["keepMicrophoneActive"], false);
    assert_eq!(result.config["microphoneBoost"], 0.0);
    assert_eq!(result.config["captionBackgroundOpacity"], 0.0);
    assert_eq!(result.config["summaryTemplateId"], "general");
    assert_eq!(result.config["polishKeywords"], "");
    assert_eq!(result.config["polishPresetId"], "custom-9158016c");
    assert_eq!(
        result.config["polishCustomPresets"][0],
        json!({
            "id": "custom-9158016c",
            "name": "Imported Preset (915801)",
            "context": "Use concise academic Chinese."
        })
    );
    assert_eq!(
        result.config["polishKeywordSets"][0],
        json!({
            "id": "polish-keywords-d74b61fc",
            "name": "Imported Keywords (d74b61)",
            "enabled": true,
            "keywords": "Sona\nASR"
        })
    );
    assert_eq!(
        result.config["textReplacementSets"][0]["name"],
        "Default Rules"
    );
    assert_eq!(
        result.config["textReplacementSets"][0]["rules"][0],
        json!({ "id": "r1", "from": "foo", "to": "bar" })
    );
    assert_eq!(
        result.config["hotwordSets"][0]["rules"][1],
        json!({ "id": "hw-1", "text": "Tauri" })
    );
    assert_eq!(
        result.config["speakerProfiles"][0],
        json!({
            "id": "speaker-1",
            "name": "Alice",
            "enabled": true,
            "samples": [
                {
                    "id": "sample-1",
                    "filePath": "C:/audio.wav",
                    "sourceName": "Sample",
                    "durationSeconds": 0.0
                }
            ]
        })
    );
    assert_eq!(result.config["llmSettings"]["activeProvider"], "open_ai");
    assert_eq!(
        result.config["llmSettings"]["selections"]["summaryModelId"],
        "open_ai-gpt-4-1-mini"
    );
}

#[test]
fn config_core_normalizes_saved_current_config_without_false_migration() {
    let saved = json!({
        "configVersion": 7,
        "asr": {
            "selections": {
                "live": { "engine": "local-sherpa", "mode": "streaming", "modelId": null, "modelPath": "C:/models/live" },
                "caption": { "engine": "local-sherpa", "mode": "streaming", "modelId": null, "modelPath": "C:/models/live" },
                "voiceTyping": { "engine": "local-sherpa", "mode": "streaming", "modelId": null, "modelPath": "C:/models/live" },
                "batch": { "engine": "local-sherpa", "mode": "batch", "modelId": null, "modelPath": "C:/models/offline" }
            }
        },
        "streamingModelPath": "C:/models/live",
        "offlineModelPath": "C:/models/offline",
        "summaryEnabled": true,
        "summaryTemplateId": "meeting",
        "summaryCustomTemplates": [],
        "polishPresetId": "meeting",
        "polishCustomPresets": [],
        "polishKeywordSets": [],
        "speakerProfiles": [],
        "speakerSegmentationModelPath": "",
        "speakerEmbeddingModelPath": "",
        "logLevel": "debug",
        "llmSettings": {
            "activeProvider": "open_ai",
            "customProviders": {},
            "modelDiscovery": {},
            "providers": {
                "open_ai": { "apiHost": "https://api.openai.com", "apiKey": "" }
            },
            "models": {
                "model-1": { "id": "model-1", "provider": "open_ai", "model": "gpt-4.1-mini" }
            },
            "modelOrder": ["model-1"],
            "selections": {
                "polishModelId": "model-1",
                "translationModelId": "model-1",
                "summaryModelId": "model-1"
            }
        },
        "httpServerEnabled": false,
        "httpServerHost": "127.0.0.1",
        "httpServerPort": 14200,
        "httpServerApiKey": "",
        "keepMicrophoneActive": true
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(!result.migrated);
    assert_eq!(result.config["configVersion"], 7);
    assert_eq!(
        result.config["asr"]["selections"]["voiceTyping"]["modelPath"],
        "C:/models/live"
    );
    assert_eq!(result.config["summaryTemplateId"], "meeting");
    assert_eq!(result.config["polishPresetId"], "meeting");
    assert_eq!(result.config["logLevel"], "debug");
    assert_eq!(result.config["keepMicrophoneActive"], true);
}

#[test]
fn config_core_upgrades_current_config_without_asr_to_new_asr_shape() {
    let saved = json!({
        "configVersion": 6,
        "streamingModelPath": "C:/models/live",
        "offlineModelPath": "C:/models/offline",
        "summaryEnabled": true,
        "summaryTemplateId": "meeting",
        "summaryCustomTemplates": [],
        "polishPresetId": "meeting",
        "polishCustomPresets": [],
        "polishKeywordSets": [],
        "speakerProfiles": [],
        "speakerSegmentationModelPath": "",
        "speakerEmbeddingModelPath": "",
        "logLevel": "info",
        "llmSettings": {
            "activeProvider": "google_translate_free",
            "providers": {
                "google_translate_free": {
                    "apiHost": "https://translate.googleapis.com/translate_a/single",
                    "apiKey": ""
                }
            },
            "models": {},
            "modelOrder": [],
            "selections": {}
        }
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(result.config["configVersion"], 7);
    assert_eq!(
        result.config["asr"]["selections"]["caption"]["modelPath"],
        "C:/models/live"
    );
    assert_eq!(
        result.config["asr"]["selections"]["batch"]["modelPath"],
        "C:/models/offline"
    );
    assert_eq!(result.config["streamingModelPath"], "C:/models/live");
    assert_eq!(result.config["offlineModelPath"], "C:/models/offline");
}

#[test]
fn config_core_migrates_volcengine_asr_selection_to_online_provider_shape() {
    let saved = json!({
        "configVersion": 7,
        "asr": {
            "selections": {
                "live": {
                    "engine": "volcengine-doubao",
                    "mode": "streaming",
                    "modelId": null,
                    "modelPath": "",
                    "providerId": "volcengine-doubao",
                    "profileId": "volcengine-doubao-default"
                },
                "caption": {
                    "engine": "volcengine-doubao",
                    "mode": "streaming",
                    "modelId": null,
                    "modelPath": "",
                    "providerId": "volcengine-doubao",
                    "profileId": "volcengine-doubao-default"
                },
                "voiceTyping": {
                    "engine": "volcengine-doubao",
                    "mode": "streaming",
                    "modelId": null,
                    "modelPath": "",
                    "providerId": "volcengine-doubao",
                    "profileId": "volcengine-doubao-default"
                },
                "batch": {
                    "engine": "volcengine-doubao",
                    "mode": "batch",
                    "modelId": null,
                    "modelPath": "",
                    "providerId": "volcengine-doubao",
                    "profileId": "volcengine-doubao-default"
                }
            },
            "providers": {
                "volcengineDoubao": {
                    "apiKey": " volc-test-key ",
                    "streamingEndpoint": "",
                    "streamingResourceId": "",
                    "batchEndpoint": "",
                    "batchResourceId": ""
                }
            }
        },
        "streamingModelPath": "C:/models/live",
        "offlineModelPath": "C:/models/offline",
        "summaryEnabled": true,
        "summaryTemplateId": "meeting",
        "summaryCustomTemplates": [],
        "polishPresetId": "meeting",
        "polishCustomPresets": [],
        "polishKeywordSets": [],
        "speakerProfiles": [],
        "speakerSegmentationModelPath": "",
        "speakerEmbeddingModelPath": "",
        "logLevel": "info",
        "llmSettings": {
            "activeProvider": "google_translate_free",
            "providers": {
                "google_translate_free": {
                    "apiHost": "https://translate.googleapis.com/translate_a/single",
                    "apiKey": ""
                }
            },
            "models": {},
            "modelOrder": [],
            "selections": {}
        }
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(
        result.config["asr"]["selections"]["batch"],
        json!({
            "engine": "online",
            "mode": "batch",
            "modelId": null,
            "modelPath": "",
            "providerId": "volcengine-doubao",
            "profileId": "volcengine-doubao-default"
        })
    );
    assert_eq!(
        result.config["asr"]["providers"]["online"]["volcengine-doubao"],
        json!({
            "apiKey": "volc-test-key",
            "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
            "streamingResourceId": "volc.seedasr.sauc.duration",
            "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash",
            "batchResourceId": "volc.bigasr.auc_turbo"
        })
    );
    assert_eq!(result.config["streamingModelPath"], "C:/models/live");
    assert_eq!(result.config["offlineModelPath"], "C:/models/offline");
}

#[test]
fn config_core_normalizes_saved_volcengine_async_batch_provider_to_flash() {
    let saved = json!({
        "configVersion": 7,
        "asr": {
            "selections": {
                "batch": {
                    "engine": "volcengine-doubao",
                    "mode": "batch",
                    "modelId": null,
                    "modelPath": "",
                    "providerId": "volcengine-doubao",
                    "profileId": "volcengine-doubao-default"
                }
            },
            "providers": {
                "volcengineDoubao": {
                    "apiKey": "volc-test-key",
                    "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
                    "streamingResourceId": "volc.seedasr.sauc.duration",
                    "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/idle/submit",
                    "batchResourceId": "volc.bigasr.auc_idle"
                }
            }
        }
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(
        result.config["asr"]["providers"]["online"]["volcengine-doubao"]["batchEndpoint"],
        "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash"
    );
    assert_eq!(
        result.config["asr"]["providers"]["online"]["volcengine-doubao"]["batchResourceId"],
        "volc.bigasr.auc_turbo"
    );
}

#[test]
fn config_core_normalizes_current_null_summary_enabled_to_true() {
    let saved = json!({
        "configVersion": 6,
        "summaryEnabled": null,
        "summaryTemplateId": "meeting",
        "summaryCustomTemplates": [],
        "polishPresetId": "meeting",
        "polishCustomPresets": [],
        "polishKeywordSets": [],
        "speakerProfiles": [],
        "speakerSegmentationModelPath": "",
        "speakerEmbeddingModelPath": "",
        "logLevel": "info",
        "llmSettings": {
            "activeProvider": "google_translate_free",
            "providers": {
                "google_translate_free": {
                    "apiHost": "https://translate.googleapis.com/translate_a/single",
                    "apiKey": ""
                }
            },
            "models": {},
            "modelOrder": [],
            "selections": {}
        }
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(result.config["summaryEnabled"], true);
}

#[test]
fn config_core_preserves_custom_llm_providers() {
    let saved = json!({
        "configVersion": 6,
        "summaryEnabled": true,
        "summaryTemplateId": "meeting",
        "summaryCustomTemplates": [],
        "polishPresetId": "meeting",
        "polishCustomPresets": [],
        "polishKeywordSets": [],
        "speakerProfiles": [],
        "speakerSegmentationModelPath": "",
        "speakerEmbeddingModelPath": "",
        "logLevel": "debug",
        "llmSettings": {
            "activeProvider": "custom-acme",
            "customProviders": {
                "custom-acme": {
                    "id": "custom-acme",
                    "name": "Acme Gateway",
                    "strategy": "openai_responses",
                    "createdAt": "2026-05-18T08:00:00.000Z"
                }
            },
            "providers": {
                "custom-acme": {
                    "apiHost": "https://gateway.example.com",
                    "apiKey": "test-key"
                }
            },
            "models": {
                "model-1": { "id": "model-1", "provider": "custom-acme", "model": "gpt-4o" }
            },
            "modelOrder": ["model-1"],
            "selections": {
                "polishModelId": "model-1",
                "translationModelId": "model-1",
                "summaryModelId": "model-1"
            }
        }
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(
        result.config["llmSettings"]["activeProvider"],
        "custom-acme"
    );
    assert_eq!(
        result.config["llmSettings"]["customProviders"]["custom-acme"]["strategy"],
        "openai_responses"
    );
    assert_eq!(
        result.config["llmSettings"]["providers"]["custom-acme"]["apiPath"],
        "/v1/responses"
    );
    assert_eq!(
        result.config["llmSettings"]["models"]["model-1"]["provider"],
        "custom-acme"
    );
}

#[test]
fn llm_state_preserves_runtime_metadata_discovery_and_reasoning() {
    let normalized = ensure_llm_state(&json!({
        "llmSettings": {
            "activeProvider": "open_ai",
            "providers": {
                "open_ai": { "apiHost": "https://api.openai.com", "apiKey": "key" }
            },
            "models": {
                "model-1": {
                    "id": "model-1",
                    "provider": "open_ai",
                    "model": "gpt-test",
                    "source": "discovered",
                    "metadata": {
                        "displayName": "GPT Test",
                        "cacheReadPrice": 0.5,
                        "inputModalities": ["text", "image", "invalid"],
                        "supportsStructuredOutput": true,
                        "metadataSources": ["provider", "models_dev"]
                    },
                    "metadataOverrides": {
                        "cacheReadPrice": true,
                        "metadataSources": true,
                        "unknown": true
                    }
                }
            },
            "modelOrder": ["model-1"],
            "modelDiscovery": {
                "open_ai": {
                    "fetchedAt": "2026-07-15T00:00:00Z",
                    "expiresAt": "2026-07-16T00:00:00Z"
                }
            },
            "selections": {
                "polishModelId": "model-1",
                "polishReasoningEnabled": true,
                "polishReasoningLevel": "high"
            }
        }
    }));

    let metadata = &normalized["models"]["model-1"]["metadata"];
    assert_eq!(
        (
            metadata["displayName"].as_str(),
            metadata["inputModalities"].as_array().map(Vec::len),
            metadata["supportsStructuredOutput"].as_bool(),
        ),
        (Some("GPT Test"), Some(2), Some(true))
    );
    assert_eq!(
        (
            normalized["selections"]["polishReasoningEnabled"].as_bool(),
            normalized["selections"]["polishReasoningLevel"].as_str(),
            normalized["modelDiscovery"]["open_ai"]["expiresAt"].as_str(),
        ),
        (Some(true), Some("high"), Some("2026-07-16T00:00:00Z"))
    );
    assert_eq!(
        normalized["models"]["model-1"]["metadataOverrides"],
        json!({ "cacheReadPrice": true })
    );
}

#[test]
fn config_core_migrates_legacy_openai_compatible_to_custom_provider() {
    let saved = json!({
        "configVersion": 6,
        "summaryEnabled": true,
        "summaryTemplateId": "meeting",
        "summaryCustomTemplates": [],
        "polishPresetId": "meeting",
        "polishCustomPresets": [],
        "polishKeywordSets": [],
        "speakerProfiles": [],
        "speakerSegmentationModelPath": "",
        "speakerEmbeddingModelPath": "",
        "logLevel": "info",
        "llmSettings": {
            "activeProvider": "open_ai_compatible",
            "providers": {
                "open_ai_compatible": {
                    "apiHost": "https://compat.example.com",
                    "apiKey": "compat-key"
                }
            },
            "models": {
                "model-1": { "id": "model-1", "provider": "open_ai_compatible", "model": "compat-model" }
            },
            "modelOrder": ["model-1"],
            "selections": {
                "polishModelId": "model-1",
                "translationModelId": "model-1",
                "summaryModelId": "model-1"
            }
        }
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(
        result.config["llmSettings"]["activeProvider"],
        "custom-openai-compatible"
    );
    assert_eq!(
        result.config["llmSettings"]["customProviders"]["custom-openai-compatible"],
        json!({
            "id": "custom-openai-compatible",
            "name": "OpenAI Compatible",
            "strategy": "openai_compatible",
            "createdAt": "2026-05-18T00:00:00.000Z"
        })
    );
    assert_eq!(
        result.config["llmSettings"]["providers"]["custom-openai-compatible"]["apiHost"],
        "https://compat.example.com"
    );
    assert_eq!(
        result.config["llmSettings"]["models"]["model-1"]["provider"],
        "custom-openai-compatible"
    );
}

#[test]
fn config_core_cleans_up_dirty_data_in_online_providers_and_falls_back_to_defaults() {
    let saved = json!({
        "configVersion": 7,
        "asr": {
            "selections": {
                "live": { "engine": "online", "mode": "streaming", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" },
                "caption": { "engine": "online", "mode": "streaming", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" },
                "voiceTyping": { "engine": "online", "mode": "streaming", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" },
                "batch": { "engine": "online", "mode": "batch", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" }
            },
            "providers": {
                "online": {
                    "volcengine-doubao": {
                        "apiKey": 12345, // invalid type
                        "streamingEndpoint": "custom-endpoint",
                        "unknownKey": "garbage"
                    },
                    "groq-whisper": {
                        "apiKey": "groq-key"
                    },
                    "garbage-provider": {
                        "apiKey": "should-be-deleted"
                    }
                }
            }
        }
    });

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);

    // Volcengine cleanup: invalid apiKey type falls back to default empty string, unknownKey is removed, valid string is kept
    let volc = &result.config["asr"]["providers"]["online"]["volcengine-doubao"];
    assert_eq!(volc["apiKey"], "");
    assert_eq!(volc["streamingEndpoint"], "custom-endpoint");
    assert!(volc.get("unknownKey").is_none());

    // Groq kept its api key, other defaults populated
    let groq = &result.config["asr"]["providers"]["online"]["groq-whisper"];
    assert_eq!(groq["apiKey"], "groq-key");
    assert_eq!(
        groq["batchEndpoint"],
        "https://api.groq.com/openai/v1/audio/transcriptions"
    );

    // Unknown providers are garbage collected
    assert!(
        result.config["asr"]["providers"]["online"]
            .get("garbage-provider")
            .is_none()
    );

    // Mistral should be fully hydrated from defaults
    let mistral = &result.config["asr"]["providers"]["online"]["mistral-voxtral"];
    assert_eq!(mistral["apiKey"], "");
    assert_eq!(
        mistral["batchEndpoint"],
        "https://api.mistral.ai/v1/audio/transcriptions"
    );
}
