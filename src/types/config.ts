/**
 * Sub-config interfaces for AppConfig.
 *
 * AppConfig is split into domain-specific slices so that consumers
 * can subscribe only to the fields they care about, reducing
 * unnecessary re-renders.
 */

import type { LlmSettings, SummaryCustomTemplate, SummaryTemplateId } from './transcript';
import type { SpeakerProfile } from './speaker';

// ---------------------------------------------------------------------------
// UI preferences
// ---------------------------------------------------------------------------

/** Application-level UI preferences. */
export interface UIConfig {
  /** Application UI language preference. */
  appLanguage: 'auto' | 'en' | 'zh';
  /** Application theme preference. */
  theme?: 'auto' | 'light' | 'dark';
  /** Font preference. */
  font?: 'system' | 'serif' | 'sans' | 'mono' | 'arial' | 'georgia';
  /** Whether to minimize to tray on exit. Default: true. */
  minimizeToTrayOnExit?: boolean;
  /** Whether to automatically check for updates on startup. */
  autoCheckUpdates?: boolean;
  /** Workspace view mode. Default: 'list' */
  projectsViewMode?: 'list' | 'grid' | 'table';
}

// ---------------------------------------------------------------------------
// Shortcuts
// ---------------------------------------------------------------------------

/** Customizable keyboard shortcuts. */
export interface ShortcutConfig {
  /** Shortcut to start/stop live recording. Default: 'Ctrl + Space'. */
  liveRecordShortcut?: string;
}

// ---------------------------------------------------------------------------
// Audio / input devices
// ---------------------------------------------------------------------------

/** Audio input device settings. */
export interface AudioConfig {
  /** ID of the selected microphone device. Default: 'default'. */
  microphoneId?: string;
  /** Microphone boost factor (1.0 to 5.0). Default: 1.0. */
  microphoneBoost?: number;
  /** ID of the selected system audio device. Default: 'default'. */
  systemAudioDeviceId?: string;
  /** Whether to mute system audio during recording. Default: false. */
  muteDuringRecording?: boolean;
}

// ---------------------------------------------------------------------------
// Local model paths and management
// ---------------------------------------------------------------------------

/** Model paths and related management settings. */
export interface ModelConfig {
  /** Path to streaming model (e.g. sherpa-onnx streaming sensevoice). */
  streamingModelPath: string;
  /** Path to offline model (e.g. sherpa-onnx offline sensevoice). */
  offlineModelPath: string;
  /** Path to punctuation model. */
  punctuationModelPath?: string;
  /** Path to VAD model. */
  vadModelPath?: string;
  /** Path to the speaker segmentation model. */
  speakerSegmentationModelPath?: string;
  /** Path to the speaker embedding model. */
  speakerEmbeddingModelPath?: string;
}

// ---------------------------------------------------------------------------
// Caption / subtitle window
// ---------------------------------------------------------------------------

/** Caption (subtitle) overlay window settings. */
export interface CaptionConfig {
  /** Whether the caption window is locked (click-through). Default: false. */
  lockWindow?: boolean;
  /** Whether the caption window is always on top. Default: true. */
  alwaysOnTop?: boolean;
  /** Whether to start captioning on launch. Default: false. */
  startOnLaunch?: boolean;
  /** Width of the caption window in pixels. Default: 800. */
  captionWindowWidth?: number;
  /** Font size of the caption text in pixels. Default: 24. */
  captionFontSize?: number;
  /** Font color of the caption text (HEX). Default: '#ffffff'. */
  captionFontColor?: string;
  /** Background opacity of the caption window (0.0 to 1.0). Default: 0.6. */
  captionBackgroundOpacity?: number;
}

// ---------------------------------------------------------------------------
// Transcription parameters
// ---------------------------------------------------------------------------

/** Transcription engine parameters. */
export interface TranscriptionConfig {
  /** Selected language for transcription. */
  language: string;
  /** Enable Timeline/Subtitle Mode (split by punctuation). */
  enableTimeline?: boolean;
  /** Enable Inverse Text Normalization. */
  enableITN?: boolean;
  /** VAD buffer size in seconds. Default: 5. */
  vadBufferSize?: number;
  /** Max concurrent transcription tasks. Default: 2. */
  maxConcurrent?: number;
}

// ---------------------------------------------------------------------------
// LLM assistant
// ---------------------------------------------------------------------------

export interface PolishCustomPreset {
  /** Stable preset id. */
  id: string;
  /** User-visible preset name. */
  name: string;
  /** Context text sent to the polish prompt. */
  context: string;
}

