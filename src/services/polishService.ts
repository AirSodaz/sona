import { useHistoryStore } from '../stores/historyStore';
import { getEffectiveConfigSnapshot } from '../stores/effectiveConfigStore';
import { updateTranscriptSegment } from '../stores/transcriptCoordinator';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { useTranscriptSidecarStore } from '../stores/transcriptSidecarStore';
import type { TranscriptSegment } from '../types/transcript';
import type { AppConfig } from '../types/config';
import { historyService } from './historyService';
import { getFeatureLlmConfig } from './llm/runtime';
import { resolvePolishPreset } from '../utils/polishPresets';
import { resolvePolishKeywords } from '../utils/polishKeywords';
import type {
  PolishSegmentsRequest,
  PolishedSegment,
} from './llmTaskService';
import {
  applySegmentItemsToTranscriptJob,
  runConfiguredSegmentTask,
  runTranscriptSegmentTaskJob,
} from './llm/segmentTask';

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
        await this.polishSegmentsWithConfig(
          config,
          segments,
          async (items) => {
            await this.applyPolishedSegments(items, jobHistoryId);
          },
          taskId,
        );
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

  private async applyPolishedSegments(polishedSegments: PolishedSegment[], jobHistoryId: string) {
    await applySegmentItemsToTranscriptJob({
      jobHistoryId,
      items: polishedSegments,
      logLabel: 'PolishService',
      getCurrentHistoryId: () => useTranscriptSessionStore.getState().sourceHistoryId || 'current',
      applyToCurrentTranscript: (items) => {
        this.applyPolishedSegmentsToCurrentTranscript(items);
      },
      loadTranscript: (filename) => historyService.loadTranscript(filename),
      updateTranscript: (historyId, segmentsToSave) => useHistoryStore.getState().updateTranscript(historyId, segmentsToSave),
      mergeIntoSegments: (segmentsToMerge, items) => {
        const polishedMap = buildPolishedSegmentMap(items);
        return applyPolishedChunkToSegments(segmentsToMerge, polishedMap);
      },
    });
  }

  private applyPolishedSegmentsToCurrentTranscript(
    polishedSegments: PolishedSegment[],
  ) {
    polishedSegments.forEach(({ id, text }) => {
      updateTranscriptSegment(id, { text });
    });
  }
}

export const polishService = new PolishService();
