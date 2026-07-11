import { v4 as uuidv4 } from 'uuid';
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

export type LlmTaskType = 'polish' | 'translate' | 'summary';

export interface LlmSegmentInput {
  id: string;
  text: string;
}

export interface PolishedSegment {
  id: string;
  text: string;
}

export interface TranslatedSegment {
  id: string;
  translation: string;
}

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

export interface SummarySegmentInput extends LlmSegmentInput {
  start: number;
  end: number;
  isFinal: boolean;
}

export interface SummarizeTranscriptRequest {
  taskId: string;
  config: LlmConfig;
  template: ResolvedSummaryTemplate;
  segments: SummarySegmentInput[];
  chunkCharBudget?: number;
}

export interface TranscriptSummaryResult {
  templateId: SummaryTemplateId;
  content: string;
}

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

export interface LlmTaskProgressPayload {
  taskId: string;
  taskType: LlmTaskType;
  completedChunks: number;
  totalChunks: number;
}

export interface LlmTaskTextPayload {
  taskId: string;
  taskType: 'summary';
  text: string;
  delta: string;
}

interface LlmTaskChunkPayloadBase<TItems> {
  taskId: string;
  taskType: LlmTaskType;
  chunkIndex: number;
  totalChunks: number;
  items: TItems[];
}

export interface PolishTaskChunkPayload extends LlmTaskChunkPayloadBase<PolishedSegment> {
  taskType: 'polish';
}

export interface TranslateTaskChunkPayload extends LlmTaskChunkPayloadBase<TranslatedSegment> {
  taskType: 'translate';
}

export type LlmTaskChunkPayload = PolishTaskChunkPayload | TranslateTaskChunkPayload;

export function createLlmTaskId(taskType: LlmTaskType): string {
  return `${taskType}-${uuidv4()}`;
}
