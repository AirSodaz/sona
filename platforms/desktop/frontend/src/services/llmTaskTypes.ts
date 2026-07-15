import { v4 as uuidv4 } from 'uuid';
import type {
  LlmSegmentInput as GeneratedLlmSegmentInput,
  LlmTaskChunkPayload as GeneratedLlmTaskChunkPayload,
  LlmTaskProgressPayload as GeneratedLlmTaskProgressPayload,
  LlmTaskTextPayload as GeneratedLlmTaskTextPayload,
  LlmTaskType as GeneratedLlmTaskType,
  PolishedSegment as GeneratedPolishedSegment,
  SummarySegmentInput as GeneratedSummarySegmentInput,
  TranscriptSummaryResult as GeneratedTranscriptSummaryResult,
  TranslatedSegment as GeneratedTranslatedSegment,
} from '../bindings';
import {
  HistorySummaryPayload,
  LlmConfig,
  ResolvedSummaryTemplate,
  SummaryTemplateId,
  TranscriptSegment,
} from '../types/transcript';
import type { HistoryItem } from '../types/history';
import { TauriEvent } from './tauri/events';

export const LLM_TASK_PROGRESS_EVENT = TauriEvent.llm.taskProgress;
export const LLM_TASK_CHUNK_EVENT = TauriEvent.llm.taskChunk;
export const LLM_TASK_TEXT_EVENT = TauriEvent.llm.taskText;
export const LLM_TRANSCRIPT_JOB_UPDATE_EVENT = TauriEvent.llm.transcriptJobUpdate;

export type LlmTaskType = GeneratedLlmTaskType;

export type LlmSegmentInput = GeneratedLlmSegmentInput;

export type PolishedSegment = GeneratedPolishedSegment;

export type TranslatedSegment = GeneratedTranslatedSegment;

export interface PolishSegmentsRequest {
  taskId: string;
  config: LlmConfig;
  segments: LlmSegmentInput[];
  chunkSize?: number;
  context?: string;
  keywords?: string;
}

export interface TranslateSegmentsRequest {
  taskId: string;
  config: LlmConfig;
  segments: LlmSegmentInput[];
  chunkSize?: number;
  targetLanguage: string;
  targetLanguageName?: string; // Add English descriptive name field
}

export type SummarySegmentInput = Omit<GeneratedSummarySegmentInput, 'start' | 'end'> & {
  start: number;
  end: number;
};

export interface SummarizeTranscriptRequest {
  taskId: string;
  config: LlmConfig;
  template: ResolvedSummaryTemplate;
  segments: SummarySegmentInput[];
  chunkCharBudget?: number;
}

export type TranscriptSummaryResult = GeneratedTranscriptSummaryResult & {
  templateId: SummaryTemplateId;
};

interface TranscriptLlmJobRequestBase {
  taskId: string;
  taskType: LlmTaskType;
  jobHistoryId?: string | null;
  config: LlmConfig;
  segments: TranscriptSegment[];
}

export interface TranslateTranscriptLlmJobRequest extends TranscriptLlmJobRequestBase {
  taskType: 'translate';
  targetLanguage: string;
  targetLanguageName?: string; // Add English descriptive name field
}

export interface PolishTranscriptLlmJobRequest extends TranscriptLlmJobRequestBase {
  taskType: 'polish';
  context?: string;
  keywords?: string;
}

export interface SummaryTranscriptLlmJobRequest extends TranscriptLlmJobRequestBase {
  taskType: 'summary';
  template: ResolvedSummaryTemplate;
}

export type TranscriptLlmJobRequest =
  | TranslateTranscriptLlmJobRequest
  | PolishTranscriptLlmJobRequest
  | SummaryTranscriptLlmJobRequest;

export interface TranscriptLlmJobResult {
  taskId: string;
  taskType: LlmTaskType;
  jobHistoryId?: string | null;
  segments?: TranscriptSegment[];
  summary?: HistorySummaryPayload;
  historyItem?: Partial<HistoryItem>;
}

export type LlmTaskProgressPayload = GeneratedLlmTaskProgressPayload;

export type LlmTaskTextPayload = Omit<GeneratedLlmTaskTextPayload, 'taskType' | 'reset'> & {
  taskType: 'summary';
  reset?: boolean;
};

export type PolishTaskChunkPayload = Omit<
  GeneratedLlmTaskChunkPayload<PolishedSegment>,
  'taskType'
> & {
  taskType: 'polish';
};

export type TranslateTaskChunkPayload = Omit<
  GeneratedLlmTaskChunkPayload<TranslatedSegment>,
  'taskType'
> & {
  taskType: 'translate';
};

export type LlmTaskChunkPayload = PolishTaskChunkPayload | TranslateTaskChunkPayload;

export function createLlmTaskId(taskType: LlmTaskType): string {
  return `${taskType}-${uuidv4()}`;
}
