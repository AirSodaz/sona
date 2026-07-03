import { create } from 'zustand';
import { useShallow } from 'zustand/shallow';
import { createLlmSettings } from '../services/llm/state';
import {
  DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
} from '../services/onlineAsrProviders';
import type {
  AppConfig,
  UIConfig,
  ShortcutConfig,
  AudioConfig,
  ModelConfig,
  CaptionConfig,
  TranscriptionConfig,
  VocabularyConfig,
  VoiceTypingConfig,
  ApiServerConfig,
  LlmAssistantConfig,
  HistoryStorageConfig,
} from '../types/config';

// ---------------------------------------------------------------------------
// Default values
// ---------------------------------------------------------------------------

export const DEFAULT_CONFIG: AppConfig = {
  // Base
  configVersion: 7,

  // UI
  appLanguage: 'auto',
  theme: 'auto',
  font: 'system',
  minimizeToTrayOnExit: true,
  autoCheckUpdates: true,
  logLevel: 'info',

  // Shortcuts
  liveRecordShortcut: 'Ctrl + Space',

  // Audio
  microphoneId: 'default',
  microphoneBoost: undefined, // will default to 1.0 at usage site
  systemAudioDeviceId: 'default',
  muteDuringRecording: false,
  keepMicrophoneActive: false,

  // Model
  asr: {
    selections: {
      live: {
        engine: 'local-sherpa',
        mode: 'streaming',
        modelId: null,
        modelPath: '',
      },
      caption: {
        engine: 'local-sherpa',
        mode: 'streaming',
        modelId: null,
        modelPath: '',
      },
      voiceTyping: {
        engine: 'local-sherpa',
        mode: 'streaming',
        modelId: null,
        modelPath: '',
      },
      batch: {
        engine: 'local-sherpa',
        mode: 'offline',
        modelId: null,
        modelPath: '',
      },
    },
    providers: {
      online: {
        [VOLCENGINE_DOUBAO_PROVIDER_ID]: {
          ...DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
        },
      },
    },
  },
  streamingModelPath: '',
  offlineModelPath: '',
  punctuationModelPath: '',
  vadModelPath: '',
  speakerSegmentationModelPath: '',
  speakerEmbeddingModelPath: '',
  modelDownloadMirror: 'direct',

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
  batchVadEnabled: true,
  vadBufferSize: 5,
  maxConcurrent: 2,
  gpuAcceleration: 'auto',

  // LLM Assistant
  llmSettings: createLlmSettings(),
  summaryEnabled: true,
  summaryTemplateId: 'general',
  summaryCustomTemplates: [],
  translationLanguage: 'zh',
  polishKeywords: '',
  polishPresetId: 'general',
  polishCustomPresets: [],
  autoPolish: false,
  autoPolishFrequency: 5,
  llmRequestTimeoutSeconds: 180,

  // Voice Typing
  voiceTypingEnabled: false,
  voiceTypingShortcut: 'Alt+V',
  voiceTypingMode: 'hold',

  // API Server
  httpServerEnabled: false,
  httpServerHost: '127.0.0.1',
  httpServerPort: 14200,
  httpServerApiKey: '',
  httpServerMaxConcurrent: 2,
  httpServerMaxQueueSize: 100,
  httpServerMaxUploadSizeMB: 50,
  httpServerJobTtlMinutes: 60,
  httpServerIpWhitelist: 'localhost',

  // Vocabulary
  textReplacementSets: [],
  hotwordSets: [],
  polishKeywordSets: [],
  speakerProfiles: [],
  hotwords: [],

  // History storage
  historyAudioRetentionDays: null,
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
  'appLanguage', 'theme', 'font', 'minimizeToTrayOnExit', 'autoCheckUpdates', 'logLevel', 'projectsViewMode'
];

const SHORTCUT_KEYS: (keyof ShortcutConfig)[] = [
  'liveRecordShortcut',
];

const AUDIO_KEYS: (keyof AudioConfig)[] = [
  'microphoneId', 'microphoneBoost', 'systemAudioDeviceId', 'muteDuringRecording', 'keepMicrophoneActive',
];

const MODEL_KEYS: (keyof ModelConfig)[] = [
  'asr',
  'streamingModelPath',
  'offlineModelPath',
  'punctuationModelPath',
  'vadModelPath',
  'speakerSegmentationModelPath',
  'speakerEmbeddingModelPath',
];

const CAPTION_KEYS: (keyof CaptionConfig)[] = [
  'lockWindow', 'alwaysOnTop', 'startOnLaunch',
  'captionWindowWidth', 'captionFontSize', 'captionFontColor', 'captionBackgroundOpacity',
];

const TRANSCRIPTION_KEYS: (keyof TranscriptionConfig)[] = [
  'language', 'enableTimeline', 'enableITN',
  'batchVadEnabled', 'vadBufferSize', 'maxConcurrent', 'gpuAcceleration',
];

const LLM_KEYS: (keyof LlmAssistantConfig)[] = [
  'llmSettings', 'summaryEnabled', 'translationLanguage',
  'summaryTemplateId', 'summaryCustomTemplates',
  'polishKeywords', 'polishPresetId', 'polishCustomPresets',
  'autoPolish', 'autoPolishFrequency', 'llmRequestTimeoutSeconds',
];

const VOCABULARY_KEYS: (keyof VocabularyConfig)[] = [
  'textReplacementSets',
  'hotwordSets',
  'polishKeywordSets',
  'speakerProfiles',
  'hotwords',
];

const VOICE_TYPING_KEYS: (keyof VoiceTypingConfig)[] = [
  'voiceTypingEnabled', 'voiceTypingShortcut', 'voiceTypingMode'
];
const API_SERVER_KEYS: (keyof ApiServerConfig)[] = [
  'httpServerEnabled',
  'httpServerHost',
  'httpServerPort',
  'httpServerApiKey',
  'httpServerMaxConcurrent',
  'httpServerMaxQueueSize',
  'httpServerMaxUploadSizeMB',
  'httpServerJobTtlMinutes',
  'httpServerIpWhitelist',
  'gpuAcceleration',
];
const HISTORY_STORAGE_KEYS: (keyof HistoryStorageConfig)[] = [
  'historyAudioRetentionDays',
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

/** Voice Typing (dictation) settings. */
export function useVoiceTypingConfig(): VoiceTypingConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, VOICE_TYPING_KEYS)));
}

/** LLM assistant and auto-polish settings. */
export function useLlmAssistantConfig(): LlmAssistantConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, LLM_KEYS)));
}

/** HTTP API Server settings. */
export function useApiServerConfig(): ApiServerConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, API_SERVER_KEYS)));
}

/** History audio retention and storage settings. */
export function useHistoryStorageConfig(): HistoryStorageConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, HISTORY_STORAGE_KEYS)));
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
