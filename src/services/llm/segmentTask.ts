import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../../types/config';
import type { LlmConfig, TranscriptSegment } from '../../types/transcript';
import { normalizeError } from '../../utils/errorUtils';
import { logger } from '../../utils/logger';
import {
  createLlmTaskId,
  listenToLlmTaskChunks,
  listenToLlmTaskProgress,
  type LlmTaskType,
  type PolishedSegment,
  type PolishSegmentsRequest,
  type TranslatedSegment,
  type TranslateSegmentsRequest,
} from '../llmTaskService';
import { getFeatureLlmConfig, isLlmConfigComplete } from './runtime';

type SegmentTaskFeature = 'translation' | 'polish';
type SegmentTaskType = Extract<LlmTaskType, 'translate' | 'polish'>;
type SegmentTaskCommand = 'translate_transcript_segments' | 'polish_transcript_segments';
type SegmentTaskItem = PolishedSegment | TranslatedSegment;
type SegmentTaskRequest = TranslateSegmentsRequest | PolishSegmentsRequest;

interface RunConfiguredSegmentTaskOptions<
  TConfig extends Pick<AppConfig, 'llmSettings'>,
  TItem extends SegmentTaskItem,
  TRequest extends SegmentTaskRequest,
> {
  feature: SegmentTaskFeature;
  taskType: SegmentTaskType;
  command: SegmentTaskCommand;
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
  runTask: (taskId: string, jobHistoryId: string) => Promise<void>;
  onStart?: (jobHistoryId: string) => void | Promise<void>;
  onProgress?: (progress: number, jobHistoryId: string) => void | Promise<void>;
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
  loadTranscript: (filename: string) => Promise<TranscriptSegment[] | null>;
  updateTranscript: (historyId: string, segments: TranscriptSegment[]) => Promise<void>;
  mergeIntoSegments: (segments: TranscriptSegment[], items: TItem[]) => TranscriptSegment[];
}

function calculateProgressPercent(completedChunks: number, totalChunks: number): number {
  return Math.round((completedChunks / Math.max(totalChunks, 1)) * 100);
}

export async function runConfiguredSegmentTask<
  TConfig extends Pick<AppConfig, 'llmSettings'>,
  TItem extends SegmentTaskItem,
  TRequest extends SegmentTaskRequest,
>({
  feature,
  taskType,
  command,
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
    const items = await invoke<TItem[]>(command, {
      request: buildRequest({
        taskId,
        llmConfig: resolvedLlmConfig,
        config,
        segments,
      }),
    });

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

export async function runTranscriptSegmentTaskJob<TTaskType extends SegmentTaskType>({
  taskType,
  segments,
  sourceHistoryId,
  runTask,
  onStart,
  onProgress,
  onSuccess,
  onError,
  onFinally,
}: RunTranscriptSegmentTaskJobOptions<TTaskType>): Promise<void> {
  if (!segments || segments.length === 0) {
    return;
  }

  const jobHistoryId = sourceHistoryId || 'current';
  const taskId = createLlmTaskId(taskType);
  const unlistenProgress = await listenToLlmTaskProgress(
    taskId,
    taskType,
    ({ completedChunks, totalChunks }) => {
      if (!onProgress) {
        return;
      }
      void onProgress(calculateProgressPercent(completedChunks, totalChunks), jobHistoryId);
    },
  );

  try {
    await onStart?.(jobHistoryId);
    await runTask(taskId, jobHistoryId);
    await onSuccess?.(jobHistoryId);
  } catch (error) {
    await onError?.(jobHistoryId, error);
    throw Object.assign(new Error(normalizeError(error).message), { cause: error });
  } finally {
    unlistenProgress();
    await onFinally?.(jobHistoryId);
  }
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
    const backgroundSegments = await loadTranscript(`${jobHistoryId}.json`);
    if (!backgroundSegments) {
      return;
    }

    await updateTranscript(jobHistoryId, mergeIntoSegments(backgroundSegments, items));
  } catch (error) {
    logger.error(`[${logLabel}] Failed to update background record segments:`, error);
  }
}
