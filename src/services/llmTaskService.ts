import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { v4 as uuidv4 } from 'uuid';
import { LlmConfig, SummaryTemplate } from '../types/transcript';

export const LLM_TASK_PROGRESS_EVENT = 'llm-task-progress';
export const LLM_TASK_CHUNK_EVENT = 'llm-task-chunk';

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
  scenarioPrompt?: string;
}

export interface TranslateSegmentsRequest {
  taskId: string;
  config: LlmConfig;
  segments: LlmSegmentInput[];
  chunkSize?: number;
  targetLanguage: string;
}

export interface SummarySegmentInput extends LlmSegmentInput {
  start: number;
  end: number;
  isFinal: boolean;
}

export interface SummarizeTranscriptRequest {
  taskId: string;
  config: LlmConfig;
  template: SummaryTemplate;
  segments: SummarySegmentInput[];
  chunkCharBudget?: number;
}

export interface TranscriptSummaryResult {
  template: SummaryTemplate;
  content: string;
}

export interface LlmTaskProgressPayload {
  taskId: string;
  taskType: LlmTaskType;
  completedChunks: number;
  totalChunks: number;
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

export async function listenToLlmTaskProgress(
  taskId: string,
  taskType: LlmTaskType,
  onProgress: (payload: LlmTaskProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<LlmTaskProgressPayload>(LLM_TASK_PROGRESS_EVENT, ({ payload }) => {
    if (payload.taskId === taskId && payload.taskType === taskType) {
      onProgress(payload);
    }
  });
}

export async function listenToLlmTaskChunks<TPayload extends LlmTaskChunkPayload>(
  taskId: string,
  taskType: TPayload['taskType'],
  onChunk: (payload: TPayload) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen<LlmTaskChunkPayload>(LLM_TASK_CHUNK_EVENT, ({ payload }) => {
    if (payload.taskId === taskId && payload.taskType === taskType) {
      void onChunk(payload as TPayload);
    }
  });
}
