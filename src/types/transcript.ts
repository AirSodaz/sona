/**
 * Core data structure for transcript segments.
 * This is the "source of truth" for all transcription data.
 */
export interface TranscriptSegment {
  /** Unique identifier (UUID). */
  id: string;
  /** Start time in seconds. */
  start: number;
  /** End time in seconds. */
  end: number;
  /** The transcribed text content. */
  text: string;
  /** True if the segment is finalized (not a partial/in-progress result). */
  isFinal: boolean;
  /** List of tokens in the segment. */
  tokens?: string[];
  /** Start time of each token. */
  timestamps?: number[];
  /** Duration of each token. */
  durations?: number[];
  /** Translated text content. */
  translation?: string;
}

/**
 * Application operation mode.
 */
export type AppMode = 'live' | 'batch' | 'history';

/**
 * Processing status for batch imports.
 */
export type ProcessingStatus = 'idle' | 'loading' | 'processing' | 'complete' | 'error';

export type LlmProvider =
  | 'open_ai'
  | 'open_ai_responses'
  | 'azure_openai'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'deep_seek'
  | 'kimi'
  | 'silicon_flow'
  | 'qwen'
  | 'qwen_portal'
  | 'minimax_global'
  | 'minimax_cn'
  | 'openrouter'
  | 'lm_studio'
  | 'groq'
  | 'x_ai'
  | 'mistral_ai'
  | 'perplexity'
  | 'volcengine'
  | 'chatglm'
  | 'open_ai_compatible';

export type LlmProviderStrategy =
  | 'openai_compatible'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'azure_openai'
  | 'openai_responses'
  | 'openai_compatible_custom_path'
  | 'perplexity';

export type LlmFeature = 'polish' | 'translation';

export interface LlmProviderSetting {
  /** Provider API host or endpoint. */
  apiHost: string;
  /** Provider API key or token. */
  apiKey: string;
  /** Optional provider-specific API path override. */
  apiPath?: string;
  /** Optional provider-specific API version. */
  apiVersion?: string;
  /** Temperature (0.0 to 2.0). */
  temperature?: number;
}

export interface LlmModelEntry {
  /** Stable identifier for the configured model entry. */
  id: string;
  /** Provider used for this model entry. */
  provider: LlmProvider;
  /** Model name or deployment name. */
  model: string;
}

export interface LlmFeatureSelections {
  /** Selected model entry for polish. */
  polishModelId?: string;
  /** Selected model entry for translation. */
  translationModelId?: string;
  /** Temperature override for polish. */
  polishTemperature?: number;
  /** Temperature override for translation. */
  translationTemperature?: number;
}

export interface LlmSettings {
  /** Currently active provider. */
  activeProvider: LlmProvider;
  /** Per-provider saved settings. */
  providers: Partial<Record<LlmProvider, LlmProviderSetting>>;
  /** Added models keyed by ID. */
  models: Record<string, LlmModelEntry>;
  /** Ordered list of added model IDs. */
  modelOrder: string[];
  /** Feature-specific model selections. */
  selections: LlmFeatureSelections;
}

export interface LlmConfig {
  /** LLM provider kind. */
  provider: LlmProvider;
  /** LLM base URL. */
  baseUrl: string;
  /** LLM API key. */
  apiKey: string;
  /** LLM model name. */
  model: string;
  /** Optional provider-specific path. */
  apiPath?: string;
  /** Optional provider-specific API version. */
  apiVersion?: string;
  /** LLM temperature (0.0 to 2.0). */
  temperature?: number;
}

/**
 * Configuration for the application.
 */
export interface AppConfig {
  /** Path to streaming model (e.g. sherpa-onnx streaming sensevoice). */
  streamingModelPath: string;
  /** Path to offline model (e.g. sherpa-onnx offline sensevoice). */
  offlineModelPath: string;
  /** Selected language for transcription. */
  language: string;
  /** Application UI language preference. */
  appLanguage: 'auto' | 'en' | 'zh';
  /** IDs of enabled ITN models. */
  enabledITNModels?: string[];
  /** Order of ITN models (IDs) for sorting. */
  itnRulesOrder?: string[];
  /** Enable Inverse Text Normalization. */
  enableITN?: boolean;
  /** Enable Timeline/Subtitle Mode (split by punctuation). */
  enableTimeline?: boolean;
  /** Path to punctuation model. */
  punctuationModelPath?: string;
  /** Application theme preference. */
  theme?: 'auto' | 'light' | 'dark';
  /** Font preference. */
  font?: 'system' | 'serif' | 'sans' | 'mono' | 'arial' | 'georgia';
  /** Path to VAD model. */
  vadModelPath?: string;
  /** VAD buffer size in seconds. Default: 5. */
  vadBufferSize?: number;
  /** Max concurrent transcription tasks. Default: 2. */
  maxConcurrent?: number;
  /** Whether to minimize to tray on exit. Default: true. */
  minimizeToTrayOnExit?: boolean;
  /** Whether the caption window is locked (click-through). Default: false. */
  lockWindow?: boolean;
  /** Whether the caption window is always on top. Default: true. */
  alwaysOnTop?: boolean;
  /** ID of the selected microphone device. Default: 'default'. */
  microphoneId?: string;
  /** Microphone boost factor (1.0 to 5.0). Default: 1.0. */
  microphoneBoost?: number;
  /** ID of the selected system audio device. Default: 'default'. */
  systemAudioDeviceId?: string;
  /** Whether to mute system audio during recording. Default: false. */
  muteDuringRecording?: boolean;
  /** Whether to start captioning on launch. Default: false. */
  startOnLaunch?: boolean;
  /** Width of the caption window in pixels. Default: 800. */
  captionWindowWidth?: number;
  /** Font size of the caption text in pixels. Default: 24. */
  captionFontSize?: number;
  /** Font color of the caption text (HEX). Default: '#ffffff'. */
  captionFontColor?: string;
  /** LLM provider settings keyed by provider. */
  llmSettings?: LlmSettings;
  /** Target translation language. Default: 'zh'. */
  translationLanguage?: string;
  /** Keywords for polishing. */
  polishKeywords?: string;
  /** Context for polishing. */
  polishContext?: string;
  /** Scenario preset for polishing. */
  polishScenario?: string;
  /** Whether to automatically polish the transcript. */
  autoPolish?: boolean;
  /** Frequency of auto-polishing in segments (for live recording). */
  autoPolishFrequency?: number;
  /** Whether to automatically check for updates on startup. */
  autoCheckUpdates?: boolean;
}
