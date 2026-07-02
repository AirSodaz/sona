import type { AppConfig } from '../../types/config';
import type { LlmConfig, TranscriptSegment } from '../../types/transcript';
import { normalizeError } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { createLlmTaskId, type LlmTaskType, type LlmTaskTextPayload, type PolishedSegment, type PolishSegmentsRequest, type TranslatedSegment, type TranslateSegmentsRequest } from '../llmTaskTypes';
import { listenToLlmTaskChunks, listenToLlmTaskProgress, listenToLlmTaskText } from '../llmTaskEvents';
import { getFeatureLlmConfig, isLlmConfigComplete } from './configUtils';
import {
  polishTranscriptSegments,
  translateTranscriptSegments,
} from '../tauri/llm';
import {
  buildLlmTaskLedgerRecord,
  createLlmTaskLedgerId,
  isTaskLedgerCancelRequested,
  patchTaskLedgerRecord,
  upsertTaskLedgerRecord,
} from '../taskLedgerBuilders';

type SegmentTaskFeature = 'translation' | 'polish';
type SegmentTaskType = Extract<LlmTaskType, 'translate' | 'polish'>;
type SegmentTaskItem = PolishedSegment | TranslatedSegment;
type SegmentTaskRequest = TranslateSegmentsRequest | PolishSegmentsRequest;
type TranscriptLlmTaskType = LlmTaskType;

interface RunConfiguredSegmentTaskOptions<
  TConfig extends Pick<AppConfig, 'llmSettings'>,
  TItem extends SegmentTaskItem,
  TRequest extends SegmentTaskRequest,
> {
  feature: SegmentTaskFeature;
  taskType: SegmentTaskType;
  config: TConfig;
  segments: TranscriptSegment[];
  taskIdOverride?: string;
  onChunk?: (items: TItem[]) => void | Promise<void>;
  buildRequest: (params: {
    taskId: string;
    llmConfig: LlmConfig;
    config: TConfig;
    segments: TranscriptSegment[];
  }) => TRequest;
}

interface RunTranscriptSegmentTaskJobOptions<TTaskType extends SegmentTaskType> {
  taskType: TTaskType;
  segments: TranscriptSegment[];
  sourceHistoryId: string | null;
  targetLanguage?: string;
  runTask: (taskId: string, jobHistoryId: string) => Promise<void>;
  onStart?: (jobHistoryId: string) => void | Promise<void>;
  onProgress?: (progress: number, jobHistoryId: string) => void | Promise<void>;
  onSuccess?: (jobHistoryId: string) => void | Promise<void>;
  onError?: (jobHistoryId: string, error: unknown) => void | Promise<void>;
  onFinally?: (jobHistoryId: string) => void | Promise<void>;
}

interface RunTranscriptLlmTaskJobOptions<TTaskType extends TranscriptLlmTaskType> {
  taskType: TTaskType;
  segments: TranscriptSegment[];
  sourceHistoryId: string | null;
  targetLanguage?: string;
  templateId?: string;
  runTask: (taskId: string, jobHistoryId: string) => Promise<void>;
  onStart?: (jobHistoryId: string) => void | Promise<void>;
  onProgress?: (progress: number, jobHistoryId: string) => void | Promise<void>;
  onText?: TTaskType extends 'summary'
    ? (payload: LlmTaskTextPayload, jobHistoryId: string) => void | Promise<void>
    : never;
  onSuccess?: (jobHistoryId: string) => void | Promise<void>;
  onError?: (jobHistoryId: string, error: unknown) => void | Promise<void>;
  onFinally?: (jobHistoryId: string) => void | Promise<void>;
}

interface ApplySegmentItemsToTranscriptJobOptions<TItem> {
  jobHistoryId: string;
  items: TItem[];
  logLabel: string;
  getCurrentHistoryId: () => string;
  applyToCurrentTranscript: (items: TItem[]) => void | Promise<void>;
  loadTranscript: (historyId: string) => Promise<TranscriptSegment[] | null>;
  updateTranscript: (historyId: string, segments: TranscriptSegment[]) => Promise<void>;
  mergeIntoSegments: (segments: TranscriptSegment[], items: TItem[]) => TranscriptSegment[];
}

function calculateProgressPercent(completedChunks: number, totalChunks: number): number {
  return Math.round((completedChunks / Math.max(totalChunks, 1)) * 100);
}

function patchCancelledLlmTask(ledgerId: string): void {
  patchTaskLedgerRecord(ledgerId, {
    status: 'cancelled',
    progress: 0,
    cancelable: false,
    retryable: false,
  });
}

export async function runConfiguredSegmentTask<
  TConfig extends Pick<AppConfig, 'llmSettings'>,
  TItem extends SegmentTaskItem,
  TRequest extends SegmentTaskRequest,
