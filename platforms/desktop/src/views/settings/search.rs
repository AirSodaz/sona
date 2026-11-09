use super::SettingsTab;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SettingsSearchHit {
    pub id: &'static str,
    pub title: &'static str,
    pub tab: SettingsTab,
    pub keywords: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SearchSubsection {
    ModelScenarioBatch(bool),
    VoiceTyping(bool),
    Vocabulary(&'static str),
}

pub(super) fn subsection_for_hit(id: &str) -> Option<SearchSubsection> {
    match id {
        "asr-model" | "asr-batch" | "vad-batch" => Some(SearchSubsection::ModelScenarioBatch(true)),
        "asr-live" | "vad-live" | "punctuation-model" => {
            Some(SearchSubsection::ModelScenarioBatch(false))
        }
        "voice-typing"
        | "voice-typing-mode"
        | "voice-typing-processing"
        | "voice-typing-context"
        | "voice-typing-history" => Some(SearchSubsection::VoiceTyping(true)),
        "translation" | "subtitle-window" => Some(SearchSubsection::VoiceTyping(false)),
        "dictionary" | "replacement" => Some(SearchSubsection::Vocabulary("recognition")),
        "prompt" => Some(SearchSubsection::Vocabulary("polish")),
        "context-preset" => Some(SearchSubsection::Vocabulary("polish")),
        "summary-template" => Some(SearchSubsection::Vocabulary("summary")),
        "vocabulary-translation" => Some(SearchSubsection::Vocabulary("translation")),
        "speaker" => Some(SearchSubsection::Vocabulary("speakers")),
        _ => None,
    }
}

const SEARCH_INDEX: &[SettingsSearchHit] = &[
    SettingsSearchHit {
        id: "language",
        title: "Interface language",
        tab: SettingsTab::General,
        keywords: "language locale 中文 语言 界面 internationalization",
    },
    SettingsSearchHit {
        id: "theme",
        title: "Theme and appearance",
        tab: SettingsTab::General,
        keywords: "theme dark light appearance 主题 外观 深色 浅色",
    },
    SettingsSearchHit {
        id: "font",
        title: "Font and typography",
        tab: SettingsTab::General,
        keywords: "font typeface 字体 字号",
    },
    SettingsSearchHit {
        id: "startup",
        title: "Startup and system tray",
        tab: SettingsTab::General,
        keywords: "startup boot tray minimize 开机自启 托盘 最小化",
    },
    SettingsSearchHit {
        id: "updates",
        title: "Update checks and diagnostics",
        tab: SettingsTab::General,
        keywords: "update diagnostics log debug 更新 诊断 日志",
    },
    SettingsSearchHit {
        id: "dashboard",
        title: "Usage overview",
        tab: SettingsTab::Dashboard,
        keywords: "dashboard statistics overview duration tokens 仪表盘 统计 时长 消耗",
    },
    SettingsSearchHit {
        id: "record-shortcut",
        title: "Recording shortcuts",
        tab: SettingsTab::Shortcuts,
        keywords: "shortcut hotkey record pause resume 快捷键 热键 录音 暂停",
    },
    SettingsSearchHit {
        id: "playback-shortcut",
        title: "Playback shortcuts",
        tab: SettingsTab::Shortcuts,
        keywords: "playback seek volume mute 播放 快进 快退 音量 静音",
    },
    SettingsSearchHit {
        id: "editor-shortcut",
        title: "Transcript editor shortcuts",
        tab: SettingsTab::Shortcuts,
        keywords: "editor transcript search find shortcut 编辑器 转录 搜索 查找",
    },
    SettingsSearchHit {
        id: "microphone",
        title: "Microphone and input device",
        tab: SettingsTab::Microphone,
        keywords: "microphone input device audio 麦克风 输入设备 音频",
    },
    SettingsSearchHit {
        id: "asr-model",
        title: "Speech recognition models",
        tab: SettingsTab::Models,
        keywords: "asr speech recognition model batch realtime offline catalog whisper sensevoice 语音识别 模型 批量 实时 离线 模型库",
    },
    SettingsSearchHit {
        id: "asr-live",
        title: "Real-time recognition model",
        tab: SettingsTab::Models,
        keywords: "live realtime streaming recognition model 实时识别 实时转录 在线识别",
    },
    SettingsSearchHit {
        id: "asr-batch",
        title: "Batch recognition model",
        tab: SettingsTab::Models,
        keywords: "batch import file transcription recognition model 批量识别 批量导入 文件转录",
    },
    SettingsSearchHit {
        id: "vad-live",
        title: "Real-time voice activity detection",
        tab: SettingsTab::Models,
        keywords: "vad silero silence detection sensitivity live 静音检测 语音活动检测 灵敏度 实时",
    },
    SettingsSearchHit {
        id: "vad-batch",
        title: "Batch audio segmentation",
        tab: SettingsTab::Models,
        keywords: "batch vad segmentation split silence file 批量vad 批量分段 静音切分 文件分段",
    },
    SettingsSearchHit {
        id: "punctuation-model",
        title: "Punctuation model",
        tab: SettingsTab::Models,
        keywords: "punct punctuation punc model 标点 标点模型 标点符号",
    },
    SettingsSearchHit {
        id: "download-mirror",
        title: "Model download mirror",
        tab: SettingsTab::Models,
        keywords: "mirror huggingface modelscope download proxy 镜像源 魔搭 下载加速 代理",
    },
    SettingsSearchHit {
        id: "gpu",
        title: "GPU acceleration",
        tab: SettingsTab::Models,
        keywords: "gpu cuda acceleration 显卡 加速",
    },
    SettingsSearchHit {
        id: "dictionary",
        title: "Vocabulary and dictionary",
        tab: SettingsTab::Vocabulary,
        keywords: "vocabulary dictionary hotword term yaml weights 词汇 词典 热词 词汇表 自定义词库",
    },
    SettingsSearchHit {
        id: "replacement",
        title: "Text replacements",
        tab: SettingsTab::Vocabulary,
        keywords: "replace replacement correction 替换 修正",
    },
    SettingsSearchHit {
        id: "prompt",
        title: "Polish and summary prompts",
        tab: SettingsTab::Vocabulary,
        keywords: "prompt polish summary template context preset 润色 摘要 提示词 模板 情境预设",
    },
    SettingsSearchHit {
        id: "context-preset",
        title: "Polish context presets",
        tab: SettingsTab::Vocabulary,
        keywords: "context prompt preset style instruction 润色预设 情境预设 提示词 写作风格",
    },
    SettingsSearchHit {
        id: "summary-template",
        title: "Summary templates",
        tab: SettingsTab::Vocabulary,
        keywords: "summary template minutes meeting outline 摘要 总结模板 会议纪要",
    },
    SettingsSearchHit {
        id: "vocabulary-translation",
        title: "Translation automation",
        tab: SettingsTab::Vocabulary,
        keywords: "translation auto translate target language default language 翻译 自动翻译 默认目标语言 目标语言 fanyi",
    },
    SettingsSearchHit {
        id: "speaker",
        title: "Speaker profiles",
        tab: SettingsTab::Vocabulary,
        keywords: "speaker profile voice enrollment sample 说话人 声纹 样本",
    },
    SettingsSearchHit {
        id: "translation",
        title: "Translation language",
        tab: SettingsTab::Subtitle,
        keywords: "translation language subtitles captions target language 翻译 语言 字幕 目标语言",
    },
    SettingsSearchHit {
        id: "subtitle-window",
        title: "Subtitle window and appearance",
        tab: SettingsTab::Subtitle,
        keywords: "subtitle caption overlay float always on top lock font size color opacity background 字幕 悬浮窗 实时字幕 置顶 字号 颜色 透明度 背景色",
    },
    SettingsSearchHit {
        id: "voice-typing",
        title: "Voice typing",
        tab: SettingsTab::Subtitle,
        keywords: "voice typing shortcut dictation speech to text 语音输入 听写 上屏 快捷键",
    },
    SettingsSearchHit {
        id: "voice-typing-mode",
        title: "Voice typing trigger mode",
        tab: SettingsTab::Subtitle,
        keywords: "voice typing hold toggle trigger 按住说话 点击切换 触发模式",
    },
    SettingsSearchHit {
        id: "voice-typing-processing",
        title: "Voice typing processing mode",
        tab: SettingsTab::Subtitle,
        keywords: "voice typing raw polish speed ai 极速听写 智能润色 处理模式 直出",
    },
    SettingsSearchHit {
        id: "voice-typing-context",
        title: "Voice typing context awareness",
        tab: SettingsTab::Subtitle,
        keywords: "voice typing context app preset window rules 情境感知 规则 自适应 窗口联动",
    },
    SettingsSearchHit {
        id: "voice-typing-history",
        title: "Voice typing history",
        tab: SettingsTab::Subtitle,
        keywords: "voice typing history record recall clipboard 历史记录 找回 重输",
    },
    SettingsSearchHit {
        id: "llm-provider",
        title: "Language model providers and API keys",
        tab: SettingsTab::LlmService,
        keywords: "llm provider api key openai model 大语言模型 服务商 密钥",
    },
    SettingsSearchHit {
        id: "automation",
        title: "Automation rules",
        tab: SettingsTab::Automation,
        keywords: "automation rule trigger export 自动化 规则 触发 导出",
    },
    SettingsSearchHit {
        id: "storage",
        title: "Data and model locations",
        tab: SettingsTab::Storage,
        keywords: "storage directory data model cleanup retention 存储 目录 数据 模型 清理 保留",
    },
    SettingsSearchHit {
        id: "sync",
        title: "Cloud sync and encryption",
        tab: SettingsTab::Sync,
        keywords: "sync cloud encryption recovery key device 同步 云端 加密 恢复密钥 设备",
    },
    SettingsSearchHit {
        id: "about",
        title: "Version and updates",
        tab: SettingsTab::About,
        keywords: "about version update release 关于 版本 更新",
    },
    SettingsSearchHit {
        id: "api-server",
        title: "API server and authentication",
        tab: SettingsTab::ApiServer,
        keywords: "api server endpoint authentication token 接口 服务端 鉴权 令牌",
    },
    SettingsSearchHit {
        id: "mcp",
        title: "MCP agents and permissions",
        tab: SettingsTab::Mcp,
        keywords: "mcp agent permission audit log tool 智能体 权限 审计 日志 工具",
    },
];

pub(super) fn search_settings(query: &str) -> Vec<SettingsSearchHit> {
    let query = query.trim().to_lowercase();
    let tokens = query.split_whitespace().collect::<Vec<_>>();
    let mut ranked = SEARCH_INDEX
        .iter()
        .copied()
        .filter_map(|hit| {
            let title = hit.title.to_lowercase();
            let keywords = hit.keywords.to_lowercase();
            let haystack = format!("{title} {keywords}");
            if !tokens.iter().all(|token| haystack.contains(token)) {
                return None;
            }
            let score = if title == query {
                3
            } else if title.starts_with(&query) {
                2
            } else if title.contains(&query) {
                1
            } else {
                0
            };
            Some((score, hit))
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(score, hit)| (std::cmp::Reverse(*score), hit.title));
    ranked.into_iter().map(|(_, hit)| hit).collect()
}

#[cfg(test)]
mod tests {
    use super::{SearchSubsection, search_settings, subsection_for_hit};
    use crate::views::settings::SettingsTab;

    #[test]
    fn search_matches_localized_terms_and_ranks_exact_setting_titles() {
        assert_eq!(search_settings("theme")[0].id, "theme");
        assert!(
            search_settings("麦克风")
                .iter()
                .any(|hit| hit.tab == SettingsTab::Microphone)
        );
        assert!(search_settings("no such setting").is_empty());
    }

    #[test]
    fn search_exposes_react_parity_subsections_and_routes() {
        let cases = [
            ("batch segmentation", "vad-batch", SettingsTab::Models),
            ("real-time recognition", "asr-live", SettingsTab::Models),
            ("subtitle overlay", "subtitle-window", SettingsTab::Subtitle),
            (
                "voice typing context",
                "voice-typing-context",
                SettingsTab::Subtitle,
            ),
            (
                "summary templates",
                "summary-template",
                SettingsTab::Vocabulary,
            ),
            (
                "auto translate default language",
                "vocabulary-translation",
                SettingsTab::Vocabulary,
            ),
        ];

        for (query, expected_id, expected_tab) in cases {
            assert!(
                search_settings(query)
                    .iter()
                    .any(|hit| { hit.id == expected_id && hit.tab == expected_tab }),
                "missing expected settings search hit for {query:?}"
            );
        }
        assert_eq!(
            subsection_for_hit("vocabulary-translation"),
            Some(SearchSubsection::Vocabulary("translation"))
        );
        assert_eq!(
            subsection_for_hit("summary-template"),
            Some(SearchSubsection::Vocabulary("summary"))
        );
    }
}
