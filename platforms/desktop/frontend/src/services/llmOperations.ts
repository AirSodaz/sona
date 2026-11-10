import type { LlmGenerateCommandRequest } from '../types/dashboard';
import type {
  PolishedSegment,
  PolishSegmentsRequest,
  TranscriptLlmJobRequest,
  TranscriptLlmJobResult,
  TranslatedSegment,
  TranslateSegmentsRequest,
} from '../types/llmTask';
import type { LlmConfig } from '../types/transcript';
import { type LlmJobPriority, llmJobScheduler } from './llm/llmJobScheduler';
import {
  generateLlmText as rawGenerateLlmText,
  polishTranscriptSegments as rawPolishTranscriptSegments,
  runTranscriptLlmJob as rawRunTranscriptLlmJob,
  translateTranscriptSegments as rawTranslateTranscriptSegments,
} from './tauri/llm';

export interface LlmExecutionScheduleOptions {
  priority?: LlmJobPriority;
  taskId?: string;
  cancelCheck?: () => boolean;
  signal?: AbortSignal;
  onStart?: () => void | Promise<void>;
  onCancelledWhileQueued?: () => void;
}

function scheduleLlmOperation<T>(
  config: Pick<LlmConfig, 'provider' | 'strategy' | 'model'>,
  defaultPriority: LlmJobPriority,
  options: LlmExecutionScheduleOptions | undefined,
  taskId: string | undefined,
  runner: () => Promise<T>
): Promise<T> {
  return llmJobScheduler.schedule(
    {
      config,
      priority: options?.priority ?? defaultPriority,
      taskId: options?.taskId ?? taskId,
      cancelCheck: options?.cancelCheck,
      signal: options?.signal,
      onStart: options?.onStart,
      onCancelledWhileQueued: options?.onCancelledWhileQueued,
    },
    runner
  );
}

export function runTranscriptLlmJob(
  request: TranscriptLlmJobRequest,
  options?: LlmExecutionScheduleOptions
): Promise<TranscriptLlmJobResult> {
  return scheduleLlmOperation(request.config, 'normal', options, request.taskId, () =>
    rawRunTranscriptLlmJob(request)
  );
}

export function polishTranscriptSegments(
  request: PolishSegmentsRequest,
  options?: LlmExecutionScheduleOptions
): Promise<PolishedSegment[]> {
  return scheduleLlmOperation(request.config, 'normal', options, request.taskId, () =>
    rawPolishTranscriptSegments(request)
  );
}

export function translateTranscriptSegments(
  request: TranslateSegmentsRequest,
  options?: LlmExecutionScheduleOptions
): Promise<TranslatedSegment[]> {
  return scheduleLlmOperation(request.config, 'normal', options, request.taskId, () =>
    rawTranslateTranscriptSegments(request)
  );
}

export function generateLlmText(
  request: LlmGenerateCommandRequest,
  options?: LlmExecutionScheduleOptions
): Promise<string> {
  return scheduleLlmOperation(request.config, 'high', options, undefined, () =>
    rawGenerateLlmText(request)
  );
}
