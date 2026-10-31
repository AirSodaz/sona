import { create } from 'zustand';
import { useShallow } from 'zustand/shallow';
import { createLlmSettings } from '../services/llm/state';
import {
  DEFAULT_VOLCENGINE_DOUBAO_ASR_CONFIG,
  VOLCENGINE_DOUBAO_PROVIDER_ID,
} from '../services/onlineAsrProviders';
import { DEFAULT_VOICE_TYPING_CONTEXT_RULES } from '../services/voiceTyping/voiceTypingContext';
import type {
  ApiServerConfig,
  AppConfig,
  AudioConfig,
  CaptionConfig,
  EmbyProcessingConfig,
  HistoryStorageConfig,
  LlmAssistantConfig,
  McpConfig,
  ModelConfig,
  ShortcutConfig,
  TranscriptionConfig,
  UIConfig,
  VocabularyConfig,
  VoiceTypingConfig,
} from '../types/config';
import { DEFAULT_EMBY_TOOL_OVERRIDES } from '../types/emby';

// ---------------------------------------------------------------------------
// Default values
// ---------------------------------------------------------------------------
export const CURRENT_CONFIG_VERSION = 8;

export const DEFAULT_CONFIG: AppConfig = {
  // Base
  configVersion: CURRENT_CONFIG_VERSION,
  // UI
  appLanguage: 'auto',
  theme: 'auto',
  font: 'system',
  minimizeToTrayOnExit: true,
  autoStart: false,
  silentStart: false,
  autoCheckUpdates: true,
  enableCloudSync: false,
  logLevel: 'info',

  // Shortcuts
  liveRecordShortcut: 'Ctrl + Space',

  // Audio
  microphoneId: 'default',
  microphoneBoost: undefined, // will default to 1.0 at usage site
  systemAudioDeviceId: 'default',
  muteDuringRecording: false,
  keepMicrophoneActive: false,
  ffmpegEnabled: false,
  ffmpegPath: '',
  // Model
  asr: {
    selections: {
      live: {
        engine: 'local',
        mode: 'streaming',
        modelId: null,
        modelPath: '',
      },
      caption: {
        engine: 'local',
        mode: 'streaming',
        modelId: null,
        modelPath: '',
      },
      voiceTyping: {
        engine: 'local',
        mode: 'streaming',
        modelId: null,
        modelPath: '',
      },
      batch: {
        engine: 'local',
        mode: 'batch',
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
  batchModelPath: '',
  livePunctuationModelPath: '',
  liveVadModelPath: '',
  liveSpeakerSegmentationModelPath: '',
  liveSpeakerEmbeddingModelPath: '',
  liveAlignmentModelPath: '',
  batchPunctuationModelPath: '',
  batchVadModelPath: '',
  batchSpeakerSegmentationModelPath: '',
  batchSpeakerEmbeddingModelPath: '',
  batchAlignmentModelPath: '',
  speakerDiarizationSensitivity: 'balanced',
  modelDownloadMirror: 'auto',
  // Caption
  lockWindow: false,
  alwaysOnTop: true,
  startOnLaunch: false,
  captionWindowWidth: 800,
  captionWindowX: null,
  captionWindowY: null,
  captionFontSize: 22,
  captionFontColor: '#ffffff',
  captionTranslationFontSize: 16,
  captionTranslationFontColor: '#6ee7b7',
  captionBackgroundColor: '#17171a',
  captionBackgroundOpacity: 0.72,
  captionDisplayMode: 'bilingual',
  captionBilingualOrder: 'original_first',
  captionDisplayPriority: 'translation_main',
  captionTextStroke: false,
  captionStrokeWidth: 2,
  captionStrokeColor: '#000000',
  captionTextShadow: true,
  captionTextAlign: 'center',
  captionDisappearDelay: 4.0,
  captionLockShortcut: 'Alt+C',
  realtimeTranslationEnabled: false,
  realtimeTranslationSourceLanguage: 'en',
  realtimeTranslationTargetLanguage: 'zh',
  realtimeTranslationEngine: 'onnx',
  realtimeTranslationModelId: null,

  // Transcription
  language: 'auto',
  batchLanguage: 'auto',
  enableTimeline: false,
  enableITN: true,
  batchVadEnabled: true,
  liveSentenceOnly: false,
  liveVadBufferSize: 5,
  batchVadBufferSize: 5,
  maxConcurrent: 2,
  gpuAcceleration: 'auto',

  // LLM Assistant
  llmSettings: createLlmSettings(),
  summaryEnabled: true,
  autoSummary: false,
  summaryTemplateId: 'general',
  summaryCustomTemplates: [],
  translationLanguage: 'zh',
  autoTranslate: false,
  polishKeywords: '',
  polishPresetId: 'general',
  polishCustomPresets: [],
  autoPolish: false,
  autoPolishFrequency: 5,
  llmRequestTimeoutSeconds: 180,

  // Voice Typing
  voiceTypingEnabled: false,
  voiceTypingShortcut: 'Alt+V',
  voiceTypingTranslateShortcut: 'Alt+Shift+V',
  voiceTypingTargetLanguage: 'en',
  voiceTypingMode: 'hold',
  voiceTypingProcessingMode: 'raw',
  voiceTypingSoundEnabled: true,
  voiceTypingCjkSpacingEnabled: true,
  voiceTypingPolishPrompt: '',
  voiceTypingPlacement: 'caret',
  voiceTypingQuickRecallShortcut: 'Alt+Shift+H',
  voiceTypingContextAwarenessEnabled: true,
  voiceTypingContextPreset: 'auto',
  voiceTypingContextRules: DEFAULT_VOICE_TYPING_CONTEXT_RULES,
  voiceTypingRealtimePreview: true,
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
  dictionaryContent: '',

  // History storage
  historyAudioRetentionDays: null,

  // MCP Agent Control
  mcpEnabled: true,
  mcpPermissionMode: 'full',
  mcpAllowRecording: true,
  mcpAllowTranscriptEdit: true,
  mcpAllowDestructive: false,
  mcpAllowSettingsUpdate: false,
  mcpAllowModelManagement: true,
  mcpNotifyOnMutation: true,
  mcpAuditLogEnabled: true,
  embyProcessingSettings: DEFAULT_EMBY_TOOL_OVERRIDES,
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
  'appLanguage',
  'theme',
  'font',
  'minimizeToTrayOnExit',
  'autoStart',
  'silentStart',
  'autoCheckUpdates',
  'enableCloudSync',
  'logLevel',
  'projectsViewMode',
];

const SHORTCUT_KEYS: (keyof ShortcutConfig)[] = ['liveRecordShortcut'];

const AUDIO_KEYS: (keyof AudioConfig)[] = [
  'microphoneId',
  'microphoneBoost',
  'systemAudioDeviceId',
  'muteDuringRecording',
  'keepMicrophoneActive',
  'ffmpegEnabled',
  'ffmpegPath',
];

const MODEL_KEYS: (keyof ModelConfig)[] = [
  'asr',
  'streamingModelPath',
  'batchModelPath',
  'livePunctuationModelPath',
  'liveVadModelPath',
  'liveSpeakerSegmentationModelPath',
  'liveSpeakerEmbeddingModelPath',
  'liveAlignmentModelPath',
  'batchPunctuationModelPath',
  'batchVadModelPath',
  'batchSpeakerSegmentationModelPath',
  'batchSpeakerEmbeddingModelPath',
  'batchAlignmentModelPath',
  'speakerDiarizationSensitivity',
  'modelDownloadMirror',
];

const CAPTION_KEYS: (keyof CaptionConfig)[] = [
  'lockWindow',
  'alwaysOnTop',
  'startOnLaunch',
  'captionWindowWidth',
  'captionWindowX',
  'captionWindowY',
  'captionFontSize',
  'captionFontColor',
  'captionTranslationFontSize',
  'captionTranslationFontColor',
  'captionBackgroundColor',
  'captionBackgroundOpacity',
  'captionDisplayMode',
  'captionBilingualOrder',
  'realtimeTranslationEnabled',
  'realtimeTranslationSourceLanguage',
  'realtimeTranslationTargetLanguage',
  'realtimeTranslationEngine',
  'realtimeTranslationModelId',
  'captionDisplayPriority',
  'captionTextStroke',
  'captionStrokeWidth',
  'captionStrokeColor',
  'captionTextShadow',
  'captionTextAlign',
  'captionDisappearDelay',
  'captionLockShortcut',
];

const TRANSCRIPTION_KEYS: (keyof TranscriptionConfig)[] = [
  'language',
  'liveLanguage',
  'batchLanguage',
  'enableTimeline',
  'enableITN',
  'batchVadEnabled',
  'liveSentenceOnly',
  'liveVadBufferSize',
  'batchVadBufferSize',
  'maxConcurrent',
  'gpuAcceleration',
];

const LLM_KEYS: (keyof LlmAssistantConfig)[] = [
  'llmSettings',
  'summaryEnabled',
  'autoSummary',
  'translationLanguage',
  'autoTranslate',
  'summaryTemplateId',
  'summaryCustomTemplates',
  'polishKeywords',
  'polishPresetId',
  'polishCustomPresets',
  'autoPolish',
  'autoPolishFrequency',
  'llmRequestTimeoutSeconds',
];
const VOCABULARY_KEYS: (keyof VocabularyConfig)[] = [
  'textReplacementSets',
  'hotwordSets',
  'polishKeywordSets',
  'speakerProfiles',
  'hotwords',
  'dictionaryContent',
];

const VOICE_TYPING_KEYS: (keyof VoiceTypingConfig)[] = [
  'voiceTypingEnabled',
  'voiceTypingShortcut',
  'voiceTypingTranslateShortcut',
  'voiceTypingTargetLanguage',
  'voiceTypingMode',
  'voiceTypingProcessingMode',
  'voiceTypingSoundEnabled',
  'voiceTypingCjkSpacingEnabled',
  'voiceTypingPolishPrompt',
  'voiceTypingPlacement',
  'voiceTypingQuickRecallShortcut',
  'voiceTypingContextAwarenessEnabled',
  'voiceTypingContextPreset',
  'voiceTypingContextRules',
  'voiceTypingRealtimePreview',
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
const HISTORY_STORAGE_KEYS: (keyof HistoryStorageConfig)[] = ['historyAudioRetentionDays'];

const MCP_KEYS: (keyof McpConfig)[] = [
  'mcpEnabled',
  'mcpPermissionMode',
  'mcpAllowRecording',
  'mcpAllowTranscriptEdit',
  'mcpAllowDestructive',
  'mcpAllowSettingsUpdate',
  'mcpAllowModelManagement',
  'mcpNotifyOnMutation',
  'mcpAuditLogEnabled',
];
const EMBY_PROCESSING_KEYS: (keyof EmbyProcessingConfig)[] = ['embyProcessingSettings'];
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

/** Model paths (streaming, batch, VAD, punctuation). */
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

/** MCP Agent Control settings. */
export function useMcpConfig(): McpConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, MCP_KEYS)));
}

/** Emby processing settings overrides. */
export function useEmbyProcessingConfig(): EmbyProcessingConfig {
  return useConfigStore(useShallow((s) => pickConfig(s.config, EMBY_PROCESSING_KEYS)));
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