>({
  feature,
  taskType,
  config,
  segments,
  taskIdOverride,
  onChunk,
  buildRequest,
}: RunConfiguredSegmentTaskOptions<TConfig, TItem, TRequest>): Promise<TItem[]> {
  const llmConfig = getFeatureLlmConfig(config, feature);

  if (!llmConfig || !isLlmConfigComplete(llmConfig)) {
    throw new Error('LLM Service not fully configured.');
  }

  const resolvedLlmConfig = llmConfig;

  if (!segments || segments.length === 0) {
    return [];
  }

  const taskId = taskIdOverride || createLlmTaskId(taskType);
  let receivedChunkEvent = false;
  const unlistenChunk = onChunk
    ? await listenToLlmTaskChunks(taskId, taskType, async (payload) => {
      receivedChunkEvent = true;
      await onChunk(payload.items as TItem[]);
    })
    : () => undefined;

  try {
    const request = buildRequest({
      taskId,
      llmConfig: resolvedLlmConfig,
      config,
      segments,
    });
    const items = (
      taskType === 'translate'
        ? await translateTranscriptSegments(request as TranslateSegmentsRequest)
        : await polishTranscriptSegments(request as PolishSegmentsRequest)
    ) as TItem[];

    // Buffered providers still need to reuse the same consumer path so callers
    // do not branch on whether streamed chunk events arrived.
    if (onChunk && !receivedChunkEvent) {
      await onChunk(items);
    }

    return items;
  } catch (error) {
    throw Object.assign(new Error(normalizeError(error).message), { cause: error });
  } finally {
    unlistenChunk();
  }
}

export async function runTranscriptLlmTaskJob<TTaskType extends TranscriptLlmTaskType>({
  taskType,
  segments,
  sourceHistoryId,
  targetLanguage,
  templateId,
  runTask,
  onStart,
  onProgress,
  onText,
  onSuccess,
  onError,
  onFinally,
}: RunTranscriptLlmTaskJobOptions<TTaskType>): Promise<void> {
  if (!segments || segments.length === 0) {
    return;
  }

  const jobHistoryId = sourceHistoryId || 'current';
  const taskId = createLlmTaskId(taskType);
  const ledgerId = createLlmTaskLedgerId(taskId);
  let unlistenProgress: () => void = () => undefined;
  let unlistenText: () => void = () => undefined;

  try {
    upsertTaskLedgerRecord(buildLlmTaskLedgerRecord({
      taskId,
      taskType,
      jobHistoryId,
      targetLanguage,
      templateId,
    }));
    await onStart?.(jobHistoryId);
    unlistenProgress = await listenToLlmTaskProgress(
      taskId,
      taskType,
      ({ completedChunks, totalChunks }) => {
        if (isTaskLedgerCancelRequested(ledgerId)) {
          return;
        }

        const progress = calculateProgressPercent(completedChunks, totalChunks);
        patchTaskLedgerRecord(ledgerId, {
          status: 'running',
          progress,
        });
        if (onProgress) {
          void onProgress(progress, jobHistoryId);
        }
      },
    );
    if (taskType === 'summary' && onText) {
      unlistenText = await listenToLlmTaskText(
        taskId,
        'summary',
        (payload) => {
          if (isTaskLedgerCancelRequested(ledgerId)) {
            return;
          }

          void (onText as (payload: LlmTaskTextPayload, jobHistoryId: string) => void | Promise<void>)(
            payload,
            jobHistoryId,
          );
        },
      );
    }
    await runTask(taskId, jobHistoryId);

    if (isTaskLedgerCancelRequested(ledgerId)) {
      patchCancelledLlmTask(ledgerId);
      return;
    }

    await onSuccess?.(jobHistoryId);
    patchTaskLedgerRecord(ledgerId, {
      status: 'succeeded',
      progress: 100,
      cancelable: false,
      retryable: false,
    });
  } catch (error) {
    if (isTaskLedgerCancelRequested(ledgerId)) {
      patchCancelledLlmTask(ledgerId);
    } else {
      await onError?.(jobHistoryId, error);
      patchTaskLedgerRecord(ledgerId, {
        status: 'failed',
        progress: 0,
        cancelable: false,
        retryable: true,
        errorMessage: normalizeError(error).message,
      });
    }
    throw Object.assign(new Error(normalizeError(error).message), { cause: error });
  } finally {
    unlistenProgress();
    unlistenText();
    await onFinally?.(jobHistoryId);
  }
}

export async function runTranscriptSegmentTaskJob<TTaskType extends SegmentTaskType>({
  taskType,
  segments,
  sourceHistoryId,
  targetLanguage,
  runTask,
  onStart,
  onProgress,
  onSuccess,
  onError,
  onFinally,
}: RunTranscriptSegmentTaskJobOptions<TTaskType>): Promise<void> {
  return runTranscriptLlmTaskJob({
    taskType,
    segments,
    sourceHistoryId,
    targetLanguage,
    runTask,
    onStart,
    onProgress,
    onSuccess,
    onError,
    onFinally,
  });
}

export async function applySegmentItemsToTranscriptJob<TItem>({
  jobHistoryId,
  items,
  logLabel,
  getCurrentHistoryId,
  applyToCurrentTranscript,
  loadTranscript,
  updateTranscript,
  mergeIntoSegments,
}: ApplySegmentItemsToTranscriptJobOptions<TItem>): Promise<void> {
  const currentHistoryId = getCurrentHistoryId();

  if (currentHistoryId === jobHistoryId) {
    await applyToCurrentTranscript(items);
    return;
  }

  // Unsaved "current" work has no durable history file to patch once the user
  // has navigated away, so late results are intentionally dropped.
  if (jobHistoryId === 'current') {
    return;
  }

  try {
    const backgroundSegments = await loadTranscript(jobHistoryId);
    if (!backgroundSegments) {
      return;
    }

    await updateTranscript(jobHistoryId, mergeIntoSegments(backgroundSegments, items));
  } catch (error) {
    logger.error(`[${logLabel}] Failed to update background record segments:`, error);
  }
}
