/**
 * Core data structure for transcript segments.
 * This is the "source of truth" for all transcription data.
 */
import type {
  HistorySummaryPayload_Serialize as CoreHistorySummaryPayload,
  TranscriptSummaryRecordPayload,
} from '../bindings';
import type { SummaryTemplateId as TranscriptSummaryTemplateId } from './llm';
import type { SpeakerAttribution, SpeakerTag } from './speaker';

export type TranscriptTimingLevel = 'token' | 'segment';
export type TranscriptTimingSource = 'model' | 'derived';

export interface TranscriptTimingUnit {
  text: string;
  start: number;
  end: number;
}

export interface TranscriptTiming {
  level: TranscriptTimingLevel;
  source: TranscriptTimingSource;
  units: TranscriptTimingUnit[];
}

export interface TranscriptSegment {
  id: string;
  start: number;
  end: number;
  text: string;
  isFinal: boolean;
  timing?: TranscriptTiming;
  tokens?: string[];
  timestamps?: number[];
  durations?: number[];
  translation?: string;
  speaker?: SpeakerTag;
  speakerAttribution?: SpeakerAttribution;
}

export interface TranscriptUpdate {
  removeIds: string[];
  upsertSegments: TranscriptSegment[];
}

/**
 * Application operation mode.
 */
export type AppMode = 'live' | 'batch' | 'projects';

/**
 * Processing status for batch imports.
 */
export type ProcessingStatus = 'idle' | 'loading' | 'processing' | 'complete' | 'error';

export type {
  BuiltInLlmProvider,
  CustomLlmProvider,
  CustomLlmProviderId,
  CustomLlmProviderStrategy,
  LlmCapabilityPolicy,
  LlmCompletionOptions,
  LlmCompletionRequest,
  LlmCompletionResponse,
  LlmConfig,
  LlmExecutionMetadata,
  LlmFeature,
  LlmFeatureSelections,
  LlmGenerateSource,
  LlmJsonValue,
  LlmDiscoveredModelSummary,
  LlmModelDiscoveryStatus,
  LlmModelEntry,
  LlmModelMetadata,
  LlmModelMetadataSource,
  LlmModelSource,
  LlmModality,
  LlmPromptCachePolicy,
  LlmProvider,
  LlmProviderSetting,
  LlmProviderStrategy,
  LlmResponseFormat,
  LlmResponseFormatKind,
  LlmSettings,
  LlmTokenUsage,
  ResolvedSummaryTemplate,
  SummaryCustomTemplate,
  SummaryTemplate,
  SummaryTemplateId,
} from './llm';
export { DEFAULT_SUMMARY_TEMPLATE, DEFAULT_SUMMARY_TEMPLATE_ID } from './llm';

export type TranscriptSummaryRecord = TranscriptSummaryRecordPayload;

export interface TranscriptSummaryState {
  /** Currently selected template in the summary panel. */
  activeTemplateId: TranscriptSummaryTemplateId;
  /** Saved summary record. */
  record?: TranscriptSummaryRecord;
  /** Temporary streamed summary content that is not persisted automatically. */
  streamingContent?: string;
  /** Whether a summary is currently being generated. */
  isGenerating: boolean;
  /** Progress percentage for summary generation. */
  generationProgress: number;
}

export type HistorySummaryPayload = CoreHistorySummaryPayload;

// AppConfig is now defined as a composite type in ./config.ts.
// Re-exported here for backward compatibility.
export type { AppConfig, UIConfig, AudioConfig, ModelConfig, CaptionConfig, TranscriptionConfig, LlmAssistantConfig } from './config';
