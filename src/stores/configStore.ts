import { create } from 'zustand';
import { useShallow } from 'zustand/shallow';
import { createLlmSettings } from '../services/llmConfig';
import type {
  AppConfig,
  UIConfig,
  ShortcutConfig,
  AudioConfig,
  ModelConfig,
  CaptionConfig,
  TranscriptionConfig,
  VocabularyConfig,
  LlmAssistantConfig,
} from '../types/config';

// ---------------------------------------------------------------------------
// Default values
// ---------------------------------------------------------------------------

export const DEFAULT_CONFIG: AppConfig = {
  // Base
  configVersion: 1,

  // UI
  appLanguage: 'auto',
  theme: 'auto',
  font: 'system',
  minimizeToTrayOnExit: true,
  autoCheckUpdates: true,

  // Shortcuts
  liveRecordShortcut: 'Ctrl + Space',

  // Audio
  microphoneId: 'default',
  microphoneBoost: undefined, // will default to 1.0 at usage site
  systemAudioDeviceId: 'default',
  muteDuringRecording: false,

  // Model
  streamingModelPath: '',
  offlineModelPath: '',
  punctuationModelPath: '',
  vadModelPath: '',

  // Caption
  lockWindow: false,
  alwaysOnTop: true,
  startOnLaunch: false,
  captionWindowWidth: 800,
  captionFontSize: 24,
  captionFontColor: '#ffffff',
  captionBackgroundOpacity: 0.6,

  // Transcription
  language: 'auto',
  enableTimeline: false,
  enableITN: true,
  vadBufferSize: 5,
  maxConcurrent: 2,

  // LLM Assistant
  llmSettings: createLlmSettings(),
  translationLanguage: 'zh',
  polishKeywords: undefined,
  polishContext: undefined,
  polishScenario: undefined,
  autoPolish: false,
  autoPolishFrequency: 5,

  // Vocabulary
  textReplacementSets: [],
  hotwordSets: [],
  hotwords: [],
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

interface ConfigState {
  config: AppConfig;
  setConfig: (patch: Partial<AppConfig>) => void;
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: DEFAULT_CONFIG,
  setConfig: (patch) =>
    set((state) => ({
      config: { ...state.config, ...patch },
    })),
}));

// ---------------------------------------------------------------------------
// Domain-specific selectors
// ---------------------------------------------------------------------------

// Each selector picks only the fields in its domain and uses `shallow`
// equality so components re-render only when their domain changes.

const UI_KEYS: (keyof UIConfig)[] = [
  'appLanguage', 'theme', 'font', 'minimizeToTrayOnExit', 'autoCheckUpdates',
];

const SHORTCUT_KEYS: (keyof ShortcutConfig)[] = [
  'liveRecordShortcut',
];

const AUDIO_KEYS: (keyof AudioConfig)[] = [
  'microphoneId', 'microphoneBoost', 'systemAudioDeviceId', 'muteDuringRecording',
];

const MODEL_KEYS: (keyof ModelConfig)[] = [
  'streamingModelPath', 'offlineModelPath', 'punctuationModelPath', 'vadModelPath',
];

const CAPTION_KEYS: (keyof CaptionConfig)[] = [
  'lockWindow', 'alwaysOnTop', 'startOnLaunch',
  'captionWindowWidth', 'captionFontSize', 'captionFontColor', 'captionBackgroundOpacity',
];

const TRANSCRIPTION_KEYS: (keyof TranscriptionConfig)[] = [
  'language', 'enableTimeline', 'enableITN',
  'vadBufferSize', 'maxConcurrent',
];

const LLM_KEYS: (keyof LlmAssistantConfig)[] = [
  'llmSettings', 'translationLanguage',
  'polishKeywords', 'polishContext', 'polishScenario',
  'autoPolish', 'autoPolishFrequency',
];

const VOCABULARY_KEYS: (keyof VocabularyConfig)[] = [
  'textReplacementSets',
  'hotwordSets',
  'hotwords',
];

/** Pick a subset of keys from the config. */
function pickConfig<K extends keyof AppConfig>(config: AppConfig, keys: K[]): Pick<AppConfig, K> {
  const result = {} as Pick<AppConfig, K>;
  for (const key of keys) {
    result[key] = config[key];
  }
  return result;
}

/** UI preferences (theme, font, language, tray, updates). */
export function useUIConfig(): UIConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, UI_KEYS)));
}

/** Customizable keyboard shortcuts. */
export function useShortcutConfig(): ShortcutConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, SHORTCUT_KEYS)));
}

/** Audio input device settings. */
export function useAudioConfig(): AudioConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, AUDIO_KEYS)));
}

/** Model paths (streaming, offline, VAD, punctuation). */
export function useModelConfig(): ModelConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, MODEL_KEYS)));
}

/** Caption overlay window settings. */
export function useCaptionConfig(): CaptionConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, CAPTION_KEYS)));
}

/** Transcription engine parameters. */
export function useTranscriptionConfig(): TranscriptionConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, TRANSCRIPTION_KEYS)));
}

/** Vocabulary and custom dictionary settings. */
export function useVocabularyConfig(): VocabularyConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, VOCABULARY_KEYS)));
}

/** LLM assistant and auto-polish settings. */
export function useLlmAssistantConfig(): LlmAssistantConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, LLM_KEYS)));
}

// ---------------------------------------------------------------------------
// Convenience: full config (same as before)
// ---------------------------------------------------------------------------

/** Read the full AppConfig. Prefer domain-specific hooks when possible. */
export function useAppConfig(): AppConfig {
  return useConfigStore((s) => s.config);
}

/** Update config (partial merge). */
export function useSetConfig() {
  return useConfigStore((s) => s.setConfig);
}
