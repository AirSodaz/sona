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
  | 'google_translate'
  | 'google_translate_free'
  | 'open_ai_compatible';

export type LlmProviderStrategy =
  | 'openai_compatible'
  | 'anthropic'
  | 'gemini'
  | 'ollama'
  | 'azure_openai'
  | 'openai_responses'
  | 'openai_compatible_custom_path'
  | 'google_translate'
  | 'google_translate_free'
  | 'perplexity';

export type SummaryTemplate = 'general' | 'meeting' | 'lecture';

export const DEFAULT_SUMMARY_TEMPLATE: SummaryTemplate = 'general';

export interface TranscriptSummaryRecord {
  /** Template used to generate the summary. */
  template: SummaryTemplate;
  /** Read-only summary content. */
  content: string;
  /** ISO timestamp when the summary was generated. */
  generatedAt: string;
  /** Fingerprint of the transcript source used to create the summary. */
  sourceFingerprint: string;
}

export interface TranscriptSummaryState {
  /** Currently selected template in the summary panel. */
  activeTemplate: SummaryTemplate;
  /** Saved summary records keyed by template. */
  records: Partial<Record<SummaryTemplate, TranscriptSummaryRecord>>;
  /** Whether a summary is currently being generated. */
  isGenerating: boolean;
  /** Progress percentage for summary generation. */
  generationProgress: number;
}

export interface HistorySummaryPayload {
  /** Currently selected template for the history item. */
  activeTemplate: SummaryTemplate;
  /** Persisted summary records keyed by template. */
  records: Partial<Record<SummaryTemplate, TranscriptSummaryRecord>>;
}

export type LlmFeature = 'polish' | 'translation' | 'summary';

export interface LlmProviderSetting {
  /** Provider API host or endpoint. */
  apiHost: string;
  /** Provider API key or token. */
  apiKey: string;
  /** Optional provider-specific API path override. */
  apiPath?: string;
  /** Optional provider-specific API version. */
  apiVersion?: string;
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
  /** Selected model entry for summary. */
  summaryModelId?: string;
  /** Temperature override for polish. */
  polishTemperature?: number;
  /** Temperature override for translation. */
  translationTemperature?: number;
  /** Temperature override for summary. */
  summaryTemperature?: number;
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

// AppConfig is now defined as a composite type in ./config.ts.
// Re-exported here for backward compatibility.
export type { AppConfig, UIConfig, AudioConfig, ModelConfig, CaptionConfig, TranscriptionConfig, LlmAssistantConfig } from './config';
