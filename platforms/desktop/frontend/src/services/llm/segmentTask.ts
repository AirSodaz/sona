import type { AppConfig } from '../../types/config';
import type { LlmConfig, TranscriptSegment } from '../../types/transcript';
import { normalizeError } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import { polishTranscriptSegments, translateTranscriptSegments } from '../llmOperations';
import {
  listenToLlmTaskChunks,
  listenToLlmTaskProgress,
  listenToLlmTaskText,
} from '../llmTaskEvents';
import {
  createLlmTaskId,
  type LlmTaskTextPayload,
  type LlmTaskType,
  type PolishedSegment,
  type PolishSegmentsRequest,
  type TranslatedSegment,
  type TranslateSegmentsRequest,
} from '../llmTaskTypes';
import { resolveTaskProjectContext } from '../task/taskProjectContext';
import {
  captureTaskTarget,
  resolveTargetHistoryId,
  resolveTargetSidecarKey,
  type TaskTarget,
} from '../task/taskTarget';
import {
  buildLlmTaskLedgerRecord,
  createLlmTaskLedgerId,
  isTaskLedgerCancelRequested,
  patchTaskLedgerRecord,
  upsertTaskLedgerRecord,
} from '../taskLedgerBuilders';
import { getFeatureLlmConfig, isLlmConfigComplete } from './configUtils';
import { isLlmJobCancelledError, type LlmJobPriority } from './llmJobScheduler';

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
  priority?: LlmJobPriority;
  signal?: AbortSignal;
  cancelCheck?: () => boolean;
  onStart?: () => void | Promise<void>;
  onChunk?: (items: TItem[]) => void | Promise<void>;
  buildRequest: (params: {
    taskId: string;
    llmConfig: LlmConfig;
    config: TConfig;
    segments: TranscriptSegment[];
  }) => TRequest;
}

export interface TranscriptLlmTaskRunContext {
  target: TaskTarget;
  resolveHistoryId: () => string | null;
  resolveSidecarKey: () => string;
  markRunning: () => void;
  isCancelled: () => boolean;
}

interface RunTranscriptSegmentTaskJobOptions<TTaskType extends SegmentTaskType> {
  taskType: TTaskType;
  segments: TranscriptSegment[];
  sourceHistoryId: string | null;
  originSessionId?: string | null;
  targetLanguage?: string;
  runTask: (
    taskId: string,
    sidecarKey: string,
    context: TranscriptLlmTaskRunContext
  ) => Promise<void>;
  onStart?: (sidecarKey: string) => void | Promise<void>;
  onProgress?: (progress: number, sidecarKey: string) => void | Promise<void>;
  onSuccess?: (sidecarKey: string) => void | Promise<void>;
  onError?: (sidecarKey: string, error: unknown) => void | Promise<void>;
  onFinally?: (sidecarKey: string) => void | Promise<void>;
}

interface RunTranscriptLlmTaskJobOptions<TTaskType extends TranscriptLlmTaskType> {
  taskType: TTaskType;
  segments: TranscriptSegment[];
  sourceHistoryId: string | null;
  originSessionId?: string | null;
  targetLanguage?: string;
  templateId?: string;
  runTask: (
    taskId: string,
    sidecarKey: string,
    context: TranscriptLlmTaskRunContext
  ) => Promise<void>;
  onStart?: (sidecarKey: string) => void | Promise<void>;
  onProgress?: (progress: number, sidecarKey: string) => void | Promise<void>;
  onText?: TTaskType extends 'summary'
    ? (payload: LlmTaskTextPayload, sidecarKey: string) => void | Promise<void>
    : never;
  onSuccess?: (sidecarKey: string) => void | Promise<void>;
  onError?: (sidecarKey: string, error: unknown) => void | Promise<void>;
  onFinally?: (sidecarKey: string) => void | Promise<void>;
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
  priority,
  signal,
  cancelCheck,
  onStart,
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
    const scheduleOptions = {
      priority: priority ?? ('normal' as const),
      taskId,
      signal,
      cancelCheck,
      onStart,
    };
    const items = (
      taskType === 'translate'
        ? await translateTranscriptSegments(request as TranslateSegmentsRequest, scheduleOptions)
        : await polishTranscriptSegments(request as PolishSegmentsRequest, scheduleOptions)
    ) as TItem[];

