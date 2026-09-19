use serde_json::{Map, Value, json};

use crate::ports::asr::online_asr_providers;

pub const CURRENT_CONFIG_VERSION: i64 = 8;
pub const DEFAULT_POLISH_PRESET_ID: &str = "clean";
pub const DEFAULT_SUMMARY_TEMPLATE_ID: &str = "general";
pub const DEFAULT_LLM_PROVIDER: &str = "google_translate_free";
pub const LEGACY_OPENAI_COMPATIBLE_PROVIDER: &str = "custom-openai-compatible";
pub const LEGACY_OPENAI_COMPATIBLE_CREATED_AT: &str = "2026-05-18T00:00:00.000Z";

pub const BUILTIN_POLISH_PRESET_IDS: [&str; 3] = ["clean", "verbatim", "formal"];
pub const BUILTIN_SUMMARY_TEMPLATE_IDS: [&str; 3] = ["general", "meeting", "lecture"];

pub fn default_config() -> Value {
    let mut config = Map::new();
    for (key, value) in [
        ("configVersion", json!(CURRENT_CONFIG_VERSION)),
        ("appLanguage", json!("auto")),
        ("theme", json!("auto")),
        ("font", json!("system")),
        ("minimizeToTrayOnExit", json!(true)),
        ("autoCheckUpdates", json!(true)),
        ("logLevel", json!("info")),
        ("liveRecordShortcut", json!("Ctrl + Space")),
        ("microphoneId", json!("default")),
        ("systemAudioDeviceId", json!("default")),
        ("muteDuringRecording", json!(false)),
        ("keepMicrophoneActive", json!(false)),
        ("ffmpegPath", json!("")),
        ("asr", default_asr_config()),
        ("streamingModelPath", json!("")),
        ("batchModelPath", json!("")),
        ("livePunctuationModelPath", json!("")),
        ("liveVadModelPath", json!("")),
        ("liveSpeakerSegmentationModelPath", json!("")),
        ("liveSpeakerEmbeddingModelPath", json!("")),
        ("liveAlignmentModelPath", json!("")),
        ("batchPunctuationModelPath", json!("")),
        ("batchVadModelPath", json!("")),
        ("batchSpeakerSegmentationModelPath", json!("")),
        ("batchSpeakerEmbeddingModelPath", json!("")),
        ("batchAlignmentModelPath", json!("")),
        ("lockWindow", json!(false)),
        ("alwaysOnTop", json!(true)),
        ("startOnLaunch", json!(false)),
        ("captionWindowWidth", json!(800)),
        ("captionFontSize", json!(24)),
        ("captionFontColor", json!("#ffffff")),
        ("captionBackgroundColor", json!("#000000")),
        ("captionBackgroundOpacity", json!(0.6)),
        ("language", json!("auto")),
        ("enableTimeline", json!(false)),
        ("enableITN", json!(true)),
        ("batchVadEnabled", json!(true)),
        ("liveVadBufferSize", json!(5)),
        ("batchVadBufferSize", json!(5)),
        ("maxConcurrent", json!(2)),
        ("gpuAcceleration", json!("auto")),
        ("llmSettings", create_llm_settings()),
        ("summaryEnabled", json!(true)),
        ("summaryTemplateId", json!(DEFAULT_SUMMARY_TEMPLATE_ID)),
        ("summaryCustomTemplates", json!([])),
        ("translationLanguage", json!("zh")),
        ("polishKeywords", json!("")),
        ("polishPresetId", json!(DEFAULT_POLISH_PRESET_ID)),
        ("polishCustomPresets", json!([])),
        ("autoPolish", json!(false)),
        ("autoPolishFrequency", json!(5)),
        ("voiceTypingEnabled", json!(false)),
        ("voiceTypingShortcut", json!("Alt+V")),
        ("voiceTypingMode", json!("hold")),
        ("voiceTypingProcessingMode", json!("raw")),
        ("voiceTypingSoundEnabled", json!(true)),
        ("voiceTypingCjkSpacingEnabled", json!(true)),
        ("voiceTypingPolishPrompt", json!("")),
        ("voiceTypingPlacement", json!("caret")),
        ("voiceTypingQuickRecallShortcut", json!("Alt + Shift + H")),
        ("voiceTypingContextAwarenessEnabled", json!(true)),
        ("voiceTypingContextPreset", json!("auto")),
        ("voiceTypingContextRules", create_default_voice_typing_context_rules()),
        ("textReplacementSets", json!([])),
        ("hotwordSets", json!([])),
        ("polishKeywordSets", json!([])),
        ("speakerProfiles", json!([])),
        ("speakerDiarizationSensitivity", json!("balanced")),
        ("hotwords", json!([])),
        ("httpServerEnabled", json!(false)),
        ("httpServerPort", json!(14200)),
        ("httpServerHost", json!("127.0.0.1")),
        ("httpServerApiKey", json!("")),
        ("historyAudioRetentionDays", Value::Null),
    ] {
        config.insert(key.to_string(), value);
    }
    Value::Object(config)
}

pub fn default_asr_config() -> Value {
    let mut config = Map::new();
    let mut selections = Map::new();
    for key in ["live", "caption", "voiceTyping"] {
        selections.insert(
            key.to_string(),
            json!({
                "engine": "local",
                "mode": "streaming",
                "modelId": null,
                "modelPath": ""
            }),
        );
    }
    selections.insert(
        "batch".to_string(),
        json!({
            "engine": "local",
            "mode": "batch",
            "modelId": null,
            "modelPath": ""
        }),
    );
    config.insert("selections".to_string(), Value::Object(selections));

    let mut online_providers = Map::new();
    for provider in online_asr_providers() {
        online_providers.insert(provider.id.clone(), provider.defaults.clone());
    }
    let mut providers = Map::new();
    providers.insert("online".to_string(), Value::Object(online_providers));
    config.insert("providers".to_string(), Value::Object(providers));

    Value::Object(config)
}

