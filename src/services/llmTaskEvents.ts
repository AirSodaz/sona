import { listen, type UnlistenFn } from './tauri/platform/events';
import { LLM_TASK_PROGRESS_EVENT, LLM_TASK_CHUNK_EVENT, LLM_TASK_TEXT_EVENT, LLM_TRANSCRIPT_JOB_UPDATE_EVENT, type LlmTaskType, type LlmTaskProgressPayload, type LlmTaskChunkPayload, type LlmTaskTextPayload, type TranscriptLlmJobResult } from './llmTaskTypes';;

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

export async function listenToLlmTaskText(
  taskId: string,
  taskType: LlmTaskTextPayload['taskType'],
  onText: (payload: LlmTaskTextPayload) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen<LlmTaskTextPayload>(LLM_TASK_TEXT_EVENT, ({ payload }) => {
    if (payload.taskId === taskId && payload.taskType === taskType) {
      void onText(payload);
    }
  });
}

export async function listenToTranscriptLlmJobUpdates(
  taskId: string,
  taskType: LlmTaskType,
  onUpdate: (payload: TranscriptLlmJobResult) => void | Promise<void>,
): Promise<UnlistenFn> {
  return listen<TranscriptLlmJobResult>(LLM_TRANSCRIPT_JOB_UPDATE_EVENT, ({ payload }) => {
    if (payload.taskId === taskId && payload.taskType === taskType) {
      void onUpdate(payload);
    }
  });
}