/** LLM assistant and auto-polish settings. */
export interface LlmAssistantConfig {
  /** LLM provider settings keyed by provider. */
  llmSettings?: LlmSettings;
  /** Whether Summary is available in the transcript editor. Default: true. */
  summaryEnabled?: boolean;
  /** Selected global summary template id. */
  summaryTemplateId?: SummaryTemplateId;
  /** User-defined summary templates. */
  summaryCustomTemplates?: SummaryCustomTemplate[];
  /** Target translation language. Default: 'zh'. */
  translationLanguage?: string;
  /** Keywords for polishing. */
  polishKeywords?: string;
  /** Selected global polish preset id. */
  polishPresetId?: string;
  /** User-defined polish presets. */
  polishCustomPresets?: PolishCustomPreset[];
  /** Deprecated legacy context, retained only for migration. */
  polishContext?: string;
  /** Deprecated legacy scenario, retained only for migration. */
  polishScenario?: string;
  /** Whether to automatically polish the transcript. */
  autoPolish?: boolean;
  /** Frequency of auto-polishing in segments (for live recording). */
  autoPolishFrequency?: number;
}

// ---------------------------------------------------------------------------
// Vocabulary / Text Replacement
// ---------------------------------------------------------------------------

/** A single text replacement rule. */
export interface TextReplacementRule {
  /** Unique ID for the rule. */
  id: string;
  /** Text to find. */
  from: string;
  /** Text to replace with. */
  to: string;
}

/** A collection of text replacement rules with shared settings. */
export interface TextReplacementRuleSet {
  /** Unique ID for the rule set. */
  id: string;
  /** Display name for the rule set. */
  name: string;
  /** Whether the rule set is currently active. */
  enabled: boolean;
  /** Whether to ignore case during matching for all rules in this set. */
  ignoreCase: boolean;
  /** List of rules in this set. */
  rules: TextReplacementRule[];
}

export interface HotwordRule {
  id: string;
  text: string;
}

export interface HotwordRuleSet {
  id: string;
  name: string;
  enabled: boolean;
  rules: HotwordRule[];
}

export interface PolishKeywordRuleSet {
  id: string;
  name: string;
  enabled: boolean;
  keywords: string;
}

export interface LegacyTextReplacementRule {
  id: string;
  from: string;
  to: string;
}

/** Vocabulary and custom dictionary settings. */
export interface VocabularyConfig {
  /** List of text replacement rule sets. */
  textReplacementSets?: TextReplacementRuleSet[];
  /** List of hotword rule sets. */
  hotwordSets?: HotwordRuleSet[];
  /** List of polish keyword rule sets. */
  polishKeywordSets?: PolishKeywordRuleSet[];
  /** Global speaker profiles library. */
  speakerProfiles?: SpeakerProfile[];
  /** Deprecated: Custom hotwords for ASR. Use hotwordSets instead. */
  hotwords?: string[];
  /** Deprecated: use textReplacementSets instead. */
  textReplacements?: LegacyTextReplacementRule[];
}

// ---------------------------------------------------------------------------
// Base
// ---------------------------------------------------------------------------

/** Base application configuration fields. */
export interface BaseConfig {
  /** Schema version for the configuration file. Default: 1. */
  configVersion?: number;
}

// ---------------------------------------------------------------------------
// Voice Typing
// ---------------------------------------------------------------------------

/** Voice Typing (dictation) settings. */
export interface VoiceTypingConfig {
  /** Whether voice typing feature is enabled. Default: false. */
  voiceTypingEnabled?: boolean;
  /** Global shortcut to trigger voice typing. Default: 'Alt+V'. */
  voiceTypingShortcut?: string;
  /** Interaction mode: 'hold' (Push-to-Talk) or 'toggle'. Default: 'hold'. */
  voiceTypingMode?: 'hold' | 'toggle';
}

// ---------------------------------------------------------------------------
// Composite AppConfig
// ---------------------------------------------------------------------------

/**
 * Full application configuration.
 *
 * This is the intersection of all domain-specific config slices.
 * Existing code that references `AppConfig` continues to work unchanged.
 */
export type AppConfig =
  BaseConfig &
  UIConfig &
  ShortcutConfig &
  AudioConfig &
  ModelConfig &
  CaptionConfig &
  TranscriptionConfig &
  VocabularyConfig &
  VoiceTypingConfig &
  LlmAssistantConfig;
