use gpui_kit::assets::IconName as LucideIcon;
use sona_core::sync::SyncConflictResolution;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Dashboard,
    Shortcuts,
    Microphone,
    Models,
    Subtitle,
    Vocabulary,
    LlmService,
    Automation,
    Storage,
    Sync,
    About,
    ApiServer,
    Mcp,
}

pub type TabItem = (SettingsTab, &'static str, LucideIcon);
pub type TabGroup = (&'static str, &'static [TabItem]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutTarget {
    LiveRecord,
    VoiceTyping,
    VoiceTypingTranslate,
    VoiceTypingQuickRecall,
}

#[derive(Clone)]
pub enum SyncAction {
    Run,
    Pause(bool),
    Lock,
    Resolve(String, SyncConflictResolution),
}

#[derive(Clone, Debug, PartialEq)]
pub enum OnlineProviderTestState {
    Loading,
    Success { latency_ms: u64 },
    Error { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AboutUpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Installing,
    Downloaded,
    Error(String),
}
