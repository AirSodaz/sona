/**
 * Core data structure for transcript segments.
 * This is the "source of truth" for all transcription data.
 */
import type { SummaryTemplateId as TranscriptSummaryTemplateId } from './llm';
import type { SpeakerAttribution, SpeakerTag } from './speaker';

export type TranscriptTimingLevel = 'token' | 'segment';
export type TranscriptTimingSource = 'model' | 'derived';

export interface TranscriptTimingUnit {
  /** Text rendered for this timing unit. */
  text: string;
  /** Start time in seconds. */
  start: number;
  /** End time in seconds. */
  end: number;
}

export interface TranscriptTiming {
  /** Whether this timing data is token-level or only segment-level. */
  level: TranscriptTimingLevel;
  /** Whether the timing came directly from the model or was derived later. */
  source: TranscriptTimingSource;
  /** Ordered timing units used for rendering and seek interactions. */
  units: TranscriptTimingUnit[];
}

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
  /** Normalized timing metadata for editor seek/highlight behavior. */
  timing?: TranscriptTiming;
  /** Legacy raw token list kept for compatibility and lazy upgrades. */
  tokens?: string[];
  /** Legacy token start times kept for compatibility and lazy upgrades. */
  timestamps?: number[];
  /** Legacy token durations kept for compatibility and lazy upgrades. */
  durations?: number[];
  /** Translated text content. */
  translation?: string;
  /** Optional speaker metadata kept separate from transcript text. */
  speaker?: SpeakerTag;
  /** Optional speaker attribution metadata used for correction and review flows. */
  speakerAttribution?: SpeakerAttribution;
}

export interface TranscriptUpdate {
  /** Segment IDs that should be removed before upserts are applied. */
  removeIds: string[];
  /** Segments to insert or replace after removals. */
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
  LlmConfig,
  LlmFeature,
  LlmFeatureSelections,
  LlmModelEntry,
  LlmProvider,
  LlmProviderSetting,
  LlmProviderStrategy,
  LlmSettings,
  ResolvedSummaryTemplate,
  SummaryCustomTemplate,
  SummaryTemplate,
  SummaryTemplateId,
} from './llm';
export { DEFAULT_SUMMARY_TEMPLATE, DEFAULT_SUMMARY_TEMPLATE_ID } from './llm';

export interface TranscriptSummaryRecord {
  /** Template used when the summary was last generated or saved. */
  templateId: TranscriptSummaryTemplateId;
  /** Summary content. */
  content: string;
  /** ISO timestamp when the summary was generated. */
  generatedAt: string;
  /** Fingerprint of the transcript source used to create the summary. */
  sourceFingerprint: string;
}

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

export interface HistorySummaryPayload {
  /** Currently selected template for the history item. */
  activeTemplateId: TranscriptSummaryTemplateId;
  /** Persisted summary record. */
  record?: TranscriptSummaryRecord;
}

// AppConfig is now defined as a composite type in ./config.ts.
// Re-exported here for backward compatibility.
export type { AppConfig, UIConfig, AudioConfig, ModelConfig, CaptionConfig, TranscriptionConfig, LlmAssistantConfig } from './config';