    // Buffered providers still need to reuse the same consumer path so callers
    // do not branch on whether streamed chunk events arrived.
    if (onChunk && !receivedChunkEvent) {
      await onChunk(items);
    }

    return items;
  } catch (error) {
    if (isLlmJobCancelledError(error)) {
      throw error;
    }
    throw Object.assign(new Error(normalizeError(error).message), { cause: error });
  } finally {
    unlistenChunk();
  }
}

export async function runTranscriptLlmTaskJob<TTaskType extends TranscriptLlmTaskType>({
  taskType,
  segments,
  sourceHistoryId,
  originSessionId,
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

  const target = captureTaskTarget({
    sessionId: originSessionId,
    historyId: sourceHistoryId,
  });
  const resolveSidecarKey = () => resolveTargetSidecarKey(target);
  const initialSidecarKey = resolveSidecarKey();
  const taskId = createLlmTaskId(taskType);
  const ledgerId = createLlmTaskLedgerId(taskId);
  let unlistenProgress: () => void = () => undefined;
  let unlistenText: () => void = () => undefined;
  let markedRunning = false;

  const markRunning = () => {
    if (markedRunning || isTaskLedgerCancelRequested(ledgerId)) {
      return;
    }
    markedRunning = true;
    patchTaskLedgerRecord(ledgerId, {
      status: 'running',
    });
  };
  const isCancelled = () => isTaskLedgerCancelRequested(ledgerId);

  try {
    const taskProjectId = resolveTaskProjectContext({
      sessionId: originSessionId,
      historyId: sourceHistoryId,
      fallbackToActive: false,
    }).projectId;
    upsertTaskLedgerRecord(
      buildLlmTaskLedgerRecord({
        taskId,
        taskType,
        jobHistoryId: resolveTargetHistoryId(target),
        targetLanguage,
        templateId,
        tagIds: taskProjectId ? [taskProjectId] : undefined,
        status: 'pending',
      })
    );
    await onStart?.(resolveSidecarKey());
    unlistenProgress = await listenToLlmTaskProgress(
      taskId,
      taskType,
      ({ completedChunks, totalChunks }) => {
        if (isCancelled()) {
          return;
        }

        const progress = calculateProgressPercent(completedChunks, totalChunks);
        markedRunning = true;
        patchTaskLedgerRecord(ledgerId, {
          status: 'running',
          progress,
        });
        if (onProgress) {
          void onProgress(progress, resolveSidecarKey());
        }
      }
    );
    if (taskType === 'summary' && onText) {
      unlistenText = await listenToLlmTaskText(taskId, 'summary', (payload) => {
        if (isCancelled()) {
          return;
        }

        void (onText as (payload: LlmTaskTextPayload, sidecarKey: string) => void | Promise<void>)(
          payload,
          resolveSidecarKey()
        );
      });
    }

    await runTask(taskId, initialSidecarKey, {
      target,
      resolveHistoryId: () => resolveTargetHistoryId(target),
      resolveSidecarKey,
      markRunning,
      isCancelled,
    });

    if (isCancelled()) {
      patchCancelledLlmTask(ledgerId);
      return;
    }

    await onSuccess?.(resolveSidecarKey());
    patchTaskLedgerRecord(ledgerId, {
      status: 'succeeded',
      progress: 100,
      cancelable: false,
      retryable: false,
    });
  } catch (error) {
    if (isLlmJobCancelledError(error)) {
      patchCancelledLlmTask(ledgerId);
      return;
    }
    if (isCancelled()) {
      patchCancelledLlmTask(ledgerId);
    } else {
      await onError?.(resolveSidecarKey(), error);
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
    await onFinally?.(resolveSidecarKey());
  }
}

export async function runTranscriptSegmentTaskJob<TTaskType extends SegmentTaskType>({
  taskType,
  segments,
  sourceHistoryId,
  originSessionId,
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
    originSessionId,
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

  // Unsaved draft work has no durable history file to patch once the user
  // has navigated away, so late results are intentionally dropped.
  if (!jobHistoryId || jobHistoryId === 'current' || jobHistoryId.startsWith('session:')) {
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
