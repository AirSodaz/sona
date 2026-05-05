import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import type { TranscriptSegment } from '../types/transcript';
import type { AppConfig } from '../types/config';
import { getFeatureLlmConfig } from './llm/runtime';
import { resolvePolishPreset } from '../utils/polishPresets';
import { resolvePolishKeywords } from '../utils/polishKeywords';
import type {
  PolishSegmentsRequest,
  PolishedSegment,
  TranscriptLlmJobResult,
} from './llmTaskService';
import { listenToTranscriptLlmJobUpdates } from './llmTaskService';
import {
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
} from './llm/segmentTask';
import { runTranscriptLlmJob } from './tauri/llm';
import {
  createLlmTaskLedgerId,
  isTaskLedgerCancelRequested,
} from './taskLedgerRuntime';

function buildPolishedSegmentMap(polishedChunk: PolishedSegment[]): Map<string, PolishedSegment> {
  const polishedMap = new Map<string, PolishedSegment>();
  for (let index = 0; index < polishedChunk.length; index += 1) {
    polishedMap.set(polishedChunk[index].id, polishedChunk[index]);
  }
  return polishedMap;
}

function applyPolishedChunkToSegments(
  segments: TranscriptSegment[],
  polishedMap: ReadonlyMap<string, PolishedSegment>,
): TranscriptSegment[] {
  // Merge by id so polishing only rewrites transcript text and keeps each segment's
  // existing timing, speaker, and translation fields intact.
  return segments.map((segment) => {
    const polished = polishedMap.get(segment.id);
    return polished ? { ...segment, text: polished.text } : segment;
  });
}

class PolishService {
  async polishSegmentsWithConfig(
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets' | 'polishKeywordSets'>,
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
    taskIdOverride?: string,
  ): Promise<PolishedSegment[]> {
    return runConfiguredSegmentTask({
      feature: 'polish',
      taskType: 'polish',
      config,
      segments,
      onChunk: onChunkPolished,
      taskIdOverride,
      buildRequest: ({ taskId, llmConfig, segments: inputSegments }) => this.buildRequest(
        taskId,
        llmConfig,
        config,
        inputSegments,
      ),
    });
  }

  async polishSegments(
    segments: TranscriptSegment[],
    onChunkPolished?: (polishedChunk: PolishedSegment[]) => void | Promise<void>,
  ): Promise<PolishedSegment[]> {
    return this.polishSegmentsWithConfig(getEffectiveConfigSnapshot(), segments, onChunkPolished);
  }

  async polishTranscript() {
    const sessionStore = useTranscriptSessionStore.getState();
    const sidecarStore = useTranscriptSidecarStore.getState();
    const config = getEffectiveConfigSnapshot();
    const segments = sessionStore.segments;
    await runTranscriptSegmentTaskJob({
      taskType: 'polish',
      segments,
      sourceHistoryId: sessionStore.sourceHistoryId,
      onStart: (jobHistoryId) => {
        sidecarStore.updateLlmState({ isPolishing: true, polishProgress: 0 }, jobHistoryId);
      },
      onProgress: (polishProgress, jobHistoryId) => {
        // Progress tracks the transcript that launched the job, not whichever record the
        // user is viewing by the time a late chunk event arrives.
        useTranscriptSidecarStore.getState().updateLlmState({ polishProgress }, jobHistoryId);
      },
      runTask: async (taskId, jobHistoryId) => {
        const preset = resolvePolishPreset(config.polishPresetId, config.polishCustomPresets);
        const unlistenJobUpdates = await listenToTranscriptLlmJobUpdates(
          taskId,
          'polish',
          (payload) => {
            if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
              return;
            }
            this.applyTranscriptJobUpdate(payload);
          },
        );
        try {
          const result = await runTranscriptLlmJob({
            taskId,
            taskType: 'polish',
            jobHistoryId: jobHistoryId === 'current' ? null : jobHistoryId,
            config: getFeatureLlmConfig(config, 'polish')!,
            segments,
            context: preset.context,
            keywords: resolvePolishKeywords(config.polishKeywordSets),
          });
          if (isTaskLedgerCancelRequested(createLlmTaskLedgerId(taskId))) {
            return;
          }
          this.applyTranscriptJobUpdate(result);
        } finally {
          unlistenJobUpdates();
        }
      },
      onFinally: (jobHistoryId) => {
        useTranscriptSidecarStore.getState().updateLlmState({
          isPolishing: false,
          polishProgress: 0,
        }, jobHistoryId);
      },
    });
  }

  applyPolishedSegmentsInMemory(
    segments: TranscriptSegment[],
    polishedChunk: PolishedSegment[],
  ): TranscriptSegment[] {
    return applyPolishedChunkToSegments(segments, buildPolishedSegmentMap(polishedChunk));
  }

  private buildRequest(
    taskId: string,
    llmConfig: NonNullable<ReturnType<typeof getFeatureLlmConfig>>,
    config: Pick<AppConfig, 'llmSettings' | 'polishPresetId' | 'polishCustomPresets' | 'polishKeywordSets'>,
    segments: TranscriptSegment[],
  ): PolishSegmentsRequest {
    const preset = resolvePolishPreset(config.polishPresetId, config.polishCustomPresets);

    return {
      taskId,
      config: llmConfig,
      segments: segments.map(({ id, text }) => ({ id, text })),
      context: preset.context,
      keywords: resolvePolishKeywords(config.polishKeywordSets),
    };
  }

  private applyTranscriptJobUpdate(payload: TranscriptLlmJobResult) {
    if (!payload.segments) {
      return;
    }

    const currentHistoryId = useTranscriptSessionStore.getState().sourceHistoryId || 'current';
    const payloadHistoryId = payload.jobHistoryId || 'current';
    if (currentHistoryId !== payloadHistoryId) {
      return;
    }

    useTranscriptSessionStore.getState().setSegments(payload.segments);
  }
}

export const polishService = new PolishService();
