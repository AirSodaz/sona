import { v4 as uuidv4 } from 'uuid';
import type { LlmTaskType } from '../types/llmTask';
import { TauriEvent } from './tauri/events';

export type {
  LlmSegmentInput,
  LlmTaskChunkPayload,
  LlmTaskProgressPayload,
  LlmTaskTextPayload,
  LlmTaskType,
  PolishSegmentsRequest,
  PolishedSegment,
  PolishTaskChunkPayload,
  PolishTranscriptLlmJobRequest,
  SummarizeTranscriptRequest,
  SummarySegmentInput,
  SummaryTranscriptLlmJobRequest,
  TranscriptLlmJobRequest,
  TranscriptLlmJobResult,
  TranscriptSummaryResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
  TranslateTaskChunkPayload,
  TranslateTranscriptLlmJobRequest,
} from '../types/llmTask';

export const LLM_TASK_PROGRESS_EVENT = TauriEvent.llm.taskProgress;
export const LLM_TASK_CHUNK_EVENT = TauriEvent.llm.taskChunk;
export const LLM_TASK_TEXT_EVENT = TauriEvent.llm.taskText;
export const LLM_TRANSCRIPT_JOB_UPDATE_EVENT = TauriEvent.llm.transcriptJobUpdate;

export function createLlmTaskId(taskType: LlmTaskType): string {
  return `${taskType}-${uuidv4()}`;
}