pub fn create_llm_settings() -> Value {
    json!({
        "activeProvider": DEFAULT_LLM_PROVIDER,
        "customProviders": {},
        "providers": {
            DEFAULT_LLM_PROVIDER: {
                "apiHost": "https://translate.googleapis.com/translate_a/single",
                "apiKey": ""
            }
        },
        "models": {},
        "modelOrder": [],
        "selections": {}
    })
}
pub fn create_default_voice_typing_context_rules() -> Value {
    json!([
        {
            "id": "developer",
            "name": "Developer",
            "icon": "💻",
            "badgeColor": "#818cf8",
            "appsByPlatform": {
                "windows": [
                    "code.exe", "cursor.exe", "idea64.exe", "devenv.exe", "clion64.exe",
                    "pycharm64.exe", "webstorm64.exe", "goland64.exe", "rider64.exe",
                    "sublime_text.exe", "zed.exe", "neovide.exe", "nvim.exe", "vim.exe",
                    "emacs.exe", "windowsterminal.exe", "powershell.exe", "cmd.exe",
                    "alacritty.exe", "wezterm-gui.exe", "kitty.exe", "warp.exe",
                    "git-bash.exe", "conhost.exe", "mintty.exe", "postman.exe",
                    "dbeaver.exe", "datagrip64.exe", "navicat.exe"
                ],
                "macos": [
                    "Visual Studio Code", "Code", "Xcode", "Cursor", "Zed",
                    "Sublime Text", "IntelliJ IDEA", "CLion", "PyCharm", "WebStorm",
                    "GoLand", "Rider", "DataGrip", "Postman", "DBeaver", "Terminal",
                    "iTerm2", "Alacritty", "kitty", "Warp", "Neovide", "MacVim"
                ],
                "linux": [
                    "code", "cursor", "zed", "sublime_text", "idea", "clion",
                    "pycharm", "webstorm", "goland", "rider", "datagrip", "postman",
                    "dbeaver", "gnome-terminal", "konsole", "alacritty", "kitty",
                    "wezterm", "warp-terminal", "xfce4-terminal"
                ]
            },
            "titlePatterns": [
                "github", "gitlab", "stackoverflow", "localhost", "pull request", "leetcode"
            ],
            "promptDirective": "Developer & Engineering Mode: Preserve technical terms, exact library/module names, and code identifiers (camelCase, PascalCase, snake_case, kebab-case, UPPER_CASE, CLI flags like --flag). Never spell out code symbols as prose. Keep it succinct and technical. Never add trailing periods or punctuation unless dictating explanatory comments.",
            "stripTrailingPunctuation": true,
            "isBuiltin": true,
            "enabled": true
        },
        {
            "id": "chat",
            "name": "Chat",
            "icon": "💬",
            "badgeColor": "#34d399",
            "appsByPlatform": {
                "windows": [
                    "wechat.exe", "qq.exe", "slack.exe", "discord.exe", "telegram.exe",
                    "dingtalk.exe", "feishu.exe", "teams.exe", "lark.exe", "whatsapp.exe",
                    "signal.exe", "skype.exe", "line.exe", "element.exe"
                ],
                "macos": [
                    "WeChat", "QQ", "Slack", "Discord", "Telegram", "DingTalk",
                    "Feishu", "Lark", "Microsoft Teams", "WhatsApp", "Signal",
                    "Skype", "LINE", "Element", "Messages"
                ],
                "linux": [
                    "wechat", "qq", "slack", "discord", "telegram-desktop", "dingtalk",
                    "feishu", "lark", "teams", "whatsapp-for-linux", "signal-desktop",
                    "skypeforlinux", "element-desktop"
                ]
            },
            "titlePatterns": [
                "slack |", "discord", "telegram", "wechat", "messages"
            ],
            "promptDirective": "Instant Messaging & Chat Mode: Use a natural, conversational, lightweight tone. Split long rambling speech into clear, compact phrases. Separate clauses with spaces rather than heavy commas. Never add trailing periods or full stops.",
            "stripTrailingPunctuation": true,
            "isBuiltin": true,
            "enabled": true
        },
        {
            "id": "formal",
            "name": "Formal",
            "icon": "📄",
            "badgeColor": "#f59e0b",
            "appsByPlatform": {
                "windows": [
                    "winword.exe", "excel.exe", "powerpnt.exe", "outlook.exe",
                    "foxmail.exe", "thunderbird.exe", "wps.exe", "wpp.exe", "et.exe",
                    "notion.exe", "obsidian.exe", "typora.exe", "logseq.exe",
                    "craft.exe", "acrobat.exe"
                ],
                "macos": [
                    "Microsoft Word", "Microsoft Excel", "Microsoft PowerPoint",
                    "Pages", "Numbers", "Keynote", "Microsoft Outlook", "Mail",
                    "Foxmail", "Thunderbird", "WPS Office", "Notion", "Obsidian",
                    "Typora", "Logseq", "Craft", "TextEdit"
                ],
                "linux": [
                    "libreoffice", "soffice.bin", "wps", "wpp", "et", "notion-app",
                    "obsidian", "typora", "thunderbird"
                ]
            },
            "titlePatterns": [
                "document", "report", "notion", "obsidian", "word", "excel", "sheets", "docs"
            ],
            "promptDirective": "Formal Writing & Document Mode: Convert colloquial spoken expressions into rigorous, well-structured, professional written language. Ensure complete grammatical sentence structures and proper punctuation.",
            "stripTrailingPunctuation": false,
            "isBuiltin": true,
            "enabled": true
        }
    ])
}
