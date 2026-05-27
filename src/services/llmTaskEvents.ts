import { listen, type UnlistenFn } from './tauri/platform/events';
import { LLM_TASK_PROGRESS_EVENT, LLM_TASK_CHUNK_EVENT, LLM_TASK_TEXT_EVENT, LLM_TRANSCRIPT_JOB_UPDATE_EVENT, type LlmTaskType, type LlmTaskProgressPayload, type LlmTaskChunkPayload, type LlmTaskTextPayload, type TranscriptLlmJobResult } from './llmTaskTypes';

export interface LlmTaskEventsPorts {
  listen: typeof listen;
}

export class LlmTaskEvents {
  constructor(private readonly ports: LlmTaskEventsPorts) {}

  listenToLlmTaskProgress = async (
    taskId: string,
    taskType: LlmTaskType,
    onProgress: (payload: LlmTaskProgressPayload) => void,
  ): Promise<UnlistenFn> => {
    return this.ports.listen<LlmTaskProgressPayload>(LLM_TASK_PROGRESS_EVENT, ({ payload }) => {
      if (payload.taskId === taskId && payload.taskType === taskType) {
        onProgress(payload);
      }
    });
  }

  listenToLlmTaskChunks = async <TPayload extends LlmTaskChunkPayload>(
    taskId: string,
    taskType: TPayload['taskType'],
    onChunk: (payload: TPayload) => void | Promise<void>,
  ): Promise<UnlistenFn> => {
    return this.ports.listen<LlmTaskChunkPayload>(LLM_TASK_CHUNK_EVENT, ({ payload }) => {
      if (payload.taskId === taskId && payload.taskType === taskType) {
        void onChunk(payload as TPayload);
      }
    });
  }

  listenToLlmTaskText = async (
    taskId: string,
    taskType: LlmTaskTextPayload['taskType'],
    onText: (payload: LlmTaskTextPayload) => void | Promise<void>,
  ): Promise<UnlistenFn> => {
    return this.ports.listen<LlmTaskTextPayload>(LLM_TASK_TEXT_EVENT, ({ payload }) => {
      if (payload.taskId === taskId && payload.taskType === taskType) {
        void onText(payload);
      }
    });
  }

  listenToTranscriptLlmJobUpdates = async (
    taskId: string,
    taskType: LlmTaskType,
    onUpdate: (payload: TranscriptLlmJobResult) => void | Promise<void>,
  ): Promise<UnlistenFn> => {
    return this.ports.listen<TranscriptLlmJobResult>(LLM_TRANSCRIPT_JOB_UPDATE_EVENT, ({ payload }) => {
      if (payload.taskId === taskId && payload.taskType === taskType) {
        void onUpdate(payload);
      }
    });
  }
}

export function createLlmTaskEvents(ports: LlmTaskEventsPorts): LlmTaskEvents {
  return new LlmTaskEvents(ports);
}

export const llmTaskEvents = createLlmTaskEvents({
  listen,
});

export const {
  listenToLlmTaskProgress,
  listenToLlmTaskChunks,
  listenToLlmTaskText,
  listenToTranscriptLlmJobUpdates,
} = llmTaskEvents;
